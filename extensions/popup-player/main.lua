-- Popup Terminal Player (kitty, alacritty, wezterm, foot, ghostty, konsole, gnome-terminal, xterm, etc.)

local TERMINALS = {
    { id = "auto",           label = "Auto" },
    { id = "kitty",          label = "Kitty" },
    { id = "alacritty",      label = "Alacritty" },
    { id = "wezterm",        label = "WezTerm" },
    { id = "foot",           label = "Foot (Wayland)" },
    { id = "ghostty",        label = "Ghostty" },
    { id = "xfce4-terminal", label = "XFCE4" },
    { id = "konsole",        label = "Konsole (KDE)" },
    { id = "gnome-terminal", label = "GNOME" },
    { id = "kgx",            label = "GNOME Console" },
    { id = "mate-terminal",  label = "MATE" },
    { id = "qterminal",      label = "QTerminal (LXQt)" },
    { id = "lxterminal",     label = "LXTerminal" },
    { id = "cosmic-term",    label = "COSMIC Term" },
    { id = "tilix",          label = "Tilix" },
    { id = "terminator",     label = "Terminator" },
    { id = "xterm",          label = "XTerm (X11)" },
    { id = "urxvt",          label = "URxvt" },
    { id = "st",             label = "st" },
}

local LAYOUTS = {
    { id = "full",    label = "Full (Art + Lyrics + EQ)" },
    { id = "compact", label = "Compact Bar" },
    { id = "art",     label = "Picture Focus" },
    { id = "lyrics",  label = "Lyrics Focus" },
}

local REACTIVE_MODES = {
    { id = "hue_loop",   label = "Vocal Hue Loop" },
    { id = "beat_pulse", label = "Beat Pulse" },
    { id = "vocal_wave", label = "Vocal Wave" },
    { id = "accent",     label = "Classic Accent" },
    { id = "custom",     label = "Custom Color" },
}

local ART_MODES = {
    { id = "cover", label = "Cover Picture" },
    { id = "ascii", label = "ASCII Art" },
    { id = "both",  label = "Cover + Fallback ASCII" },
    { id = "none",  label = "No Art" },
}

local ASCII_PRESETS = {
    { id = "vinyl",       label = "Spinning Vinyl" },
    { id = "cassette",    label = "Retro Cassette" },
    { id = "boombox",     label = "Boombox" },
    { id = "note",        label = "Floating Notes" },
    { id = "orbit",       label = "Cosmic Orbit" },
    { id = "wave",        label = "Neon Wave" },
    { id = "eq_tower",    label = "EQ Towers" },
    { id = "cover_ascii", label = "Cover -> ASCII" },
    { id = "auto",        label = "Auto Cycle" },
}

local LIST_SOURCES = {
    { id = "upcoming", label = "Upcoming + Queue" },
    { id = "playlist", label = "Playlist Context" },
    { id = "queue",    label = "Queue Only" },
}

local EQ_STYLES = {
    { id = "blocks",   label = "Blocks" },
    { id = "mirrored", label = "Mirrored" },
    { id = "wave",     label = "Wave" },
    { id = "bars",     label = "Bars" },
    { id = "dots",     label = "Braille Dots" },
}

local PROGRESS_STYLES = {
    { id = "line",    label = "Line (━─)" },
    { id = "blocks",  label = "Blocks (█░)" },
    { id = "dots",    label = "Dots (●·)" },
    { id = "braille", label = "Braille (⣿⣀)" },
}

local BORDER_STYLES = {
    { id = "rounded", label = "Rounded" },
    { id = "double",  label = "Double" },
    { id = "heavy",   label = "Heavy" },
    { id = "minimal", label = "Frameless" },
}

local DEFAULTS = {
    enabled = true,
    terminal = "auto",
    layout = "full",
    show_picture = true,
    show_lyrics = true,
    show_eq = true,
    reactive_mode = "hue_loop",
    custom_hex = "#38bdf8",
    transparent_bg = true,
    auto_open_on_track = false,
    cols = 84,
    rows = 24,
    wm_class = "tntron-popup",
    art_mode = "cover",
    ascii_preset = "vinyl",
    show_list = true,
    list_source = "upcoming",
    list_max_items = 6,
    eq_bars_style = "blocks",
    eq_rows = 2,
    progress_style = "line",
    border_style = "rounded",
    show_album = true,
    show_badges = true,
    show_footer_hints = true,
    auto_resize_layout = true,
}

local st = { cfg = {}, hex_draft = "#38bdf8", class_draft = "tntron-popup" }

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
    tntron.storage.set("popup_cfg", st.cfg)
    tntron.ui.set_popup_config(st.cfg)
end

local function make_btns(items, prefix, cur_val)
    local out = {}
    for _, item in ipairs(items) do
        table.insert(out, tntron.ui.button(prefix .. item.id, item.label, { active = cur_val == item.id }))
    end
    return out
end

local function render(pb)
    local term_btns = make_btns(TERMINALS, "term:", st.cfg.terminal)
    local layout_btns = make_btns(LAYOUTS, "lay:", st.cfg.layout)
    local mode_btns = make_btns(REACTIVE_MODES, "rm:", st.cfg.reactive_mode)
    local art_btns = make_btns(ART_MODES, "artm:", st.cfg.art_mode)
    local ascii_btns = make_btns(ASCII_PRESETS, "asc:", st.cfg.ascii_preset)
    local list_src_btns = make_btns(LIST_SOURCES, "lsrc:", st.cfg.list_source)
    local eq_style_btns = make_btns(EQ_STYLES, "eqst:", st.cfg.eq_bars_style)
    local prog_btns = make_btns(PROGRESS_STYLES, "prog:", st.cfg.progress_style)
    local border_btns = make_btns(BORDER_STYLES, "brd:", st.cfg.border_style)

    local now_txt = (pb and pb.title and pb.title ~= "")
        and (pb.title .. " — " .. (pb.artist or ""))
        or "Nothing playing yet"

    tntron.ui.register_sidebar({
        id = "popup-player",
        label = "Popup",
        icon = "keyboard",
        widgets = {
            tntron.ui.row({
                tntron.ui.toggle("enabled", "Enable Popup Player", st.cfg.enabled),
                tntron.ui.button("launch", "Launch Popup", { icon = "play", accent = st.cfg.enabled }),
                tntron.ui.button("close",  "Close Popup",  { icon = "close" }),
            }),
            tntron.ui.text("Now playing preview: " .. now_txt, true),
            tntron.ui.heading("Terminal Emulator"),
            tntron.ui.row(term_btns),
            tntron.ui.heading("Popup Layout & Window Auto-Resize"),
            tntron.ui.row(layout_btns),
            tntron.ui.row({
                tntron.ui.toggle("auto_resize_layout", "Auto-Resize Layout to Window", st.cfg.auto_resize_layout),
                tntron.ui.toggle("show_picture",   "Show Cover / Art Panel", st.cfg.show_picture),
                tntron.ui.toggle("show_lyrics",    "Show Synced Lyrics", st.cfg.show_lyrics),
                tntron.ui.toggle("show_eq",        "Show Reactive EQ", st.cfg.show_eq),
                tntron.ui.toggle("show_album",     "Show Album", st.cfg.show_album),
                tntron.ui.toggle("show_badges",    "Show Status Badges", st.cfg.show_badges),
                tntron.ui.toggle("show_footer_hints", "Show Shortcut Hints", st.cfg.show_footer_hints),
                tntron.ui.toggle("transparent_bg", "Transparent Background", st.cfg.transparent_bg),
                tntron.ui.toggle("auto_open_on_track", "Auto-open on Track Change", st.cfg.auto_open_on_track),
            }),

            tntron.ui.heading("Cover & ASCII Art Modes"),
            tntron.ui.row(art_btns),
            tntron.ui.row(ascii_btns),

            tntron.ui.heading("Track List Pane"),
            tntron.ui.toggle("show_list", "Show Interactive Playlist / Queue List ([j/k] + [Enter] to play)", st.cfg.show_list),
            tntron.ui.row(list_src_btns),
            tntron.ui.slider("list_max_items", "Max List Rows", 1, 20, 1, st.cfg.list_max_items, "rows"),

            tntron.ui.heading("Spectrum EQ, Progress & Borders"),
            tntron.ui.row(eq_style_btns),
            tntron.ui.slider("eq_rows", "EQ Height (Rows)", 1, 5, 1, st.cfg.eq_rows, "rows"),
            tntron.ui.row(prog_btns),
            tntron.ui.row(border_btns),

            tntron.ui.heading("Music-Reactive Color Style"),
            tntron.ui.row(mode_btns),
            tntron.ui.input("custom_hex", "Custom Color (#RRGGBB)", st.hex_draft, {
                placeholder = "#38bdf8",
                submit_action = "set_hex",
                submit_label = "Set",
            }),
            tntron.ui.heading("Initial Popup Window Geometry"),
            tntron.ui.slider("cols", "Launch Columns (Width)",  48, 160, 4, st.cfg.cols, "cols"),
            tntron.ui.slider("rows", "Launch Rows (Height)",   14, 48,  2, st.cfg.rows, "rows"),
            tntron.ui.input("wm_class", "Window Class (for Hyprland/WM float rules)", st.class_draft, {
                placeholder = "tntron-popup",
                submit_action = "set_class",
                submit_label = "Set Class",
            }),
        },
    })
end

function on_load(_ctx)
    st.cfg = copy_cfg(tntron.storage.get("popup_cfg", nil))
    st.hex_draft = st.cfg.custom_hex or "#38bdf8"
    st.class_draft = st.cfg.wm_class or "tntron-popup"
    push_cfg()
    render(tntron.playback.get())
end

function on_unload(_ctx)
    tntron.ui.set_popup_config({ enabled = false })
    tntron.ui.close_popup()
end

function on_event(_ctx, event_name, _payload)
    if event_name == "trackChange" and st.cfg.enabled and st.cfg.auto_open_on_track then
        tntron.ui.launch_popup()
    end
    render(tntron.playback.get())
end

function on_action(_ctx, action_id, val)
    if action_id == "enabled" or action_id == "show_picture" or action_id == "show_lyrics"
        or action_id == "show_eq" or action_id == "transparent_bg" or action_id == "auto_open_on_track"
        or action_id == "show_list" or action_id == "show_album" or action_id == "show_badges"
        or action_id == "show_footer_hints" or action_id == "auto_resize_layout"
    then
        st.cfg[action_id] = (val == true)
        push_cfg()
        if action_id == "enabled" and not st.cfg.enabled then
            tntron.ui.close_popup()
        end
    elseif action_id == "cols" or action_id == "rows"
        or action_id == "list_max_items" or action_id == "eq_rows"
    then
        st.cfg[action_id] = math.floor(tonumber(val) or st.cfg[action_id])
        push_cfg()
    elseif action_id:sub(1, 5) == "term:" then
        st.cfg.terminal = action_id:sub(6)
        push_cfg()
    elseif action_id:sub(1, 4) == "lay:" then
        st.cfg.layout = action_id:sub(5)
        push_cfg()
    elseif action_id:sub(1, 3) == "rm:" then
        st.cfg.reactive_mode = action_id:sub(4)
        push_cfg()
    elseif action_id:sub(1, 5) == "artm:" then
        st.cfg.art_mode = action_id:sub(6)
        push_cfg()
    elseif action_id:sub(1, 4) == "asc:" then
        st.cfg.ascii_preset = action_id:sub(5)
        push_cfg()
    elseif action_id:sub(1, 5) == "lsrc:" then
        st.cfg.list_source = action_id:sub(6)
        push_cfg()
    elseif action_id:sub(1, 5) == "eqst:" then
        st.cfg.eq_bars_style = action_id:sub(6)
        push_cfg()
    elseif action_id:sub(1, 5) == "prog:" then
        st.cfg.progress_style = action_id:sub(6)
        push_cfg()
    elseif action_id:sub(1, 4) == "brd:" then
        st.cfg.border_style = action_id:sub(5)
        push_cfg()
    elseif action_id == "set_hex" then
        local c = tostring(val or st.hex_draft or ""):gsub("%s+", "")
        if c ~= "" and c:sub(1, 1) ~= "#" then c = "#" .. c end
        st.hex_draft = c
        st.cfg.custom_hex = c
        st.cfg.reactive_mode = "custom"
        push_cfg()
    elseif action_id == "set_class" then
        local w = tostring(val or st.class_draft or ""):match("^%s*(.-)%s*$")
        if w ~= "" then
            st.class_draft = w
            st.cfg.wm_class = w
            push_cfg()
        end
    elseif action_id == "launch" then
        push_cfg()
        tntron.ui.launch_popup()
    elseif action_id == "close" then
        tntron.ui.close_popup()
    end
    render(tntron.playback.get())
end
