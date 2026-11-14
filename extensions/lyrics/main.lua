-- Lyrics (Openify lyrics + Tunotron lyrics_viewer + LRCLIB)

local st = {
    track_path = "",
    title = "",
    artist = "",
    source = "Waiting",
    loading = false,
    error = nil,
    lines = {},
}

local function find_active_idx(lines, pos)
    local active = nil
    for i, l in ipairs(lines) do
        if (tonumber(l.time_sec) or 0) <= (pos + 0.1) then
            active = i
        else
            break
        end
    end
    return active
end

local function fmt(sec)
    local s = math.max(0, math.floor(sec or 0))
    return string.format("%02d:%02d", math.floor(s / 60), s % 60)
end

local function render(pb)
    local widgets = {}
    local title = pb and pb.title or "No track playing"
    local artist = pb and pb.artist or ""
    table.insert(widgets, tntron.ui.heading(title))
    table.insert(widgets, tntron.ui.row({
        tntron.ui.text(artist ~= "" and artist or "Select or play a track", true),
        tntron.ui.badge(st.loading and "Searching..." or st.source, { icon = "lyrics", accent = (#st.lines > 0) }),
        tntron.ui.button("reload_lrc", "Refresh", { icon = "refresh" }),
    }))

    if st.error and #st.lines == 0 then
        table.insert(widgets, tntron.ui.text("Lookup info: " .. tostring(st.error), true))
    elseif #st.lines == 0 then
        table.insert(widgets, tntron.ui.text("No synced lines loaded yet for this track.", true))
    else
        local pos = pb and pb.position_sec or 0
        local active = find_active_idx(st.lines, pos) or 1
        local start_i = math.max(1, active - 4)
        local end_i = math.min(#st.lines, start_i + 14)
        for i = start_i, end_i do
            local line = st.lines[i]
            local is_cur = (i == active)
            local ts = fmt(line.time_sec)
            table.insert(widgets, tntron.ui.button(
                "seek:" .. tostring(i),
                string.format("[%s] %s", ts, line.text),
                { active = is_cur, accent = is_cur }
            ))
        end
    end

    tntron.ui.register_sidebar({
        id = "lyrics-view",
        label = "Lyrics",
        icon = "lyrics",
        widgets = widgets,
    })
end

local function fetch_for_current(pb)
    if not pb or pb.track_path == "" then
        st.track_path = ""
        st.lines = {}
        st.source = "Idle"
        render(pb)
        return
    end
    st.track_path = pb.track_path
    st.title = pb.title
    st.artist = pb.artist
    st.source = "Searching..."
    st.loading = true
    st.error = nil
    st.lines = {}
    tntron.lyrics.load(pb.track_path, pb.title, pb.artist, pb.duration_sec or 0)
    render(pb)
end

function on_load(_ctx)
    fetch_for_current(tntron.playback.get())
end

function on_event(_ctx, event_name, payload)
    local pb = tntron.playback.get()
    if event_name == "trackChange" then
        fetch_for_current(pb)
    elseif event_name == "lyricsLoaded" and type(payload) == "table" then
        if payload.path == st.track_path or st.track_path == "" then
            st.loading = false
            st.source = payload.source or "Loaded"
            st.error = payload.error
            st.lines = type(payload.lines) == "table" and payload.lines or {}
            render(pb)
        end
    end
end

function on_tick(_ctx)
    if #st.lines > 0 then
        render(tntron.playback.get())
    end
end

function on_action(_ctx, action_id, _val)
    local pb = tntron.playback.get()
    if action_id == "reload_lrc" then
        fetch_for_current(pb)
    elseif action_id:sub(1, 5) == "seek:" then
        local idx = tonumber(action_id:sub(6))
        local line = idx and st.lines[idx]
        if line and line.time_sec then
            tntron.playback.seek(tonumber(line.time_sec) or 0)
        end
    end
end
