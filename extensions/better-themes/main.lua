-- Better Themes (Tunotron developer palettes + Openify searchable theme picker + Brand Logo Customizer)

local BUILTINS = {
    { slug = "openify",  label = "Openify" },
    { slug = "dark",     label = "Pure Dark" },
    { slug = "cyberpunk",label = "Cyberpunk" },
    { slug = "terminal", label = "Terminal" },
    { slug = "clouds",   label = "Clouds" },
    { slug = "pharoh",   label = "Pharoh" },
    { slug = "turk",     label = "Crimson Moon" },
}

local PALETTES = {
    {
        slug = "catppuccin", label = "Catppuccin",
        base = "#11111b", sidebar = "#181825", main = "#1e1e2e",
        card = "#24273a", card_hover = "#313244",
        text = "#cdd6f4", text_sub = "#a6adc8", accent = "#cba6f7",
        logo_color = "#cba6f7",
    },
    {
        slug = "nord", label = "Nord",
        base = "#242933", sidebar = "#2e3440", main = "#2e3440",
        card = "#3b4252", card_hover = "#434c5e",
        text = "#eceff4", text_sub = "#d8dee9", accent = "#88c0d0",
        logo_color = "#88c0d0",
    },
    {
        slug = "dracula", label = "Dracula",
        base = "#191a21", sidebar = "#21222c", main = "#282a36",
        card = "#343746", card_hover = "#44475a",
        text = "#f8f8f2", text_sub = "#9aa2c4", accent = "#bd93f9",
        logo_color = "#ff79c6",
    },
    {
        slug = "gruvbox", label = "Gruvbox",
        base = "#1d2021", sidebar = "#282828", main = "#282828",
        card = "#32302f", card_hover = "#3c3836",
        text = "#ebdbb2", text_sub = "#a89984", accent = "#fabd2f",
        logo_color = "#fe8019",
    },
    {
        slug = "tokyo-night", label = "Tokyo Night",
        base = "#16161e", sidebar = "#1a1b26", main = "#1a1b26",
        card = "#24283b", card_hover = "#292e42",
        text = "#c0caf5", text_sub = "#7aa2f7", accent = "#7dcfff",
        logo_color = "#7dcfff",
    },
    {
        slug = "monokai", label = "Monokai",
        base = "#1e1f1c", sidebar = "#272822", main = "#272822",
        card = "#32332c", card_hover = "#3e3d32",
        text = "#f8f8f2", text_sub = "#a6a28c", accent = "#a6e22e",
        logo_color = "#a6e22e",
    },
}

local LOGO_MODES = {
    { id = "themed",       label = "Theme Accent" },
    { id = "monochrome",   label = "Mono Contrast" },
    { id = "custom_color", label = "Custom Hex" },
    { id = "custom_image", label = "Custom File (.svg/.png/.jxl)" },
}

local LOGO_FRAMES = {
    { id = "clean", label = "Clean" },
    { id = "badge", label = "Badge" },
    { id = "glow",  label = "Neon Glow" },
}

local QUICK_COLORS = { "#1db954", "#00f0ff", "#cba6f7", "#ff79c6", "#fabd2f", "#ffffff" }

local st = {
    selected = "openify",
    filter = "",
    logo_enabled = true,
    logo_mode = "themed",
    logo_hex = "#1db954",
    logo_path = "",
    logo_frame = "clean",
    logo_size = 22,
    logo_pulse = true,
}

local function matches_filter(label)
    if st.filter == "" then return true end
    return label:lower():find(st.filter:lower(), 1, true) ~= nil
end

local function push_logo_config()
    tntron.ui.set_logo_config({
        enabled = st.logo_enabled,
        mode = st.logo_mode,
        custom_hex = st.logo_hex,
        custom_path = st.logo_path,
        frame_style = st.logo_frame,
        size_px = st.logo_size,
        beat_pulse = st.logo_pulse,
    })
end

local function save_logo_state()
    tntron.storage.set("logo_enabled", st.logo_enabled)
    tntron.storage.set("logo_mode", st.logo_mode)
    tntron.storage.set("logo_hex", st.logo_hex)
    tntron.storage.set("logo_path", st.logo_path)
    tntron.storage.set("logo_frame", st.logo_frame)
    tntron.storage.set("logo_size", st.logo_size)
    tntron.storage.set("logo_pulse", st.logo_pulse)
end

local function render_settings()
    local builtin_btns = {}
    for _, t in ipairs(BUILTINS) do
        if matches_filter(t.label) then
            table.insert(builtin_btns, tntron.ui.button("use:" .. t.slug, t.label, {
                icon = "palette",
                active = (st.selected == t.slug),
            }))
        end
    end

    local dev_btns = {}
    for _, t in ipairs(PALETTES) do
        if matches_filter(t.label) then
            table.insert(dev_btns, tntron.ui.button("use:" .. t.slug, t.label, {
                icon = "palette",
                active = (st.selected == t.slug),
            }))
        end
    end

    local widgets = {
        tntron.ui.input("theme_filter", "Search Themes", st.filter, {
            placeholder = "Filter e.g. Nord, Dracula, Cyberpunk...",
            submit_action = "apply_filter",
            submit_label = "Filter",
        }),
    }

    if #builtin_btns > 0 then
        table.insert(widgets, tntron.ui.text("Built-in Themes", true))
        table.insert(widgets, tntron.ui.row(builtin_btns))
    end
    if #dev_btns > 0 then
        table.insert(widgets, tntron.ui.text("Developer Palettes", true))
        table.insert(widgets, tntron.ui.row(dev_btns))
    end
    if #builtin_btns == 0 and #dev_btns == 0 then
        table.insert(widgets, tntron.ui.row({
            tntron.ui.text("No themes match '" .. st.filter .. "'.", true),
            tntron.ui.button("clear_filter", "Clear Filter", { icon = "close" }),
        }))
    elseif st.filter ~= "" then
        table.insert(widgets, tntron.ui.button("clear_filter", "Clear Filter", { icon = "close" }))
    end

    table.insert(widgets, tntron.ui.heading("Theme Brand Logo (tntron Mark)"))
    table.insert(widgets, tntron.ui.text(
        "Every theme includes the signature dissolving-play tntron logo. Customize its tint, frame, beat pulse, or replace it with your own .svg/.png/.jxl.",
        true
    ))
    table.insert(widgets, tntron.ui.row({
        tntron.ui.toggle("logo_enabled", "Show Brand Logo", st.logo_enabled),
        tntron.ui.toggle("logo_pulse", "Beat Pulse", st.logo_pulse),
        tntron.ui.button("logo_reset", "Reset Default Logo", { icon = "refresh" }),
    }))

    local mode_btns = {}
    for _, m in ipairs(LOGO_MODES) do
        table.insert(mode_btns, tntron.ui.button("logo_mode:" .. m.id, m.label, {
            icon = "layers",
            active = (st.logo_mode == m.id),
        }))
    end
    table.insert(widgets, tntron.ui.text("Logo Color & Source Mode", true))
    table.insert(widgets, tntron.ui.row(mode_btns))

    local frame_btns = {}
    for _, f in ipairs(LOGO_FRAMES) do
        table.insert(frame_btns, tntron.ui.button("logo_frame:" .. f.id, f.label, {
            icon = "star",
            active = (st.logo_frame == f.id),
        }))
    end
    table.insert(widgets, tntron.ui.text("Logo Frame Style", true))
    table.insert(widgets, tntron.ui.row(frame_btns))

    table.insert(widgets, tntron.ui.slider("logo_size", "Sidebar Logo Size", 14, 40, 2, st.logo_size, "px"))

    local hex_swatches = {}
    for _, hx in ipairs(QUICK_COLORS) do
        table.insert(hex_swatches, tntron.ui.button("logo_hex:" .. hx, hx, {
            icon = "palette",
            active = (st.logo_mode == "custom_color" and st.logo_hex:lower() == hx:lower()),
        }))
    end
    table.insert(widgets, tntron.ui.row(hex_swatches))

    table.insert(widgets, tntron.ui.input("logo_hex_input", "Custom Logo Tint Hex", st.logo_hex, {
        placeholder = "#1db954",
        submit_action = "apply_logo_hex",
        submit_label = "Apply Hex",
    }))
    table.insert(widgets, tntron.ui.input("logo_path_input", "Custom Logo File (.svg / .png / .jxl)", st.logo_path, {
        placeholder = "/path/to/custom-logo.svg",
        submit_action = "apply_logo_path",
        submit_label = "Use File",
    }))

    tntron.ui.register_settings({
        id = "better-themes",
        title = "Better Themes & Brand Logo",
        widgets = widgets,
    })
end

function on_load(_ctx)
    for _, t in ipairs(PALETTES) do
        tntron.themes.register(t)
    end
    st.selected = tntron.storage.get("selected_slug", "openify")
    st.logo_enabled = tntron.storage.get("logo_enabled", true)
    st.logo_mode = tntron.storage.get("logo_mode", "themed")
    st.logo_hex = tntron.storage.get("logo_hex", "#1db954")
    st.logo_path = tntron.storage.get("logo_path", "")
    st.logo_frame = tntron.storage.get("logo_frame", "clean")
    st.logo_size = tntron.storage.get("logo_size", 22)
    st.logo_pulse = tntron.storage.get("logo_pulse", true)
    push_logo_config()
    render_settings()
end

function on_action(_ctx, action_id, val)
    if action_id == "apply_filter" then
        st.filter = type(val) == "string" and val or ""
    elseif action_id == "clear_filter" then
        st.filter = ""
    elseif action_id == "logo_enabled" then
        st.logo_enabled = (val == true)
    elseif action_id == "logo_pulse" then
        st.logo_pulse = (val == true)
    elseif action_id == "logo_size" then
        local n = tonumber(val)
        if n then st.logo_size = math.max(14, math.min(40, math.floor(n + 0.5))) end
    elseif action_id == "apply_logo_hex" then
        local hx = type(val) == "string" and val or ""
        if hx ~= "" then
            st.logo_hex = hx
            st.logo_mode = "custom_color"
            tntron.ui.toast("Logo tint: " .. hx)
        end
    elseif action_id == "apply_logo_path" then
        local p = type(val) == "string" and val or ""
        st.logo_path = p
        if p ~= "" then
            st.logo_mode = "custom_image"
            tntron.ui.toast("Custom logo loaded")
        else
            st.logo_mode = "themed"
        end
    elseif action_id == "logo_reset" then
        st.logo_enabled = true
        st.logo_mode = "themed"
        st.logo_hex = "#1db954"
        st.logo_path = ""
        st.logo_frame = "clean"
        st.logo_size = 22
        st.logo_pulse = true
        tntron.ui.toast("Logo reset to theme default")
    else
        local slug = action_id:match("^use:(.+)$")
        local lm = action_id:match("^logo_mode:(.+)$")
        local lf = action_id:match("^logo_frame:(.+)$")
        local lh = action_id:match("^logo_hex:(.+)$")
        if slug then
            st.selected = slug
            tntron.themes.set_active(slug)
            tntron.storage.set("selected_slug", slug)
            tntron.ui.toast("Theme switched: " .. slug)
        elseif lm then
            st.logo_mode = lm
            if lm == "custom_image" and st.logo_path == "" then
                tntron.ui.toast("Enter a .svg/.png/.jxl path below and click Use File")
            end
        elseif lf then
            st.logo_frame = lf
        elseif lh then
            st.logo_hex = lh
            st.logo_mode = "custom_color"
        end
    end
    save_logo_state()
    push_logo_config()
    render_settings()
end
