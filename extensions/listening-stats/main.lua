-- Listening Stats & Play Analytics (ported from Tunotron stats.lua)

local st = { total_sec = 0, plays = {}, last_path = "", tick_flush = 0 }

local function fmt_time(sec)
    local s = math.floor(sec or 0)
    local m = math.floor(s / 60)
    local h = math.floor(m / 60)
    if h > 0 then return string.format("%dh %dm %ds", h, m % 60, s % 60) end
    return string.format("%dm %02ds", m, s % 60)
end

local function sorted_top(limit)
    local rows = {}
    for k, v in pairs(st.plays) do
        if type(v) == "table" then
            table.insert(rows, { title = v.title or k, artist = v.artist or "", count = tonumber(v.count) or 0 })
        end
    end
    table.sort(rows, function(a, b) return a.count > b.count end)
    local out = {}
    for i = 1, math.min(limit, #rows) do table.insert(out, rows[i]) end
    return out
end

local function render()
    local top = sorted_top(8)
    local widgets = {
        tntron.ui.row({
            tntron.ui.badge("Listening Time: " .. fmt_time(st.total_sec), { icon = "clock", accent = true }),
            tntron.ui.badge("Unique Tracks: " .. tostring(#top), { icon = "chart" }),
        }),
        tntron.ui.row({
            tntron.ui.button("reset_stats", "Reset Analytics", { icon = "close" }),
        }),
        tntron.ui.heading("Most Played Tracks"),
    }

    if #top == 0 then
        table.insert(widgets, tntron.ui.text("Play songs to populate your listening leaderboard.", true))
    else
        for i, item in ipairs(top) do
            table.insert(widgets, tntron.ui.text(
                string.format("%d. %s — %s (%d plays)", i, item.title, item.artist, item.count),
                false
            ))
        end
    end

    tntron.ui.register_sidebar({
        id = "stats-view",
        label = "Stats",
        icon = "chart",
        widgets = widgets,
    })
end

function on_load(_ctx)
    st.total_sec = tonumber(tntron.storage.get("total_sec", 0)) or 0
    local p = tntron.storage.get("plays", {})
    st.plays = type(p) == "table" and p or {}
    render()
end

function on_event(_ctx, event_name)
    if event_name == "trackChange" then
        local pb = tntron.playback.get()
        if pb and pb.track_path ~= "" and pb.track_path ~= st.last_path then
            st.last_path = pb.track_path
            local cur = st.plays[pb.track_path]
            local cnt = (type(cur) == "table" and tonumber(cur.count) or 0) + 1
            st.plays[pb.track_path] = { title = pb.title, artist = pb.artist, count = cnt }
            tntron.storage.set("plays", st.plays)
            render()
        end
    end
end

function on_tick(_ctx)
    local pb = tntron.playback.get()
    if pb and pb.playing then
        st.total_sec = st.total_sec + 1
        st.tick_flush = st.tick_flush + 1
        if st.tick_flush >= 5 then
            st.tick_flush = 0
            tntron.storage.set("total_sec", st.total_sec)
        end
        render()
    end
end

function on_action(_ctx, action_id, _val)
    if action_id == "reset_stats" then
        st.total_sec = 0
        st.plays = {}
        st.last_path = ""
        tntron.storage.set("total_sec", 0)
        tntron.storage.set("plays", {})
        tntron.ui.toast("Listening stats reset")
        render()
    end
end
