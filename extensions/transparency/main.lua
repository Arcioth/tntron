-- Glass & Transparency (Per-color alpha sliders + transparent window compositor support)

local DEFAULTS = {
    enabled = true,
    base_alpha = 0.78,
    surface_alpha = 0.72,
    card_alpha = 0.68,
    player_bar_alpha = 0.85,
    accent_alpha = 1.00,
    text_alpha = 1.00,
    tint_hex = "",
    bg_mode = "none",
    bg_image_path = "",
    bg_image_alpha = 0.55,
    bg_fit = "cover",
    tint2_hex = "#1b1035",
    hide_ui = false,
    keep_player = true,
    keep_effects = true,
    keep_visualizer = true,
    keep_lyrics = false,
    keep_artwork = false,
    keep_topbar = false,
    keep_sidebar = false,
    keep_queue = false,
}

local BG_MODES = {
    { id = "none",     label = "None" },
    { id = "color",    label = "Solid Color" },
    { id = "gradient", label = "Gradient" },
    { id = "image",    label = "Custom Image" },
    { id = "cover",    label = "Album Cover" },
}

local BG_FITS = {
    { id = "cover",   label = "Fill Crop (Cover)" },
    { id = "contain", label = "Fit Inside (Contain)" },
    { id = "fill",    label = "Stretch" },
}

local PRESETS = {
    solid = {
        label = "Solid (100%)",
        cfg = { base_alpha = 1.0, surface_alpha = 1.0, card_alpha = 1.0, player_bar_alpha = 1.0, accent_alpha = 1.0, text_alpha = 1.0 },
    },
    frosted = {
        label = "Frosted (80%)",
        cfg = { base_alpha = 0.80, surface_alpha = 0.78, card_alpha = 0.74, player_bar_alpha = 0.88, accent_alpha = 1.0, text_alpha = 1.0 },
    },
    glass = {
        label = "Glass (58%)",
        cfg = { base_alpha = 0.55, surface_alpha = 0.58, card_alpha = 0.52, player_bar_alpha = 0.72, accent_alpha = 0.95, text_alpha = 1.0 },
    },
    ghost = {
        label = "Ghost (32%)",
        cfg = { base_alpha = 0.25, surface_alpha = 0.34, card_alpha = 0.30, player_bar_alpha = 0.48, accent_alpha = 0.90, text_alpha = 1.0 },
    },
    clear = {
        label = "Ultra Clear (12%)",
        cfg = { base_alpha = 0.08, surface_alpha = 0.16, card_alpha = 0.14, player_bar_alpha = 0.24, accent_alpha = 0.85, text_alpha = 0.98 },
    },
}

local st = {
    cfg = {},
    show_topbar_btn = true,
    tint_draft = "",
    tint2_draft = "#1b1035",
    bg_path_draft = "",
}

local function copy_cfg(src)
    local out = {}
    for k, v in pairs(DEFAULTS) do
        if type(src) == "table" and src[k] ~= nil then
            out[k] = src[k]
        else
            out[k] = v
        end
    end
    return out
end

local function push_cfg()
    tntron.storage.set("transparency_cfg", st.cfg)
    tntron.ui.set_transparency_config(st.cfg)
end

local function pct_info(name, role, alpha)
    local op = math.floor((tonumber(alpha) or 1.0) * 100 + 0.5)
    local tr = 100 - op
    return tntron.ui.card({
        eyebrow = role,
        title = string.format("%s — %d%% opaque", name, op),
        subtitle = string.format("%d%% transparent", tr),
        icon = "palette",
        accent = tr > 0,
    })
end

local function render()
    local preset_btns = {}
    for _, k in ipairs({ "solid", "frosted", "glass", "ghost", "clear" }) do
        local p = PRESETS[k]
        table.insert(preset_btns, tntron.ui.button("pre:" .. k, p.label, { icon = "layers" }))
    end
    local bg_mode_btns = {}
    for _, m in ipairs(BG_MODES) do
        table.insert(bg_mode_btns, tntron.ui.button("bgm:" .. m.id, m.label, { active = st.cfg.bg_mode == m.id }))
    end
    local bg_fit_btns = {}
    for _, f in ipairs(BG_FITS) do
        table.insert(bg_fit_btns, tntron.ui.button("bgf:" .. f.id, f.label, { active = st.cfg.bg_fit == f.id }))
    end

    local base_label = (st.cfg.tint_hex and st.cfg.tint_hex ~= "") and ("Base (" .. st.cfg.tint_hex .. ")") or "Window Base Color"

    tntron.ui.register_sidebar({
        id = "transparency",
        label = "Opacity",
        icon = "layers",
        widgets = {
            tntron.ui.row({
                tntron.ui.toggle("enabled", "Enable Window & Layer Transparency", st.cfg.enabled),
                tntron.ui.button("toggle_hide_ui", st.cfg.hide_ui and "Show Full UI" or "Hide UI (Effects + Player Focus)", { icon = "layers", accent = st.cfg.hide_ui }),
                tntron.ui.toggle("show_topbar_btn", "Show Hide-UI Button in Top-Right", st.show_topbar_btn),
                tntron.ui.button("clear_tint", "Clear Base Tint", { icon = "close" }),
            }),

            tntron.ui.heading("Focus Mode — Keep Visible When UI Is Hidden"),
            tntron.ui.text("Hide the rest of the interface to spotlight background effects, player controls, and only what you select (press Esc or top-right Show UI to exit).", true),
            tntron.ui.row({
                tntron.ui.toggle("keep_effects",    "Effects & Background", st.cfg.keep_effects),
                tntron.ui.toggle("keep_player",     "Player Bar",           st.cfg.keep_player),
                tntron.ui.toggle("keep_visualizer", "Center Visualizer",    st.cfg.keep_visualizer),
                tntron.ui.toggle("keep_artwork",    "Track Cover & Info",   st.cfg.keep_artwork),
                tntron.ui.toggle("keep_lyrics",     "Synced Lyrics",        st.cfg.keep_lyrics),
                tntron.ui.toggle("keep_topbar",     "Top Bar",              st.cfg.keep_topbar),
                tntron.ui.toggle("keep_sidebar",    "Navigation Sidebar",   st.cfg.keep_sidebar),
                tntron.ui.toggle("keep_queue",      "Queue Drawer",         st.cfg.keep_queue),
            }),

            tntron.ui.heading("Custom Background"),
            tntron.ui.row(bg_mode_btns),
            tntron.ui.row(bg_fit_btns),
            tntron.ui.slider("bg_image_alpha", "Custom Background Opacity", 0.05, 1.0, 0.02, st.cfg.bg_image_alpha, ""),
            tntron.ui.input("bg_image_path", "Custom Background Image Path (~/Pictures/wall.png)", st.bg_path_draft, {
                placeholder = "~/Pictures/wallpaper.png",
                submit_action = "set_bg_image",
                submit_label = "Use Image",
            }),
            tntron.ui.input("tint_hex", "Primary Tint / Gradient Start (#RRGGBB)", st.tint_draft, {
                placeholder = "#0b0e17",
                submit_action = "set_tint",
                submit_label = "Apply Color",
            }),
            tntron.ui.input("tint2_hex", "Gradient End Color (#RRGGBB)", st.tint2_draft, {
                placeholder = "#1b1035",
                submit_action = "set_tint2",
                submit_label = "Apply End",
            }),

            tntron.ui.heading("Transparency Presets"),
            tntron.ui.row(preset_btns),

            tntron.ui.heading("Color Opacity / Transparency Breakdown"),
            tntron.ui.grid(2, {
                pct_info(base_label, "WINDOW BASE", st.cfg.base_alpha),
                pct_info("Sidebar & Panels", "SURFACE", st.cfg.surface_alpha),
                pct_info("Cards & Rows", "CARD LAYER", st.cfg.card_alpha),
                pct_info("Bottom Playbar", "PLAYER BAR", st.cfg.player_bar_alpha),
                pct_info("Theme Accent", "HIGHLIGHT", st.cfg.accent_alpha),
                pct_info("Text & Icons", "FOREGROUND", st.cfg.text_alpha),
            }),

            tntron.ui.heading("Per-Color Transparency Controls"),
            tntron.ui.slider("base_alpha",       "Window Base Opacity",       0.0, 1.0, 0.02, st.cfg.base_alpha, ""),
            tntron.ui.slider("surface_alpha",    "Sidebar & Surface Opacity", 0.0, 1.0, 0.02, st.cfg.surface_alpha, ""),
            tntron.ui.slider("card_alpha",       "Cards & Rows Opacity",      0.0, 1.0, 0.02, st.cfg.card_alpha, ""),
            tntron.ui.slider("player_bar_alpha", "Bottom Player Bar Opacity", 0.0, 1.0, 0.02, st.cfg.player_bar_alpha, ""),
            tntron.ui.slider("accent_alpha",     "Accent Color Opacity",      0.1, 1.0, 0.02, st.cfg.accent_alpha, ""),
            tntron.ui.slider("text_alpha",       "Text & Foreground Opacity", 0.15, 1.0, 0.02, st.cfg.text_alpha, ""),
        },
    })
end

function on_load(_ctx)
    st.cfg = copy_cfg(tntron.storage.get("transparency_cfg", nil))
    st.cfg.hide_ui = false
    st.show_topbar_btn = tntron.storage.get("show_topbar_btn", true)
    st.tint_draft = st.cfg.tint_hex or ""
    st.tint2_draft = st.cfg.tint2_hex or "#1b1035"
    st.bg_path_draft = st.cfg.bg_image_path or ""
    push_cfg()
    tntron.ui.set_layout_config({ show_topbar_hide_ui = st.show_topbar_btn })
    render()
end

function on_unload(_ctx)
    tntron.ui.set_transparency_config({ enabled = false, hide_ui = false })
end

function on_action(_ctx, action_id, val)
    if action_id == "enabled" then
        st.cfg.enabled = (val == true)
        if not st.cfg.enabled then st.cfg.hide_ui = false end
        push_cfg()
    elseif action_id == "toggle_hide_ui" then
        st.cfg.hide_ui = not st.cfg.hide_ui
        if st.cfg.hide_ui then st.cfg.enabled = true end
        push_cfg()
    elseif action_id == "show_topbar_btn" then
        st.show_topbar_btn = (val == true)
        tntron.storage.set("show_topbar_btn", st.show_topbar_btn)
        tntron.ui.set_layout_config({ show_topbar_hide_ui = st.show_topbar_btn })
        render()
        return
    elseif action_id == "show_ui" then
        st.cfg.hide_ui = false
        push_cfg()
    elseif action_id == "keep_player" or action_id == "keep_effects" or action_id == "keep_visualizer"
        or action_id == "keep_lyrics" or action_id == "keep_artwork" or action_id == "keep_topbar"
        or action_id == "keep_sidebar" or action_id == "keep_queue"
    then
        st.cfg[action_id] = (val == true)
        push_cfg()
    elseif action_id == "base_alpha" or action_id == "surface_alpha" or action_id == "card_alpha"
        or action_id == "player_bar_alpha" or action_id == "accent_alpha" or action_id == "text_alpha"
        or action_id == "bg_image_alpha"
    then
        st.cfg[action_id] = tonumber(val) or st.cfg[action_id]
        st.cfg.enabled = true
        push_cfg()
    elseif action_id:sub(1, 4) == "pre:" then
        local pre = PRESETS[action_id:sub(5)]
        if pre then
            for k, v in pairs(pre.cfg) do st.cfg[k] = v end
            st.cfg.enabled = true
            push_cfg()
            tntron.ui.toast("Transparency: " .. pre.label)
        end
    elseif action_id:sub(1, 4) == "bgm:" then
        st.cfg.bg_mode = action_id:sub(5)
        st.cfg.enabled = true
        push_cfg()
    elseif action_id:sub(1, 4) == "bgf:" then
        st.cfg.bg_fit = action_id:sub(5)
        st.cfg.enabled = true
        push_cfg()
    elseif action_id == "set_bg_image" then
        local p = tostring(val or st.bg_path_draft or ""):match("^%s*(.-)%s*$")
        st.bg_path_draft = p
        st.cfg.bg_image_path = p
        if p ~= "" then st.cfg.bg_mode = "image" end
        st.cfg.enabled = true
        push_cfg()
    elseif action_id == "set_tint" then
        local hex = tostring(val or st.tint_draft or ""):gsub("%s+", "")
        if hex ~= "" and hex:sub(1, 1) ~= "#" then hex = "#" .. hex end
        st.tint_draft = hex
        st.cfg.tint_hex = hex
        st.cfg.enabled = true
        push_cfg()
    elseif action_id == "set_tint2" then
        local hex = tostring(val or st.tint2_draft or ""):gsub("%s+", "")
        if hex ~= "" and hex:sub(1, 1) ~= "#" then hex = "#" .. hex end
        st.tint2_draft = hex
        st.cfg.tint2_hex = hex
        if st.cfg.bg_mode == "none" then st.cfg.bg_mode = "gradient" end
        st.cfg.enabled = true
        push_cfg()
    elseif action_id == "clear_tint" then
        st.tint_draft = ""
        st.cfg.tint_hex = ""
        st.cfg.bg_mode = "none"
        push_cfg()
    end
    render()
end
