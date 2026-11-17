-- Minecraft Link (Fabric 26.2) Extension
-- Exposes localhost TCP bridge (default 127.0.0.1:26262) + reacts to live Minecraft telemetry.

local DEFAULTS = {
    tcp_enabled = true,
    tcp_host = "127.0.0.1",
    tcp_port = 26262,
    auth_token = "",
    show_topbar = true,
    show_player_widget = false,
    announce_tracks = true,
    announce_mode = "toast", -- "toast" | "actionbar" | "chat"
    pause_on_game_pause = false,
    duck_on_pause = false,
    duck_volume = 35,
    combat_boost = false,
    combat_preset = "bass", -- "bass" | "rock"
    combat_speed = 1.0,
    dimension_dsp = false,
    dimension_themes = false,
    underwater_muffle = true,
    toast_on_connect = true,
}

local st = {
    cfg = {},
    port_draft = "26262",
    token_draft = "",
    was_connected = false,
    auto_paused = false,
    ducked_prev_vol = nil,
    combat_active = false,
    last_dim_key = "",
    last_underwater = false,
    tick_n = 0,
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

local function save_and_push_bridge()
    tntron.storage.set("mc_link_cfg", st.cfg)
    tntron.mc.set_config({
        enabled = true,
        tcp_enabled = st.cfg.tcp_enabled,
        tcp_host = st.cfg.tcp_host,
        tcp_port = tonumber(st.cfg.tcp_port) or 26262,
        auth_token = st.cfg.auth_token or "",
    })
end

local function pretty_dim(dim)
    if not dim or dim == "" then return "None" end
    if dim:find("nether") then return "The Nether" end
    if dim:find("end") then return "The End" end
    if dim:find("overworld") then return "Overworld" end
    return dim
end

local function apply_reactive_rules(tel)
    if not tel or not tel.connected then
        if st.ducked_prev_vol ~= nil then
            tntron.playback.set_volume(st.ducked_prev_vol)
            st.ducked_prev_vol = nil
        end
        if st.auto_paused then
            tntron.playback.play()
            st.auto_paused = false
        end
        if st.combat_active then
            tntron.audio.set_preset("flat")
            if math.abs((tonumber(st.cfg.combat_speed) or 1.0) - 1.0) > 0.01 then
                tntron.playback.set_speed(1.0)
            end
            st.combat_active = false
        end
        if st.last_underwater then
            tntron.audio.clear_filter()
            st.last_underwater = false
        end
        return
    end

    local pb = tntron.playback.get()

    -- 1. Pause / Duck on Minecraft Pause Menu
    local game_paused = (tel.paused == true) or (tel.in_world == false)
    if st.cfg.pause_on_game_pause then
        if game_paused and pb and pb.playing and not st.auto_paused then
            tntron.playback.pause()
            st.auto_paused = true
        elseif not game_paused and st.auto_paused then
            tntron.playback.play()
            st.auto_paused = false
        end
    elseif st.cfg.duck_on_pause then
        if game_paused and st.ducked_prev_vol == nil and pb then
            st.ducked_prev_vol = tonumber(pb.volume) or 100
            tntron.playback.set_volume(tonumber(st.cfg.duck_volume) or 35)
        elseif not game_paused and st.ducked_prev_vol ~= nil then
            tntron.playback.set_volume(st.ducked_prev_vol)
            st.ducked_prev_vol = nil
        end
    end

    -- 2. Combat / Boss / Primed TNT Boost
    if st.cfg.combat_boost then
        local intense = (tel.in_combat == true) or (tel.boss_active == true) or (tel.tnt_nearby == true)
        if intense and not st.combat_active then
            st.combat_active = true
            tntron.audio.set_preset(st.cfg.combat_preset or "bass")
            local spd = tonumber(st.cfg.combat_speed) or 1.0
            if math.abs(spd - 1.0) > 0.01 then
                tntron.playback.set_speed(spd)
            end
        elseif not intense and st.combat_active then
            st.combat_active = false
            tntron.audio.set_preset("flat")
            if math.abs((tonumber(st.cfg.combat_speed) or 1.0) - 1.0) > 0.01 then
                tntron.playback.set_speed(1.0)
            end
        end
    elseif st.combat_active then
        st.combat_active = false
        tntron.audio.set_preset("flat")
    end

    -- 3. Underwater Muffle
    local uw = (tel.underwater == true) and (st.cfg.underwater_muffle == true)
    if uw ~= st.last_underwater then
        st.last_underwater = uw
        if uw then
            tntron.audio.set_filter("lowpass=f=720")
        else
            tntron.audio.clear_filter()
        end
    end

    -- 4. Dimension Reactive DSP & Themes
    local dim = tostring(tel.dimension or "")
    if dim ~= "" and dim ~= st.last_dim_key then
        st.last_dim_key = dim
        if st.cfg.dimension_dsp then
            if dim:find("end") then
                tntron.audio.set_spatial(true, 0.14)
            elseif dim:find("nether") then
                tntron.audio.set_spatial(false, 0.08)
                tntron.audio.set_preset("warm")
            else
                tntron.audio.set_spatial(false, 0.08)
                tntron.audio.set_preset("flat")
            end
        end
        if st.cfg.dimension_themes then
            if dim:find("nether") then
                tntron.themes.set_active("red")
            elseif dim:find("end") then
                tntron.themes.set_active("purple")
            elseif dim:find("overworld") then
                tntron.themes.set_active("openify")
            end
        end
    end
end

local function render()
    local tel = tntron.mc.status() or {}
    local connected = (tel.connected == true)
    local player = (tel.player_name and tel.player_name ~= "") and tel.player_name or "Player"
    local dim_label = pretty_dim(tel.dimension)

    if st.cfg.show_topbar then
        local txt = connected and ("MC: " .. player) or ("MC :" .. tostring(st.cfg.tcp_port))
        tntron.ui.set_topbar_slot({
            text = txt,
            icon = "globe",
            accent = connected,
            action_id = "open_mc_tab",
        })
    else
        tntron.ui.clear_topbar_slot()
    end

    if st.cfg.show_player_widget then
        local ptxt = connected and ("MC · " .. dim_label) or "MC Offline"
        tntron.ui.set_player_widget({
            text = ptxt,
            icon = "layers",
            accent = connected,
            action_id = "open_mc_tab",
        })
    else
        tntron.ui.clear_player_widget()
    end

    local status_str = connected
        and string.format(
            "Connected (%d client%s) · Player: %s · %s (%s) · HP %.1f/%.1f · %d cmds",
            tonumber(tel.clients_count) or 1,
            (tonumber(tel.clients_count) or 1) == 1 and "" or "s",
            player,
            dim_label,
            tostring(tel.biome or "unknown"),
            tonumber(tel.health) or 0,
            tonumber(tel.max_health) or 20,
            tonumber(tel.commands_received) or 0
        )
        or string.format(
            "Waiting for Minecraft Fabric 26.2 mod on tcp://%s:%d (or Unix IPC socket)",
            tostring(st.cfg.tcp_host or "127.0.0.1"),
            tonumber(st.cfg.tcp_port) or 26262
        )

    local flags = {}
    if tel.paused then table.insert(flags, "Paused") end
    if tel.in_combat then table.insert(flags, "Combat") end
    if tel.boss_active then table.insert(flags, "Boss: " .. tostring(tel.boss_name or "Active")) end
    if tel.tnt_nearby then table.insert(flags, "TNT Nearby!") end
    if tel.underwater then table.insert(flags, "Underwater") end
    if tel.weather and tel.weather ~= "" and tel.weather ~= "clear" then
        table.insert(flags, "Weather: " .. tostring(tel.weather))
    end
    local flags_str = (#flags > 0) and ("Live Flags: " .. table.concat(flags, " · ")) or "Live Flags: Calm"

    local widgets = {
        tntron.ui.heading("Minecraft Fabric 26.2 Link"),
        tntron.ui.row({
            tntron.ui.badge(connected and ("MC Connected · " .. player) or "MC Standby", { icon = "globe", accent = connected }),
            tntron.ui.badge("TCP " .. tostring(st.cfg.tcp_host) .. ":" .. tostring(st.cfg.tcp_port), { icon = "sliders", accent = st.cfg.tcp_enabled }),
            tntron.ui.button("refresh_status", "Refresh", { icon = "refresh" }),
        }),
        tntron.ui.text(status_str, true),
        tntron.ui.text(flags_str, true),

        tntron.ui.heading("Bridge & Connection"),
        tntron.ui.row({
            tntron.ui.toggle("tcp_enabled", "Enable Localhost TCP Bridge", st.cfg.tcp_enabled),
            tntron.ui.toggle("toast_on_connect", "Toast on Connect/Disconnect", st.cfg.toast_on_connect),
            tntron.ui.toggle("show_topbar", "Topbar Status Pill", st.cfg.show_topbar),
            tntron.ui.toggle("show_player_widget", "Player Bar Widget", st.cfg.show_player_widget),
        }),
        tntron.ui.row({
            tntron.ui.button("port:26262", "Port 26262", { active = tonumber(st.cfg.tcp_port) == 26262 }),
            tntron.ui.button("port:26263", "Port 26263", { active = tonumber(st.cfg.tcp_port) == 26263 }),
            tntron.ui.button("port:37420", "Port 37420", { active = tonumber(st.cfg.tcp_port) == 37420 }),
        }),
        tntron.ui.input("port_draft", "Custom TCP Port (1024-65535)", st.port_draft, {
            placeholder = "26262",
            submit_action = "apply_port",
            submit_label = "Apply Port",
        }),
        tntron.ui.input("token_draft", "Optional Auth Token (blank = open localhost)", st.token_draft, {
            placeholder = "Leave empty for seamless local connect",
            submit_action = "apply_token",
            submit_label = "Save Token",
        }),

        tntron.ui.heading("In-Game Track Announcements"),
        tntron.ui.row({
            tntron.ui.toggle("announce_tracks", "Push Track Changes to Minecraft", st.cfg.announce_tracks),
            tntron.ui.button("amode:toast", "Toast", { active = st.cfg.announce_mode == "toast" }),
            tntron.ui.button("amode:actionbar", "Actionbar", { active = st.cfg.announce_mode == "actionbar" }),
            tntron.ui.button("amode:chat", "Chat", { active = st.cfg.announce_mode == "chat" }),
        }),
        tntron.ui.row({
            tntron.ui.button("test_toast", "Test Toast", { icon = "bell", accent = true }),
            tntron.ui.button("test_actionbar", "Test Actionbar", { icon = "music" }),
            tntron.ui.button("test_chat", "Send Now Playing to Chat", { icon = "lyrics" }),
        }),

        tntron.ui.heading("Game-Reactive Audio Automation"),
        tntron.ui.text("Automatically adapt tntron playback, EQ, and themes to your live Minecraft state.", true),
        tntron.ui.row({
            tntron.ui.toggle("pause_on_game_pause", "Auto-Pause on MC Pause/Menu", st.cfg.pause_on_game_pause),
            tntron.ui.toggle("duck_on_pause", "Duck Volume on MC Pause", st.cfg.duck_on_pause),
            tntron.ui.toggle("underwater_muffle", "Underwater Low-Pass Filter", st.cfg.underwater_muffle),
        }),
        tntron.ui.slider("duck_volume", "Pause Duck Volume", 10, 80, 5, tonumber(st.cfg.duck_volume) or 35, "%"),
        tntron.ui.row({
            tntron.ui.toggle("combat_boost", "Combat / Boss / TNT Intensity Boost", st.cfg.combat_boost),
            tntron.ui.button("cpre:bass", "Combat: Bass Boost", { active = st.cfg.combat_preset == "bass" }),
            tntron.ui.button("cpre:rock", "Combat: Rock", { active = st.cfg.combat_preset == "rock" }),
        }),
        tntron.ui.slider("combat_speed", "Combat Speed Multiplier", 1.0, 1.30, 0.05, tonumber(st.cfg.combat_speed) or 1.0, "x"),
        tntron.ui.row({
            tntron.ui.toggle("dimension_dsp", "Dimension-Aware DSP (Nether Warm / End 8D)", st.cfg.dimension_dsp),
            tntron.ui.toggle("dimension_themes", "Dimension-Aware tntron Theme", st.cfg.dimension_themes),
            tntron.ui.button("reset_rules", "Reset Rules", { icon = "refresh" }),
        }),
    }

    tntron.ui.register_sidebar({
        id = "minecraft-link",
        label = "Minecraft",
        icon = "globe",
        widgets = widgets,
    })
    tntron.ui.register_settings({
        id = "minecraft-link",
        title = "Minecraft Link (Fabric 26.2)",
        widgets = widgets,
    })
end

local function push_now_playing(mode)
    local pb = tntron.playback.get()
    if not pb or not pb.title or pb.title == "" or pb.title == "No Track" then
        return
    end
    local artist = (pb.artist and pb.artist ~= "" and pb.artist ~= "Unknown Artist") and pb.artist or "tntron"
    tntron.mc.send_notice(mode or st.cfg.announce_mode or "toast", pb.title, artist)
end

function on_load(_ctx)
    st.cfg = copy_cfg(tntron.storage.get("mc_link_cfg", nil))
    st.port_draft = tostring(st.cfg.tcp_port or 26262)
    st.token_draft = tostring(st.cfg.auth_token or "")
    save_and_push_bridge()
    local tel = tntron.mc.status() or {}
    st.was_connected = (tel.connected == true)
    render()
end

function on_unload(_ctx)
    tntron.mc.set_config({ enabled = false })
    tntron.ui.unregister_sidebar("minecraft-link")
    tntron.ui.clear_topbar_slot()
    tntron.ui.clear_player_widget()
    if st.last_underwater then
        tntron.audio.clear_filter()
    end
end

function on_tick(_ctx)
    st.tick_n = st.tick_n + 1
    local tel = tntron.mc.status() or {}
    local now_conn = (tel.connected == true)
    if now_conn ~= st.was_connected then
        st.was_connected = now_conn
        if st.cfg.toast_on_connect then
            if now_conn then
                local p = (tel.player_name and tel.player_name ~= "") and tel.player_name or "Minecraft"
                tntron.ui.toast("Minecraft connected: " .. p)
            else
                tntron.ui.toast("Minecraft disconnected")
            end
        end
        apply_reactive_rules(tel)
        render()
    elseif st.tick_n % 3 == 0 and now_conn then
        render()
    end
end

function on_event(_ctx, event_name, payload)
    if event_name == "mcTelemetry" then
        local tel = tntron.mc.status() or {}
        if type(payload) == "table" then
            tel.connected = true
            for k, v in pairs(payload) do
                tel[k] = v
            end
        end
        apply_reactive_rules(tel)
    elseif event_name == "mcEvent" and type(payload) == "table" then
        local ev = tostring(payload.event or "")
        if ev == "tnt_primed" and st.cfg.combat_boost then
            tntron.audio.set_preset(st.cfg.combat_preset or "bass")
        elseif ev == "join_world" and st.cfg.announce_tracks then
            push_now_playing(st.cfg.announce_mode)
        end
        render()
    elseif event_name == "trackChange" then
        if st.cfg.announce_tracks then
            push_now_playing(st.cfg.announce_mode)
        end
    end
end

function on_action(_ctx, action_id, val)
    local bool_keys = {
        tcp_enabled = true,
        toast_on_connect = true,
        show_topbar = true,
        show_player_widget = true,
        announce_tracks = true,
        pause_on_game_pause = true,
        duck_on_pause = true,
        combat_boost = true,
        dimension_dsp = true,
        dimension_themes = true,
        underwater_muffle = true,
    }
    if bool_keys[action_id] then
        st.cfg[action_id] = (val == true)
        if action_id == "pause_on_game_pause" and st.cfg.pause_on_game_pause then
            st.cfg.duck_on_pause = false
        elseif action_id == "duck_on_pause" and st.cfg.duck_on_pause then
            st.cfg.pause_on_game_pause = false
        end
    elseif action_id == "duck_volume" then
        st.cfg.duck_volume = math.floor((tonumber(val) or 35) + 0.5)
    elseif action_id == "combat_speed" then
        st.cfg.combat_speed = math.floor(((tonumber(val) or 1.0) * 20) + 0.5) / 20
    elseif action_id == "port_draft" then
        st.port_draft = tostring(val or "")
        return
    elseif action_id == "token_draft" then
        st.token_draft = tostring(val or "")
        return
    elseif action_id == "apply_port" then
        local p = math.floor(tonumber(val) or tonumber(st.port_draft) or 26262)
        if p < 1024 or p > 65535 then
            tntron.ui.toast("Port must be between 1024 and 65535")
            return
        end
        st.cfg.tcp_port = p
        st.port_draft = tostring(p)
        tntron.ui.toast("Minecraft TCP port set to " .. tostring(p))
    elseif action_id == "apply_token" then
        local tok = tostring(val or st.token_draft or "")
        st.cfg.auth_token = tok
        st.token_draft = tok
        tntron.ui.toast(tok == "" and "Auth token cleared" or "Minecraft auth token updated")
    elseif action_id == "test_toast" then
        tntron.mc.send_notice("toast", "tntron Link", "Connected from tntron!")
        tntron.ui.toast("Sent test toast to Minecraft")
    elseif action_id == "test_actionbar" then
        push_now_playing("actionbar")
        tntron.ui.toast("Sent actionbar notice to Minecraft")
    elseif action_id == "test_chat" then
        push_now_playing("chat")
        tntron.ui.toast("Sent Now Playing to Minecraft chat")
    elseif action_id == "open_mc_tab" then
        tntron.ui.open_sidebar("minecraft-link")
    elseif action_id == "reset_rules" then
        st.cfg = copy_cfg(nil)
        st.port_draft = tostring(st.cfg.tcp_port)
        st.token_draft = st.cfg.auth_token
        tntron.audio.clear_filter()
        tntron.ui.toast("Minecraft Link rules reset")
    elseif action_id == "refresh_status" then
        -- fallthrough to render
    else
        local port_str = action_id:match("^port:(%d+)$")
        local amode = action_id:match("^amode:(.+)$")
        local cpre = action_id:match("^cpre:(.+)$")
        if port_str then
            st.cfg.tcp_port = tonumber(port_str) or 26262
            st.port_draft = tostring(st.cfg.tcp_port)
            tntron.ui.toast("Minecraft TCP port set to " .. tostring(st.cfg.tcp_port))
        elseif amode then
            st.cfg.announce_mode = amode
        elseif cpre then
            st.cfg.combat_preset = cpre
        end
    end
    save_and_push_bridge()
    render()
end
