-- Button Packs & Customizer (Openify, Win XP, Liquid Glass, Cyberpunk + full .tnt sharing)

local DEFAULTS = {
    enabled = true,
    preset = "openify",
    apply_pills = true,
    apply_primary = true,
    apply_glyphs = true,
    apply_nav = true,
    apply_rows = true,
    apply_tiles = true,
    round_tl = 18.0,
    round_tr = 18.0,
    round_br = 18.0,
    round_bl = 18.0,
    crop_tl = false,
    crop_tr = false,
    crop_br = false,
    crop_bl = false,
    crop_size = 8.0,
    hue_shift_deg = 0.0,
    hue_strength = 0.0,
    custom_tint_hex = "",
    fill_opacity = 1.0,
    outline_enabled = true,
    outline_width = 1.0,
    outline_hex = "",
    outline_alpha = 0.45,
    inline_glow = 0.0,
    outer_glow = 0.0,
    bevel_emboss = 0.0,
    gradient_enabled = false,
    gradient_strength = 0.45,
    gradient_angle_deg = 180.0,
    gradient_top_hex = "",
    gradient_bottom_hex = "",
    reactive_hover = true,
    hover_hue_shift_deg = 0.0,
    hover_glow_boost = 0.55,
    hover_outline_boost = 0.35,
    hover_brightness = 0.14,
}

local PRESETS = {
    openify = {
        label = "Openify (Default)",
        cfg = {
            enabled = true, preset = "openify",
            round_tl = 20, round_tr = 20, round_br = 20, round_bl = 20,
            crop_tl = false, crop_tr = false, crop_br = false, crop_bl = false, crop_size = 8,
            hue_shift_deg = 0, hue_strength = 0.0, custom_tint_hex = "", fill_opacity = 1.0,
            outline_enabled = true, outline_width = 1.0, outline_hex = "", outline_alpha = 0.45,
            inline_glow = 0.0, outer_glow = 0.0, bevel_emboss = 0.0,
            gradient_enabled = false, gradient_strength = 0.40, gradient_angle_deg = 180,
            gradient_top_hex = "", gradient_bottom_hex = "",
            reactive_hover = true, hover_hue_shift_deg = 0, hover_glow_boost = 0.35, hover_outline_boost = 0.25, hover_brightness = 0.14,
        },
    },
    winxp = {
        label = "Windows XP",
        cfg = {
            enabled = true, preset = "winxp",
            round_tl = 5, round_tr = 5, round_br = 4, round_bl = 4,
            crop_tl = false, crop_tr = false, crop_br = false, crop_bl = false, crop_size = 6,
            hue_shift_deg = 0, hue_strength = 0.65, custom_tint_hex = "#2d68d8", fill_opacity = 0.96,
            outline_enabled = true, outline_width = 1.6, outline_hex = "#0b2e75", outline_alpha = 0.95,
            inline_glow = 0.32, outer_glow = 0.25, bevel_emboss = 0.92,
            gradient_enabled = true, gradient_strength = 0.85, gradient_angle_deg = 180,
            gradient_top_hex = "#73a8ff", gradient_bottom_hex = "#1c50b8",
            reactive_hover = true, hover_hue_shift_deg = 18, hover_glow_boost = 0.85, hover_outline_boost = 0.55, hover_brightness = 0.22,
        },
    },
    liquid_glass = {
        label = "Liquid Glass",
        cfg = {
            enabled = true, preset = "liquid_glass",
            round_tl = 22, round_tr = 22, round_br = 22, round_bl = 22,
            crop_tl = false, crop_tr = false, crop_br = false, crop_bl = false, crop_size = 8,
            hue_shift_deg = 12, hue_strength = 0.38, custom_tint_hex = "#a5d8ff", fill_opacity = 0.30,
            outline_enabled = true, outline_width = 1.2, outline_hex = "#ffffff", outline_alpha = 0.58,
            inline_glow = 0.72, outer_glow = 0.45, bevel_emboss = 0.35,
            gradient_enabled = true, gradient_strength = 0.80, gradient_angle_deg = 155,
            gradient_top_hex = "#ffffff", gradient_bottom_hex = "#67e8f9",
            reactive_hover = true, hover_hue_shift_deg = 28, hover_glow_boost = 1.05, hover_outline_boost = 0.60, hover_brightness = 0.26,
        },
    },
    cyberpunk = {
        label = "Cyberpunk",
        cfg = {
            enabled = true, preset = "cyberpunk",
            round_tl = 0, round_tr = 6, round_br = 0, round_bl = 6,
            crop_tl = true, crop_tr = false, crop_br = true, crop_bl = false, crop_size = 9,
            hue_shift_deg = 16, hue_strength = 0.75, custom_tint_hex = "#00f0ff", fill_opacity = 0.82,
            outline_enabled = true, outline_width = 1.8, outline_hex = "#ff2a85", outline_alpha = 0.92,
            inline_glow = 0.88, outer_glow = 0.72, bevel_emboss = 0.15,
            gradient_enabled = true, gradient_strength = 0.75, gradient_angle_deg = 135,
            gradient_top_hex = "#190a2e", gradient_bottom_hex = "#062c43",
            reactive_hover = true, hover_hue_shift_deg = 130, hover_glow_boost = 1.65, hover_outline_boost = 0.85, hover_brightness = 0.30,
        },
    },
}

local st = {
    cfg = {},
    tnt_draft = "my_buttons",
    import_draft = "",
    share_paste_draft = "",
    last_share_code = "",
    status_msg = nil,
}

local function copy_defaults(src)
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
    tntron.storage.set("button_pack_cfg", st.cfg)
    tntron.ui.set_button_config(st.cfg)
end

local function make_share_json()
    local txt, _ = tntron.fs.format_tnt_share(st.cfg, "tntron-buttons-v1", st.cfg.preset or "buttons")
    return txt or ""
end

local function render()
    local saved_tnts = tntron.fs.list_tnt() or {}
    local preset_btns = {}
    for _, key in ipairs({ "openify", "winxp", "liquid_glass", "cyberpunk" }) do
        local p = PRESETS[key]
        table.insert(preset_btns, tntron.ui.button("pre:" .. key, p.label, { active = st.cfg.preset == key, icon = "palette" }))
    end

    local tnt_row = {}
    for _, item in ipairs(saved_tnts) do
        table.insert(tnt_row, tntron.ui.button("load_tnt:" .. item.name, item.name .. ".tnt", { icon = "download" }))
    end

    local widgets = {
        tntron.ui.row({
            tntron.ui.toggle("enabled", "Enable Button Customizer", st.cfg.enabled),
            tntron.ui.button("reset_def", "Reset Openify Default", { icon = "refresh" }),
        }),

        tntron.ui.heading("Premade Button Styles (All Fully Editable Below)"),
        tntron.ui.row(preset_btns),

        tntron.ui.heading("Target Button & Clickable Types"),
        tntron.ui.row({
            tntron.ui.toggle("apply_pills",   "Pill & Filter Buttons", st.cfg.apply_pills),
            tntron.ui.toggle("apply_primary", "Primary Action Buttons", st.cfg.apply_primary),
            tntron.ui.toggle("apply_glyphs",  "Transport & Icon Glyphs", st.cfg.apply_glyphs),
            tntron.ui.toggle("apply_nav",     "Sidebar & Folder Items", st.cfg.apply_nav),
            tntron.ui.toggle("apply_rows",    "Track Rows & Menus", st.cfg.apply_rows),
            tntron.ui.toggle("apply_tiles",   "Tiles, Cards & Swatches", st.cfg.apply_tiles),
        }),

        tntron.ui.heading("Corner Rounding & Selective Cropping"),
        tntron.ui.row({
            tntron.ui.button("corner:pill",  "All Round (20px)", {}),
            tntron.ui.button("corner:soft",  "Soft (8px)",       {}),
            tntron.ui.button("corner:sharp", "Sharp (0px)",      {}),
            tntron.ui.button("corner:cyber", "Cyber Cut (TL+BR)",{}),
            tntron.ui.button("corner:alt",   "Split Cut (TR+BL)",{}),
        }),
        tntron.ui.row({
            tntron.ui.toggle("crop_tl", "Crop Top-Left",     st.cfg.crop_tl),
            tntron.ui.toggle("crop_tr", "Crop Top-Right",    st.cfg.crop_tr),
            tntron.ui.toggle("crop_br", "Crop Bottom-Right", st.cfg.crop_br),
            tntron.ui.toggle("crop_bl", "Crop Bottom-Left",  st.cfg.crop_bl),
        }),
        tntron.ui.slider("crop_size", "Cropped Corner Depth", 3, 22, 1, st.cfg.crop_size, "px"),
        tntron.ui.slider("round_tl",  "Top-Left Radius",      0, 32, 1, st.cfg.round_tl, "px"),
        tntron.ui.slider("round_tr",  "Top-Right Radius",     0, 32, 1, st.cfg.round_tr, "px"),
        tntron.ui.slider("round_br",  "Bottom-Right Radius",  0, 32, 1, st.cfg.round_br, "px"),
        tntron.ui.slider("round_bl",  "Bottom-Left Radius",   0, 32, 1, st.cfg.round_bl, "px"),

        tntron.ui.heading("Button Hue, Tint & Opacity"),
        tntron.ui.slider("hue_shift_deg", "Base Hue Rotation", -180, 180, 5, st.cfg.hue_shift_deg, "deg"),
        tntron.ui.slider("hue_strength",  "Hue Tint Mix",      0.0, 1.0, 0.05, st.cfg.hue_strength, ""),
        tntron.ui.slider("fill_opacity",  "Button Fill Opacity", 0.05, 1.0, 0.05, st.cfg.fill_opacity, ""),
        tntron.ui.input("custom_tint_hex", "Custom Button Hue Color (#RRGGBB)", st.cfg.custom_tint_hex or "", {
            placeholder = "#00f0ff (leave blank for theme accent)",
            submit_action = "set_tint_hex",
            submit_label = "Apply",
        }),

        tntron.ui.heading("Outline, Inline Glow & Bevel Emboss"),
        tntron.ui.toggle("outline_enabled", "Show Button Outline", st.cfg.outline_enabled),
        tntron.ui.slider("outline_width", "Outline Thickness", 0.5, 4.0, 0.25, st.cfg.outline_width, "px"),
        tntron.ui.slider("outline_alpha", "Outline Opacity",   0.0, 1.0, 0.05, st.cfg.outline_alpha, ""),
        tntron.ui.input("outline_hex", "Outline Color (#RRGGBB)", st.cfg.outline_hex or "", {
            placeholder = "#ff2a85 (optional)",
            submit_action = "set_outline_hex",
            submit_label = "Set Outline",
        }),
        tntron.ui.slider("inline_glow",  "Inline Inner Glow", 0.0, 1.5, 0.05, st.cfg.inline_glow, "x"),
        tntron.ui.slider("outer_glow",   "Outer Halo Glow",   0.0, 1.5, 0.05, st.cfg.outer_glow, "x"),
        tntron.ui.slider("bevel_emboss", "3D Bevel Emboss",   0.0, 1.0, 0.05, st.cfg.bevel_emboss, ""),

        tntron.ui.heading("Gradient Overlay"),
        tntron.ui.toggle("gradient_enabled", "Enable Linear Gradient Overlay", st.cfg.gradient_enabled),
        tntron.ui.slider("gradient_strength",  "Gradient Contrast", 0.0, 1.0, 0.05, st.cfg.gradient_strength, ""),
        tntron.ui.slider("gradient_angle_deg", "Gradient Angle",    0, 360, 15, st.cfg.gradient_angle_deg, "deg"),
        tntron.ui.input("gradient_top_hex", "Gradient Start (#RRGGBB)", st.cfg.gradient_top_hex or "", {
            placeholder = "#73a8ff",
            submit_action = "set_grad_top",
            submit_label = "Set Top",
        }),
        tntron.ui.input("gradient_bottom_hex", "Gradient End (#RRGGBB)", st.cfg.gradient_bottom_hex or "", {
            placeholder = "#1c50b8",
            submit_action = "set_grad_bot",
            submit_label = "Set End",
        }),

        tntron.ui.heading("Reactive Hover State"),
        tntron.ui.toggle("reactive_hover", "Animate Hue & Glow on Hover", st.cfg.reactive_hover),
        tntron.ui.slider("hover_hue_shift_deg", "Hover Hue Shift",    -180, 180, 5, st.cfg.hover_hue_shift_deg, "deg"),
        tntron.ui.slider("hover_glow_boost",    "Hover Glow Surge",    0.0, 2.0, 0.05, st.cfg.hover_glow_boost, "x"),
        tntron.ui.slider("hover_outline_boost", "Hover Outline Boost", 0.0, 1.0, 0.05, st.cfg.hover_outline_boost, ""),
        tntron.ui.slider("hover_brightness",    "Hover Fill Lift",     0.0, 0.7, 0.05, st.cfg.hover_brightness, ""),

        tntron.ui.heading("Share Copy/Paste & .tnt File Export"),
        tntron.ui.text("Copy & paste share codes follow the exact same .tnt envelope JSON structure.", true),
        tntron.ui.row({
            tntron.ui.button("copy_share", "Copy Share Code", { icon = "download", accent = true }),
            tntron.ui.button("paste_clip", "Paste from Clipboard", { icon = "refresh" }),
        }),
        tntron.ui.input("share_paste", "Or paste .tnt / share JSON here", st.share_paste_draft, {
            placeholder = "{ \"tnt_format\": \"tntron-buttons-v1\", ... }",
            submit_action = "apply_share_json",
            submit_label = "Apply JSON",
        }),
        tntron.ui.input("tnt_name", "Export .tnt Preset", st.tnt_draft, {
            placeholder = "my_buttons or ~/Documents/buttons.tnt",
            submit_action = "export_tnt",
            submit_label = "Export .tnt",
        }),
        tntron.ui.input("tnt_import", "Import .tnt File", st.import_draft, {
            placeholder = "preset_name or ~/Documents/buttons.tnt",
            submit_action = "import_tnt",
            submit_label = "Load .tnt",
        }),
    }

    if #tnt_row > 0 then
        table.insert(widgets, tntron.ui.text("Saved .tnt Files:", true))
        table.insert(widgets, tntron.ui.row(tnt_row))
    end
    if st.status_msg then
        table.insert(widgets, tntron.ui.badge(st.status_msg, { icon = "sliders", accent = true }))
    end

    tntron.ui.register_sidebar({
        id = "button-packs",
        label = "Buttons",
        icon = "sliders",
        widgets = widgets,
    })
end

function on_load(_ctx)
    st.cfg = copy_defaults(tntron.storage.get("button_pack_cfg", nil))
    push_cfg()
    render()
end

function on_unload(_ctx)
    tntron.ui.set_button_config({ enabled = false })
end

local function normalize_hex(v)
    local h = tostring(v or ""):gsub("%s+", "")
    if h ~= "" and h:sub(1, 1) ~= "#" then h = "#" .. h end
    return h
end

function on_action(_ctx, action_id, val)
    if action_id == "enabled" or action_id == "outline_enabled" or action_id == "gradient_enabled"
        or action_id == "reactive_hover"
        or action_id == "crop_tl" or action_id == "crop_tr" or action_id == "crop_br" or action_id == "crop_bl"
        or action_id == "apply_pills" or action_id == "apply_primary" or action_id == "apply_glyphs"
        or action_id == "apply_nav" or action_id == "apply_rows" or action_id == "apply_tiles"
    then
        st.cfg[action_id] = (val == true)
        if action_id ~= "enabled" then st.cfg.preset = "custom" end
        push_cfg()
    elseif action_id == "crop_size" or action_id == "round_tl" or action_id == "round_tr"
        or action_id == "round_br" or action_id == "round_bl"
        or action_id == "hue_shift_deg" or action_id == "hue_strength" or action_id == "fill_opacity"
        or action_id == "outline_width" or action_id == "outline_alpha"
        or action_id == "inline_glow" or action_id == "outer_glow" or action_id == "bevel_emboss"
        or action_id == "gradient_strength" or action_id == "gradient_angle_deg"
        or action_id == "hover_hue_shift_deg" or action_id == "hover_glow_boost"
        or action_id == "hover_outline_boost" or action_id == "hover_brightness"
    then
        st.cfg[action_id] = tonumber(val) or st.cfg[action_id]
        st.cfg.preset = "custom"
        st.cfg.enabled = true
        push_cfg()
    elseif action_id:sub(1, 4) == "pre:" then
        local pre = PRESETS[action_id:sub(5)]
        if pre then
            for k, v in pairs(pre.cfg) do st.cfg[k] = v end
            push_cfg()
            st.status_msg = "Applied style: " .. pre.label
            tntron.ui.toast(st.status_msg)
        end
    elseif action_id:sub(1, 7) == "corner:" then
        local mode = action_id:sub(8)
        st.cfg.preset = "custom"
        st.cfg.enabled = true
        if mode == "pill" then
            st.cfg.round_tl, st.cfg.round_tr, st.cfg.round_br, st.cfg.round_bl = 20, 20, 20, 20
            st.cfg.crop_tl, st.cfg.crop_tr, st.cfg.crop_br, st.cfg.crop_bl = false, false, false, false
        elseif mode == "soft" then
            st.cfg.round_tl, st.cfg.round_tr, st.cfg.round_br, st.cfg.round_bl = 8, 8, 8, 8
            st.cfg.crop_tl, st.cfg.crop_tr, st.cfg.crop_br, st.cfg.crop_bl = false, false, false, false
        elseif mode == "sharp" then
            st.cfg.round_tl, st.cfg.round_tr, st.cfg.round_br, st.cfg.round_bl = 0, 0, 0, 0
            st.cfg.crop_tl, st.cfg.crop_tr, st.cfg.crop_br, st.cfg.crop_bl = false, false, false, false
        elseif mode == "cyber" then
            st.cfg.round_tl, st.cfg.round_tr, st.cfg.round_br, st.cfg.round_bl = 0, 6, 0, 6
            st.cfg.crop_tl, st.cfg.crop_tr, st.cfg.crop_br, st.cfg.crop_bl = true, false, true, false
        elseif mode == "alt" then
            st.cfg.round_tl, st.cfg.round_tr, st.cfg.round_br, st.cfg.round_bl = 6, 0, 6, 0
            st.cfg.crop_tl, st.cfg.crop_tr, st.cfg.crop_br, st.cfg.crop_bl = false, true, false, true
        end
        push_cfg()
    elseif action_id == "set_tint_hex" then
        st.cfg.custom_tint_hex = normalize_hex(val)
        st.cfg.preset = "custom"
        push_cfg()
    elseif action_id == "set_outline_hex" then
        st.cfg.outline_hex = normalize_hex(val)
        st.cfg.preset = "custom"
        push_cfg()
    elseif action_id == "set_grad_top" then
        st.cfg.gradient_top_hex = normalize_hex(val)
        st.cfg.gradient_enabled = true
        st.cfg.preset = "custom"
        push_cfg()
    elseif action_id == "set_grad_bot" then
        st.cfg.gradient_bottom_hex = normalize_hex(val)
        st.cfg.gradient_enabled = true
        st.cfg.preset = "custom"
        push_cfg()
    elseif action_id == "reset_def" then
        st.cfg = copy_defaults(PRESETS.openify.cfg)
        push_cfg()
        st.status_msg = "Reset to Openify default buttons"
    elseif action_id == "copy_share" then
        local txt = make_share_json()
        st.share_paste_draft = txt
        tntron.ui.copy_clipboard(txt)
        st.status_msg = "Copied button .tnt share JSON to clipboard!"
        tntron.ui.toast("Copied button pack share code")
    elseif action_id == "paste_clip" then
        tntron.ui.paste_clipboard()
    elseif action_id == "clipboard_pasted" or action_id == "apply_share_json" then
        local raw = tostring(val or st.share_paste_draft or "")
        st.share_paste_draft = raw
        local loaded, err = tntron.fs.parse_tnt_share(raw)
        if type(loaded) == "table" then
            st.cfg = copy_defaults(loaded)
            push_cfg()
            st.status_msg = "Applied shared button pack configuration!"
            tntron.ui.toast(st.status_msg)
        else
            st.status_msg = "Share parse error: " .. tostring(err)
            tntron.ui.toast(st.status_msg)
        end
    elseif action_id == "export_tnt" then
        local target = tostring(val or st.tnt_draft or ""):match("^%s*(.-)%s*$")
        if target ~= "" then
            st.tnt_draft = target
            local saved_path, err = tntron.fs.save_tnt(target, st.cfg, "tntron-buttons-v1")
            if saved_path then
                st.status_msg = "Exported: " .. saved_path
                tntron.ui.toast("Exported .tnt -> " .. saved_path)
            else
                st.status_msg = "Export error: " .. tostring(err)
            end
        end
    elseif action_id == "import_tnt" then
        local target = tostring(val or st.import_draft or ""):match("^%s*(.-)%s*$")
        if target ~= "" then
            st.import_draft = target
            local loaded, err = tntron.fs.load_tnt(target)
            if type(loaded) == "table" then
                st.cfg = copy_defaults(loaded)
                push_cfg()
                st.status_msg = "Loaded .tnt: " .. target
            else
                st.status_msg = "Load error: " .. tostring(err)
            end
        end
    elseif action_id:sub(1, 9) == "load_tnt:" then
        local name = action_id:sub(10)
        local loaded, err = tntron.fs.load_tnt(name)
        if type(loaded) == "table" then
            st.cfg = copy_defaults(loaded)
            push_cfg()
            st.status_msg = "Applied " .. name .. ".tnt"
        else
            st.status_msg = "Load error: " .. tostring(err)
        end
    end
    render()
end
