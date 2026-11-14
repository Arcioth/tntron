-- Better Player Controls (ported from Tunotron better_player.lua)

local st = { speed = 1.0, show_badge = true }

local function render(pb)
    local muted = pb and pb.muted or false
    if st.show_badge and math.abs(st.speed - 1.0) > 0.01 then
        tntron.ui.set_player_widget({
            text = string.format("%.2fx", st.speed),
            icon = "play",
            accent = true,
            action_id = "reset_speed",
        })
    elseif st.show_badge and muted then
        tntron.ui.set_player_widget({
            text = "MUTED",
            icon = "mute",
            accent = false,
            action_id = "mute_toggle",
        })
    else
        tntron.ui.clear_player_widget()
    end

    tntron.ui.register_settings({
        id = "better-player",
        title = "Better Player Controls",
        widgets = {
            tntron.ui.slider("speed", "Playback Speed", 0.5, 2.0, 0.05, st.speed, "x"),
            tntron.ui.row({
                tntron.ui.button("sp:0.75", "0.75x", { active = math.abs(st.speed - 0.75) < 0.02 }),
                tntron.ui.button("sp:1.00", "1.00x", { active = math.abs(st.speed - 1.00) < 0.02 }),
                tntron.ui.button("sp:1.25", "1.25x", { active = math.abs(st.speed - 1.25) < 0.02 }),
                tntron.ui.button("sp:1.50", "1.50x", { active = math.abs(st.speed - 1.50) < 0.02 }),
                tntron.ui.button("mute_toggle", muted and "Unmute" or "Mute", { icon = muted and "mute" or "volume", active = muted }),
            }),
            tntron.ui.toggle("badge_toggle", "Show active speed/mute pill in player bar", st.show_badge),
        },
    })
end

function on_load(_ctx)
    st.speed = tonumber(tntron.storage.get("speed", 1.0)) or 1.0
    st.show_badge = (tntron.storage.get("show_badge", true) ~= false)
    if math.abs(st.speed - 1.0) > 0.01 then
        tntron.playback.set_speed(st.speed)
    end
    render(tntron.playback.get())
end

function on_event(_ctx, event_name)
    if event_name == "playbackStateChange" then
        render(tntron.playback.get())
    end
end

function on_action(_ctx, action_id, val)
    local pb = tntron.playback.get()
    if action_id == "speed" then
        st.speed = tonumber(val) or 1.0
        tntron.playback.set_speed(st.speed)
        tntron.storage.set("speed", st.speed)
    elseif action_id:sub(1, 3) == "sp:" then
        st.speed = tonumber(action_id:sub(4)) or 1.0
        tntron.playback.set_speed(st.speed)
        tntron.storage.set("speed", st.speed)
    elseif action_id == "reset_speed" then
        st.speed = 1.0
        tntron.playback.set_speed(1.0)
        tntron.storage.set("speed", 1.0)
    elseif action_id == "mute_toggle" then
        local cur = pb and pb.muted or false
        tntron.playback.set_mute(not cur)
    elseif action_id == "badge_toggle" then
        st.show_badge = (val == true)
        tntron.storage.set("show_badge", st.show_badge)
    end
    render(tntron.playback.get())
end
