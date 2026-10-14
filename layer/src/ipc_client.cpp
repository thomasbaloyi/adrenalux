#include "ipc_client.hpp"
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <cstring>
#include <filesystem>

namespace adrenalux::layer {

IpcClient::IpcClient() {
    const char* runtime_dir = std::getenv("XDG_RUNTIME_DIR");
    if (runtime_dir && std::filesystem::exists(runtime_dir)) {
        socket_path_ = std::string(runtime_dir) + "/" + ipc::DEFAULT_SOCKET_NAME;
    } else {
        socket_path_ = "/tmp/adrenalux-" + std::to_string(getuid()) + ".sock";
    }
}

IpcClient::~IpcClient() {
    disconnect();
}

bool IpcClient::connect() {
    if (connected_) return true;

    fd_ = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd_ < 0) return false;

    struct sockaddr_un addr{};
    addr.sun_family = AF_UNIX;
    std::strncpy(addr.sun_path, socket_path_.c_str(), sizeof(addr.sun_path) - 1);

    if (::connect(fd_, reinterpret_cast<struct sockaddr*>(&addr), sizeof(addr)) < 0) {
        close(fd_);
        fd_ = -1;
        return false;
    }

    connected_ = true;
    return true;
}

void IpcClient::disconnect() {
    if (fd_ >= 0) {
        close(fd_);
        fd_ = -1;
    }
    connected_ = false;
}

bool IpcClient::is_connected() const {
    return connected_;
}

ipc::ChillStatePayload IpcClient::get_chill_state() {
    auto now = std::chrono::steady_clock::now();
    // Cache queries for 16ms to avoid IPC overhead on every single presented frame
    if (std::chrono::duration_cast<std::chrono::milliseconds>(now - last_chill_query_).count() < 16) {
        return cached_chill_state_;
    }
    last_chill_query_ = now;

    if (!connected_ && !connect()) {
        return cached_chill_state_;
    }

    ipc::MessageHeader req{};
    req.type = ipc::MessageType::GetChillState;
    if (write(fd_, &req, sizeof(req)) != sizeof(req)) {
        disconnect();
        return cached_chill_state_;
    }

    ipc::MessageHeader resp{};
    if (read(fd_, &resp, sizeof(resp)) != sizeof(resp) || resp.type != ipc::MessageType::ChillState) {
        disconnect();
        return cached_chill_state_;
    }

    if (read(fd_, &cached_chill_state_, sizeof(cached_chill_state_)) != sizeof(cached_chill_state_)) {
        disconnect();
        return cached_chill_state_;
    }

    return cached_chill_state_;
}

ipc::TelemetryPayload IpcClient::get_telemetry() {
    auto now = std::chrono::steady_clock::now();
    if (std::chrono::duration_cast<std::chrono::milliseconds>(now - last_telemetry_query_).count() < 100) {
        return cached_telemetry_;
    }
    last_telemetry_query_ = now;

    if (!connected_ && !connect()) {
        return cached_telemetry_;
    }

    ipc::MessageHeader req{};
    req.type = ipc::MessageType::GetTelemetry;
    if (write(fd_, &req, sizeof(req)) != sizeof(req)) {
        disconnect();
        return cached_telemetry_;
    }

    ipc::MessageHeader resp{};
    if (read(fd_, &resp, sizeof(resp)) != sizeof(resp) || resp.type != ipc::MessageType::TelemetryData) {
        disconnect();
        return cached_telemetry_;
    }

    read(fd_, &cached_telemetry_, sizeof(cached_telemetry_));
    return cached_telemetry_;
}

void IpcClient::trigger_replay() {
    if (!connected_ && !connect()) return;

    ipc::MessageHeader req{};
    req.type = ipc::MessageType::TriggerReplay;
    req.payload_size = sizeof(ipc::ReplayTriggerPayload);

    ipc::ReplayTriggerPayload payload{0};
    write(fd_, &req, sizeof(req));
    write(fd_, &payload, sizeof(payload));

    ipc::MessageHeader resp{};
    read(fd_, &resp, sizeof(resp));
    ipc::ReplayResultPayload res{};
    read(fd_, &res, sizeof(res));
}

} // namespace adrenalux::layer
