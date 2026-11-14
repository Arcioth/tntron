-- Playback Queue Manager (ported from Tunotron queue.lua)

local st = { q = {} }

local function render()
    local q = tntron.queue.list() or {}
    st.q = q
    tntron.ui.clear_topbar_slot()

    local widgets = {
        tntron.ui.row({
            tntron.ui.heading(string.format("Active Queue (%d)", #q)),
            tntron.ui.button("enqueue_favs", "Enqueue Favorites", { icon = "star", accent = true }),
            tntron.ui.button("enqueue_dir",  "Enqueue Folder",    { icon = "plus" }),
            tntron.ui.button("clear_q",      "Clear Queue",       { icon = "close" }),
        }),
    }

    if #q == 0 then
        table.insert(widgets, tntron.ui.text("Queue is currently empty.", true))
    else
        for i, item in ipairs(q) do
            table.insert(widgets, tntron.ui.button(
                "play_q:" .. tostring(i),
                string.format("%d. %s — %s", i, item.title or "Track", item.artist or "Unknown"),
                { icon = "play" }
            ))
        end
    end

    tntron.ui.register_sidebar({
        id = "queue-mgr",
        label = "Queue+",
        icon = "queue",
        widgets = widgets,
    })
end

function on_load(_ctx)
    render()
end

function on_event(_ctx, _ev)
    render()
end

function on_tick(_ctx)
    render()
end

function on_action(_ctx, action_id, _val)
    if action_id == "enqueue_favs" then
        local favs = tntron.library.favorites() or {}
        for _, p in ipairs(favs) do tntron.queue.add(p) end
        tntron.ui.toast("Enqueued " .. tostring(#favs) .. " favorites")
    elseif action_id == "enqueue_dir" then
        local cur = tntron.library.current_folder() or { tracks = {} }
        local n = 0
        for _, t in ipairs(cur.tracks or {}) do
            tntron.queue.add(t.path)
            n = n + 1
        end
        tntron.ui.toast("Enqueued " .. tostring(n) .. " tracks")
    elseif action_id == "clear_q" then
        tntron.queue.clear()
        tntron.ui.toast("Queue cleared")
    elseif action_id == "next_in_q" then
        tntron.playback.next()
    elseif action_id:sub(1, 7) == "play_q:" then
        local idx = tonumber(action_id:sub(8))
        local item = idx and st.q[idx]
        if item and item.path then tntron.playback.play_path(item.path) end
    end
    render()
end
