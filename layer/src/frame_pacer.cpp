#include "frame_pacer.hpp"
#include <time.h>
#include <thread>

namespace adrenalux::layer {

FramePacer::FramePacer() {
    last_present_time_ = std::chrono::steady_clock::now();
    fps_timer_ = last_present_time_;
}

void FramePacer::pace_frame(uint32_t target_fps) {
    auto now = std::chrono::steady_clock::now();
    auto elapsed_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(now - last_present_time_).count();

    if (target_fps > 0) {
        int64_t target_interval_ns = 1000000000LL / target_fps;
        int64_t sleep_ns = target_interval_ns - elapsed_ns;

        // If rendering finished early, sleep the remainder
        if (sleep_ns > 50000) { // Only sleep if > 50 microseconds
            struct timespec req{};
            req.tv_sec = sleep_ns / 1000000000LL;
            req.tv_nsec = sleep_ns % 1000000000LL;
            clock_nanosleep(CLOCK_MONOTONIC, 0, &req, nullptr);
            now = std::chrono::steady_clock::now();
            elapsed_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(now - last_present_time_).count();
        }
    }

    last_present_time_ = now;
    current_frametime_ms_ = elapsed_ns / 1000000.0f;

    // Calculate rolling FPS
    frames_in_last_sec_++;
    auto sec_delta = std::chrono::duration_cast<std::chrono::milliseconds>(now - fps_timer_).count();
    if (sec_delta >= 500) {
        current_fps_ = (frames_in_last_sec_ * 1000.0f) / sec_delta;
        frames_in_last_sec_ = 0;
        fps_timer_ = now;
    }

    frame_count_++;
}

float FramePacer::get_current_fps() const {
    return current_fps_;
}

float FramePacer::get_frametime_ms() const {
    return current_frametime_ms_;
}

} // namespace adrenalux::layer
