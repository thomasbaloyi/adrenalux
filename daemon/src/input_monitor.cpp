#include "input_monitor.hpp"
#include <libevdev/libevdev.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/epoll.h>
#include <filesystem>
#include <iostream>

namespace adrenalux::daemon {

static int64_t get_current_epoch_ms() {
    return std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now().time_since_epoch()
    ).count();
}

InputMonitor::InputMonitor() {
    last_input_time_ms_ = get_current_epoch_ms();
    config_.enabled = true;
    config_.idle_fps = 30;
    config_.active_fps = 0; // uncapped
    config_.idle_timeout_ms = 1500;
}

InputMonitor::~InputMonitor() {
    stop();
}

void InputMonitor::record_input_activity() {
    last_input_time_ms_ = get_current_epoch_ms();
}

uint64_t InputMonitor::get_idle_delta_ms() const {
    int64_t now = get_current_epoch_ms();
    int64_t last = last_input_time_ms_.load();
    if (now >= last) {
        return static_cast<uint64_t>(now - last);
    }
    return 0;
}

ipc::ChillStatePayload InputMonitor::get_chill_state() const {
    ipc::ChillStatePayload state{};
    state.enabled = config_.enabled;
    uint64_t delta = get_idle_delta_ms();
    state.last_input_delta_ms = delta;

    if (config_.enabled && delta > config_.idle_timeout_ms) {
        state.is_throttled = true;
        state.current_target_fps = config_.idle_fps;
    } else {
        state.is_throttled = false;
        state.current_target_fps = config_.active_fps;
    }

    return state;
}

void InputMonitor::update_config(const ipc::ChillConfigPayload& config) {
    config_ = config;
}

void InputMonitor::discover_devices() {
    namespace fs = std::filesystem;
    const fs::path input_dir = "/dev/input";

    if (!fs::exists(input_dir)) return;

    for (const auto& entry : fs::directory_iterator(input_dir)) {
        std::string fname = entry.path().filename().string();
        if (fname.rfind("event", 0) != 0) continue;

        int fd = open(entry.path().c_str(), O_RDONLY | O_NONBLOCK);
        if (fd < 0) {
            continue;
        }

        struct libevdev* dev = nullptr;
        if (libevdev_new_from_fd(fd, &dev) == 0) {
            // Check if device has keys, relative (mouse), or absolute (gamepad/touch) events
            bool is_input = libevdev_has_event_type(dev, EV_KEY) ||
                            libevdev_has_event_type(dev, EV_REL) ||
                            libevdev_has_event_type(dev, EV_ABS);
            if (is_input) {
                open_fds_.push_back(fd);
            } else {
                close(fd);
            }
            libevdev_free(dev);
        } else {
            close(fd);
        }
    }

    if (!open_fds_.empty()) {
        has_device_access_ = true;
        std::cout << "[InputMonitor] Successfully opened " << open_fds_.size() 
                  << " evdev devices for Open Chill tracking." << std::endl;
    } else {
        has_device_access_ = false;
        std::cout << "[InputMonitor] Notice: Direct /dev/input/ access not available (user not in 'input' group). "
                  << "Relying on in-game overlay input reporting fallback." << std::endl;
    }
}

bool InputMonitor::start() {
    if (running_) return true;

    discover_devices();
    running_ = true;
    worker_thread_ = std::thread(&InputMonitor::worker_loop, this);
    return true;
}

void InputMonitor::stop() {
    if (!running_) return;

    running_ = false;
    if (worker_thread_.joinable()) {
        worker_thread_.join();
    }

    for (int fd : open_fds_) {
        close(fd);
    }
    open_fds_.clear();
}

void InputMonitor::worker_loop() {
    if (open_fds_.empty()) {
        // Without direct /dev/input permissions, simply keep thread alive to process timer checks
        while (running_) {
            std::this_thread::sleep_for(std::chrono::milliseconds(250));
        }
        return;
    }

    int epoll_fd = epoll_create1(0);
    if (epoll_fd < 0) return;

    for (int fd : open_fds_) {
        struct epoll_event ev{};
        ev.events = EPOLLIN;
        ev.data.fd = fd;
        epoll_ctl(epoll_fd, EPOLL_CTL_ADD, fd, &ev);
    }

    constexpr int MAX_EVENTS = 32;
    struct epoll_event events[MAX_EVENTS];

    while (running_) {
        int nfds = epoll_wait(epoll_fd, events, MAX_EVENTS, 200);
        if (nfds > 0) {
            record_input_activity();
            // Drain buffer
            struct input_event ie{};
            for (int i = 0; i < nfds; ++i) {
                while (read(events[i].data.fd, &ie, sizeof(ie)) > 0) {}
            }
        }
    }

    close(epoll_fd);
}

} // namespace adrenalux::daemon
