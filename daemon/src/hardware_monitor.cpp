#include "hardware_monitor.hpp"
#include <fstream>
#include <iostream>

namespace adrenalux::daemon {

HardwareMonitor::HardwareMonitor() {
    init();
}

bool HardwareMonitor::find_amd_gpu() {
    namespace fs = std::filesystem;
    const fs::path drm_base = "/sys/class/drm";

    if (!fs::exists(drm_base)) {
        return false;
    }

    // Iterate through card0, card1, etc.
    for (const auto& entry : fs::directory_iterator(drm_base)) {
        std::string filename = entry.path().filename().string();
        if (filename.rfind("card", 0) != 0 || filename.find('-') != std::string::npos) {
            continue; // Skip renderD* and connectors like card1-DP-1
        }

        fs::path device_dir = entry.path() / "device";
        fs::path vendor_file = device_dir / "vendor";

        if (fs::exists(vendor_file)) {
            std::ifstream vf(vendor_file);
            std::string vendor_id;
            vf >> vendor_id;
            // 0x1002 is AMD/ATI
            if (vendor_id == "0x1002") {
                card_path_ = device_dir;
                
                // Locate hwmon directory inside device
                fs::path hwmon_base = device_dir / "hwmon";
                if (fs::exists(hwmon_base)) {
                    for (const auto& hw : fs::directory_iterator(hwmon_base)) {
                        if (hw.path().filename().string().rfind("hwmon", 0) == 0) {
                            hwmon_path_ = hw.path();
                            return true;
                        }
                    }
                }
            }
        }
    }

    return false;
}

bool HardwareMonitor::init() {
    if (find_amd_gpu()) {
        initialized_ = true;
        std::cout << "[HardwareMonitor] Discovered AMD GPU at " << card_path_ 
                  << " (hwmon: " << hwmon_path_ << ")" << std::endl;
        update();
        return true;
    }

    std::cerr << "[HardwareMonitor] No supported AMD GPU found in /sys/class/drm" << std::endl;
    return false;
}

int64_t HardwareMonitor::read_sysfs_int(const std::filesystem::path& path, int64_t default_val) {
    if (!std::filesystem::exists(path)) {
        return default_val;
    }
    std::ifstream file(path);
    if (!file.is_open()) {
        return default_val;
    }
    int64_t val = default_val;
    file >> val;
    return val;
}

void HardwareMonitor::update() {
    if (!initialized_) return;

    // Throttle reads to at most once every 100ms
    auto now = std::chrono::steady_clock::now();
    if (std::chrono::duration_cast<std::chrono::milliseconds>(now - last_update_).count() < 100) {
        return;
    }
    last_update_ = now;

    // Temperatures (milli-Celsius)
    current_telemetry_.gpu_temp_edge_mc = static_cast<int32_t>(read_sysfs_int(hwmon_path_ / "temp1_input", 0));
    current_telemetry_.gpu_temp_junction_mc = static_cast<int32_t>(read_sysfs_int(hwmon_path_ / "temp2_input", 0));
    current_telemetry_.gpu_temp_mem_mc = static_cast<int32_t>(read_sysfs_int(hwmon_path_ / "temp3_input", 0));

    // Clocks (sysfs is in Hz -> convert to MHz)
    int64_t freq1 = read_sysfs_int(hwmon_path_ / "freq1_input", 0);
    int64_t freq2 = read_sysfs_int(hwmon_path_ / "freq2_input", 0);
    current_telemetry_.core_clock_mhz = static_cast<uint32_t>(freq1 / 1000000);
    current_telemetry_.mem_clock_mhz = static_cast<uint32_t>(freq2 / 1000000);

    // Power (sysfs is in micro-watts -> convert to milliwatts)
    int64_t pwr_avg = read_sysfs_int(hwmon_path_ / "power1_average", 0);
    int64_t pwr_cap = read_sysfs_int(hwmon_path_ / "power1_cap", 0);
    current_telemetry_.power_draw_mw = static_cast<uint32_t>(pwr_avg / 1000);
    current_telemetry_.power_cap_mw = static_cast<uint32_t>(pwr_cap / 1000);

    // Fan RPM and PWM
    current_telemetry_.fan_rpm = static_cast<uint32_t>(read_sysfs_int(hwmon_path_ / "fan1_input", 0));
    current_telemetry_.fan_pwm = static_cast<uint32_t>(read_sysfs_int(hwmon_path_ / "pwm1", 0));

    // VRAM usage (sysfs bytes -> Megabytes)
    int64_t vram_used_bytes = read_sysfs_int(card_path_ / "mem_info_vram_used", 0);
    int64_t vram_total_bytes = read_sysfs_int(card_path_ / "mem_info_vram_total", 0);
    current_telemetry_.vram_used_mb = static_cast<uint32_t>(vram_used_bytes / (1024 * 1024));
    current_telemetry_.vram_total_mb = static_cast<uint32_t>(vram_total_bytes / (1024 * 1024));
}

ipc::TelemetryPayload HardwareMonitor::get_latest_telemetry() const {
    return current_telemetry_;
}

} // namespace adrenalux::daemon
