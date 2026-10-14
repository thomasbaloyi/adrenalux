add_executable(adrenalux-ctl
    src/main.cpp
)

target_include_directories(adrenalux-ctl PRIVATE
    ${CMAKE_SOURCE_DIR}/include
)

target_link_libraries(adrenalux-ctl PRIVATE pthread)

install(TARGETS adrenalux-ctl DESTINATION bin)
