use std::ffi::{c_char, c_void};

type VkResult = i32;
type VkDevice = *mut c_void;
type VkPhysicalDevice = *mut c_void;
type VkInstance = *mut c_void;
type VkQueue = *mut c_void;
type VkSwapchainKHR = *mut c_void;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct VkExtent2D {
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct VkSwapchainCreateInfoKHR {
    pub s_type: u32,
    pub p_next: *const c_void,
    pub flags: u32,
    pub surface: *mut c_void,
    pub min_image_count: u32,
    pub image_format: u32,
    pub image_color_space: u32,
    pub image_extent: VkExtent2D,
    pub image_array_layers: u32,
    pub image_usage: u32,
    pub image_sharing_mode: u32,
    pub queue_family_index_count: u32,
    pub p_queue_family_indices: *const u32,
    pub pre_transform: u32,
    pub composite_alpha: u32,
    pub present_mode: u32,
    pub clipped: u32,
    pub old_swapchain: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct AdrenaluxTelemetry {
    pub gpu_temp_edge_c: f32,
    pub gpu_temp_junction_c: f32,
    pub gpu_temp_mem_c: f32,
    pub core_clock_mhz: u32,
    pub mem_clock_mhz: u32,
    pub power_draw_w: f32,
    pub power_cap_w: f32,
    pub fan_rpm: u32,
    pub fan_pwm: u32,
    pub vram_used_mb: u32,
    pub vram_total_mb: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct AdrenaluxChillInfo {
    pub enabled: bool,
    pub is_throttled: bool,
    pub current_target_fps: u32,
    pub last_input_delta_ms: u64,
    pub is_overlay_open: bool,
}

pub type AdrenaluxGdpaFn =
    unsafe extern "system" fn(VkDevice, *const c_char) -> unsafe extern "system" fn();
pub type AdrenaluxGipaFn =
    unsafe extern "system" fn(VkInstance, *const c_char) -> unsafe extern "system" fn();
pub type AdrenaluxQueuePresentFn = unsafe extern "system" fn(VkQueue, *const c_void) -> VkResult;

#[repr(C)]
pub struct AdrenaluxSwapchainInfo {
    pub device: VkDevice,
    pub physical_device: VkPhysicalDevice,
    pub instance: VkInstance,
    pub queue: VkQueue,
    pub queue_family: u32,
    pub swapchain: VkSwapchainKHR,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub min_image_count: u32,
    pub gdpa: Option<AdrenaluxGdpaFn>,
    pub gipa: Option<AdrenaluxGipaFn>,
}

extern "C" {
    pub fn adrenalux_hud_on_create_swapchain(info: *const AdrenaluxSwapchainInfo) -> bool;
    pub fn adrenalux_hud_on_destroy_swapchain(swapchain: VkSwapchainKHR);
    pub fn adrenalux_hud_render_and_present(
        queue: VkQueue,
        present_info: *const c_void,
        current_fps: f32,
        telemetry: *const AdrenaluxTelemetry,
        chill: *const AdrenaluxChillInfo,
        next_present: AdrenaluxQueuePresentFn,
    ) -> VkResult;
    pub fn adrenalux_hud_shutdown();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swapchain_info_struct_alignment() {
        assert_eq!(
            std::mem::size_of::<AdrenaluxSwapchainInfo>(),
            std::mem::size_of::<*mut c_void>() * 8 + 16
        );
    }
}
