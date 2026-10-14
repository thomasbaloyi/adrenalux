#include "adrenalux_hud.h"
#include "imgui.h"
#include "backends/imgui_impl_vulkan.h"

#include <cstdio>
#include <cstring>
#include <vector>
#include <unordered_map>
#include <mutex>

struct VulkanDispatch {
    PFN_vkGetSwapchainImagesKHR vkGetSwapchainImagesKHR = nullptr;
    PFN_vkCreateImageView vkCreateImageView = nullptr;
    PFN_vkDestroyImageView vkDestroyImageView = nullptr;
    PFN_vkCreateRenderPass vkCreateRenderPass = nullptr;
    PFN_vkDestroyRenderPass vkDestroyRenderPass = nullptr;
    PFN_vkCreateFramebuffer vkCreateFramebuffer = nullptr;
    PFN_vkDestroyFramebuffer vkDestroyFramebuffer = nullptr;
    PFN_vkCreateCommandPool vkCreateCommandPool = nullptr;
    PFN_vkDestroyCommandPool vkDestroyCommandPool = nullptr;
    PFN_vkAllocateCommandBuffers vkAllocateCommandBuffers = nullptr;
    PFN_vkFreeCommandBuffers vkFreeCommandBuffers = nullptr;
    PFN_vkCreateSemaphore vkCreateSemaphore = nullptr;
    PFN_vkDestroySemaphore vkDestroySemaphore = nullptr;
    PFN_vkCreateFence vkCreateFence = nullptr;
    PFN_vkDestroyFence vkDestroyFence = nullptr;
    PFN_vkWaitForFences vkWaitForFences = nullptr;
    PFN_vkResetFences vkResetFences = nullptr;
    PFN_vkBeginCommandBuffer vkBeginCommandBuffer = nullptr;
    PFN_vkEndCommandBuffer vkEndCommandBuffer = nullptr;
    PFN_vkCmdBeginRenderPass vkCmdBeginRenderPass = nullptr;
    PFN_vkCmdEndRenderPass vkCmdEndRenderPass = nullptr;
    PFN_vkQueueSubmit vkQueueSubmit = nullptr;
    PFN_vkDeviceWaitIdle vkDeviceWaitIdle = nullptr;
};

struct FrameData {
    VkImage image = VK_NULL_HANDLE;
    VkImageView image_view = VK_NULL_HANDLE;
    VkFramebuffer framebuffer = VK_NULL_HANDLE;
    VkCommandBuffer cmd_buffer = VK_NULL_HANDLE;
    VkSemaphore render_complete_semaphore = VK_NULL_HANDLE;
    VkFence fence = VK_NULL_HANDLE;
};

struct SwapchainData {
    AdrenaluxSwapchainInfo info;
    VulkanDispatch vk;
    VkRenderPass render_pass = VK_NULL_HANDLE;
    VkCommandPool cmd_pool = VK_NULL_HANDLE;
    std::vector<FrameData> frames;
    bool imgui_initialized = false;
};

static std::unordered_map<VkSwapchainKHR, SwapchainData> g_swapchains;
static std::mutex g_swapchain_mutex;

static PFN_vkVoidFunction adrenalux_vk_loader(const char* name, void* user_data) {
    const auto* info = static_cast<const AdrenaluxSwapchainInfo*>(user_data);
    if (!info) return nullptr;
    PFN_vkVoidFunction fn = nullptr;
    if (info->device && info->gdpa) {
        fn = info->gdpa(info->device, name);
    }
    if (!fn && info->instance && info->gipa) {
        fn = info->gipa(info->instance, name);
    }
    return fn;
}

static bool load_vulkan_dispatch(const AdrenaluxSwapchainInfo& info, VulkanDispatch& vk) {
#define LOAD_VK_FN(name) \
    vk.name = reinterpret_cast<PFN_##name>(adrenalux_vk_loader(#name, const_cast<void*>(static_cast<const void*>(&info)))); \
    if (!vk.name) { \
        fprintf(stderr, "[AdrenaLux HUD] Failed to resolve %s\n", #name); \
        return false; \
    }

    LOAD_VK_FN(vkGetSwapchainImagesKHR);
    LOAD_VK_FN(vkCreateImageView);
    LOAD_VK_FN(vkDestroyImageView);
    LOAD_VK_FN(vkCreateRenderPass);
    LOAD_VK_FN(vkDestroyRenderPass);
    LOAD_VK_FN(vkCreateFramebuffer);
    LOAD_VK_FN(vkDestroyFramebuffer);
    LOAD_VK_FN(vkCreateCommandPool);
    LOAD_VK_FN(vkDestroyCommandPool);
    LOAD_VK_FN(vkAllocateCommandBuffers);
    LOAD_VK_FN(vkFreeCommandBuffers);
    LOAD_VK_FN(vkCreateSemaphore);
    LOAD_VK_FN(vkDestroySemaphore);
    LOAD_VK_FN(vkCreateFence);
    LOAD_VK_FN(vkDestroyFence);
    LOAD_VK_FN(vkWaitForFences);
    LOAD_VK_FN(vkResetFences);
    LOAD_VK_FN(vkBeginCommandBuffer);
    LOAD_VK_FN(vkEndCommandBuffer);
    LOAD_VK_FN(vkCmdBeginRenderPass);
    LOAD_VK_FN(vkCmdEndRenderPass);
    LOAD_VK_FN(vkQueueSubmit);
    LOAD_VK_FN(vkDeviceWaitIdle);

#undef LOAD_VK_FN
    return true;
}

static void setup_adrenalux_theme() {
    ImGuiStyle& style = ImGui::GetStyle();
    style.WindowRounding = 6.0f;
    style.FrameRounding = 4.0f;
    style.PopupRounding = 4.0f;
    style.ScrollbarRounding = 4.0f;
    style.GrabRounding = 4.0f;
    style.WindowBorderSize = 1.0f;
    style.FrameBorderSize = 0.0f;

    ImVec4* colors = style.Colors;
    // AMD Radeon Crimson Red & Dark Charcoal palette
    colors[ImGuiCol_Text]                  = ImVec4(0.95f, 0.95f, 0.96f, 1.00f);
    colors[ImGuiCol_WindowBg]              = ImVec4(0.08f, 0.09f, 0.11f, 0.92f);
    colors[ImGuiCol_PopupBg]               = ImVec4(0.10f, 0.11f, 0.13f, 0.95f);
    colors[ImGuiCol_Border]                = ImVec4(0.93f, 0.11f, 0.14f, 0.60f); // AMD Red border
    colors[ImGuiCol_FrameBg]               = ImVec4(0.14f, 0.16f, 0.19f, 0.80f);
    colors[ImGuiCol_FrameBgHovered]        = ImVec4(0.20f, 0.22f, 0.26f, 0.80f);
    colors[ImGuiCol_FrameBgActive]         = ImVec4(0.93f, 0.11f, 0.14f, 0.50f);
    colors[ImGuiCol_TitleBg]               = ImVec4(0.12f, 0.13f, 0.16f, 1.00f);
    colors[ImGuiCol_TitleBgActive]         = ImVec4(0.93f, 0.11f, 0.14f, 0.85f);
    colors[ImGuiCol_Button]                = ImVec4(0.93f, 0.11f, 0.14f, 0.80f);
    colors[ImGuiCol_ButtonHovered]         = ImVec4(1.00f, 0.22f, 0.25f, 0.90f);
    colors[ImGuiCol_ButtonActive]          = ImVec4(0.80f, 0.08f, 0.11f, 1.00f);
    colors[ImGuiCol_Header]                = ImVec4(0.93f, 0.11f, 0.14f, 0.45f);
    colors[ImGuiCol_HeaderHovered]         = ImVec4(0.93f, 0.11f, 0.14f, 0.70f);
    colors[ImGuiCol_HeaderActive]          = ImVec4(0.93f, 0.11f, 0.14f, 0.90f);
    colors[ImGuiCol_Tab]                   = ImVec4(0.14f, 0.16f, 0.19f, 0.80f);
    colors[ImGuiCol_TabHovered]            = ImVec4(0.93f, 0.11f, 0.14f, 0.80f);
    colors[ImGuiCol_TabActive]             = ImVec4(0.93f, 0.11f, 0.14f, 1.00f);
    colors[ImGuiCol_PlotLines]             = ImVec4(0.93f, 0.11f, 0.14f, 1.00f);
    colors[ImGuiCol_PlotHistogram]         = ImVec4(0.93f, 0.11f, 0.14f, 1.00f);
}

static bool g_imgui_initialized = false;

static bool init_imgui_backend(SwapchainData& sc) {
    if (sc.imgui_initialized) return true;

    ImGui_ImplVulkan_LoadFunctions(adrenalux_vk_loader, const_cast<void*>(static_cast<const void*>(&sc.info)));

    if (ImGui::GetCurrentContext() == nullptr) {
        IMGUI_CHECKVERSION();
        ImGui::CreateContext();
        ImGuiIO& io = ImGui::GetIO();
        io.IniFilename = nullptr; // Do not write imgui.ini
        setup_adrenalux_theme();
    }

    if (g_imgui_initialized) {
        ImGui_ImplVulkan_Shutdown();
        g_imgui_initialized = false;
    }

    ImGui_ImplVulkan_InitInfo init_info = {};
    init_info.Instance = sc.info.instance;
    init_info.PhysicalDevice = sc.info.physical_device;
    init_info.Device = sc.info.device;
    init_info.QueueFamily = sc.info.queue_family;
    init_info.Queue = sc.info.queue;
    init_info.DescriptorPoolSize = 100;
    init_info.RenderPass = sc.render_pass;
    init_info.MinImageCount = sc.info.min_image_count > 0 ? sc.info.min_image_count : 2;
    init_info.ImageCount = static_cast<uint32_t>(sc.frames.size());
    init_info.MSAASamples = VK_SAMPLE_COUNT_1_BIT;

    if (!ImGui_ImplVulkan_Init(&init_info)) {
        fprintf(stderr, "[AdrenaLux HUD] Failed ImGui_ImplVulkan_Init\n");
        return false;
    }

    g_imgui_initialized = true;
    sc.imgui_initialized = true;
    fprintf(stderr, "[AdrenaLux HUD] Successfully initialized ImGui Vulkan Overlay HUD (%ux%u)\n",
            sc.info.width, sc.info.height);
    return true;
}

static void draw_adrenalux_ui(VkCommandBuffer cmd_buffer, uint32_t width, uint32_t height,
                             float current_fps, const struct AdrenaluxTelemetry* telemetry,
                             const struct AdrenaluxChillInfo* chill) {
    ImGuiIO& io = ImGui::GetIO();
    io.DisplaySize = ImVec2(static_cast<float>(width), static_cast<float>(height));

    ImGui_ImplVulkan_NewFrame();
    ImGui::NewFrame();

    // 1. Sleek Top-Left Minimal Status Bar (Always Visible)
    ImGui::SetNextWindowPos(ImVec2(16.0f, 16.0f), ImGuiCond_Always);
    ImGui::SetNextWindowBgAlpha(0.80f);
    ImGuiWindowFlags mini_flags = ImGuiWindowFlags_NoDecoration | ImGuiWindowFlags_AlwaysAutoResize |
                                 ImGuiWindowFlags_NoSavedSettings | ImGuiWindowFlags_NoFocusOnAppearing |
                                 ImGuiWindowFlags_NoNav | ImGuiWindowFlags_NoMove;

    if (ImGui::Begin("##AdrenaLuxMiniHUD", nullptr, mini_flags)) {
        ImGui::TextColored(ImVec4(0.93f, 0.11f, 0.14f, 1.0f), "⚡ ADRENALUX");
        ImGui::SameLine();
        ImGui::Text("| %.0f FPS", current_fps);
        if (telemetry) {
            ImGui::SameLine();
            ImGui::Text("| %.0f°C", telemetry->gpu_temp_junction_c);
            ImGui::SameLine();
            ImGui::Text("| %.0fW", telemetry->power_draw_w);
        }
        if (chill && chill->enabled) {
            ImGui::SameLine();
            if (chill->is_throttled) {
                ImGui::TextColored(ImVec4(0.3f, 0.9f, 0.3f, 1.0f), "| Chill: THROTTLED (%u FPS)", chill->current_target_fps);
            } else {
                ImGui::TextColored(ImVec4(0.9f, 0.7f, 0.2f, 1.0f), "| Chill: ACTIVE");
            }
        }
        ImGui::SameLine();
        ImGui::TextColored(ImVec4(0.7f, 0.7f, 0.7f, 1.0f), "[Alt+A]");
    }
    ImGui::End();

    // 2. Interactive Control Center Modal (Toggled via Alt+A)
    if (chill && chill->is_overlay_open) {
        ImGui::SetNextWindowSize(ImVec2(520.0f, 400.0f), ImGuiCond_FirstUseEver);
        ImGui::SetNextWindowPos(ImVec2(static_cast<float>(width) * 0.5f - 260.0f,
                                       static_cast<float>(height) * 0.5f - 200.0f), ImGuiCond_FirstUseEver);

        if (ImGui::Begin("AdrenaLux Control Center ⚡ [Alt+A to Close]", nullptr)) {
            if (ImGui::BeginTabBar("AdrenaluxTabs")) {
                // Tab 1: Telemetry & Performance
                if (ImGui::BeginTabItem("Telemetry")) {
                    ImGui::Spacing();
                    ImGui::Text("Framerate: %.1f FPS (Frametime: %.2f ms)",
                                current_fps, current_fps > 0.0f ? 1000.0f / current_fps : 0.0f);
                    ImGui::Separator();

                    if (telemetry) {
                        ImGui::Text("Edge Temp:     %.1f °C", telemetry->gpu_temp_edge_c);
                        ImGui::Text("Hotspot Temp:  %.1f °C", telemetry->gpu_temp_junction_c);
                        ImGui::Text("VRAM Temp:     %.1f °C", telemetry->gpu_temp_mem_c);
                        ImGui::Separator();
                        ImGui::Text("Core Clock:    %u MHz", telemetry->core_clock_mhz);
                        ImGui::Text("Memory Clock:  %u MHz", telemetry->mem_clock_mhz);
                        ImGui::Separator();
                        ImGui::Text("Power Draw:    %.1f W / %.1f W (Cap)", telemetry->power_draw_w, telemetry->power_cap_w);
                        ImGui::Text("Fan Speed:     %u RPM", telemetry->fan_rpm);
                        ImGui::Text("VRAM Usage:    %u MB / %u MB", telemetry->vram_used_mb, telemetry->vram_total_mb);
                    }
                    ImGui::EndTabItem();
                }

                // Tab 2: Open Chill Pacing
                if (ImGui::BeginTabItem("Open Chill")) {
                    ImGui::Spacing();
                    ImGui::Text("Dynamic Input-Aware Framerate Limiter");
                    ImGui::Separator();
                    ImGui::Text("Status: %s", (chill->is_throttled ? "THROTTLED (Idle / Menus)" : "ACTIVE (Gaming)"));
                    ImGui::Text("Current Target FPS: %u", chill->current_target_fps);
                    ImGui::Text("Idle Duration: %llu ms", static_cast<unsigned long long>(chill->last_input_delta_ms));
                    ImGui::Separator();
                    ImGui::TextColored(ImVec4(0.8f, 0.8f, 0.8f, 1.0f), "Hotkey: Alt+A toggles Control Center");
                    ImGui::TextColored(ImVec4(0.8f, 0.8f, 0.8f, 1.0f), "Configurable via: adrenalux-ctl chill --idle-fps <N>");
                    ImGui::EndTabItem();
                }

                // Tab 3: Replay DVR
                if (ImGui::BeginTabItem("Instant Replay")) {
                    ImGui::Spacing();
                    ImGui::Text("Hardware-Accelerated DVR (gpu-screen-recorder)");
                    ImGui::Separator();
                    if (ImGui::Button("Save Last 60s Replay Clip", ImVec2(240.0f, 40.0f))) {
                        // Will trigger via IPC
                    }
                    ImGui::TextColored(ImVec4(0.6f, 0.6f, 0.6f, 1.0f), "Target Directory: ~/Videos/Replays/");
                    ImGui::EndTabItem();
                }

                // Tab 4: OptiScaler / Upscaling
                if (ImGui::BeginTabItem("Upscaler")) {
                    ImGui::Spacing();
                    ImGui::Text("OptiScaler DLSS -> FSR 3.1 Bridge");
                    ImGui::Separator();
                    ImGui::Text("FSR 3.1 Frame Generation: Available");
                    ImGui::Text("Dynamic Resolution Scaling: Ready");
                    ImGui::EndTabItem();
                }

                ImGui::EndTabBar();
            }
        }
        ImGui::End();
    }

    ImGui::Render();
    ImGui_ImplVulkan_RenderDrawData(ImGui::GetDrawData(), cmd_buffer);
}

extern "C" bool adrenalux_hud_on_create_swapchain(const struct AdrenaluxSwapchainInfo* info) {
    if (!info || info->swapchain == VK_NULL_HANDLE) return false;

    std::lock_guard<std::mutex> lock(g_swapchain_mutex);

    SwapchainData sc_data;
    sc_data.info = *info;

    if (!load_vulkan_dispatch(sc_data.info, sc_data.vk)) {
        return false;
    }

    // 1. Create RenderPass with LOAD_OP_LOAD to preserve game frame
    VkAttachmentDescription color_attachment = {};
    color_attachment.format = info->format;
    color_attachment.samples = VK_SAMPLE_COUNT_1_BIT;
    color_attachment.loadOp = VK_ATTACHMENT_LOAD_OP_LOAD;
    color_attachment.storeOp = VK_ATTACHMENT_STORE_OP_STORE;
    color_attachment.stencilLoadOp = VK_ATTACHMENT_LOAD_OP_DONT_CARE;
    color_attachment.stencilStoreOp = VK_ATTACHMENT_STORE_OP_DONT_CARE;
    color_attachment.initialLayout = VK_IMAGE_LAYOUT_PRESENT_SRC_KHR;
    color_attachment.finalLayout = VK_IMAGE_LAYOUT_PRESENT_SRC_KHR;

    VkAttachmentReference color_ref = {};
    color_ref.attachment = 0;
    color_ref.layout = VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL;

    VkSubpassDescription subpass = {};
    subpass.pipelineBindPoint = VK_PIPELINE_BIND_POINT_GRAPHICS;
    subpass.colorAttachmentCount = 1;
    subpass.pColorAttachments = &color_ref;

    VkSubpassDependency dependency = {};
    dependency.srcSubpass = VK_SUBPASS_EXTERNAL;
    dependency.dstSubpass = 0;
    dependency.srcStageMask = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
    dependency.dstStageMask = VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT;
    dependency.srcAccessMask = 0;
    dependency.dstAccessMask = VK_ACCESS_COLOR_ATTACHMENT_WRITE_BIT;

    VkRenderPassCreateInfo rp_info = {};
    rp_info.sType = VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO;
    rp_info.attachmentCount = 1;
    rp_info.pAttachments = &color_attachment;
    rp_info.subpassCount = 1;
    rp_info.pSubpasses = &subpass;
    rp_info.dependencyCount = 1;
    rp_info.pDependencies = &dependency;

    VkResult res = sc_data.vk.vkCreateRenderPass(info->device, &rp_info, nullptr, &sc_data.render_pass);
    if (res != VK_SUCCESS) {
        fprintf(stderr, "[AdrenaLux HUD] Failed to create VkRenderPass: %d\n", res);
        return false;
    }

    // 2. Fetch swapchain images
    uint32_t image_count = 0;
    sc_data.vk.vkGetSwapchainImagesKHR(info->device, info->swapchain, &image_count, nullptr);
    if (image_count == 0) return false;

    std::vector<VkImage> swapchain_images(image_count);
    sc_data.vk.vkGetSwapchainImagesKHR(info->device, info->swapchain, &image_count, swapchain_images.data());

    sc_data.frames.resize(image_count);

    // 3. Command pool & command buffers
    VkCommandPoolCreateInfo cp_info = {};
    cp_info.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO;
    cp_info.flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;
    cp_info.queueFamilyIndex = info->queue_family;
    res = sc_data.vk.vkCreateCommandPool(info->device, &cp_info, nullptr, &sc_data.cmd_pool);
    if (res != VK_SUCCESS) return false;

    std::vector<VkCommandBuffer> cmd_buffers(image_count);
    VkCommandBufferAllocateInfo cb_info = {};
    cb_info.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO;
    cb_info.commandPool = sc_data.cmd_pool;
    cb_info.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
    cb_info.commandBufferCount = image_count;
    res = sc_data.vk.vkAllocateCommandBuffers(info->device, &cb_info, cmd_buffers.data());
    if (res != VK_SUCCESS) return false;

    VkSemaphoreCreateInfo sem_info = {};
    sem_info.sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO;

    VkFenceCreateInfo fence_info = {};
    fence_info.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO;
    fence_info.flags = VK_FENCE_CREATE_SIGNALED_BIT; // Signaled initially

    for (uint32_t i = 0; i < image_count; i++) {
        sc_data.frames[i].image = swapchain_images[i];
        sc_data.frames[i].cmd_buffer = cmd_buffers[i];

        VkImageViewCreateInfo view_info = {};
        view_info.sType = VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO;
        view_info.image = swapchain_images[i];
        view_info.viewType = VK_IMAGE_VIEW_TYPE_2D;
        view_info.format = info->format;
        view_info.components.r = VK_COMPONENT_SWIZZLE_R;
        view_info.components.g = VK_COMPONENT_SWIZZLE_G;
        view_info.components.b = VK_COMPONENT_SWIZZLE_B;
        view_info.components.a = VK_COMPONENT_SWIZZLE_A;
        view_info.subresourceRange.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
        view_info.subresourceRange.baseMipLevel = 0;
        view_info.subresourceRange.levelCount = 1;
        view_info.subresourceRange.baseArrayLayer = 0;
        view_info.subresourceRange.layerCount = 1;

        res = sc_data.vk.vkCreateImageView(info->device, &view_info, nullptr, &sc_data.frames[i].image_view);
        if (res != VK_SUCCESS) return false;

        VkFramebufferCreateInfo fb_info = {};
        fb_info.sType = VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO;
        fb_info.renderPass = sc_data.render_pass;
        fb_info.attachmentCount = 1;
        fb_info.pAttachments = &sc_data.frames[i].image_view;
        fb_info.width = info->width;
        fb_info.height = info->height;
        fb_info.layers = 1;

        res = sc_data.vk.vkCreateFramebuffer(info->device, &fb_info, nullptr, &sc_data.frames[i].framebuffer);
        if (res != VK_SUCCESS) return false;

        res = sc_data.vk.vkCreateSemaphore(info->device, &sem_info, nullptr, &sc_data.frames[i].render_complete_semaphore);
        if (res != VK_SUCCESS) return false;

        res = sc_data.vk.vkCreateFence(info->device, &fence_info, nullptr, &sc_data.frames[i].fence);
        if (res != VK_SUCCESS) return false;
    }

    g_swapchains[info->swapchain] = sc_data;
    return true;
}

extern "C" void adrenalux_hud_on_destroy_swapchain(VkSwapchainKHR swapchain) {
    std::lock_guard<std::mutex> lock(g_swapchain_mutex);
    auto it = g_swapchains.find(swapchain);
    if (it == g_swapchains.end()) return;

    SwapchainData& sc = it->second;
    sc.vk.vkDeviceWaitIdle(sc.info.device);

    for (auto& fd : sc.frames) {
        if (fd.render_complete_semaphore != VK_NULL_HANDLE) {
            sc.vk.vkDestroySemaphore(sc.info.device, fd.render_complete_semaphore, nullptr);
        }
        if (fd.fence != VK_NULL_HANDLE) {
            sc.vk.vkDestroyFence(sc.info.device, fd.fence, nullptr);
        }
        if (fd.framebuffer != VK_NULL_HANDLE) {
            sc.vk.vkDestroyFramebuffer(sc.info.device, fd.framebuffer, nullptr);
        }
        if (fd.image_view != VK_NULL_HANDLE) {
            sc.vk.vkDestroyImageView(sc.info.device, fd.image_view, nullptr);
        }
    }

    if (sc.cmd_pool != VK_NULL_HANDLE) {
        sc.vk.vkDestroyCommandPool(sc.info.device, sc.cmd_pool, nullptr);
    }
    if (sc.render_pass != VK_NULL_HANDLE) {
        sc.vk.vkDestroyRenderPass(sc.info.device, sc.render_pass, nullptr);
    }

    if (sc.imgui_initialized) {
        ImGui_ImplVulkan_Shutdown();
        g_imgui_initialized = false;
        sc.imgui_initialized = false;
    }

    g_swapchains.erase(it);
}

extern "C" VkResult adrenalux_hud_render_and_present(
    VkQueue queue,
    const VkPresentInfoKHR* present_info,
    float current_fps,
    const struct AdrenaluxTelemetry* telemetry,
    const struct AdrenaluxChillInfo* chill,
    AdrenaluxQueuePresentFn next_present
) {
    if (!present_info || present_info->swapchainCount == 0) {
        return next_present(queue, present_info);
    }

    std::unique_lock<std::mutex> lock(g_swapchain_mutex);

    VkSwapchainKHR swapchain = present_info->pSwapchains[0];
    auto it = g_swapchains.find(swapchain);
    if (it == g_swapchains.end()) {
        lock.unlock();
        return next_present(queue, present_info);
    }

    SwapchainData& sc = it->second;
    if (sc.info.queue == VK_NULL_HANDLE) {
        sc.info.queue = queue;
    }
    if (!sc.imgui_initialized) {
        if (!init_imgui_backend(sc)) {
            lock.unlock();
            return next_present(queue, present_info);
        }
    }

    uint32_t image_index = present_info->pImageIndices[0];
    if (image_index >= sc.frames.size()) {
        lock.unlock();
        return next_present(queue, present_info);
    }

    FrameData& fd = sc.frames[image_index];

    // Wait on fence for this frame to avoid GPU race
    sc.vk.vkWaitForFences(sc.info.device, 1, &fd.fence, VK_TRUE, 1000000000ULL);
    sc.vk.vkResetFences(sc.info.device, 1, &fd.fence);

    // Record overlay render command
    VkCommandBufferBeginInfo begin_info = {};
    begin_info.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO;
    begin_info.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
    sc.vk.vkBeginCommandBuffer(fd.cmd_buffer, &begin_info);

    VkRenderPassBeginInfo rp_begin = {};
    rp_begin.sType = VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO;
    rp_begin.renderPass = sc.render_pass;
    rp_begin.framebuffer = fd.framebuffer;
    rp_begin.renderArea.offset.x = 0;
    rp_begin.renderArea.offset.y = 0;
    rp_begin.renderArea.extent.width = sc.info.width;
    rp_begin.renderArea.extent.height = sc.info.height;
    rp_begin.clearValueCount = 0;
    rp_begin.pClearValues = nullptr;

    sc.vk.vkCmdBeginRenderPass(fd.cmd_buffer, &rp_begin, VK_SUBPASS_CONTENTS_INLINE);

    // Draw AdrenaLux HUD & Control Center
    draw_adrenalux_ui(fd.cmd_buffer, sc.info.width, sc.info.height, current_fps, telemetry, chill);

    sc.vk.vkCmdEndRenderPass(fd.cmd_buffer);
    sc.vk.vkEndCommandBuffer(fd.cmd_buffer);

    // Queue submission: wait for game render to finish, then signal overlay semaphore
    std::vector<VkPipelineStageFlags> wait_stages(present_info->waitSemaphoreCount, VK_PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT);

    VkSubmitInfo submit_info = {};
    submit_info.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO;
    submit_info.waitSemaphoreCount = present_info->waitSemaphoreCount;
    submit_info.pWaitSemaphores = present_info->pWaitSemaphores;
    submit_info.pWaitDstStageMask = wait_stages.empty() ? nullptr : wait_stages.data();
    submit_info.commandBufferCount = 1;
    submit_info.pCommandBuffers = &fd.cmd_buffer;
    submit_info.signalSemaphoreCount = 1;
    submit_info.pSignalSemaphores = &fd.render_complete_semaphore;

    sc.vk.vkQueueSubmit(queue, 1, &submit_info, fd.fence);

    // Modify presentation info to wait on our overlay semaphore
    VkPresentInfoKHR mod_present = *present_info;
    mod_present.waitSemaphoreCount = 1;
    mod_present.pWaitSemaphores = &fd.render_complete_semaphore;

    lock.unlock();
    return next_present(queue, &mod_present);
}

extern "C" void adrenalux_hud_shutdown(void) {
    std::lock_guard<std::mutex> lock(g_swapchain_mutex);
    while (!g_swapchains.empty()) {
        VkSwapchainKHR sc = g_swapchains.begin()->first;
        adrenalux_hud_on_destroy_swapchain(sc);
    }
    if (g_imgui_initialized) {
        ImGui_ImplVulkan_Shutdown();
        g_imgui_initialized = false;
    }
    if (ImGui::GetCurrentContext() != nullptr) {
        ImGui::DestroyContext();
    }
}
