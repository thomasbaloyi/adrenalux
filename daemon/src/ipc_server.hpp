#pragma once

#include "adrenalux/ipc_protocol.hpp"
#include "hardware_monitor.hpp"
#include "input_monitor.hpp"
#include "dvr_controller.hpp"
#include <string>
#include <atomic>
#include <thread>
#include <memory>

namespace adrenalux::daemon {

class IpcServer {
public:
    IpcServer(std::shared_ptr<HardwareMonitor> hw_mon,
              std::shared_ptr<InputMonitor> input_mon,
              std::shared_ptr<DvrController> dvr_ctrl);
    ~IpcServer();

    bool start();
    void stop();

    std::string get_socket_path() const;

private:
    void server_loop();
    void handle_client(int client_fd);

    std::shared_ptr<HardwareMonitor> hw_mon_;
    std::shared_ptr<InputMonitor> input_mon_;
    std::shared_ptr<DvrController> dvr_ctrl_;

    std::string socket_path_;
    int server_fd_{-1};
    std::atomic<bool> running_{false};
    std::thread server_thread_;
};

} // namespace adrenalux::daemon
