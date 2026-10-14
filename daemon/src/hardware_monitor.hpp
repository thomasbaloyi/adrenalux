#pragma once

#include "adrenalux/ipc_protocol.hpp"
#include <string>
#include <filesystem>
#include <chrono>

namespace adrenalux::daemon {

class HardwareMonitor {
public:
    HardwareMonitor();
    ~HardwareMonitor() = default;

    bool init();
    void update();
    ipc::TelemetryPayload get_latest_telemetry() const;

private:
    bool find_amd_gpu();
    int64_t read_sysfs_int(const std::filesystem::path& path, int64_t default_val = 0);

    bool initialized_{false};
    std::filesystem::path card_path_;
    std::filesystem::path hwmon_path_;

    ipc::TelemetryPayload current_telemetry_{};
    std::chrono::steady_clock::time_point last_update_{};
};

} // namespace adrenalux::daemon
