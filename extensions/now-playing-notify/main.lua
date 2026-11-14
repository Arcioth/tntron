-- Now Playing Info & Desktop Notifier (Openify sample-now-playing + Tunotron now_playing_notify)

local st = { desktop_enabled = true, last_path = "" }

local function fmt(sec)
    local s = math.max(0, math.floor(sec or 0))
    return string.format("%02d:%02d", math.floor(s / 60), s % 60)
end

local function render(pb)
    local counts = tntron.storage.get("play_counts", {})
    local path = pb and pb.track_path or ""
    local play_n = (type(counts) == "table" and path ~= "") and (tonumber(counts[path]) or 0) or 0

    local title = (pb and pb.title ~= "") and pb.title or "Nothing Playing"
    local artist = (pb and pb.artist ~= "") and pb.artist or "Unknown Artist"
    local album = (pb and pb.album ~= "") and pb.album or "Single / Unsorted"
    local dur = pb and pb.duration_sec or 0
    local pos = pb and pb.position_sec or 0

    tntron.ui.register_sidebar({
        id = "now-playing-card",
        label = "Now Playing",
        icon = "disc",
        widgets = {
            tntron.ui.heading(title),
            tntron.ui.text(artist .. "  ·  " .. album, false),
            tntron.ui.row({
                tntron.ui.badge("Plays: " .. tostring(play_n), { icon = "star", accent = true }),
                tntron.ui.badge(pb and pb.codec and pb.codec:upper() or "AUDIO", { icon = "music" }),
                tntron.ui.badge(fmt(pos) .. " / " .. fmt(dur), { icon = "clock" }),
            }),
            tntron.ui.progress("Track Progress", pos, math.max(1, dur)),
            tntron.ui.row({
                tntron.ui.button("send_now", "Send Notification", { icon = "bell", accent = true }),
            }),
            tntron.ui.toggle("desktop_toggle", "Notify desktop via notify-send on track change", st.desktop_enabled),
        },
    })
end

function on_load(_ctx)
    st.desktop_enabled = (tntron.storage.get("desktop_enabled", true) ~= false)
    local pb = tntron.playback.get()
    st.last_path = pb and pb.track_path or ""
    render(pb)
end

function on_event(_ctx, event_name)
    local pb = tntron.playback.get()
    if event_name == "trackChange" and pb and pb.track_path ~= "" then
        if pb.track_path ~= st.last_path then
            st.last_path = pb.track_path
            local counts = tntron.storage.get("play_counts", {})
            if type(counts) ~= "table" then counts = {} end
            counts[pb.track_path] = (tonumber(counts[pb.track_path]) or 0) + 1
            tntron.storage.set("play_counts", counts)
        end
        if st.desktop_enabled then
            tntron.notify.desktop("Now Playing", string.format("%s — %s", pb.title, pb.artist))
        end
    end
    render(pb)
end

function on_tick(_ctx)
    local pb = tntron.playback.get()
    if pb and pb.playing then
        render(pb)
    end
end

function on_action(_ctx, action_id, val)
    local pb = tntron.playback.get()
    if action_id == "desktop_toggle" then
        st.desktop_enabled = (val == true)
        tntron.storage.set("desktop_enabled", st.desktop_enabled)
    elseif action_id == "send_now" then
        if pb and pb.title ~= "" then
            tntron.notify.desktop("Now Playing", string.format("%s — %s", pb.title, pb.artist))
            tntron.ui.toast("Notification dispatched")
        end
    end
    render(pb)
end
