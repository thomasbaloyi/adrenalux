#include "ipc_server.hpp"
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <iostream>
#include <cstring>
#include <filesystem>

namespace adrenalux::daemon {

IpcServer::IpcServer(std::shared_ptr<HardwareMonitor> hw_mon,
                     std::shared_ptr<InputMonitor> input_mon,
                     std::shared_ptr<DvrController> dvr_ctrl)
    : hw_mon_(std::move(hw_mon)),
      input_mon_(std::move(input_mon)),
      dvr_ctrl_(std::move(dvr_ctrl)) {
    
    const char* runtime_dir = std::getenv("XDG_RUNTIME_DIR");
    if (runtime_dir && std::filesystem::exists(runtime_dir)) {
        socket_path_ = std::string(runtime_dir) + "/" + ipc::DEFAULT_SOCKET_NAME;
    } else {
        socket_path_ = "/tmp/adrenalux-" + std::to_string(getuid()) + ".sock";
    }
}

IpcServer::~IpcServer() {
    stop();
}

std::string IpcServer::get_socket_path() const {
    return socket_path_;
}

bool IpcServer::start() {
    if (running_) return true;

    // Remove stale socket if exists
    unlink(socket_path_.c_str());

    server_fd_ = socket(AF_UNIX, SOCK_STREAM, 0);
    if (server_fd_ < 0) {
        std::cerr << "[IpcServer] Failed to create UNIX domain socket" << std::endl;
        return false;
    }

    struct sockaddr_un addr{};
    addr.sun_family = AF_UNIX;
    std::strncpy(addr.sun_path, socket_path_.c_str(), sizeof(addr.sun_path) - 1);

    if (bind(server_fd_, reinterpret_cast<struct sockaddr*>(&addr), sizeof(addr)) < 0) {
        std::cerr << "[IpcServer] Failed to bind UNIX domain socket to " << socket_path_ << std::endl;
        close(server_fd_);
        server_fd_ = -1;
        return false;
    }

    if (listen(server_fd_, 16) < 0) {
        std::cerr << "[IpcServer] Failed to listen on socket" << std::endl;
        close(server_fd_);
        server_fd_ = -1;
        return false;
    }

    running_ = true;
    server_thread_ = std::thread(&IpcServer::server_loop, this);
    std::cout << "[IpcServer] Listening on UNIX socket: " << socket_path_ << std::endl;
    return true;
}

void IpcServer::stop() {
    if (!running_) return;

    running_ = false;
    if (server_fd_ >= 0) {
        shutdown(server_fd_, SHUT_RDWR);
        close(server_fd_);
        server_fd_ = -1;
    }

    if (server_thread_.joinable()) {
        server_thread_.join();
    }

    unlink(socket_path_.c_str());
}

void IpcServer::server_loop() {
    while (running_) {
        struct sockaddr_un client_addr{};
        socklen_t client_len = sizeof(client_addr);
        int client_fd = accept(server_fd_, reinterpret_cast<struct sockaddr*>(&client_addr), &client_len);
        if (client_fd < 0) {
            if (!running_) break;
            continue;
        }

        // Spawn client handler thread or handle directly
        std::thread([this, client_fd]() {
            handle_client(client_fd);
            close(client_fd);
        }).detach();
    }
}

void IpcServer::handle_client(int client_fd) {
    while (running_) {
        ipc::MessageHeader header{};
        ssize_t bytes_read = read(client_fd, &header, sizeof(header));
        if (bytes_read <= 0) break;

        if (header.magic != ipc::PROTOCOL_MAGIC || header.version != ipc::PROTOCOL_VERSION) {
            std::cerr << "[IpcServer] Invalid protocol magic or version received." << std::endl;
            break;
        }

        switch (header.type) {
            case ipc::MessageType::Ping: {
                ipc::MessageHeader resp{};
                resp.type = ipc::MessageType::Pong;
                resp.payload_size = 0;
                write(client_fd, &resp, sizeof(resp));
                break;
            }

            case ipc::MessageType::GetTelemetry: {
                if (hw_mon_) {
                    hw_mon_->update();
                    auto telemetry = hw_mon_->get_latest_telemetry();
                    
                    ipc::MessageHeader resp{};
                    resp.type = ipc::MessageType::TelemetryData;
                    resp.payload_size = sizeof(telemetry);

                    write(client_fd, &resp, sizeof(resp));
                    write(client_fd, &telemetry, sizeof(telemetry));
                }
                break;
            }

            case ipc::MessageType::GetChillState: {
                if (input_mon_) {
                    auto chill_state = input_mon_->get_chill_state();

                    ipc::MessageHeader resp{};
                    resp.type = ipc::MessageType::ChillState;
                    resp.payload_size = sizeof(chill_state);

                    write(client_fd, &resp, sizeof(resp));
                    write(client_fd, &chill_state, sizeof(chill_state));
                }
                break;
            }

            case ipc::MessageType::SetChillConfig: {
                if (header.payload_size == sizeof(ipc::ChillConfigPayload)) {
                    ipc::ChillConfigPayload cfg{};
                    if (read(client_fd, &cfg, sizeof(cfg)) == sizeof(cfg)) {
                        if (input_mon_) {
                            input_mon_->update_config(cfg);
                        }
                        // Echo updated state
                        auto chill_state = input_mon_->get_chill_state();
                        ipc::MessageHeader resp{};
                        resp.type = ipc::MessageType::ChillState;
                        resp.payload_size = sizeof(chill_state);
                        write(client_fd, &resp, sizeof(resp));
                        write(client_fd, &chill_state, sizeof(chill_state));
                    }
                }
                break;
            }

            case ipc::MessageType::TriggerReplay: {
                ipc::ReplayTriggerPayload trig{};
                if (header.payload_size == sizeof(trig)) {
                    read(client_fd, &trig, sizeof(trig));
                }
                ipc::ReplayResultPayload result{};
                if (dvr_ctrl_) {
                    result = dvr_ctrl_->trigger_instant_replay(trig.buffer_seconds);
                }
                ipc::MessageHeader resp{};
                resp.type = ipc::MessageType::ReplayResult;
                resp.payload_size = sizeof(result);
                write(client_fd, &resp, sizeof(resp));
                write(client_fd, &result, sizeof(result));
                break;
            }

            default:
                break;
        }
    }
}

} // namespace adrenalux::daemon
