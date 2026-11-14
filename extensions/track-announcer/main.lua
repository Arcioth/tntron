-- Track Announcer & Inspector (ported from Tunotron track_announcer.lua)

local st = { history = {}, last_path = "" }

local function fmt_sec(sec)
    local s = math.floor(tonumber(sec) or 0)
    return string.format("%d:%02d", math.floor(s / 60), s % 60)
end

local function render()
    local pb = tntron.playback.get()
    local widgets = {}

    if pb and pb.track_path ~= "" then
        table.insert(widgets, tntron.ui.heading("Active Track Inspector"))
        table.insert(widgets, tntron.ui.row({
            tntron.ui.badge(pb.codec and pb.codec:upper() or "AUDIO", { icon = "disc", accent = true }),
            tntron.ui.badge(fmt_sec(pb.duration_sec), { icon = "clock" }),
        }))
        table.insert(widgets, tntron.ui.text("Title: " .. tostring(pb.title), false))
        table.insert(widgets, tntron.ui.text("Artist: " .. tostring(pb.artist), true))
        table.insert(widgets, tntron.ui.text("Album: " .. tostring(pb.album), true))
        table.insert(widgets, tntron.ui.text("Path: " .. tostring(pb.track_path), true))
        table.insert(widgets, tntron.ui.button("announce_now", "Announce Current Track", { icon = "music", accent = true }))
    else
        table.insert(widgets, tntron.ui.text("Play a track to inspect full metadata and announcements.", true))
    end

    table.insert(widgets, tntron.ui.heading("Recent Announcements (" .. tostring(#st.history) .. ")"))
    for _, line in ipairs(st.history) do
        table.insert(widgets, tntron.ui.text(line, true))
    end

    tntron.ui.register_sidebar({
        id = "track-announcer",
        label = "Inspector",
        icon = "disc",
        widgets = widgets,
    })
end

function on_load(_ctx)
    local saved = tntron.storage.get("history", {})
    st.history = type(saved) == "table" and saved or {}
    render()
end

function on_event(_ctx, event_name)
    if event_name == "trackChange" then
        local pb = tntron.playback.get()
        if pb and pb.track_path ~= "" and pb.track_path ~= st.last_path then
            st.last_path = pb.track_path
            local entry = string.format("%s - %s (%s)", pb.artist ~= "" and pb.artist or "Unknown", pb.title, fmt_sec(pb.duration_sec))
            table.insert(st.history, 1, entry)
            while #st.history > 15 do table.remove(st.history) end
            tntron.storage.set("history", st.history)
        end
    end
    render()
end

function on_action(_ctx, action_id, _val)
    if action_id == "announce_now" then
        local pb = tntron.playback.get()
        if pb and pb.title ~= "" then
            tntron.ui.toast(string.format("%s — %s (%s)", pb.title, pb.artist, fmt_sec(pb.duration_sec)))
        else
            tntron.ui.toast("No active track")
        end
    end
end
