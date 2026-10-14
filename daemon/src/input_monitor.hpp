#pragma once

#include "adrenalux/ipc_protocol.hpp"
#include <chrono>
#include <atomic>
#include <thread>
#include <vector>
#include <string>

namespace adrenalux::daemon {

class InputMonitor {
public:
    InputMonitor();
    ~InputMonitor();

    bool start();
    void stop();

    void record_input_activity(); // Can be called directly or via IPC
    uint64_t get_idle_delta_ms() const;
    ipc::ChillStatePayload get_chill_state() const;
    void update_config(const ipc::ChillConfigPayload& config);

private:
    void worker_loop();
    void discover_devices();

    std::atomic<bool> running_{false};
    std::thread worker_thread_;

    std::atomic<int64_t> last_input_time_ms_{0};
    ipc::ChillConfigPayload config_{};
    std::vector<int> open_fds_;
    bool has_device_access_{false};
};

} // namespace adrenalux::daemon
