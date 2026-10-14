#pragma once

#include "adrenalux/ipc_protocol.hpp"
#include <string>
#include <filesystem>

namespace adrenalux::daemon {

class DvrController {
public:
    DvrController();
    ~DvrController() = default;

    bool init();
    ipc::ReplayResultPayload trigger_instant_replay(uint32_t buffer_seconds = 0);
    bool is_dvr_active() const;

private:
    pid_t find_gpu_screen_recorder_pid();

    std::filesystem::path replay_output_dir_;
    bool gsr_installed_{false};
};

} // namespace adrenalux::daemon
