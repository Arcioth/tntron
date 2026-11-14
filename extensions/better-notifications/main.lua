-- Notification History Tray (ported from Tunotron better_notifications.lua)

local st = { logs = {}, unread = 0, last_title = "" }

local function push_log(msg)
    table.insert(st.logs, 1, msg)
    while #st.logs > 25 do
        table.remove(st.logs)
    end
    st.unread = st.unread + 1
    tntron.storage.set("logs", st.logs)
end

local function render()
    tntron.ui.set_topbar_slot({
        text = st.unread > 0 and ("Alerts " .. tostring(st.unread)) or "Alerts",
        icon = "bell",
        accent = (st.unread > 0),
        action_id = "open_alerts",
    })

    local widgets = {
        tntron.ui.row({
            tntron.ui.heading("Notification History (" .. tostring(#st.logs) .. ")"),
            tntron.ui.button("mark_read", "Mark Read", { icon = "bell" }),
            tntron.ui.button("clear", "Clear All", { icon = "close" }),
        }),
    }
    if #st.logs == 0 then
        table.insert(widgets, tntron.ui.text("No logged events yet. Play or switch tracks to record alerts.", true))
    else
        for _, entry in ipairs(st.logs) do
            table.insert(widgets, tntron.ui.text(entry, false))
        end
    end

    tntron.ui.register_sidebar({
        id = "notif-tray",
        label = "Alerts",
        icon = "bell",
        widgets = widgets,
    })
end

function on_load(_ctx)
    local saved = tntron.storage.get("logs", {})
    st.logs = type(saved) == "table" and saved or {}
    render()
end

function on_event(_ctx, event_name)
    local pb = tntron.playback.get()
    if event_name == "trackChange" and pb and pb.title ~= "" and pb.title ~= st.last_title then
        st.last_title = pb.title
        push_log(string.format("Playing: %s — %s", pb.title, pb.artist))
        render()
    elseif event_name == "playbackStateChange" and pb then
        if pb.paused then
            push_log("Paused: " .. (pb.title ~= "" and pb.title or "Stream"))
            render()
        end
    end
end

function on_action(_ctx, action_id, _val)
    if action_id == "open_alerts" then
        tntron.ui.open_sidebar("notif-tray")
        st.unread = 0
    elseif action_id == "mark_read" then
        st.unread = 0
        tntron.ui.toast("All notifications marked read")
    elseif action_id == "clear" then
        st.logs = {}
        st.unread = 0
        tntron.storage.set("logs", st.logs)
        tntron.ui.toast("Notification tray cleared")
    end
    render()
end
