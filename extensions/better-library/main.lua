-- Better Library (Openify Playlist/Folder Organizer; requires openify.better-ui)

local st = {
    filter = "",
    sort_by = "az", -- "az", "count", "time"
    sort_desc = false,
    recent_dirs = {},
}

local function fmt_duration(sec)
    local total = math.max(0, math.floor(tonumber(sec) or 0))
    local hrs = math.floor(total / 3600)
    local mins = math.floor((total % 3600) / 60)
    local secs = total % 60
    if hrs > 0 then
        return string.format("%dh %dm", hrs, mins)
    end
    return string.format("%d:%02d", mins, secs)
end

local function folder_matches(f, needle)
    if needle == "" then return true end
    return (f.name or ""):lower():find(needle, 1, true) ~= nil
end

local function sort_folders(list)
    table.sort(list, function(a, b)
        local cmp
        if st.sort_by == "count" then
            cmp = (a.track_count or 0) < (b.track_count or 0)
            if (a.track_count or 0) == (b.track_count or 0) then
                cmp = (a.name or ""):lower() < (b.name or ""):lower()
            end
        elseif st.sort_by == "time" then
            cmp = (a.total_duration_sec or 0) < (b.total_duration_sec or 0)
            if math.abs((a.total_duration_sec or 0) - (b.total_duration_sec or 0)) < 0.5 then
                cmp = (a.name or ""):lower() < (b.name or ""):lower()
            end
        else
            cmp = (a.name or ""):lower() < (b.name or ""):lower()
        end
        if st.sort_desc then return not cmp end
        return cmp
    end)
end

local function render()
    local folders = tntron.library.folders() or {}
    local all = tntron.library.all_tracks() or {}
    local cur = tntron.library.current_folder() or { path = "" }
    local pb = tntron.playback.get()
    local playing_dir = ""
    if pb and pb.track_path and pb.track_path ~= "" then
        playing_dir = pb.track_path:match("^(.*)/[^/]+$") or ""
    end

    local needle = st.filter:lower()
    local matched = {}
    local total_dur = 0
    for _, t in ipairs(all) do
        total_dur = total_dur + (t.duration_sec or 0)
    end
    for _, f in ipairs(folders) do
        if folder_matches(f, needle) then
            table.insert(matched, f)
        end
    end
    sort_folders(matched)

    local widgets = {
        tntron.ui.text("Folders in your music directory act as playlists.", true),
        tntron.ui.input("filter", "Filter Folders", st.filter, {
            placeholder = "Search playlist folders...",
            submit_action = "apply_filter",
            submit_label = "Filter",
        }),
        tntron.ui.row({
            tntron.ui.button("sort:az",    "A-Z",    { active = st.sort_by == "az" }),
            tntron.ui.button("sort:count", "# Songs", { active = st.sort_by == "count" }),
            tntron.ui.button("sort:time",  "Time",   { active = st.sort_by == "time" }),
            tntron.ui.button("sort_dir",   st.sort_desc and "Desc" or "Asc", { icon = "refresh", active = st.sort_desc }),
            tntron.ui.badge(string.format("%d folders · %d songs · %s", #folders, #all, fmt_duration(total_dur)), { icon = "library", accent = true }),
        }),
    }

    if #st.recent_dirs > 0 then
        local rbtns = {}
        for i = 1, math.min(#st.recent_dirs, 4) do
            local r = st.recent_dirs[i]
            table.insert(rbtns, tntron.ui.button("open:" .. r.path, r.name, { icon = "folder" }))
        end
        table.insert(widgets, tntron.ui.heading("Recently Played Folders"))
        table.insert(widgets, tntron.ui.row(rbtns))
    end

    table.insert(widgets, tntron.ui.heading("Playlist Folders"))
    if #matched == 0 then
        table.insert(widgets, tntron.ui.text("No matching folders.", true))
    else
        local folder_cards = {}
        for _, f in ipairs(matched) do
            local is_playing = (playing_dir ~= "" and playing_dir == f.path)
            local is_open = (cur.path == f.path)
            local dur_str = fmt_duration(f.total_duration_sec)
            table.insert(folder_cards, tntron.ui.card({
                id = "play:" .. f.path,
                title = f.name,
                subtitle = string.format("%d songs · %s", f.track_count or 0, dur_str),
                badge = is_playing and "PLAYING" or (is_open and "OPEN" or nil),
                icon = "folder",
                variant = "tile",
                accent = is_playing,
                active = is_open or is_playing,
            }))
        end
        table.insert(widgets, tntron.ui.grid(3, folder_cards))
    end

    tntron.ui.register_sidebar({
        id = "better-library",
        label = "Library+",
        icon = "library",
        widgets = widgets,
    })
end

function on_load(_ctx)
    st.sort_by = tntron.storage.get("sort_by", "az")
    st.sort_desc = (tntron.storage.get("sort_desc", false) == true)
    local rec = tntron.storage.get("recent_dirs", nil)
    if type(rec) == "table" then
        st.recent_dirs = rec
    end
    render()
end

function on_event(_ctx, event_name)
    if event_name == "trackChange" then
        local pb = tntron.playback.get()
        if pb and pb.track_path and pb.track_path ~= "" then
            local dir = pb.track_path:match("^(.*)/[^/]+$")
            local name = dir and dir:match("([^/]+)$")
            if dir and name then
                local next_list = { { path = dir, name = name } }
                for _, old in ipairs(st.recent_dirs) do
                    if old.path ~= dir and #next_list < 6 then
                        table.insert(next_list, old)
                    end
                end
                st.recent_dirs = next_list
                tntron.storage.set("recent_dirs", st.recent_dirs)
            end
        end
        render()
    end
end

function on_action(_ctx, action_id, val)
    if action_id == "apply_filter" then
        st.filter = type(val) == "string" and val or ""
    elseif action_id:sub(1, 5) == "sort:" then
        st.sort_by = action_id:sub(6)
        tntron.storage.set("sort_by", st.sort_by)
    elseif action_id == "sort_dir" then
        st.sort_desc = not st.sort_desc
        tntron.storage.set("sort_desc", st.sort_desc)
    elseif action_id:sub(1, 5) == "open:" then
        tntron.library.open_folder(action_id:sub(6))
        tntron.ui.toast("Opened folder")
    elseif action_id:sub(1, 5) == "play:" then
        tntron.library.play_folder(action_id:sub(6), false)
    elseif action_id:sub(1, 8) == "shuffle:" then
        tntron.library.play_folder(action_id:sub(9), true)
    end
    render()
end
