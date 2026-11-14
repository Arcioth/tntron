-- Sleep Timer (ported from Tunotron sleep_timer.lua)

local st = { remaining = 0, fade = true, saved_vol = nil }

local function fmt(sec)
    local s = math.max(0, math.floor(sec))
    return string.format("%02d:%02d", math.floor(s / 60), s % 60)
end

local function render()
    if st.remaining > 0 then
        tntron.ui.set_topbar_slot({
            text = "Sleep " .. fmt(st.remaining),
            icon = "clock",
            accent = true,
            action_id = "cancel",
        })
    else
        tntron.ui.clear_topbar_slot()
    end

    tntron.ui.register_settings({
        id = "sleep-timer",
        title = "Sleep Timer",
        widgets = {
            tntron.ui.text(
                st.remaining > 0 and ("Active countdown: " .. fmt(st.remaining)) or "Timer is inactive.",
                st.remaining <= 0
            ),
            tntron.ui.row({
                tntron.ui.button("set:15", "15m", { icon = "clock", active = false }),
                tntron.ui.button("set:30", "30m", { icon = "clock" }),
                tntron.ui.button("set:45", "45m", { icon = "clock" }),
                tntron.ui.button("set:60", "60m", { icon = "clock" }),
                tntron.ui.button("add:5",  "+5m", { icon = "plus" }),
                tntron.ui.button("cancel", "Off", { icon = "close" }),
            }),
            tntron.ui.toggle("fade_toggle", "Fade volume gently during final 10s", st.fade),
        },
    })
end

function on_load(_ctx)
    st.fade = (tntron.storage.get("fade", true) ~= false)
    render()
end

function on_tick(_ctx)
    if st.remaining <= 0 then return end
    st.remaining = st.remaining - 1
    if st.fade and st.remaining > 0 and st.remaining <= 10 then
        local pb = tntron.playback.get()
        if pb then
            if not st.saved_vol then st.saved_vol = pb.volume end
            local target = math.max(5, math.floor((st.saved_vol * st.remaining) / 10))
            tntron.playback.set_volume(target)
        end
    end
    if st.remaining <= 0 then
        st.remaining = 0
        tntron.playback.pause()
        if st.saved_vol then
            tntron.playback.set_volume(st.saved_vol)
            st.saved_vol = nil
        end
        tntron.notify.desktop("Sleep Timer", "Playback paused by Sleep Timer.")
        tntron.ui.toast("Sleep Timer paused playback")
    end
    render()
end

function on_action(_ctx, action_id, val)
    if action_id:sub(1, 4) == "set:" then
        local mins = tonumber(action_id:sub(5)) or 15
        st.remaining = mins * 60
        st.saved_vol = nil
        tntron.ui.toast("Sleep Timer set for " .. mins .. " min")
    elseif action_id == "add:5" then
        st.remaining = st.remaining + 300
    elseif action_id == "cancel" then
        st.remaining = 0
        if st.saved_vol then
            tntron.playback.set_volume(st.saved_vol)
            st.saved_vol = nil
        end
        tntron.ui.toast("Sleep Timer cancelled")
    elseif action_id == "fade_toggle" then
        st.fade = (val == true)
        tntron.storage.set("fade", st.fade)
    end
    render()
end
