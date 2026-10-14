add_executable(adrenaluxd
    src/main.cpp
    src/hardware_monitor.cpp
    src/input_monitor.cpp
    src/dvr_controller.cpp
    src/ipc_server.cpp
)

target_include_directories(adrenaluxd PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_SOURCE_DIR}/include
)

find_package(PkgConfig REQUIRED)
pkg_check_modules(LIBEVDEV REQUIRED libevdev)

target_include_directories(adrenaluxd PRIVATE ${LIBEVDEV_INCLUDE_DIRS})
target_link_libraries(adrenaluxd PRIVATE ${LIBEVDEV_LIBRARIES} pthread)

install(TARGETS adrenaluxd DESTINATION bin)
