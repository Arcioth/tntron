-- Better Home Dashboard (Openify card-grid visual architecture)
-- Greeting + Stat Cards + Now Playing Hero Card + Recently Played Tile Grid + Shuffled Quick Picks Grid + Top Folders Grid

local st = {
    recent = {},
    path_map = {},
    picks_indices = {},
    seed = 1,
}

local function fmt_dur(sec)
    local s = math.floor(sec or 0)
    local m = math.floor(s / 60)
    local h = math.floor(m / 60)
    if h > 0 then
        return string.format("%dh %dm", h, m % 60)
    end
    return string.format("%dm", math.max(1, m))
end

local function greeting_for_hour(hour)
    local h = tonumber(hour) or 12
    if h < 12 then
        return "Good Morning"
    elseif h < 18 then
        return "Good Afternoon"
    else
        return "Good Evening"
    end
end

local function short_title(s, max_len)
    if not s then return "Untitled" end
    if #s <= max_len then return s end
    return s:sub(1, max_len - 1) .. "…"
end

local function rebuild_quick_picks(all_count)
    st.picks_indices = {}
    if all_count <= 0 then
        return
    end
    local pool = {}
    for i = 1, all_count do
        pool[i] = i
    end
    local x = math.abs(math.floor(tonumber(st.seed) or 1)) % 2147483647
    if x == 0 then x = 1337 end
    for i = all_count, 2, -1 do
        x = (x * 48271) % 2147483647
        local j = (x % i) + 1
        pool[i], pool[j] = pool[j], pool[i]
    end
    local limit = math.min(8, all_count)
    for i = 1, limit do
        table.insert(st.picks_indices, pool[i])
    end
end

local function render(ctx)
    local all = tntron.library.all_tracks() or {}
    local folders = tntron.library.folders() or {}
    local favs = tntron.library.favorites() or {}
    local pb = tntron.playback.get()
    local total_sec = 0
    st.path_map = {}

    for _, t in ipairs(all) do
        total_sec = total_sec + (tonumber(t.duration_sec) or 0)
    end

    if #all > 0 then
        local valid = (#st.picks_indices > 0)
        if valid then
            for _, idx in ipairs(st.picks_indices) do
                if idx < 1 or idx > #all then
                    valid = false
                    break
                end
            end
        end
        if not valid then
            rebuild_quick_picks(#all)
        end
    else
        st.picks_indices = {}
    end

    local hour = (ctx and ctx.local_hour) or 12
    local widgets = {
        tntron.ui.row({
            tntron.ui.heading(greeting_for_hour(hour)),
            tntron.ui.button("play_random", "Shuffle Library", { icon = "shuffle", accent = true }),
            tntron.ui.button("play_fav", "Play Favorites", { icon = "star" }),
        }),
        tntron.ui.grid(3, {
            tntron.ui.card({
                eyebrow = "LIBRARY",
                title = tostring(#all),
                subtitle = "Total Songs",
                icon = "music",
                variant = "stat",
                accent = true,
            }),
            tntron.ui.card({
                eyebrow = "PLAYTIME",
                title = #all > 0 and fmt_dur(total_sec) or "0m",
                subtitle = "Combined Duration",
                icon = "clock",
                variant = "stat",
            }),
            tntron.ui.card({
                eyebrow = "PLAYLISTS",
                title = tostring(#folders),
                subtitle = tostring(#favs) .. " starred favorites",
                icon = "folder",
                variant = "stat",
            }),
        }),
    }

    -- Now Playing Hero Card
    if pb and pb.track_path and pb.track_path ~= "" then
        local np_title = pb.title ~= "" and pb.title or "Unknown Title"
        local np_artist = pb.artist ~= "" and pb.artist or "Unknown Artist"
        local np_sub = np_artist
        if pb.bpm and tonumber(pb.bpm) then
            np_sub = string.format("%s  ·  %.0f BPM", np_artist, tonumber(pb.bpm))
        end
        local state_lbl = pb.paused and "PAUSED" or (pb.playing and "PLAYING" or "STOPPED")
        table.insert(widgets, tntron.ui.card({
            id = "toggle_pause",
            eyebrow = "NOW PLAYING",
            title = np_title,
            subtitle = np_sub,
            badge = state_lbl,
            icon = "disc",
            variant = "hero",
            accent = (pb.playing == true),
            active = (pb.playing == true),
        }))
        table.insert(widgets, tntron.ui.row({
            tntron.ui.button("toggle_pause", pb.paused and "Resume" or "Pause", { icon = pb.paused and "play" or "pause", accent = true }),
            tntron.ui.button("next_track", "Next Track", { icon = "next" }),
        }))
    else
        table.insert(widgets, tntron.ui.card({
            eyebrow = "READY TO PLAY",
            title = "Pick a song or folder below",
            subtitle = "Your recently played songs and shuffled picks appear here.",
            icon = "disc",
            variant = "hero",
        }))
    end

    -- Recently Played Cards Grid (4 columns of tall square-art cards)
    table.insert(widgets, tntron.ui.heading("Recently Played"))
    if #st.recent == 0 then
        table.insert(widgets, tntron.ui.text("Play tracks to build your Recent History.", true))
    else
        local rec_cards = {}
        for i, item in ipairs(st.recent) do
            if i > 8 then break end
            st.path_map["rec:" .. tostring(i)] = item.path
            local is_cur = pb and pb.track_path == item.path
            table.insert(rec_cards, tntron.ui.card({
                id = "play:rec:" .. tostring(i),
                title = short_title(item.title or "Track", 28),
                subtitle = short_title(item.artist or "Unknown", 26),
                icon = "music",
                variant = "tile",
                active = is_cur,
                accent = is_cur,
            }))
        end
        table.insert(widgets, tntron.ui.grid(4, rec_cards))
    end

    -- Quick Picks Grid (2 columns of compact horizontal cards)
    table.insert(widgets, tntron.ui.row({
        tntron.ui.heading("Quick Picks"),
        tntron.ui.button("reshuffle_picks", "Shuffle Picks", { icon = "refresh" }),
    }))
    if #all == 0 then
        table.insert(widgets, tntron.ui.text("No tracks found in library.", true))
    else
        local pick_cards = {}
        for i, idx in ipairs(st.picks_indices) do
            local t = all[idx]
            if t then
                st.path_map["pick:" .. tostring(i)] = t.path
                local is_cur = pb and pb.track_path == t.path
                table.insert(pick_cards, tntron.ui.card({
                    id = "play:pick:" .. tostring(i),
                    title = short_title(t.title or "Track", 34),
                    subtitle = (t.folder or "Library") .. " · " .. (t.artist or "Unknown"),
                    icon = "disc",
                    variant = "compact",
                    active = is_cur,
                }))
            end
        end
        table.insert(widgets, tntron.ui.grid(2, pick_cards))
    end

    -- Top Folders Cards Grid (3 columns sorted descending by track count)
    if #folders > 0 then
        local sorted_folders = {}
        for _, f in ipairs(folders) do
            table.insert(sorted_folders, f)
        end
        table.sort(sorted_folders, function(a, b)
            local ca = tonumber(a.track_count) or 0
            local cb = tonumber(b.track_count) or 0
            if ca ~= cb then
                return ca > cb
            end
            return (a.name or "") < (b.name or "")
        end)

        table.insert(widgets, tntron.ui.heading("Top Folders"))
        local folder_cards = {}
        local top_count = math.min(6, #sorted_folders)
        for i = 1, top_count do
            local f = sorted_folders[i]
            local key = "fld:" .. tostring(i)
            st.path_map[key] = f.path
            table.insert(folder_cards, tntron.ui.card({
                id = "play_folder:" .. key,
                title = short_title(f.name or "Folder", 24),
                subtitle = string.format("%d songs · Play folder", tonumber(f.track_count) or 0),
                icon = "folder",
                variant = "compact",
                accent = (i == 1),
            }))
        end
        table.insert(widgets, tntron.ui.grid(3, folder_cards))
    end

    tntron.ui.register_sidebar({
        id = "better-home",
        label = "Home+",
        icon = "home",
        widgets = widgets,
    })
end

local function record_current(pb)
    if not pb or not pb.track_path or pb.track_path == "" then return end
    if st.recent[1] and st.recent[1].path == pb.track_path then return end
    local filtered = { { path = pb.track_path, title = pb.title, artist = pb.artist } }
    for _, item in ipairs(st.recent) do
        if item.path ~= pb.track_path and #filtered < 10 then
            table.insert(filtered, item)
        end
    end
    st.recent = filtered
    tntron.storage.set("recent", st.recent)
end

function on_load(ctx)
    local saved = tntron.storage.get("recent", {})
    st.recent = type(saved) == "table" and saved or {}
    st.seed = (ctx and tonumber(ctx.now_sec)) or 42
    local all = tntron.library.all_tracks() or {}
    rebuild_quick_picks(#all)
    render(ctx)
end

function on_event(ctx, event_name)
    if event_name == "trackChange" then
        record_current(tntron.playback.get())
    end
    render(ctx)
end

function on_action(ctx, action_id, _val)
    if action_id == "play_random" then
        local all = tntron.library.all_tracks() or {}
        if #all > 0 then
            st.seed = st.seed + 3571 + ((ctx and tonumber(ctx.now_sec)) or 11)
            local pick_idx = (math.abs(st.seed) % #all) + 1
            local tr = all[pick_idx]
            if tr and tr.path then
                tntron.playback.play_path(tr.path)
                tntron.ui.toast("Random Discovery: " .. (tr.title or "Track"))
            end
        end
    elseif action_id == "play_fav" then
        local favs = tntron.library.favorites() or {}
        if #favs > 0 then
            tntron.playback.play_path(favs[1])
            tntron.ui.toast("Playing favorites")
        else
            tntron.ui.toast("No starred tracks yet")
        end
    elseif action_id == "toggle_pause" then
        tntron.playback.toggle()
    elseif action_id == "next_track" then
        tntron.playback.next()
    elseif action_id == "reshuffle_picks" then
        st.seed = st.seed + 7919 + ((ctx and tonumber(ctx.now_sec)) or 17)
        local all = tntron.library.all_tracks() or {}
        rebuild_quick_picks(#all)
    elseif action_id:sub(1, 5) == "play:" then
        local key = action_id:sub(6)
        local p = st.path_map[key]
        if p then
            tntron.playback.play_path(p)
        end
    elseif action_id:sub(1, 12) == "play_folder:" then
        local key = action_id:sub(13)
        local p = st.path_map[key]
        if p then
            tntron.library.play_folder(p, false)
        end
    end
    render(ctx)
end
