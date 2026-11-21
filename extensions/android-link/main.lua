-- Android & Bluetooth Link Extension
-- BlueZ AVRCP playback control + Reverse A2DP (Phone -> PC) + PipeWire Sink Switcher + Companion App TCP Bridge

local DEFAULTS = {
    lan_enabled = true,
    bind_host = "0.0.0.0",
    tcp_port = 26263,
    auth_token = "",
    show_topbar = true,
    show_player_widget = false,
    bt_volume = 100,
    bt_muted = false,
    auto_pause_pc_on_phone = true,
    auto_pause_phone_on_pc = true,
    duck_pc_on_phone = false,
    duck_volume = 30,
}

local st = {
    cfg = {},
    port_draft = "26263",
    token_draft = "",
    tick_n = 0,
    was_phone_playing = false,
    auto_paused_pc = false,
    ducked_prev_vol = nil,
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

local function save_and_push_cfg()
    tntron.storage.set("android_link_cfg", st.cfg)
    tntron.android.set_config({
        enabled = true,
        lan_enabled = st.cfg.lan_enabled,
        bind_host = st.cfg.bind_host or "0.0.0.0",
        tcp_port = tonumber(st.cfg.tcp_port) or 26263,
        auth_token = st.cfg.auth_token or "",
    })
end

local function is_any_phone_playing(status)
    if type(status.media_players) == "table" then
        for _, p in ipairs(status.media_players) do
            if p.status == "playing" then
                return true, p
            end
        end
    end
    return false, nil
end

local function run_auto_handoff(status)
    local phone_playing = is_any_phone_playing(status)
    local pb = tntron.playback.get()

    if phone_playing and not st.was_phone_playing then
        if st.cfg.auto_pause_pc_on_phone and pb and pb.playing then
            tntron.playback.pause()
            st.auto_paused_pc = true
        elseif st.cfg.duck_pc_on_phone and pb and pb.playing and st.ducked_prev_vol == nil then
            st.ducked_prev_vol = tonumber(pb.volume) or 100
            tntron.playback.set_volume(tonumber(st.cfg.duck_volume) or 30)
        end
    elseif not phone_playing and st.was_phone_playing then
        if st.ducked_prev_vol ~= nil then
            tntron.playback.set_volume(st.ducked_prev_vol)
            st.ducked_prev_vol = nil
        end
        if st.auto_paused_pc then
            st.auto_paused_pc = false
        end
    end
    st.was_phone_playing = phone_playing
end

local function render()
    local status = tntron.android.status() or {}
    local phone_playing, active_mp = is_any_phone_playing(status)
    local first_mp = active_mp or (type(status.media_players) == "table" and status.media_players[1] or nil)
    local active_src = status.active_source or "pc"
    local reverse_on = (status.reverse_bt_active == true)

    if st.cfg.show_topbar then
        local txt = "Android"
        if phone_playing and first_mp and first_mp.title ~= "" then
            txt = "BT: " .. first_mp.title
        elseif active_src == "android" then
            txt = "Source: Android"
        elseif (tonumber(status.android_clients) or 0) > 0 then
            txt = "App: " .. tostring(status.android_clients) .. " linked"
        end
        tntron.ui.set_topbar_slot({
            text = txt,
            icon = "headphones",
            accent = phone_playing or active_src == "android" or (tonumber(status.android_clients) or 0) > 0,
            action_id = "open_android_tab",
        })
    else
        tntron.ui.clear_topbar_slot()
    end

    if st.cfg.show_player_widget then
        local ptxt = active_src == "android" and "Src: Android" or "Src: PC"
        tntron.ui.set_player_widget({
            text = ptxt,
            icon = "headphones",
            accent = active_src == "android",
            action_id = "toggle_source",
        })
    else
        tntron.ui.clear_player_widget()
    end

    local widgets = {
        tntron.ui.heading("Active Audio Source"),
        tntron.ui.row({
            tntron.ui.button("src:pc", "PC (tntron)", { icon = "music", active = active_src == "pc", accent = active_src == "pc" }),
            tntron.ui.button("src:android", "Android (Reverse BT)", { icon = "headphones", active = active_src == "android", accent = active_src == "android" }),
            tntron.ui.button("refresh", "Refresh", { icon = "refresh" }),
        }),
    }

    -- Android AVRCP Now Playing
    table.insert(widgets, tntron.ui.heading("Android Playback Control (Bluetooth AVRCP)"))
    if first_mp then
        local track_title = (first_mp.title and first_mp.title ~= "") and first_mp.title or "Connected Media Player"
        local track_sub = (first_mp.artist and first_mp.artist ~= "") and (first_mp.artist .. " · " .. (first_mp.device_name or "")) or (first_mp.device_name or "Android")
        table.insert(widgets, tntron.ui.card({
            eyebrow = "ANDROID NOW PLAYING (" .. tostring(first_mp.status or "idle"):upper() .. ")",
            title = track_title,
            subtitle = track_sub,
            icon = "headphones",
            badge = reverse_on and "A2DP -> PC" or "AVRCP",
            accent = (first_mp.status == "playing"),
        }))
        table.insert(widgets, tntron.ui.row({
            tntron.ui.button("mp:prev", "Prev", { icon = "prev" }),
            tntron.ui.button("mp:toggle", (first_mp.status == "playing") and "Pause Phone" or "Play Phone", { icon = (first_mp.status == "playing") and "pause" or "play", accent = true }),
            tntron.ui.button("mp:stop", "Stop", { icon = "close" }),
            tntron.ui.button("mp:next", "Next", { icon = "next" }),
            tntron.ui.button("bt_mute_toggle", st.cfg.bt_muted and "Unmute Phone Audio" or "Mute Phone Audio", { icon = st.cfg.bt_muted and "mute" or "volume", active = st.cfg.bt_muted }),
        }))
    else
        table.insert(widgets, tntron.ui.text("No active Android Bluetooth media player detected. Pair & connect your phone below, then start playing audio on your phone.", true))
    end

    table.insert(widgets, tntron.ui.slider("bt_volume", "Phone -> PC Reverse BT Volume", 0, 150, 5, tonumber(st.cfg.bt_volume) or 100, "%"))

    -- Bluetooth Devices & Reverse A2DP
    table.insert(widgets, tntron.ui.heading("Bluetooth Devices & Reverse A2DP (Phone -> PC Speakers)"))
    local bt_state_txt = status.bluez_available
        and string.format("Adapter: %s · Powered: %s · Discoverable: %s",
            tostring(status.adapter_name or "hci0"),
            status.adapter_powered and "ON" or "OFF",
            status.adapter_discoverable and "ON" or "OFF")
        or "BlueZ (org.bluez) unavailable"
    table.insert(widgets, tntron.ui.text(bt_state_txt, true))
    table.insert(widgets, tntron.ui.row({
        tntron.ui.toggle("bt_discoverable", "Make PC Discoverable for Phone Pairing", status.adapter_discoverable == true),
    }))

    if type(status.devices) == "table" and #status.devices > 0 then
        for i, d in ipairs(status.devices) do
            if i > 10 then break end
            local label = (d.alias and d.alias ~= "") and d.alias or (d.name ~= "" and d.name or d.address)
            local kind = d.is_audio_source and "Phone / Audio Source" or (d.is_audio_sink and "Headset / Speaker" or "Bluetooth Device")
            local batt = d.battery_pct and (" · " .. tostring(d.battery_pct) .. "%") or ""
            table.insert(widgets, tntron.ui.row({
                tntron.ui.badge(label .. " (" .. kind .. batt .. ")", { icon = d.is_audio_source and "headphones" or "globe", accent = d.connected == true }),
                d.connected
                    and tntron.ui.button("btdisc:" .. d.object_path, "Disconnect", { icon = "close" })
                    or tntron.ui.button("btconn:" .. d.object_path, "Connect", { icon = "play", accent = true }),
                tntron.ui.button("bta2dp:" .. d.object_path, "Stream -> PC", { icon = "wave", active = d.connected == true and reverse_on }),
            }))
        end
    else
        table.insert(widgets, tntron.ui.text("No paired Bluetooth devices found.", true))
    end

    -- PipeWire / PulseAudio Output Sinks
    table.insert(widgets, tntron.ui.heading("PC Audio Output Switcher (PipeWire)"))
    if type(status.sinks) == "table" and #status.sinks > 0 then
        local sink_btns = {}
        for _, s in ipairs(status.sinks) do
            local tag = s.is_bluetooth and "[BT] " or ""
            table.insert(sink_btns, tntron.ui.button("sink:" .. s.name, tag .. (s.description or s.name), {
                icon = s.is_bluetooth and "headphones" or "volume",
                active = (s.is_default == true),
                accent = (s.is_default == true),
            }))
        end
        table.insert(widgets, tntron.ui.row(sink_btns))
    else
        table.insert(widgets, tntron.ui.text("No PipeWire/PulseAudio sinks found.", true))
    end

    -- Auto-Handoff Rules
    table.insert(widgets, tntron.ui.heading("Auto-Handoff & UI"))
    table.insert(widgets, tntron.ui.row({
        tntron.ui.toggle("auto_pause_pc_on_phone", "Pause PC when Phone Plays", st.cfg.auto_pause_pc_on_phone),
        tntron.ui.toggle("auto_pause_phone_on_pc", "Pause Phone when PC Plays", st.cfg.auto_pause_phone_on_pc),
        tntron.ui.toggle("duck_pc_on_phone", "Duck PC Volume on Phone Audio", st.cfg.duck_pc_on_phone),
    }))
    table.insert(widgets, tntron.ui.row({
        tntron.ui.toggle("show_topbar", "Topbar Pill", st.cfg.show_topbar),
        tntron.ui.toggle("show_player_widget", "Player Source Pill", st.cfg.show_player_widget),
    }))

    -- Companion Android App Bridge (Bluetooth RFCOMM + LAN + USB adb reverse)
    table.insert(widgets, tntron.ui.heading("Android Companion App Bridge (Bluetooth / LAN / USB)"))
    local ips = (type(status.lan_ips) == "table" and #status.lan_ips > 0)
        and table.concat(status.lan_ips, ", ")
        or "127.0.0.1"
    table.insert(widgets, tntron.ui.row({
        tntron.ui.badge("App Clients: " .. tostring(status.android_clients or 0), { icon = "globe", accent = (tonumber(status.android_clients) or 0) > 0 }),
        tntron.ui.badge("Bluetooth RFCOMM: Ch 22", { icon = "headphones", accent = status.bluez_available == true }),
        tntron.ui.badge("tcp://" .. ips .. ":" .. tostring(st.cfg.tcp_port), { icon = "sliders", accent = st.cfg.lan_enabled }),
    }))
    table.insert(widgets, tntron.ui.text("Bluetooth Link works directly with paired devices (no Wi-Fi/firewall needed). USB tip: `adb reverse tcp:" .. tostring(st.cfg.tcp_port) .. " tcp:" .. tostring(st.cfg.tcp_port) .. "`.", true))
    table.insert(widgets, tntron.ui.row({
        tntron.ui.toggle("lan_enabled", "Enable Companion App TCP Bridge", st.cfg.lan_enabled),
        tntron.ui.button("bind:0.0.0.0", "Wi-Fi LAN (0.0.0.0)", { active = st.cfg.bind_host == "0.0.0.0" }),
        tntron.ui.button("bind:127.0.0.1", "USB Only (127.0.0.1)", { active = st.cfg.bind_host == "127.0.0.1" }),
    }))
    table.insert(widgets, tntron.ui.input("port_draft", "Companion Bridge Port (1024-65535)", st.port_draft, {
        placeholder = "26263",
        submit_action = "apply_port",
        submit_label = "Apply Port",
    }))
    table.insert(widgets, tntron.ui.input("token_draft", "Optional Pairing PIN / Token", st.token_draft, {
        placeholder = "Recommended when LAN (0.0.0.0) is enabled",
        submit_action = "apply_token",
        submit_label = "Save PIN",
    }))

    if status.last_error and status.last_error ~= "" then
        table.insert(widgets, tntron.ui.text("Last notice: " .. tostring(status.last_error), true))
    end

    tntron.ui.register_sidebar({
        id = "android-link",
        label = "Android",
        icon = "headphones",
        widgets = widgets,
    })
    tntron.ui.register_settings({
        id = "android-link",
        title = "Android & Bluetooth Link",
        widgets = widgets,
    })
end

function on_load(_ctx)
    st.cfg = copy_cfg(tntron.storage.get("android_link_cfg", nil))
    st.port_draft = tostring(st.cfg.tcp_port or 26263)
    st.token_draft = tostring(st.cfg.auth_token or "")
    save_and_push_cfg()
    tntron.android.refresh()
    render()
end

function on_unload(_ctx)
    tntron.android.set_config({ enabled = false })
    tntron.ui.unregister_sidebar("android-link")
    tntron.ui.clear_topbar_slot()
    tntron.ui.clear_player_widget()
end

function on_tick(_ctx)
    st.tick_n = st.tick_n + 1
    local status = tntron.android.status() or {}
    run_auto_handoff(status)
    if st.tick_n % 3 == 0 then
        render()
    end
end

function on_event(_ctx, event_name)
    if event_name == "playbackStateChange" then
        local pb = tntron.playback.get()
        if pb and pb.playing and st.cfg.auto_pause_phone_on_pc then
            local status = tntron.android.status() or {}
            local phone_playing, mp = is_any_phone_playing(status)
            if phone_playing and mp then
                tntron.android.media("pause", mp.object_path)
            end
        end
        render()
    end
end

function on_action(_ctx, action_id, val)
    local bool_keys = {
        lan_enabled = true,
        show_topbar = true,
        show_player_widget = true,
        auto_pause_pc_on_phone = true,
        auto_pause_phone_on_pc = true,
        duck_pc_on_phone = true,
    }
    if bool_keys[action_id] then
        st.cfg[action_id] = (val == true)
        if action_id == "auto_pause_pc_on_phone" and st.cfg.auto_pause_pc_on_phone then
            st.cfg.duck_pc_on_phone = false
        elseif action_id == "duck_pc_on_phone" and st.cfg.duck_pc_on_phone then
            st.cfg.auto_pause_pc_on_phone = false
        end
        save_and_push_cfg()
    elseif action_id == "bt_discoverable" then
        tntron.android.bt_discoverable(val == true)
        tntron.ui.toast(val == true and "PC is now discoverable via Bluetooth" or "Bluetooth discoverable disabled")
    elseif action_id == "bt_volume" then
        st.cfg.bt_volume = math.floor((tonumber(val) or 100) + 0.5)
        tntron.android.bt_volume(st.cfg.bt_volume)
        tntron.storage.set("android_link_cfg", st.cfg)
    elseif action_id == "bt_mute_toggle" then
        st.cfg.bt_muted = not st.cfg.bt_muted
        tntron.android.bt_mute(st.cfg.bt_muted)
        tntron.storage.set("android_link_cfg", st.cfg)
    elseif action_id == "toggle_source" then
        local status = tntron.android.status() or {}
        local next_src = (status.active_source == "android") and "pc" or "android"
        tntron.android.switch_source(next_src)
    elseif action_id == "open_android_tab" then
        tntron.ui.open_sidebar("android-link")
    elseif action_id == "refresh" then
        tntron.android.refresh()
        tntron.ui.toast("Refreshed Bluetooth & PipeWire devices")
    elseif action_id == "port_draft" then
        st.port_draft = tostring(val or "")
        return
    elseif action_id == "token_draft" then
        st.token_draft = tostring(val or "")
        return
    elseif action_id == "apply_port" then
        local p = math.floor(tonumber(val) or tonumber(st.port_draft) or 26263)
        if p < 1024 or p > 65535 then
            tntron.ui.toast("Port must be between 1024 and 65535")
            return
        end
        st.cfg.tcp_port = p
        st.port_draft = tostring(p)
        save_and_push_cfg()
        tntron.ui.toast("Android companion bridge port set to " .. tostring(p))
    elseif action_id == "apply_token" then
        local tok = tostring(val or st.token_draft or "")
        st.cfg.auth_token = tok
        st.token_draft = tok
        save_and_push_cfg()
        tntron.ui.toast(tok == "" and "Pairing PIN cleared" or "Pairing PIN updated")
    else
        local src = action_id:match("^src:(.+)$")
        local mp_act = action_id:match("^mp:(.+)$")
        local bt_conn = action_id:match("^btconn:(.+)$")
        local bt_disc = action_id:match("^btdisc:(.+)$")
        local bt_a2dp = action_id:match("^bta2dp:(.+)$")
        local sink = action_id:match("^sink:(.+)$")
        local bind = action_id:match("^bind:(.+)$")
        if src then
            tntron.android.switch_source(src)
        elseif mp_act then
            tntron.android.media(mp_act, nil)
        elseif bt_conn then
            tntron.android.bt_connect(bt_conn)
            tntron.ui.toast("Connecting Bluetooth device...")
        elseif bt_disc then
            tntron.android.bt_disconnect(bt_disc)
            tntron.ui.toast("Disconnecting Bluetooth device...")
        elseif bt_a2dp then
            tntron.android.bt_reverse_audio(bt_a2dp)
            tntron.ui.toast("Requesting Reverse Bluetooth Audio (A2DP -> PC)...")
        elseif sink then
            tntron.android.set_sink(sink)
            tntron.ui.toast("Switched PC audio output")
        elseif bind then
            st.cfg.bind_host = bind
            save_and_push_cfg()
            tntron.ui.toast("Companion bridge bind host: " .. bind)
        end
    end
    render()
end
