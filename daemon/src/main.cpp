#include "adrenalux/version.hpp"
#include "hardware_monitor.hpp"
#include "input_monitor.hpp"
#include "dvr_controller.hpp"
#include "ipc_server.hpp"
#include <iostream>
#include <csignal>
#include <atomic>
#include <thread>
#include <chrono>

static std::atomic<bool> g_shutdown_requested{false};

void handle_signal(int sig) {
    std::cout << "\n[adrenaluxd] Received signal " << sig << ", shutting down gracefully..." << std::endl;
    g_shutdown_requested = true;
}

int main(int argc, char** argv) {
    std::cout << "========================================" << std::endl;
    std::cout << "  AdrenaLux Daemon (adrenaluxd) v" << adrenalux::VERSION_STRING << std::endl;
    std::cout << "  The Linux Gaming Performance Suite" << std::endl;
    std::cout << "========================================" << std::endl;

    std::signal(SIGINT, handle_signal);
    std::signal(SIGTERM, handle_signal);

    auto hw_mon = std::make_shared<adrenalux::daemon::HardwareMonitor>();
    auto input_mon = std::make_shared<adrenalux::daemon::InputMonitor>();
    auto dvr_ctrl = std::make_shared<adrenalux::daemon::DvrController>();

    if (!input_mon->start()) {
        std::cerr << "[adrenaluxd] Warning: Input monitor failed to start." << std::endl;
    }

    auto ipc_server = std::make_unique<adrenalux::daemon::IpcServer>(hw_mon, input_mon, dvr_ctrl);
    if (!ipc_server->start()) {
        std::cerr << "[adrenaluxd] Error: Failed to start IPC server. Exiting." << std::endl;
        return 1;
    }

    std::cout << "[adrenaluxd] Daemon is running. Press Ctrl+C to terminate." << std::endl;

    while (!g_shutdown_requested) {
        std::this_thread::sleep_for(std::chrono::milliseconds(500));
    }

    ipc_server->stop();
    input_mon->stop();

    std::cout << "[adrenaluxd] Stopped successfully." << std::endl;
    return 0;
}
