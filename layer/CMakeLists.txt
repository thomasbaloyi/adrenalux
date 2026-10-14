add_library(VkLayer_ADRENALUX_overlay SHARED
    src/layer_entry.cpp
    src/frame_pacer.cpp
    src/ipc_client.cpp
)

target_include_directories(VkLayer_ADRENALUX_overlay PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${CMAKE_SOURCE_DIR}/include
)

find_package(Vulkan REQUIRED)
target_include_directories(VkLayer_ADRENALUX_overlay PRIVATE ${Vulkan_INCLUDE_DIRS})
target_link_libraries(VkLayer_ADRENALUX_overlay PRIVATE pthread)

# Configure JSON manifest
set(LAYER_LIBRARY_PATH "$<TARGET_FILE:VkLayer_ADRENALUX_overlay>")
configure_file(
    ${CMAKE_CURRENT_SOURCE_DIR}/VkLayer_ADRENALUX_overlay.json.in
    ${CMAKE_CURRENT_BINARY_DIR}/VkLayer_ADRENALUX_overlay.json
    @ONLY
)

install(TARGETS VkLayer_ADRENALUX_overlay DESTINATION lib)
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/VkLayer_ADRENALUX_overlay.json DESTINATION share/vulkan/implicit_layer.d)
