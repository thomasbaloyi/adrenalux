#pragma once

#include <cstdint>
#include <chrono>

namespace adrenalux::layer {

class FramePacer {
public:
    FramePacer();
    ~FramePacer() = default;

    void pace_frame(uint32_t target_fps);

    float get_current_fps() const;
    float get_frametime_ms() const;

private:
    std::chrono::steady_clock::time_point last_present_time_{};
    float current_fps_{0.0f};
    float current_frametime_ms_{0.0f};

    uint64_t frame_count_{0};
    std::chrono::steady_clock::time_point fps_timer_{};
    uint32_t frames_in_last_sec_{0};
};

} // namespace adrenalux::layer
