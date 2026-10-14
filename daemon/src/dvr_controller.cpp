#include "dvr_controller.hpp"
#include <signal.h>
#include <cstring>
#include <fstream>
#include <iostream>
#include <cstdlib>

namespace adrenalux::daemon {

DvrController::DvrController() {
    init();
}

bool DvrController::init() {
    const char* home = std::getenv("HOME");
    if (home) {
        replay_output_dir_ = std::filesystem::path(home) / "Videos" / "Replays";
    } else {
        replay_output_dir_ = "/tmp/Replays";
    }

    std::error_code ec;
    std::filesystem::create_directories(replay_output_dir_, ec);

    // Check if gpu-screen-recorder exists in PATH
    int res = std::system("which gpu-screen-recorder > /dev/null 2>&1");
    gsr_installed_ = (res == 0);

    if (gsr_installed_) {
        std::cout << "[DvrController] gpu-screen-recorder detected. Replay target dir: " 
                  << replay_output_dir_ << std::endl;
    } else {
        std::cout << "[DvrController] Notice: gpu-screen-recorder is not installed in PATH. "
                  << "DVR instant replay will require gpu-screen-recorder package." << std::endl;
    }

    return true;
}

pid_t DvrController::find_gpu_screen_recorder_pid() {
    namespace fs = std::filesystem;
    const fs::path proc_dir = "/proc";

    for (const auto& entry : fs::directory_iterator(proc_dir)) {
        if (!entry.is_directory()) continue;
        std::string pid_str = entry.path().filename().string();
        if (pid_str.find_first_not_of("0123456789") != std::string::npos) continue;

        fs::path comm_file = entry.path() / "comm";
        if (fs::exists(comm_file)) {
            std::ifstream f(comm_file);
            std::string comm;
            f >> comm;
            if (comm == "gpu-screen-reco" || comm == "gpu-screen-recorder") {
                return std::stoi(pid_str);
            }
        }
    }

    return -1;
}

bool DvrController::is_dvr_active() const {
    return const_cast<DvrController*>(this)->find_gpu_screen_recorder_pid() > 0;
}

ipc::ReplayResultPayload DvrController::trigger_instant_replay(uint32_t /*buffer_seconds*/) {
    ipc::ReplayResultPayload result{};
    std::strncpy(result.saved_path, replay_output_dir_.c_str(), sizeof(result.saved_path) - 1);

    pid_t pid = find_gpu_screen_recorder_pid();
    if (pid > 0) {
        // Send SIGUSR1 to trigger replay save
        if (kill(pid, SIGUSR1) == 0) {
            result.success = true;
            std::strncpy(result.message, "Instant replay saved successfully", sizeof(result.message) - 1);
            std::cout << "[DvrController] Sent SIGUSR1 to gpu-screen-recorder (PID " << pid << ")" << std::endl;
        } else {
            result.success = false;
            std::strncpy(result.message, "Failed to send signal to gpu-screen-recorder", sizeof(result.message) - 1);
        }
    } else {
        result.success = false;
        if (!gsr_installed_) {
            std::strncpy(result.message, "gpu-screen-recorder not installed", sizeof(result.message) - 1);
        } else {
            std::strncpy(result.message, "gpu-screen-recorder replay buffer is not currently active", sizeof(result.message) - 1);
        }
    }

    return result;
}

} // namespace adrenalux::daemon
