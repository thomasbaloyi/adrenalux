fn main() {
    println!("cargo:rerun-if-changed=cpp/adrenalux_hud.cpp");
    println!("cargo:rerun-if-changed=cpp/adrenalux_hud.h");

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .define("VK_NO_PROTOTYPES", None)
        .include("cpp")
        .include("vendor/imgui")
        .include("vendor/imgui/backends")
        .include("/usr/include")
        .file("vendor/imgui/imgui.cpp")
        .file("vendor/imgui/imgui_draw.cpp")
        .file("vendor/imgui/imgui_tables.cpp")
        .file("vendor/imgui/imgui_widgets.cpp")
        .file("vendor/imgui/backends/imgui_impl_vulkan.cpp")
        .file("cpp/adrenalux_hud.cpp")
        .compile("adrenalux_imgui");
}
