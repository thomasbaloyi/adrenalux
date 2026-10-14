#pragma once

#include "adrenalux/ipc_protocol.hpp"
#include <string>
#include <atomic>
#include <chrono>

namespace adrenalux::layer {

class IpcClient {
public:
    IpcClient();
    ~IpcClient();

    bool connect();
    void disconnect();
    bool is_connected() const;

    // Fast polling methods (throttled internally)
    ipc::ChillStatePayload get_chill_state();
    ipc::TelemetryPayload get_telemetry();
    void trigger_replay();

private:
    std::string socket_path_;
    int fd_{-1};
    std::atomic<bool> connected_{false};

    ipc::ChillStatePayload cached_chill_state_{};
    std::chrono::steady_clock::time_point last_chill_query_{};

    ipc::TelemetryPayload cached_telemetry_{};
    std::chrono::steady_clock::time_point last_telemetry_query_{};
};

} // namespace adrenalux::layer
