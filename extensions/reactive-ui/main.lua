-- Reactive UI (Bass glow, side background EQ styles, blur, drum-zoom particles, vocal hue loop, .tnt export/import)

local DEFAULTS = {
    enabled = true,
    bass_glow = true,
    bass_intensity = 0.90,
    bass_radius = 0.85,
    eq_enabled = true,
    eq_style = "bars",
    eq_sides = "both",
    eq_width = 80,
    eq_opacity = 0.62,
    blur = 0.48,
    particles_enabled = true,
    particle_count = 52,
    particle_zoom = 1.35,
    glow_particles_enabled = true,
    glow_particle_intensity = 0.95,
    glow_particle_size = 1.15,
    glow_particle_style = "bokeh",
    aurora_enabled = false,
    custom_particle_enabled = false,
    custom_particle_path = "",
    custom_particle_size = 36,
    custom_particle_count = 18,
    hue_loop = true,
    hue_speed = 0.45,
    hue_vocal_react = 1.40,
    hue_saturation = 0.85,
    bg_dim = 0.72,
}

local QUICK_PROFILES = {
    neon = {
        label = "Neon Bass",
        cfg = {
            enabled = true, bass_glow = true, bass_intensity = 1.25, bass_radius = 1.10,
            eq_enabled = true, eq_style = "bars", eq_sides = "both", eq_width = 92, eq_opacity = 0.72,
            blur = 0.42, particles_enabled = true, particle_count = 64, particle_zoom = 1.65,
            hue_loop = true, hue_speed = 0.55, hue_vocal_react = 1.60, hue_saturation = 0.92, bg_dim = 0.68,
        },
    },
    cyber = {
        label = "Cyber Wave",
        cfg = {
            enabled = true, bass_glow = true, bass_intensity = 0.95, bass_radius = 0.90,
            eq_enabled = true, eq_style = "wave", eq_sides = "both", eq_width = 104, eq_opacity = 0.78,
            blur = 0.30, particles_enabled = true, particle_count = 80, particle_zoom = 1.95,
            hue_loop = true, hue_speed = 0.85, hue_vocal_react = 1.85, hue_saturation = 0.95, bg_dim = 0.62,
        },
    },
    dreamy = {
        label = "Dreamy Blur",
        cfg = {
            enabled = true, bass_glow = true, bass_intensity = 1.15, bass_radius = 1.45,
            eq_enabled = true, eq_style = "minimal", eq_sides = "both", eq_width = 68, eq_opacity = 0.50,
            blur = 0.90, particles_enabled = true, particle_count = 36, particle_zoom = 0.95,
            hue_loop = true, hue_speed = 0.30, hue_vocal_react = 1.20, hue_saturation = 0.75, bg_dim = 0.78,
        },
    },
    blocks = {
        label = "Arcade Blocks",
        cfg = {
            enabled = true, bass_glow = true, bass_intensity = 0.80, bass_radius = 0.70,
            eq_enabled = true, eq_style = "blocks", eq_sides = "both", eq_width = 86, eq_opacity = 0.68,
            blur = 0.20, particles_enabled = true, particle_count = 58, particle_zoom = 2.10,
            glow_particles_enabled = true, glow_particle_intensity = 1.10, glow_particle_size = 0.95, glow_particle_style = "sparks",
            hue_loop = true, hue_speed = 0.40, hue_vocal_react = 1.45, hue_saturation = 0.90, bg_dim = 0.74,
        },
    },
    luminous = {
        label = "Luminous Glow",
        cfg = {
            enabled = true, bass_glow = true, bass_intensity = 1.15, bass_radius = 1.25,
            eq_enabled = true, eq_style = "mirrored", eq_sides = "both", eq_width = 92, eq_opacity = 0.65,
            blur = 0.64, particles_enabled = true, particle_count = 72, particle_zoom = 1.55,
            glow_particles_enabled = true, glow_particle_intensity = 1.65, glow_particle_size = 1.50, glow_particle_style = "fireflies",
            hue_loop = true, hue_speed = 0.50, hue_vocal_react = 1.55, hue_saturation = 0.92, bg_dim = 0.64,
        },
    },
}

local GLOW_STYLES = {
    { id = "glowy",             label = "Glowy Particles" },
    { id = "bokeh",             label = "Bokeh Orbs" },
    { id = "spiral",            label = "Spiral" },
    { id = "infinity8",         label = "Infinity 8" },
    { id = "anime_speed_lines", label = "Anime Speed Lines" },
    { id = "bubbles",           label = "Bubbles" },
    { id = "flare",             label = "Lens Flare" },
    { id = "glamour",           label = "Glamour" },
    { id = "aurora",            label = "Aurora" },
    { id = "fireflies",         label = "Fireflies" },
    { id = "starburst",         label = "Starburst" },
    { id = "sparks",            label = "Comet Sparks" },
    { id = "custom_sprite",     label = "Custom .svg/.png/.jxl" },
}

local EQ_STYLES = {
    { id = "bars",     label = "Bars" },
    { id = "mirrored", label = "Mirrored" },
    { id = "wave",     label = "Wave" },
    { id = "blocks",   label = "Blocks" },
    { id = "minimal",  label = "Minimal" },
}

local EQ_SIDES = {
    { id = "both",  label = "Both Sides" },
    { id = "left",  label = "Left Edge" },
    { id = "right", label = "Right Edge" },
}

local st = {
    cfg = {},
    tnt_draft = "my_reactive",
    import_draft = "",
    share_paste_draft = "",
    last_tnt_msg = nil,
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
    tntron.storage.set("reactive_cfg", st.cfg)
    tntron.ui.set_reactive_config(st.cfg)
end

local function band_stats(pb)
    if not pb or type(pb.bands) ~= "table" or #pb.bands < 12 then
        return 0, 0, 0
    end
    local b = pb.bands
    local bass = math.floor((((b[1] or 0) + (b[2] or 0) + (b[3] or 0) + (b[4] or 0)) / 4.0) * 100 + 0.5)
    local vocal = math.floor((((b[5] or 0) + (b[6] or 0) + (b[7] or 0) + (b[8] or 0) + (b[9] or 0) + (b[10] or 0)) / 6.0) * 100 + 0.5)
    local drum = math.floor((tonumber(pb.beat_pulse) or 0) * 100 + 0.5)
    return bass, vocal, drum
end

local function render(pb)
    local bass_pct, vocal_pct, drum_pct = band_stats(pb)
    local saved_tnts = tntron.fs.list_tnt() or {}

    local style_btns = {}
    for _, s in ipairs(EQ_STYLES) do
        table.insert(style_btns, tntron.ui.button("eqs:" .. s.id, s.label, { active = st.cfg.eq_style == s.id }))
    end
    local side_btns = {}
    for _, sd in ipairs(EQ_SIDES) do
        table.insert(side_btns, tntron.ui.button("side:" .. sd.id, sd.label, { active = st.cfg.eq_sides == sd.id }))
    end
    local glow_style_btns = {}
    for _, gs in ipairs(GLOW_STYLES) do
        table.insert(glow_style_btns, tntron.ui.button("glw:" .. gs.id, gs.label, { active = st.cfg.glow_particle_style == gs.id }))
    end

    local profile_btns = {}
    for key, p in pairs(QUICK_PROFILES) do
        table.insert(profile_btns, tntron.ui.button("prof:" .. key, p.label, { icon = "wave" }))
    end

    local tnt_row = {}
    for _, item in ipairs(saved_tnts) do
        table.insert(tnt_row, tntron.ui.button("load_tnt:" .. item.name, item.name .. ".tnt", { icon = "download" }))
    end

    local widgets = {
        tntron.ui.row({
            tntron.ui.toggle("enabled", "Enable Reactive Screen Background", st.cfg.enabled),
            tntron.ui.button("reset_def", "Reset Defaults", { icon = "refresh" }),
        }),
        tntron.ui.row({
            tntron.ui.badge(string.format("Bass %d%%", bass_pct), { icon = "wave", accent = st.cfg.bass_glow and bass_pct > 25 }),
            tntron.ui.badge(string.format("Drum %d%%", drum_pct), { icon = "disc", accent = st.cfg.particles_enabled and drum_pct > 25 }),
            tntron.ui.badge(string.format("Vocals %d%%", vocal_pct), { icon = "music", accent = st.cfg.hue_loop and vocal_pct > 20 }),
        }),

        tntron.ui.heading("Quick Presets"),
        tntron.ui.row(profile_btns),

        tntron.ui.heading("Bass Edge Glow & Blur"),
        tntron.ui.toggle("bass_glow", "Glow screen edges & corners on bass", st.cfg.bass_glow),
        tntron.ui.slider("bass_intensity", "Bass Glow Intensity", 0.0, 2.0, 0.05, st.cfg.bass_intensity, "x"),
        tntron.ui.slider("bass_radius", "Bass Glow Spread", 0.15, 2.0, 0.05, st.cfg.bass_radius, "x"),
        tntron.ui.slider("blur", "Blur / Diffusion Softness", 0.0, 1.0, 0.02, st.cfg.blur, ""),
        tntron.ui.slider("bg_dim", "UI Backdrop Dimming", 0.25, 0.96, 0.02, st.cfg.bg_dim, ""),

        tntron.ui.heading("Side Background EQ"),
        tntron.ui.toggle("eq_enabled", "Show reactive EQ on screen sides", st.cfg.eq_enabled),
        tntron.ui.row(style_btns),
        tntron.ui.row(side_btns),
        tntron.ui.slider("eq_width", "Side EQ Reach", 20, 170, 2, st.cfg.eq_width, "px"),
        tntron.ui.slider("eq_opacity", "Side EQ Opacity", 0.05, 1.0, 0.05, st.cfg.eq_opacity, ""),

        tntron.ui.heading("Particles, Effect Shapes & Custom .svg/.png/.jxl Sprites"),
        tntron.ui.row({
            tntron.ui.toggle("particles_enabled", "Drum Zoom Particles", st.cfg.particles_enabled),
            tntron.ui.toggle("glow_particles_enabled", "Enable Styled / Glow Particles", st.cfg.glow_particles_enabled),
            tntron.ui.toggle("aurora_enabled", "Layer Aurora Curtains", st.cfg.aurora_enabled),
            tntron.ui.toggle("custom_particle_enabled", "Layer Custom Sprite", st.cfg.custom_particle_enabled),
        }),
        tntron.ui.row(glow_style_btns),
        tntron.ui.slider("particle_count", "Particle Count", 0, 140, 4, st.cfg.particle_count, ""),
        tntron.ui.slider("particle_zoom", "Drum Zoom Strength", 0.1, 3.0, 0.05, st.cfg.particle_zoom, "x"),
        tntron.ui.slider("glow_particle_intensity", "Particle Glow / Brightness", 0.0, 2.5, 0.05, st.cfg.glow_particle_intensity, "x"),
        tntron.ui.slider("glow_particle_size", "Particle Scale", 0.4, 2.5, 0.05, st.cfg.glow_particle_size, "x"),
        tntron.ui.input("custom_particle_path", "Custom Particle Image (.svg, .png, .jxl)", st.cfg.custom_particle_path or "", {
            placeholder = "~/Pictures/star.svg or icon.png or art.jxl",
            submit_action = "set_custom_sprite",
            submit_label = "Use Sprite",
        }),
        tntron.ui.slider("custom_particle_size", "Custom Sprite Size", 8, 160, 4, st.cfg.custom_particle_size, "px"),
        tntron.ui.slider("custom_particle_count", "Custom Sprite Count", 1, 64, 1, st.cfg.custom_particle_count, ""),

        tntron.ui.heading("Vocal-Reactive Hue Loop"),
        tntron.ui.toggle("hue_loop", "Cycle hue loop driven by vocals", st.cfg.hue_loop),
        tntron.ui.slider("hue_speed", "Base Hue Loop Speed", 0.0, 2.5, 0.05, st.cfg.hue_speed, "x"),
        tntron.ui.slider("hue_vocal_react", "Vocal Hue Reactivity", 0.0, 3.0, 0.05, st.cfg.hue_vocal_react, "x"),
        tntron.ui.slider("hue_saturation", "Hue Saturation", 0.0, 1.0, 0.05, st.cfg.hue_saturation, ""),

        tntron.ui.heading("Share Copy/Paste & .tnt Config File Save/Export"),
        tntron.ui.text("Copy/paste share JSON uses the exact .tnt file structure, or save to ~/.config/tntron/tnt/<name>.tnt.", true),
        tntron.ui.row({
            tntron.ui.button("copy_share", "Copy Share Code", { icon = "download", accent = true }),
            tntron.ui.button("paste_clip", "Paste from Clipboard", { icon = "refresh" }),
        }),
        tntron.ui.input("share_paste", "Or paste .tnt / share JSON here", st.share_paste_draft, {
            placeholder = "{ \"tnt_format\": \"tntron-reactive-v1\", ... }",
            submit_action = "apply_share_json",
            submit_label = "Apply JSON",
        }),
        tntron.ui.input("tnt_name", "Save / Export .tnt", st.tnt_draft, {
            placeholder = "preset_name or ~/Documents/preset.tnt",
            submit_action = "export_tnt",
            submit_label = "Export .tnt",
        }),
        tntron.ui.input("tnt_import", "Import .tnt path", st.import_draft, {
            placeholder = "preset_name or ~/Documents/preset.tnt",
            submit_action = "import_tnt",
            submit_label = "Load .tnt",
        }),
    }

    if #tnt_row > 0 then
        table.insert(widgets, tntron.ui.text("Saved .tnt Files:", true))
        table.insert(widgets, tntron.ui.row(tnt_row))
    end
    if st.last_tnt_msg then
        table.insert(widgets, tntron.ui.badge(st.last_tnt_msg, { icon = "sliders", accent = true }))
    end

    tntron.ui.register_sidebar({
        id = "reactive-ui",
        label = "Reactive UI",
        icon = "palette",
        widgets = widgets,
    })
end

function on_load(_ctx)
    local saved = tntron.storage.get("reactive_cfg", nil)
    st.cfg = copy_defaults(saved)
    push_cfg()
    render(tntron.playback.get())
end

function on_unload(_ctx)
    local off = copy_defaults(st.cfg)
    off.enabled = false
    tntron.ui.set_reactive_config(off)
end

function on_tick(_ctx)
    render(tntron.playback.get())
end

function on_action(_ctx, action_id, val)
    if action_id == "enabled" or action_id == "bass_glow" or action_id == "eq_enabled"
        or action_id == "particles_enabled" or action_id == "glow_particles_enabled" or action_id == "hue_loop"
        or action_id == "aurora_enabled" or action_id == "custom_particle_enabled"
    then
        st.cfg[action_id] = (val == true)
        push_cfg()
    elseif action_id == "bass_intensity" or action_id == "bass_radius" or action_id == "blur"
        or action_id == "bg_dim" or action_id == "eq_width" or action_id == "eq_opacity"
        or action_id == "particle_count" or action_id == "particle_zoom"
        or action_id == "glow_particle_intensity" or action_id == "glow_particle_size"
        or action_id == "custom_particle_size" or action_id == "custom_particle_count"
        or action_id == "hue_speed" or action_id == "hue_vocal_react" or action_id == "hue_saturation"
    then
        st.cfg[action_id] = tonumber(val) or st.cfg[action_id]
        push_cfg()
    elseif action_id == "set_custom_sprite" then
        local p = tostring(val or st.cfg.custom_particle_path or ""):match("^%s*(.-)%s*$")
        st.cfg.custom_particle_path = p
        if p ~= "" then
            st.cfg.custom_particle_enabled = true
        end
        push_cfg()
        st.last_tnt_msg = p ~= "" and ("Sprite set: " .. p) or "Cleared custom particle sprite"
    elseif action_id:sub(1, 4) == "glw:" then
        st.cfg.glow_particle_style = action_id:sub(5)
        st.cfg.glow_particles_enabled = true
        push_cfg()
    elseif action_id:sub(1, 4) == "eqs:" then
        st.cfg.eq_style = action_id:sub(5)
        push_cfg()
    elseif action_id:sub(1, 5) == "side:" then
        st.cfg.eq_sides = action_id:sub(6)
        push_cfg()
    elseif action_id:sub(1, 5) == "prof:" then
        local prof = QUICK_PROFILES[action_id:sub(6)]
        if prof then
            st.cfg = copy_defaults(prof.cfg)
            push_cfg()
            tntron.ui.toast("Applied Reactive profile: " .. prof.label)
        end
    elseif action_id == "reset_def" then
        st.cfg = copy_defaults(nil)
        push_cfg()
        tntron.ui.toast("Reactive UI reset to defaults")
    elseif action_id == "copy_share" then
        local txt, _ = tntron.fs.format_tnt_share(st.cfg, "tntron-reactive-v1", st.tnt_draft ~= "" and st.tnt_draft or "reactive_ui")
        if txt then
            st.share_paste_draft = txt
            tntron.ui.copy_clipboard(txt)
            st.last_tnt_msg = "Copied .tnt share JSON to clipboard!"
            tntron.ui.toast("Copied Reactive UI share code")
        end
    elseif action_id == "paste_clip" then
        tntron.ui.paste_clipboard()
    elseif action_id == "clipboard_pasted" or action_id == "apply_share_json" then
        local raw = tostring(val or st.share_paste_draft or "")
        st.share_paste_draft = raw
        local loaded, err = tntron.fs.parse_tnt_share(raw)
        if type(loaded) == "table" then
            st.cfg = copy_defaults(loaded)
            push_cfg()
            st.last_tnt_msg = "Applied shared .tnt configuration!"
            tntron.ui.toast(st.last_tnt_msg)
        else
            st.last_tnt_msg = "Share parse error: " .. tostring(err)
            tntron.ui.toast(st.last_tnt_msg)
        end
    elseif action_id == "export_tnt" then
        local target = tostring(val or st.tnt_draft or ""):match("^%s*(.-)%s*$")
        if target ~= "" then
            st.tnt_draft = target
            local saved_path, err = tntron.fs.save_tnt(target, st.cfg)
            if saved_path then
                st.last_tnt_msg = "Exported: " .. saved_path
                tntron.ui.toast("Exported .tnt -> " .. saved_path)
            else
                st.last_tnt_msg = "Export error: " .. tostring(err)
                tntron.ui.toast(st.last_tnt_msg)
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
                st.last_tnt_msg = "Loaded .tnt: " .. target
                tntron.ui.toast("Loaded .tnt settings")
            else
                st.last_tnt_msg = "Load error: " .. tostring(err)
                tntron.ui.toast(st.last_tnt_msg)
            end
        end
    elseif action_id:sub(1, 9) == "load_tnt:" then
        local name = action_id:sub(10)
        local loaded, err = tntron.fs.load_tnt(name)
        if type(loaded) == "table" then
            st.cfg = copy_defaults(loaded)
            push_cfg()
            st.last_tnt_msg = "Loaded: " .. name .. ".tnt"
            tntron.ui.toast("Applied " .. name .. ".tnt")
        else
            st.last_tnt_msg = "Load error: " .. tostring(err)
        end
    end
    render(tntron.playback.get())
end
