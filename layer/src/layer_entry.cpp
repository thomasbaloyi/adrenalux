#include <vulkan/vulkan.h>
#include <vulkan/vk_layer.h>
#include "ipc_client.hpp"
#include "frame_pacer.hpp"
#include <unordered_map>
#include <mutex>
#include <iostream>
#include <cstring>

#define VK_LAYER_EXPORT extern "C" __attribute__((visibility("default")))

namespace {

struct InstanceDispatch {
    PFN_vkGetInstanceProcAddr GetInstanceProcAddr{nullptr};
    PFN_vkDestroyInstance DestroyInstance{nullptr};
};

struct DeviceDispatch {
    PFN_vkGetDeviceProcAddr GetDeviceProcAddr{nullptr};
    PFN_vkDestroyDevice DestroyDevice{nullptr};
    PFN_vkQueuePresentKHR QueuePresentKHR{nullptr};
};

std::mutex g_lock;
std::unordered_map<void*, InstanceDispatch> g_instance_dispatch;
std::unordered_map<void*, DeviceDispatch> g_device_dispatch;

adrenalux::layer::IpcClient g_ipc_client;
adrenalux::layer::FramePacer g_frame_pacer;

void* get_key(void* object) {
    return *reinterpret_cast<void**>(object);
}

} // namespace

// Intercept: QueuePresentKHR
VKAPI_ATTR VkResult VKAPI_CALL adrenalux_QueuePresentKHR(
    VkQueue queue,
    const VkPresentInfoKHR* pPresentInfo) {

    // Poll Chill state from daemon
    auto chill_state = g_ipc_client.get_chill_state();

    // Pace frame based on target FPS (if chill is active or target FPS configured)
    g_frame_pacer.pace_frame(chill_state.current_target_fps);

    // Call downstream QueuePresentKHR
    DeviceDispatch* dispatch = nullptr;
    {
        std::lock_guard<std::mutex> lock(g_lock);
        auto it = g_device_dispatch.find(get_key(queue));
        if (it != g_device_dispatch.end()) {
            dispatch = &it->second;
        }
    }

    if (dispatch && dispatch->QueuePresentKHR) {
        return dispatch->QueuePresentKHR(queue, pPresentInfo);
    }

    return VK_SUCCESS;
}

// Intercept: CreateDevice
VKAPI_ATTR VkResult VKAPI_CALL adrenalux_CreateDevice(
    VkPhysicalDevice physicalDevice,
    const VkDeviceCreateInfo* pCreateInfo,
    const VkAllocationCallbacks* pAllocator,
    VkDevice* pDevice) {

    auto* layer_info = const_cast<VkLayerDeviceCreateInfo*>(
        reinterpret_cast<const VkLayerDeviceCreateInfo*>(pCreateInfo->pNext));

    while (layer_info && (layer_info->sType != VK_STRUCTURE_TYPE_LOADER_DEVICE_CREATE_INFO ||
                          layer_info->function != VK_LAYER_LINK_INFO)) {
        layer_info = const_cast<VkLayerDeviceCreateInfo*>(
            reinterpret_cast<const VkLayerDeviceCreateInfo*>(layer_info->pNext));
    }

    if (!layer_info) return VK_ERROR_INITIALIZATION_FAILED;

    auto gpa = layer_info->u.pLayerInfo->pfnNextGetDeviceProcAddr;
    auto gipa = layer_info->u.pLayerInfo->pfnNextGetInstanceProcAddr;
    layer_info->u.pLayerInfo = layer_info->u.pLayerInfo->pNext;

    auto create_device = reinterpret_cast<PFN_vkCreateDevice>(gipa(VK_NULL_HANDLE, "vkCreateDevice"));
    VkResult res = create_device(physicalDevice, pCreateInfo, pAllocator, pDevice);
    if (res != VK_SUCCESS) return res;

    DeviceDispatch dispatch{};
    dispatch.GetDeviceProcAddr = gpa;
    dispatch.DestroyDevice = reinterpret_cast<PFN_vkDestroyDevice>(gpa(*pDevice, "vkDestroyDevice"));
    dispatch.QueuePresentKHR = reinterpret_cast<PFN_vkQueuePresentKHR>(gpa(*pDevice, "vkQueuePresentKHR"));

    {
        std::lock_guard<std::mutex> lock(g_lock);
        g_device_dispatch[get_key(*pDevice)] = dispatch;
    }

    // Connect IPC client to adrenaluxd
    g_ipc_client.connect();

    return VK_SUCCESS;
}

// Intercept: DestroyDevice
VKAPI_ATTR void VKAPI_CALL adrenalux_DestroyDevice(
    VkDevice device,
    const VkAllocationCallbacks* pAllocator) {

    DeviceDispatch dispatch;
    {
        std::lock_guard<std::mutex> lock(g_lock);
        dispatch = g_device_dispatch[get_key(device)];
        g_device_dispatch.erase(get_key(device));
    }

    if (dispatch.DestroyDevice) {
        dispatch.DestroyDevice(device, pAllocator);
    }
}

// Intercept: CreateInstance
VKAPI_ATTR VkResult VKAPI_CALL adrenalux_CreateInstance(
    const VkInstanceCreateInfo* pCreateInfo,
    const VkAllocationCallbacks* pAllocator,
    VkInstance* pInstance) {

    auto* layer_info = const_cast<VkLayerInstanceCreateInfo*>(
        reinterpret_cast<const VkLayerInstanceCreateInfo*>(pCreateInfo->pNext));

    while (layer_info && (layer_info->sType != VK_STRUCTURE_TYPE_LOADER_INSTANCE_CREATE_INFO ||
                          layer_info->function != VK_LAYER_LINK_INFO)) {
        layer_info = const_cast<VkLayerInstanceCreateInfo*>(
            reinterpret_cast<const VkLayerInstanceCreateInfo*>(layer_info->pNext));
    }

    if (!layer_info) return VK_ERROR_INITIALIZATION_FAILED;

    auto gipa = layer_info->u.pLayerInfo->pfnNextGetInstanceProcAddr;
    layer_info->u.pLayerInfo = layer_info->u.pLayerInfo->pNext;

    auto create_instance = reinterpret_cast<PFN_vkCreateInstance>(gipa(VK_NULL_HANDLE, "vkCreateInstance"));
    VkResult res = create_instance(pCreateInfo, pAllocator, pInstance);
    if (res != VK_SUCCESS) return res;

    InstanceDispatch dispatch{};
    dispatch.GetInstanceProcAddr = gipa;
    dispatch.DestroyInstance = reinterpret_cast<PFN_vkDestroyInstance>(gipa(*pInstance, "vkDestroyInstance"));

    {
        std::lock_guard<std::mutex> lock(g_lock);
        g_instance_dispatch[get_key(*pInstance)] = dispatch;
    }

    return VK_SUCCESS;
}

// Intercept: DestroyInstance
VKAPI_ATTR void VKAPI_CALL adrenalux_DestroyInstance(
    VkInstance instance,
    const VkAllocationCallbacks* pAllocator) {

    InstanceDispatch dispatch;
    {
        std::lock_guard<std::mutex> lock(g_lock);
        dispatch = g_instance_dispatch[get_key(instance)];
        g_instance_dispatch.erase(get_key(instance));
    }

    if (dispatch.DestroyInstance) {
        dispatch.DestroyInstance(instance, pAllocator);
    }
}

// GetDeviceProcAddr
VK_LAYER_EXPORT VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL adrenalux_GetDeviceProcAddr(
    VkDevice device,
    const char* pName) {

    if (std::strcmp(pName, "vkGetDeviceProcAddr") == 0) return reinterpret_cast<PFN_vkVoidFunction>(adrenalux_GetDeviceProcAddr);
    if (std::strcmp(pName, "vkDestroyDevice") == 0) return reinterpret_cast<PFN_vkVoidFunction>(adrenalux_DestroyDevice);
    if (std::strcmp(pName, "vkQueuePresentKHR") == 0) return reinterpret_cast<PFN_vkVoidFunction>(adrenalux_QueuePresentKHR);

    std::lock_guard<std::mutex> lock(g_lock);
    auto it = g_device_dispatch.find(get_key(device));
    if (it != g_device_dispatch.end() && it->second.GetDeviceProcAddr) {
        return it->second.GetDeviceProcAddr(device, pName);
    }
    return nullptr;
}

// GetInstanceProcAddr
VK_LAYER_EXPORT VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL adrenalux_GetInstanceProcAddr(
    VkInstance instance,
    const char* pName) {

    if (std::strcmp(pName, "vkGetInstanceProcAddr") == 0) return reinterpret_cast<PFN_vkVoidFunction>(adrenalux_GetInstanceProcAddr);
    if (std::strcmp(pName, "vkCreateInstance") == 0) return reinterpret_cast<PFN_vkVoidFunction>(adrenalux_CreateInstance);
    if (std::strcmp(pName, "vkDestroyInstance") == 0) return reinterpret_cast<PFN_vkVoidFunction>(adrenalux_DestroyInstance);
    if (std::strcmp(pName, "vkCreateDevice") == 0) return reinterpret_cast<PFN_vkVoidFunction>(adrenalux_CreateDevice);
    if (std::strcmp(pName, "vkGetDeviceProcAddr") == 0) return reinterpret_cast<PFN_vkVoidFunction>(adrenalux_GetDeviceProcAddr);
    if (std::strcmp(pName, "vkDestroyDevice") == 0) return reinterpret_cast<PFN_vkVoidFunction>(adrenalux_DestroyDevice);
    if (std::strcmp(pName, "vkQueuePresentKHR") == 0) return reinterpret_cast<PFN_vkVoidFunction>(adrenalux_QueuePresentKHR);

    std::lock_guard<std::mutex> lock(g_lock);
    auto it = g_instance_dispatch.find(get_key(instance));
    if (it != g_instance_dispatch.end() && it->second.GetInstanceProcAddr) {
        return it->second.GetInstanceProcAddr(instance, pName);
    }
    return nullptr;
}

// Negotiate Loader Layer Interface
VK_LAYER_EXPORT VKAPI_ATTR VkResult VKAPI_CALL adrenalux_NegotiateLoaderLayerInterfaceVersion(
    VkNegotiateLayerInterface* pVersionStruct) {

    if (!pVersionStruct || pVersionStruct->sType != LAYER_NEGOTIATE_INTERFACE_STRUCT) {
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    if (pVersionStruct->loaderLayerInterfaceVersion >= 2) {
        pVersionStruct->pfnGetInstanceProcAddr = adrenalux_GetInstanceProcAddr;
        pVersionStruct->pfnGetDeviceProcAddr = adrenalux_GetDeviceProcAddr;
        pVersionStruct->pfnGetPhysicalDeviceProcAddr = nullptr;
    }

    if (pVersionStruct->loaderLayerInterfaceVersion > 2) {
        pVersionStruct->loaderLayerInterfaceVersion = 2;
    }

    return VK_SUCCESS;
}
