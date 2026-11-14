-- Daetron & Daeboard Hardware Sync Extension
-- Drives /run/daeboard/daeboard.sock with music-reactive lighting and monitors daetron rules.

local st = {
    connected = false,
    reply = "not checked",
    mode = "beat_pulse", -- "static", "beat_pulse", "bass_flash", "tempo_rainbow"
    color_hex = "a855f7",
    brightness = 2,
    hex_draft = "a855f7",
    hue_step = 0,
    topbar = false,
    bin_path = "",
    tick_n = 0,
    daetron = { running = false, rules_dir = "", rules = {} },
}

local COLORS = {
    { hex = "a855f7", label = "Violet" },
    { hex = "14b8a6", label = "Teal" },
    { hex = "ef4444", label = "Crimson" },
    { hex = "f59e0b", label = "Amber" },
    { hex = "3b82f6", label = "Cobalt" },
    { hex = "ec4899", label = "Rose" },
    { hex = "ffffff", label = "White" },
}

local RAINBOW = { "ef4444", "f97316", "eab308", "22c55e", "06b6d4", "3b82f6", "a855f7", "ec4899" }

local function probe_hw()
    local ping = tntron.hw.daeboard_ping() or { connected = false, reply = "err" }
    st.connected = (ping.connected == true)
    st.reply = ping.reply or ""
    st.bin_path = ping.bin_path or ""
    st.daetron = tntron.hw.daetron_status() or { running = false, rules_dir = "", rules = {} }
end

local function drive_lighting(pb)
    if not st.connected or not pb or not pb.playing then
        return
    end
    local beat = tonumber(pb.beat_pulse) or 0.0
    local energy = tonumber(pb.energy) or 0.0
    local bass = 0.0
    if type(pb.bands) == "table" then
        bass = ((pb.bands[1] or 0) + (pb.bands[2] or 0) + (pb.bands[3] or 0)) / 3.0
    end

    if st.mode == "beat_pulse" then
        local lvl = 1
        if beat > 0.35 or energy > 0.45 then
            lvl = 3
        elseif beat > 0.12 or energy > 0.22 then
            lvl = 2
        end
        tntron.hw.daeboard_brightness(math.min(3, math.max(1, lvl)))
        tntron.hw.daeboard_color(st.color_hex)
    elseif st.mode == "bass_flash" then
        local col = st.color_hex
        local lvl = 1
        if bass > 0.48 or beat > 0.38 then
            col = "ef4444"
            lvl = 3
        elseif bass > 0.24 then
            col = "f59e0b"
            lvl = 2
        else
            col = st.color_hex
            lvl = 1
        end
        tntron.hw.daeboard_color(col)
        tntron.hw.daeboard_brightness(lvl)
    elseif st.mode == "tempo_rainbow" then
        local step = (beat > 0.25) and 2 or 1
        st.hue_step = (st.hue_step + step) % #RAINBOW
        tntron.hw.daeboard_color(RAINBOW[st.hue_step + 1])
        tntron.hw.daeboard_brightness(beat > 0.25 and 3 or 2)
    end
end

local function render()
    local pb = tntron.playback.get()
    if st.topbar then
        local top_txt = st.connected and ("RGB: " .. st.mode) or "RGB Offline"
        tntron.ui.set_topbar_slot({
            text = top_txt,
            icon = "sliders",
            accent = st.connected and pb and pb.playing,
            action_id = "cycle_mode",
        })
    else
        tntron.ui.clear_topbar_slot()
    end

    local color_btns = {}
    for _, c in ipairs(COLORS) do
        table.insert(color_btns, tntron.ui.button("col:" .. c.hex, c.label, { active = st.color_hex == c.hex }))
    end

    local rules_count = type(st.daetron.rules) == "table" and #st.daetron.rules or 0
    local dt_status = st.daetron.running
        and string.format("Running (PID %s · %d rules)", tostring(st.daetron.pid or "?"), rules_count)
        or string.format("Not running (%d rules found in %s)", rules_count, st.daetron.rules_dir or "")

    local widgets = {
        tntron.ui.heading("Daeboard Keyboard Backlight (/run/daeboard/daeboard.sock)"),
        tntron.ui.row({
            tntron.ui.badge(st.connected and "Daeboard: Connected" or ("Daeboard: " .. st.reply), { icon = "keyboard", accent = st.connected }),
            st.connected
                and tntron.ui.button("daeboard_stop", "Stop Daeboard", { icon = "close" })
                or tntron.ui.button("daeboard_start", "Run Daeboard", { icon = "play", accent = true }),
            tntron.ui.button("ping", "Ping Socket", { icon = "refresh" }),
        }),
        tntron.ui.text(
            st.bin_path ~= ""
                and ("Binary: " .. st.bin_path .. "  (Opens sudo prompt to start systemd service)")
                or "Binary not found in ~/Documents/daeboard/daeboard or PATH",
            true
        ),
        tntron.ui.heading("Music-Reactive Lighting Mode"),
        tntron.ui.row({
            tntron.ui.button("mode:static",        "Static Hex",    { active = st.mode == "static" }),
            tntron.ui.button("mode:beat_pulse",    "Beat Pulse",    { active = st.mode == "beat_pulse" }),
            tntron.ui.button("mode:bass_flash",    "Bass Flash",    { active = st.mode == "bass_flash" }),
            tntron.ui.button("mode:tempo_rainbow", "Tempo Rainbow", { active = st.mode == "tempo_rainbow" }),
        }),
        tntron.ui.heading("Keyboard Color & Brightness"),
        tntron.ui.row(color_btns),
        tntron.ui.input("hex_draft", "Custom RGB Hex", st.hex_draft, {
            placeholder = "RRGGBB (e.g. ff3b8b)...",
            submit_action = "apply_hex",
            submit_label = "Set Color",
        }),
        tntron.ui.row({
            tntron.ui.button("bri:0", "Off (0)",  { active = st.brightness == 0 }),
            tntron.ui.button("bri:1", "Low (1)",  { active = st.brightness == 1 }),
            tntron.ui.button("bri:2", "Med (2)",  { active = st.brightness == 2 }),
            tntron.ui.button("bri:3", "High (3)", { active = st.brightness == 3 }),
        }),
        tntron.ui.heading("Daetron Daemon & Rules"),
        tntron.ui.text(dt_status, true),
        tntron.ui.row({
            st.daetron.running
                and tntron.ui.button("daetron_reload", "Reload Rules (SIGHUP)", { icon = "refresh", accent = true })
                or tntron.ui.button("daetron_start", "Run Daetron", { icon = "play", accent = true }),
            tntron.ui.button("ping", "Refresh Status", { icon = "search" }),
        }),
        tntron.ui.toggle("topbar_toggle", "Show Daeboard badge in top bar", st.topbar),
    }

    if rules_count > 0 then
        table.insert(widgets, tntron.ui.text("Rules: " .. table.concat(st.daetron.rules, ", "), true))
    end

    tntron.ui.register_sidebar({
        id = "daetron-daeboard",
        label = "Daeboard",
        icon = "keyboard",
        widgets = widgets,
    })
end

function on_load(_ctx)
    st.mode = tntron.storage.get("mode", "beat_pulse")
    st.color_hex = tntron.storage.get("color_hex", "a855f7")
    st.hex_draft = st.color_hex
    st.brightness = tonumber(tntron.storage.get("brightness", 2)) or 2
    st.topbar = (tntron.storage.get("topbar", false) == true)
    probe_hw()
    render()
end

function on_tick(_ctx)
    st.tick_n = st.tick_n + 1
    if not st.connected and (st.tick_n % 5 == 0) then
        local was = st.connected
        probe_hw()
        if st.connected ~= was then
            render()
        end
    end
    local pb = tntron.playback.get()
    drive_lighting(pb)
end

function on_event(_ctx, event_name)
    if event_name == "trackChange" or event_name == "playbackStateChange" then
        probe_hw()
        local pb = tntron.playback.get()
        drive_lighting(pb)
        render()
    end
end

function on_action(_ctx, action_id, val)
    if action_id == "ping" then
        probe_hw()
        tntron.ui.toast(st.connected and "Daeboard connected (pong)" or ("Daeboard: " .. st.reply))
    elseif action_id == "daeboard_start" then
        tntron.hw.daeboard_start()
        probe_hw()
    elseif action_id == "daeboard_stop" then
        tntron.hw.daeboard_stop()
        probe_hw()
    elseif action_id == "daetron_start" then
        tntron.hw.daetron_start()
        probe_hw()
    elseif action_id == "daetron_reload" then
        tntron.hw.daetron_reload()
        probe_hw()
    elseif action_id:sub(1, 5) == "mode:" then
        st.mode = action_id:sub(6)
        tntron.storage.set("mode", st.mode)
        if st.mode == "static" and st.connected then
            tntron.hw.daeboard_color(st.color_hex)
            tntron.hw.daeboard_brightness(st.brightness)
        end
        tntron.ui.toast("Daeboard mode: " .. st.mode)
    elseif action_id == "cycle_mode" then
        local nxt = { static = "beat_pulse", beat_pulse = "bass_flash", bass_flash = "tempo_rainbow", tempo_rainbow = "static" }
        st.mode = nxt[st.mode] or "beat_pulse"
        tntron.storage.set("mode", st.mode)
    elseif action_id:sub(1, 4) == "col:" then
        st.color_hex = action_id:sub(5)
        st.hex_draft = st.color_hex
        tntron.storage.set("color_hex", st.color_hex)
        if st.connected then
            tntron.hw.daeboard_color(st.color_hex)
        end
    elseif action_id == "apply_hex" then
        local clean = (type(val) == "string" and val or ""):gsub("^#", ""):lower()
        if #clean == 6 and clean:match("^[0-9a-f]+$") then
            st.color_hex = clean
            st.hex_draft = clean
            tntron.storage.set("color_hex", clean)
            if st.connected then
                tntron.hw.daeboard_color(clean)
            end
            tntron.ui.toast("Daeboard hex #" .. clean)
        else
            tntron.ui.toast("Enter 6 hex digits (e.g. a855f7)")
        end
    elseif action_id:sub(1, 4) == "bri:" then
        st.brightness = tonumber(action_id:sub(5)) or 2
        tntron.storage.set("brightness", st.brightness)
        if st.connected then
            tntron.hw.daeboard_brightness(st.brightness)
        end
    elseif action_id == "topbar_toggle" then
        st.topbar = (val == true)
        tntron.storage.set("topbar", st.topbar)
    end
    render()
end
