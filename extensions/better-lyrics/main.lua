-- Better Lyrics (Openify synced lyrics + timing offset + skip lines)

local st = { track_path = "", offset = 0.0, skip_lines = 0, lines = {}, show_bar = true }

local function get_saved_num(map_key, path, default_val)
    if path == "" then return default_val end
    local tbl = tntron.storage.get(map_key, {})
    if type(tbl) == "table" and tbl[path] ~= nil then
        return tonumber(tbl[path]) or default_val
    end
    return default_val
end

local function save_map_num(map_key, path, val)
    if path == "" then return end
    local tbl = tntron.storage.get(map_key, {})
    if type(tbl) ~= "table" then tbl = {} end
    tbl[path] = val
    tntron.storage.set(map_key, tbl)
end

local function active_index(pb)
    if not pb or #st.lines == 0 then return nil end
    local adj = (pb.position_sec or 0) + st.offset
    local base_idx = nil
    for i, l in ipairs(st.lines) do
        if (tonumber(l.time_sec) or 0) <= (adj + 0.1) then
            base_idx = i
        else
            break
        end
    end
    if not base_idx then return nil end
    return math.max(1, math.min(#st.lines, base_idx + st.skip_lines))
end

local function render(pb)
    local act = active_index(pb)
    local cur_line = act and st.lines[act] and st.lines[act].text or nil

    if st.show_bar and cur_line and cur_line ~= "" then
        local short = #cur_line > 42 and (cur_line:sub(1, 39) .. "...") or cur_line
        tntron.ui.set_player_widget({
            text = short,
            icon = "lyrics",
            accent = true,
            action_id = "nudge_back",
        })
    else
        tntron.ui.clear_player_widget()
    end

    local wid = {
        tntron.ui.heading(pb and pb.title ~= "" and pb.title or "Better Lyrics"),
        tntron.ui.slider("offset", "Timing Offset", -5.0, 5.0, 0.25, st.offset, "s"),
        tntron.ui.row({
            tntron.ui.button("nudge_back", "-0.5s", { icon = "prev" }),
            tntron.ui.button("nudge_zero", "0.0s", { active = math.abs(st.offset) < 0.01 }),
            tntron.ui.button("nudge_fwd",  "+0.5s", { icon = "next" }),
            tntron.ui.button("skip_prev",  "Skip -1"),
            tntron.ui.button("skip_zero",  "Skip: " .. tostring(st.skip_lines), { active = st.skip_lines ~= 0 }),
            tntron.ui.button("skip_next",  "Skip +1"),
            tntron.ui.button("resync", "Sync Now", { icon = "refresh", accent = true }),
        }),
        tntron.ui.toggle("bar_toggle", "Show live lyric line in player bar", st.show_bar),
    }

    if #st.lines == 0 then
        table.insert(wid, tntron.ui.text("Waiting for synced lyrics (.lrc / LRCLIB)...", true))
    else
        local center = act or 1
        local lo = math.max(1, center - 4)
        local hi = math.min(#st.lines, lo + 9)
        for i = lo, hi do
            local line_txt = st.lines[i].text or ""
            if line_txt ~= "" then
                if i == act then
                    table.insert(wid, tntron.ui.heading(">> " .. line_txt))
                else
                    table.insert(wid, tntron.ui.text(line_txt, true))
                end
            end
        end
    end

    tntron.ui.register_sidebar({
        id = "better-lyrics",
        label = "Lyrics Sync+",
        icon = "lyrics",
        widgets = wid,
    })
end

local function request_lines(pb)
    if not pb or pb.track_path == "" then
        st.track_path = ""
        st.offset = 0.0
        st.skip_lines = 0
        st.lines = {}
        render(pb)
        return
    end
    st.track_path = pb.track_path
    st.offset = get_saved_num("offsets", pb.track_path, 0.0)
    st.skip_lines = math.floor(get_saved_num("skips", pb.track_path, 0))
    st.lines = {}
    tntron.lyrics.load(pb.track_path, pb.title, pb.artist, pb.duration_sec or 0)
    render(pb)
end

function on_load(_ctx)
    st.show_bar = (tntron.storage.get("show_bar", true) ~= false)
    request_lines(tntron.playback.get())
end

function on_event(_ctx, event_name, payload)
    local pb = tntron.playback.get()
    if event_name == "trackChange" then
        request_lines(pb)
    elseif event_name == "lyricsLoaded" and type(payload) == "table" then
        st.lines = type(payload.lines) == "table" and payload.lines or {}
        render(pb)
    end
end

function on_tick(_ctx)
    render(tntron.playback.get())
end

function on_action(_ctx, action_id, val)
    local pb = tntron.playback.get()
    if action_id == "offset" then
        st.offset = tonumber(val) or 0.0
        save_map_num("offsets", st.track_path, st.offset)
    elseif action_id == "nudge_back" then
        st.offset = math.max(-5.0, st.offset - 0.5)
        save_map_num("offsets", st.track_path, st.offset)
    elseif action_id == "nudge_fwd" then
        st.offset = math.min(5.0, st.offset + 0.5)
        save_map_num("offsets", st.track_path, st.offset)
    elseif action_id == "nudge_zero" then
        st.offset = 0.0
        save_map_num("offsets", st.track_path, 0.0)
    elseif action_id == "skip_prev" then
        st.skip_lines = math.max(-10, st.skip_lines - 1)
        save_map_num("skips", st.track_path, st.skip_lines)
    elseif action_id == "skip_next" then
        st.skip_lines = math.min(10, st.skip_lines + 1)
        save_map_num("skips", st.track_path, st.skip_lines)
    elseif action_id == "skip_zero" then
        st.skip_lines = 0
        save_map_num("skips", st.track_path, 0)
    elseif action_id == "resync" then
        request_lines(pb)
    elseif action_id == "bar_toggle" then
        st.show_bar = (val == true)
        tntron.storage.set("show_bar", st.show_bar)
    end
    render(pb)
end
