#![allow(
    clippy::missing_safety_doc,
    clippy::missing_transmute_annotations,
    clippy::manual_c_str_literals,
    clippy::transmute_null_to_fn
)]

mod client;
mod pacer;
pub mod renderer;

use client::LayerIpcClient;
use pacer::FramePacer;
use renderer::{
    adrenalux_hud_on_create_swapchain, adrenalux_hud_on_destroy_swapchain,
    adrenalux_hud_render_and_present, AdrenaluxChillInfo, AdrenaluxSwapchainInfo,
    AdrenaluxTelemetry, VkSwapchainCreateInfoKHR,
};
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

type VkResult = i32;
type VkInstance = *mut c_void;
type VkPhysicalDevice = *mut c_void;
type VkDevice = *mut c_void;
type VkQueue = *mut c_void;
type VkSwapchainKHR = *mut c_void;

#[allow(non_camel_case_types)]
type PFN_vkVoidFunction = unsafe extern "system" fn();

const VK_SUCCESS: VkResult = 0;
const VK_ERROR_INITIALIZATION_FAILED: VkResult = -3;
const LAYER_NEGOTIATE_INTERFACE_STRUCT: u32 = 1;

const VK_STRUCTURE_TYPE_LOADER_INSTANCE_CREATE_INFO: u32 = 47;
const VK_STRUCTURE_TYPE_LOADER_DEVICE_CREATE_INFO: u32 = 48;
const VK_LAYER_LINK_INFO: u32 = 0;

static LOGGED_FIRST_FRAME: AtomicBool = AtomicBool::new(false);

#[repr(C)]
struct VkGenericHeader {
    s_type: u32,
    p_next: *const c_void,
}

#[repr(C)]
pub struct VkNegotiateLayerInterface {
    pub s_type: u32,
    pub p_next: *mut c_void,
    pub loader_layer_interface_version: u32,
    pub pfn_get_instance_proc_addr:
        unsafe extern "system" fn(VkInstance, *const c_char) -> PFN_vkVoidFunction,
    pub pfn_get_device_proc_addr:
        unsafe extern "system" fn(VkDevice, *const c_char) -> PFN_vkVoidFunction,
    pub pfn_get_physical_device_proc_addr: *mut c_void,
}

#[repr(C)]
struct VkLayerInstanceLink {
    p_next: *mut VkLayerInstanceLink,
    pfn_next_get_instance_proc_addr:
        unsafe extern "system" fn(VkInstance, *const c_char) -> PFN_vkVoidFunction,
    pfn_next_get_physical_device_proc_addr: *mut c_void,
}

#[repr(C)]
struct VkLayerInstanceCreateInfo {
    s_type: u32,
    p_next: *const c_void,
    function: u32,
    u: VkLayerInstanceCreateInfoUnion,
}

#[repr(C)]
union VkLayerInstanceCreateInfoUnion {
    p_layer_info: *mut VkLayerInstanceLink,
}

#[repr(C)]
struct VkLayerDeviceLink {
    p_next: *mut VkLayerDeviceLink,
    pfn_next_get_instance_proc_addr:
        unsafe extern "system" fn(VkInstance, *const c_char) -> PFN_vkVoidFunction,
    pfn_next_get_device_proc_addr:
        unsafe extern "system" fn(VkDevice, *const c_char) -> PFN_vkVoidFunction,
}

#[repr(C)]
struct VkLayerDeviceCreateInfo {
    s_type: u32,
    p_next: *const c_void,
    function: u32,
    u: VkLayerDeviceCreateInfoUnion,
}

#[repr(C)]
union VkLayerDeviceCreateInfoUnion {
    p_layer_info: *mut VkLayerDeviceLink,
}

struct InstanceDispatch {
    instance: VkInstance,
    gipa: unsafe extern "system" fn(VkInstance, *const c_char) -> PFN_vkVoidFunction,
    destroy_instance: Option<unsafe extern "system" fn(VkInstance, *const c_void)>,
}

#[derive(Clone)]
struct DeviceDispatch {
    #[allow(dead_code)]
    device: VkDevice,
    physical_device: VkPhysicalDevice,
    instance: VkInstance,
    queue: VkQueue,
    queue_family: u32,
    gdpa: unsafe extern "system" fn(VkDevice, *const c_char) -> PFN_vkVoidFunction,
    destroy_device: Option<unsafe extern "system" fn(VkDevice, *const c_void)>,
    get_device_queue: Option<unsafe extern "system" fn(VkDevice, u32, u32, *mut VkQueue)>,
    create_swapchain_khr: Option<
        unsafe extern "system" fn(
            VkDevice,
            *const VkSwapchainCreateInfoKHR,
            *const c_void,
            *mut VkSwapchainKHR,
        ) -> VkResult,
    >,
    destroy_swapchain_khr:
        Option<unsafe extern "system" fn(VkDevice, VkSwapchainKHR, *const c_void)>,
    queue_present_khr: Option<unsafe extern "system" fn(VkQueue, *const c_void) -> VkResult>,
}

struct LayerState {
    instance_dispatch: HashMap<usize, InstanceDispatch>,
    device_dispatch: HashMap<usize, DeviceDispatch>,
    ipc: LayerIpcClient,
    pacer: FramePacer,
}

unsafe impl Send for InstanceDispatch {}
unsafe impl Send for DeviceDispatch {}
unsafe impl Send for LayerState {}

static STATE: Mutex<Option<LayerState>> = Mutex::new(None);

fn with_state<F, R>(f: F) -> R
where
    F: FnOnce(&mut LayerState) -> R,
{
    let mut lock = STATE.lock().unwrap();
    if lock.is_none() {
        *lock = Some(LayerState {
            instance_dispatch: HashMap::new(),
            device_dispatch: HashMap::new(),
            ipc: LayerIpcClient::new(),
            pacer: FramePacer::new(),
        });
    }
    f(lock.as_mut().unwrap())
}

unsafe fn get_dispatch_key(obj: *mut c_void) -> usize {
    *(obj as *mut usize)
}

unsafe fn get_instance_chain_info(create_info: *const c_void) -> *mut VkLayerInstanceCreateInfo {
    if create_info.is_null() {
        return std::ptr::null_mut();
    }
    let mut header = (*(create_info as *const VkGenericHeader)).p_next;
    while !header.is_null() {
        let h = header as *const VkLayerInstanceCreateInfo;
        if (*h).s_type == VK_STRUCTURE_TYPE_LOADER_INSTANCE_CREATE_INFO
            && (*h).function == VK_LAYER_LINK_INFO
        {
            return header as *mut VkLayerInstanceCreateInfo;
        }
        header = (*(header as *const VkGenericHeader)).p_next;
    }
    std::ptr::null_mut()
}

unsafe fn get_device_chain_info(create_info: *const c_void) -> *mut VkLayerDeviceCreateInfo {
    if create_info.is_null() {
        return std::ptr::null_mut();
    }
    let mut header = (*(create_info as *const VkGenericHeader)).p_next;
    while !header.is_null() {
        let h = header as *const VkLayerDeviceCreateInfo;
        if (*h).s_type == VK_STRUCTURE_TYPE_LOADER_DEVICE_CREATE_INFO
            && (*h).function == VK_LAYER_LINK_INFO
        {
            return header as *mut VkLayerDeviceCreateInfo;
        }
        header = (*(header as *const VkGenericHeader)).p_next;
    }
    std::ptr::null_mut()
}

// Intercept: QueuePresentKHR
#[no_mangle]
pub unsafe extern "system" fn adrenalux_QueuePresentKHR(
    queue: VkQueue,
    present_info: *const c_void,
) -> VkResult {
    let (target_fps, fps, telemetry, chill, present_fn) = with_state(|state| {
        let chill = state.ipc.get_chill_state();
        let telemetry = state.ipc.get_telemetry();
        let fps = state.pacer.get_fps();
        let key = get_dispatch_key(queue);
        let present_fn = state
            .device_dispatch
            .get(&key)
            .or_else(|| state.device_dispatch.values().next())
            .and_then(|d| d.queue_present_khr);

        let c_telemetry = AdrenaluxTelemetry {
            gpu_temp_edge_c: telemetry.gpu_temp_edge_c,
            gpu_temp_junction_c: telemetry.gpu_temp_junction_c,
            gpu_temp_mem_c: telemetry.gpu_temp_mem_c,
            core_clock_mhz: telemetry.core_clock_mhz,
            mem_clock_mhz: telemetry.mem_clock_mhz,
            power_draw_w: telemetry.power_draw_w,
            power_cap_w: telemetry.power_cap_w,
            fan_rpm: telemetry.fan_rpm,
            fan_pwm: telemetry.fan_pwm,
            vram_used_mb: telemetry.vram_used_mb,
            vram_total_mb: telemetry.vram_total_mb,
        };

        let c_chill = AdrenaluxChillInfo {
            enabled: chill.enabled,
            is_throttled: chill.is_throttled,
            current_target_fps: chill.current_target_fps,
            last_input_delta_ms: chill.last_input_delta_ms,
            is_overlay_open: chill.is_overlay_open,
        };

        (
            chill.current_target_fps,
            fps,
            c_telemetry,
            c_chill,
            present_fn,
        )
    });

    if !LOGGED_FIRST_FRAME.swap(true, Ordering::Relaxed) {
        eprintln!(
            "[AdrenaLux Layer] Intercepted active game swapchain! Open Chill Target FPS: {}",
            if target_fps == 0 {
                "Uncapped".to_string()
            } else {
                target_fps.to_string()
            }
        );
    }

    with_state(|state| {
        state.pacer.pace_frame(target_fps);
    });

    if let Some(f) = present_fn {
        adrenalux_hud_render_and_present(queue, present_info, fps, &telemetry, &chill, f)
    } else {
        VK_SUCCESS
    }
}

// Intercept: CreateSwapchainKHR
#[no_mangle]
pub unsafe extern "system" fn adrenalux_CreateSwapchainKHR(
    device: VkDevice,
    p_create_info: *const VkSwapchainCreateInfoKHR,
    p_allocator: *const c_void,
    p_swapchain: *mut VkSwapchainKHR,
) -> VkResult {
    let key = get_dispatch_key(device);
    let (create_fn, dispatch_opt) = with_state(|state| {
        let d = state.device_dispatch.get(&key).cloned();
        (d.as_ref().and_then(|x| x.create_swapchain_khr), d)
    });

    let res = if let Some(f) = create_fn {
        f(device, p_create_info, p_allocator, p_swapchain)
    } else {
        VK_SUCCESS
    };

    if res == VK_SUCCESS && !p_swapchain.is_null() && !p_create_info.is_null() {
        if let Some(dispatch) = dispatch_opt {
            let info = *p_create_info;
            let gipa_fn = with_state(|s| {
                s.instance_dispatch
                    .get(&get_dispatch_key(dispatch.instance))
                    .map(|i| i.gipa)
            });

            let swapchain_info = AdrenaluxSwapchainInfo {
                device,
                physical_device: dispatch.physical_device,
                instance: dispatch.instance,
                queue: dispatch.queue,
                queue_family: dispatch.queue_family,
                swapchain: *p_swapchain,
                width: info.image_extent.width,
                height: info.image_extent.height,
                format: info.image_format,
                min_image_count: info.min_image_count,
                gdpa: Some(dispatch.gdpa),
                gipa: gipa_fn,
            };

            let ok = adrenalux_hud_on_create_swapchain(&swapchain_info);
            eprintln!(
                "[AdrenaLux Layer] Created swapchain {}x{} (format: {}) -> HUD initialized: {}",
                info.image_extent.width, info.image_extent.height, info.image_format, ok
            );
        }
    }

    res
}

// Intercept: DestroySwapchainKHR
#[no_mangle]
pub unsafe extern "system" fn adrenalux_DestroySwapchainKHR(
    device: VkDevice,
    swapchain: VkSwapchainKHR,
    p_allocator: *const c_void,
) {
    adrenalux_hud_on_destroy_swapchain(swapchain);

    let key = get_dispatch_key(device);
    let destroy_fn = with_state(|state| {
        state
            .device_dispatch
            .get(&key)
            .and_then(|d| d.destroy_swapchain_khr)
    });

    if let Some(f) = destroy_fn {
        f(device, swapchain, p_allocator);
    }
}

// Intercept: GetDeviceQueue
#[no_mangle]
pub unsafe extern "system" fn adrenalux_GetDeviceQueue(
    device: VkDevice,
    queue_family_index: u32,
    queue_index: u32,
    p_queue: *mut VkQueue,
) {
    let key = get_dispatch_key(device);
    let get_queue_fn = with_state(|state| {
        state
            .device_dispatch
            .get(&key)
            .and_then(|d| d.get_device_queue)
    });

    if let Some(f) = get_queue_fn {
        f(device, queue_family_index, queue_index, p_queue);
        if !p_queue.is_null() && !(*p_queue).is_null() {
            let queue_key = get_dispatch_key(*p_queue);
            with_state(|state| {
                if let Some(mut dispatch) = state.device_dispatch.get(&key).cloned() {
                    dispatch.queue = *p_queue;
                    dispatch.queue_family = queue_family_index;
                    state.device_dispatch.insert(key, dispatch.clone());
                    state.device_dispatch.insert(queue_key, dispatch);
                }
            });
        }
    }
}

// Intercept: CreateDevice
#[no_mangle]
pub unsafe extern "system" fn adrenalux_CreateDevice(
    physical_device: VkPhysicalDevice,
    create_info: *const c_void,
    allocator: *const c_void,
    device: *mut VkDevice,
) -> VkResult {
    let chain_info = get_device_chain_info(create_info);
    if chain_info.is_null() {
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    let layer_link = (*chain_info).u.p_layer_info;
    if layer_link.is_null() {
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    let next_gdpa = (*layer_link).pfn_next_get_device_proc_addr;
    let next_gipa = (*layer_link).pfn_next_get_instance_proc_addr;
    (*chain_info).u.p_layer_info = (*layer_link).p_next;

    let create_fn_name = CStr::from_bytes_with_nul(b"vkCreateDevice\0").unwrap();
    let create_fn: unsafe extern "system" fn(
        VkPhysicalDevice,
        *const c_void,
        *const c_void,
        *mut VkDevice,
    ) -> VkResult = std::mem::transmute(next_gipa(std::ptr::null_mut(), create_fn_name.as_ptr()));

    let res = create_fn(physical_device, create_info, allocator, device);
    if res != VK_SUCCESS {
        return res;
    }

    let key = get_dispatch_key(*device);
    let destroy_name = CStr::from_bytes_with_nul(b"vkDestroyDevice\0").unwrap();
    let get_queue_name = CStr::from_bytes_with_nul(b"vkGetDeviceQueue\0").unwrap();
    let present_name = CStr::from_bytes_with_nul(b"vkQueuePresentKHR\0").unwrap();
    let create_sc_name = CStr::from_bytes_with_nul(b"vkCreateSwapchainKHR\0").unwrap();
    let destroy_sc_name = CStr::from_bytes_with_nul(b"vkDestroySwapchainKHR\0").unwrap();

    let destroy_fn = next_gdpa(*device, destroy_name.as_ptr());
    let get_queue_fn = next_gdpa(*device, get_queue_name.as_ptr());
    let present_fn = next_gdpa(*device, present_name.as_ptr());
    let create_sc_fn = next_gdpa(*device, create_sc_name.as_ptr());
    let destroy_sc_fn = next_gdpa(*device, destroy_sc_name.as_ptr());

    let instance = with_state(|state| {
        state
            .instance_dispatch
            .values()
            .next()
            .map(|i| i.instance)
            .unwrap_or(std::ptr::null_mut())
    });

    with_state(|state| {
        state.device_dispatch.insert(
            key,
            DeviceDispatch {
                device: *device,
                physical_device,
                instance,
                queue: std::ptr::null_mut(),
                queue_family: 0,
                gdpa: next_gdpa,
                destroy_device: if destroy_fn as usize != 0 {
                    Some(std::mem::transmute(destroy_fn))
                } else {
                    None
                },
                get_device_queue: if get_queue_fn as usize != 0 {
                    Some(std::mem::transmute(get_queue_fn))
                } else {
                    None
                },
                create_swapchain_khr: if create_sc_fn as usize != 0 {
                    Some(std::mem::transmute(create_sc_fn))
                } else {
                    None
                },
                destroy_swapchain_khr: if destroy_sc_fn as usize != 0 {
                    Some(std::mem::transmute(destroy_sc_fn))
                } else {
                    None
                },
                queue_present_khr: if present_fn as usize != 0 {
                    Some(std::mem::transmute(present_fn))
                } else {
                    None
                },
            },
        );
    });

    VK_SUCCESS
}

// Intercept: DestroyDevice
#[no_mangle]
pub unsafe extern "system" fn adrenalux_DestroyDevice(device: VkDevice, allocator: *const c_void) {
    let key = get_dispatch_key(device);
    let destroy_fn = with_state(|state| {
        state
            .device_dispatch
            .remove(&key)
            .and_then(|d| d.destroy_device)
    });

    if let Some(f) = destroy_fn {
        f(device, allocator);
    }
}

// Intercept: CreateInstance
#[no_mangle]
pub unsafe extern "system" fn adrenalux_CreateInstance(
    create_info: *const c_void,
    allocator: *const c_void,
    instance: *mut VkInstance,
) -> VkResult {
    let chain_info = get_instance_chain_info(create_info);
    if chain_info.is_null() {
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    let layer_link = (*chain_info).u.p_layer_info;
    if layer_link.is_null() {
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    let next_gipa = (*layer_link).pfn_next_get_instance_proc_addr;
    (*chain_info).u.p_layer_info = (*layer_link).p_next;

    let create_fn_name = CStr::from_bytes_with_nul(b"vkCreateInstance\0").unwrap();
    let create_fn: unsafe extern "system" fn(
        *const c_void,
        *const c_void,
        *mut VkInstance,
    ) -> VkResult = std::mem::transmute(next_gipa(std::ptr::null_mut(), create_fn_name.as_ptr()));

    let res = create_fn(create_info, allocator, instance);
    if res != VK_SUCCESS {
        return res;
    }

    let key = get_dispatch_key(*instance);
    let destroy_name = CStr::from_bytes_with_nul(b"vkDestroyInstance\0").unwrap();
    let destroy_fn = next_gipa(*instance, destroy_name.as_ptr());

    with_state(|state| {
        state.instance_dispatch.insert(
            key,
            InstanceDispatch {
                instance: *instance,
                gipa: next_gipa,
                destroy_instance: if destroy_fn as usize != 0 {
                    Some(std::mem::transmute(destroy_fn))
                } else {
                    None
                },
            },
        );
    });

    VK_SUCCESS
}

// Intercept: DestroyInstance
#[no_mangle]
pub unsafe extern "system" fn adrenalux_DestroyInstance(
    instance: VkInstance,
    allocator: *const c_void,
) {
    let key = get_dispatch_key(instance);
    let destroy_fn = with_state(|state| {
        state
            .instance_dispatch
            .remove(&key)
            .and_then(|d| d.destroy_instance)
    });

    if let Some(f) = destroy_fn {
        f(instance, allocator);
    }
}

// GetDeviceProcAddr
#[no_mangle]
pub unsafe extern "system" fn adrenalux_GetDeviceProcAddr(
    device: VkDevice,
    name: *const c_char,
) -> PFN_vkVoidFunction {
    let s = CStr::from_ptr(name).to_bytes();
    match s {
        b"vkGetDeviceProcAddr" => std::mem::transmute(adrenalux_GetDeviceProcAddr as *const ()),
        b"vkDestroyDevice" => std::mem::transmute(adrenalux_DestroyDevice as *const ()),
        b"vkGetDeviceQueue" => std::mem::transmute(adrenalux_GetDeviceQueue as *const ()),
        b"vkQueuePresentKHR" => std::mem::transmute(adrenalux_QueuePresentKHR as *const ()),
        b"vkCreateSwapchainKHR" => std::mem::transmute(adrenalux_CreateSwapchainKHR as *const ()),
        b"vkDestroySwapchainKHR" => std::mem::transmute(adrenalux_DestroySwapchainKHR as *const ()),
        _ => {
            let key = get_dispatch_key(device);
            let gdpa = with_state(|state| state.device_dispatch.get(&key).map(|d| d.gdpa));
            if let Some(f) = gdpa {
                f(device, name)
            } else {
                std::mem::transmute(std::ptr::null::<()>())
            }
        }
    }
}

// GetInstanceProcAddr
#[no_mangle]
pub unsafe extern "system" fn adrenalux_GetInstanceProcAddr(
    instance: VkInstance,
    name: *const c_char,
) -> PFN_vkVoidFunction {
    let s = CStr::from_ptr(name).to_bytes();
    match s {
        b"vkGetInstanceProcAddr" => std::mem::transmute(adrenalux_GetInstanceProcAddr as *const ()),
        b"vkCreateInstance" => std::mem::transmute(adrenalux_CreateInstance as *const ()),
        b"vkDestroyInstance" => std::mem::transmute(adrenalux_DestroyInstance as *const ()),
        b"vkCreateDevice" => std::mem::transmute(adrenalux_CreateDevice as *const ()),
        b"vkGetDeviceProcAddr" => std::mem::transmute(adrenalux_GetDeviceProcAddr as *const ()),
        b"vkDestroyDevice" => std::mem::transmute(adrenalux_DestroyDevice as *const ()),
        b"vkGetDeviceQueue" => std::mem::transmute(adrenalux_GetDeviceQueue as *const ()),
        b"vkQueuePresentKHR" => std::mem::transmute(adrenalux_QueuePresentKHR as *const ()),
        b"vkCreateSwapchainKHR" => std::mem::transmute(adrenalux_CreateSwapchainKHR as *const ()),
        b"vkDestroySwapchainKHR" => std::mem::transmute(adrenalux_DestroySwapchainKHR as *const ()),
        _ => {
            let key = get_dispatch_key(instance);
            let gipa = with_state(|state| state.instance_dispatch.get(&key).map(|d| d.gipa));
            if let Some(f) = gipa {
                f(instance, name)
            } else {
                std::mem::transmute(std::ptr::null::<()>())
            }
        }
    }
}

// Negotiate Loader Layer Interface
#[no_mangle]
pub unsafe extern "system" fn adrenalux_NegotiateLoaderLayerInterfaceVersion(
    version_struct: *mut VkNegotiateLayerInterface,
) -> VkResult {
    if version_struct.is_null() || (*version_struct).s_type != LAYER_NEGOTIATE_INTERFACE_STRUCT {
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    if (*version_struct).loader_layer_interface_version >= 2 {
        (*version_struct).pfn_get_instance_proc_addr = adrenalux_GetInstanceProcAddr;
        (*version_struct).pfn_get_device_proc_addr = adrenalux_GetDeviceProcAddr;
        (*version_struct).pfn_get_physical_device_proc_addr = std::ptr::null_mut();
    }

    if (*version_struct).loader_layer_interface_version > 2 {
        (*version_struct).loader_layer_interface_version = 2;
    }

    VK_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negotiate_valid_version() {
        let mut iface = VkNegotiateLayerInterface {
            s_type: LAYER_NEGOTIATE_INTERFACE_STRUCT,
            p_next: std::ptr::null_mut(),
            loader_layer_interface_version: 2,
            pfn_get_instance_proc_addr: unsafe { std::mem::transmute(std::ptr::null::<()>()) },
            pfn_get_device_proc_addr: unsafe { std::mem::transmute(std::ptr::null::<()>()) },
            pfn_get_physical_device_proc_addr: std::ptr::null_mut(),
        };

        let res = unsafe { adrenalux_NegotiateLoaderLayerInterfaceVersion(&mut iface) };
        assert_eq!(res, VK_SUCCESS);
        assert_eq!(
            iface.pfn_get_instance_proc_addr as usize,
            adrenalux_GetInstanceProcAddr as *const () as usize
        );
        assert_eq!(
            iface.pfn_get_device_proc_addr as usize,
            adrenalux_GetDeviceProcAddr as *const () as usize
        );
    }

    #[test]
    fn test_negotiate_invalid_struct() {
        let mut iface = VkNegotiateLayerInterface {
            s_type: 999, // Invalid
            p_next: std::ptr::null_mut(),
            loader_layer_interface_version: 2,
            pfn_get_instance_proc_addr: unsafe { std::mem::transmute(std::ptr::null::<()>()) },
            pfn_get_device_proc_addr: unsafe { std::mem::transmute(std::ptr::null::<()>()) },
            pfn_get_physical_device_proc_addr: std::ptr::null_mut(),
        };

        let res = unsafe { adrenalux_NegotiateLoaderLayerInterfaceVersion(&mut iface) };
        assert_eq!(res, VK_ERROR_INITIALIZATION_FAILED);
    }

    #[test]
    fn test_proc_addr_resolution() {
        unsafe {
            let null_inst: VkInstance = std::ptr::null_mut();
            let name_gpa = CStr::from_bytes_with_nul(b"vkGetInstanceProcAddr\0").unwrap();
            let fn_ptr = adrenalux_GetInstanceProcAddr(null_inst, name_gpa.as_ptr());
            assert_eq!(
                fn_ptr as usize,
                adrenalux_GetInstanceProcAddr as *const () as usize
            );

            let name_ci = CStr::from_bytes_with_nul(b"vkCreateInstance\0").unwrap();
            let fn_ptr2 = adrenalux_GetInstanceProcAddr(null_inst, name_ci.as_ptr());
            assert_eq!(
                fn_ptr2 as usize,
                adrenalux_CreateInstance as *const () as usize
            );

            let name_cd = CStr::from_bytes_with_nul(b"vkCreateDevice\0").unwrap();
            let fn_ptr3 = adrenalux_GetInstanceProcAddr(null_inst, name_cd.as_ptr());
            assert_eq!(
                fn_ptr3 as usize,
                adrenalux_CreateDevice as *const () as usize
            );

            let name_present = CStr::from_bytes_with_nul(b"vkQueuePresentKHR\0").unwrap();
            let fn_ptr4 = adrenalux_GetInstanceProcAddr(null_inst, name_present.as_ptr());
            assert_eq!(
                fn_ptr4 as usize,
                adrenalux_QueuePresentKHR as *const () as usize
            );

            let name_create_sc = CStr::from_bytes_with_nul(b"vkCreateSwapchainKHR\0").unwrap();
            let fn_ptr5 = adrenalux_GetInstanceProcAddr(null_inst, name_create_sc.as_ptr());
            assert_eq!(
                fn_ptr5 as usize,
                adrenalux_CreateSwapchainKHR as *const () as usize
            );

            let name_destroy_sc = CStr::from_bytes_with_nul(b"vkDestroySwapchainKHR\0").unwrap();
            let fn_ptr6 = adrenalux_GetInstanceProcAddr(null_inst, name_destroy_sc.as_ptr());
            assert_eq!(
                fn_ptr6 as usize,
                adrenalux_DestroySwapchainKHR as *const () as usize
            );
        }
    }

    #[test]
    fn test_pnext_chain_parser() {
        #[repr(C)]
        struct DummyStruct {
            s_type: u32,
            p_next: *const c_void,
        }

        let mut layer_link = VkLayerInstanceLink {
            p_next: std::ptr::null_mut(),
            pfn_next_get_instance_proc_addr: unsafe { std::mem::transmute(std::ptr::null::<()>()) },
            pfn_next_get_physical_device_proc_addr: std::ptr::null_mut(),
        };

        let layer_info = VkLayerInstanceCreateInfo {
            s_type: VK_STRUCTURE_TYPE_LOADER_INSTANCE_CREATE_INFO,
            p_next: std::ptr::null(),
            function: VK_LAYER_LINK_INFO,
            u: VkLayerInstanceCreateInfoUnion {
                p_layer_info: &mut layer_link,
            },
        };

        let dummy = DummyStruct {
            s_type: 9999,
            p_next: &layer_info as *const _ as *const c_void,
        };

        let create_info = DummyStruct {
            s_type: 1,
            p_next: &dummy as *const _ as *const c_void,
        };

        unsafe {
            let found = get_instance_chain_info(&create_info as *const _ as *const c_void);
            assert!(!found.is_null());
            assert_eq!(
                (*found).s_type,
                VK_STRUCTURE_TYPE_LOADER_INSTANCE_CREATE_INFO
            );
            assert_eq!((*found).function, VK_LAYER_LINK_INFO);
        }
    }
}
