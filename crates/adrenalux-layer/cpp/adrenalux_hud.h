#pragma once

#ifndef VK_NO_PROTOTYPES
#define VK_NO_PROTOTYPES
#endif

#include <vulkan/vulkan.h>
#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

struct AdrenaluxTelemetry {
    float gpu_temp_edge_c;
    float gpu_temp_junction_c;
    float gpu_temp_mem_c;
    uint32_t core_clock_mhz;
    uint32_t mem_clock_mhz;
    float power_draw_w;
    float power_cap_w;
    uint32_t fan_rpm;
    uint32_t fan_pwm;
    uint32_t vram_used_mb;
    uint32_t vram_total_mb;
};

struct AdrenaluxChillInfo {
    bool enabled;
    bool is_throttled;
    uint32_t current_target_fps;
    uint64_t last_input_delta_ms;
    bool is_overlay_open;
};

typedef PFN_vkVoidFunction (*AdrenaluxGdpaFn)(VkDevice, const char*);
typedef PFN_vkVoidFunction (*AdrenaluxGipaFn)(VkInstance, const char*);
typedef VkResult (*AdrenaluxQueuePresentFn)(VkQueue, const VkPresentInfoKHR*);

struct AdrenaluxSwapchainInfo {
    VkDevice device;
    VkPhysicalDevice physical_device;
    VkInstance instance;
    VkQueue queue;
    uint32_t queue_family;
    VkSwapchainKHR swapchain;
    uint32_t width;
    uint32_t height;
    VkFormat format;
    uint32_t min_image_count;
    AdrenaluxGdpaFn gdpa;
    AdrenaluxGipaFn gipa;
};

bool adrenalux_hud_on_create_swapchain(const struct AdrenaluxSwapchainInfo* info);
void adrenalux_hud_on_destroy_swapchain(VkSwapchainKHR swapchain);
VkResult adrenalux_hud_render_and_present(
    VkQueue queue,
    const VkPresentInfoKHR* present_info,
    float current_fps,
    const struct AdrenaluxTelemetry* telemetry,
    const struct AdrenaluxChillInfo* chill,
    AdrenaluxQueuePresentFn next_present
);
void adrenalux_hud_shutdown(void);

#ifdef __cplusplus
}
#endif
