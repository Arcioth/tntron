-- Better Layouts (Tunotron-style full layout customizer + densities + zones)

local st = {
    layout = "table",
    density = "Normal",
    badge = true,
    sidebar_side = "left",
    show_banner = true,
    show_recent_strip = true,
    show_col_album = true,
    show_col_duration = true,
    show_telemetry_strip = false,
    show_player_viz = true,
    toast_zone = "top-right",
}

local LAYOUT_LABELS = {
    table = "Table",
    cards = "Cards",
    gallery = "Gallery",
    minimal = "Minimal",
}

local function apply_to_host()
    tntron.ui.set_layout(st.layout)
    tntron.ui.set_density(st.density)
    tntron.ui.set_layout_config({
        sidebar_side = st.sidebar_side,
        show_banner = st.show_banner,
        show_recent_strip = st.show_recent_strip,
        show_col_album = st.show_col_album,
        show_col_duration = st.show_col_duration,
        show_telemetry_strip = st.show_telemetry_strip,
        show_player_viz = st.show_player_viz,
        toast_zone = st.toast_zone,
    })
end

local function render()
    tntron.ui.clear_topbar_slot()

    tntron.ui.register_settings({
        id = "better-layouts",
        title = "Better Layouts & Tunotron HUD",
        widgets = {
            tntron.ui.text("Customize Openify Home view, density, sidebar placement, visible columns, and toast corner.", true),
            tntron.ui.row({
                tntron.ui.button("l:table",   "Table",   { icon = "layers", active = st.layout == "table" }),
                tntron.ui.button("l:cards",   "Cards",   { icon = "layers", active = st.layout == "cards" }),
                tntron.ui.button("l:gallery", "Gallery", { icon = "layers", active = st.layout == "gallery" }),
                tntron.ui.button("l:minimal", "Minimal", { icon = "layers", active = st.layout == "minimal" }),
            }),
            tntron.ui.row({
                tntron.ui.button("d:Compact", "Compact Rows", { active = st.density == "Compact" }),
                tntron.ui.button("d:Normal",  "Normal Rows",  { active = st.density == "Normal" }),
                tntron.ui.button("d:Big",     "Large Rows",   { active = st.density == "Big" }),
            }),
            tntron.ui.row({
                tntron.ui.button("side:left",  "Sidebar Left",  { active = st.sidebar_side == "left" }),
                tntron.ui.button("side:right", "Sidebar Right", { active = st.sidebar_side == "right" }),
            }),
            tntron.ui.row({
                tntron.ui.toggle("show_banner",     "Collection Banner", st.show_banner),
                tntron.ui.toggle("show_recent",     "Recents Strip",     st.show_recent_strip),
                tntron.ui.toggle("show_col_album",  "Album Column",      st.show_col_album),
                tntron.ui.toggle("show_col_dur",    "Duration Column",   st.show_col_duration),
                tntron.ui.toggle("show_player_viz", "Playbar Visualizer", st.show_player_viz),
            }),
            tntron.ui.row({
                tntron.ui.button("zone:top-right",    "Toast Top-R",    { active = st.toast_zone == "top-right" }),
                tntron.ui.button("zone:top-left",     "Toast Top-L",    { active = st.toast_zone == "top-left" }),
                tntron.ui.button("zone:bottom-right", "Toast Bot-R",    { active = st.toast_zone == "bottom-right" }),
                tntron.ui.button("zone:bottom-left",  "Toast Bot-L",    { active = st.toast_zone == "bottom-left" }),
            }),
            tntron.ui.toggle("badge_toggle", "Show layout preset pill in top bar", st.badge),
        },
    })
end

function on_load(_ctx)
    st.layout = tntron.storage.get("layout", "table")
    st.density = tntron.storage.get("density", "Normal")
    st.badge = (tntron.storage.get("badge", true) ~= false)
    st.sidebar_side = tntron.storage.get("sidebar_side", "left")
    st.show_banner = (tntron.storage.get("show_banner", true) ~= false)
    st.show_recent_strip = (tntron.storage.get("show_recent_strip", true) ~= false)
    st.show_col_album = (tntron.storage.get("show_col_album", true) ~= false)
    st.show_col_duration = (tntron.storage.get("show_col_duration", true) ~= false)
    st.show_telemetry_strip = false
    st.show_player_viz = (tntron.storage.get("show_player_viz", true) ~= false)
    st.toast_zone = tntron.storage.get("toast_zone", "top-right")
    apply_to_host()
    render()
end

function on_action(_ctx, action_id, val)
    if action_id:sub(1, 2) == "l:" then
        st.layout = action_id:sub(3)
        tntron.storage.set("layout", st.layout)
    elseif action_id == "cycle_layout" then
        local nxt = { table = "cards", cards = "gallery", gallery = "minimal", minimal = "table" }
        st.layout = nxt[st.layout] or "table"
        tntron.storage.set("layout", st.layout)
    elseif action_id:sub(1, 2) == "d:" then
        st.density = action_id:sub(3)
        tntron.storage.set("density", st.density)
    elseif action_id:sub(1, 5) == "side:" then
        st.sidebar_side = action_id:sub(6)
        tntron.storage.set("sidebar_side", st.sidebar_side)
    elseif action_id == "show_banner" then
        st.show_banner = (val == true)
        tntron.storage.set("show_banner", st.show_banner)
    elseif action_id == "show_recent" then
        st.show_recent_strip = (val == true)
        tntron.storage.set("show_recent_strip", st.show_recent_strip)
    elseif action_id == "show_col_album" then
        st.show_col_album = (val == true)
        tntron.storage.set("show_col_album", st.show_col_album)
    elseif action_id == "show_col_dur" then
        st.show_col_duration = (val == true)
        tntron.storage.set("show_col_duration", st.show_col_duration)
    elseif action_id == "show_telemetry" then
        st.show_telemetry_strip = (val == true)
        tntron.storage.set("show_telemetry_strip", st.show_telemetry_strip)
    elseif action_id == "show_player_viz" then
        st.show_player_viz = (val == true)
        tntron.storage.set("show_player_viz", st.show_player_viz)
    elseif action_id:sub(1, 5) == "zone:" then
        st.toast_zone = action_id:sub(6)
        tntron.storage.set("toast_zone", st.toast_zone)
    elseif action_id == "badge_toggle" then
        st.badge = (val == true)
        tntron.storage.set("badge", st.badge)
    end
    apply_to_host()
    render()
end
