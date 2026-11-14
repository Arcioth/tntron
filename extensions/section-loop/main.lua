-- A-B Section Looper (ported from Tunotron section_loop.lua)

local st = { a = nil, b = nil, enabled = true, track = "" }

local function fmt(sec)
    if not sec then return "--:--" end
    local s = math.floor(sec)
    return string.format("%02d:%02d", math.floor(s / 60), s % 60)
end

local function is_looping()
    return st.enabled and st.a ~= nil and st.b ~= nil and st.b > (st.a + 0.3)
end

local function render(pb)
    local cur_track = pb and pb.title or "No track"
    local status = string.format("A: %s   |   B: %s", fmt(st.a), fmt(st.b))

    if is_looping() then
        tntron.ui.set_player_widget({
            text = string.format("A-B %s-%s", fmt(st.a), fmt(st.b)),
            icon = "bookmark",
            accent = true,
            action_id = "clear",
        })
    elseif st.a ~= nil then
        tntron.ui.set_player_widget({
            text = string.format("A %s...", fmt(st.a)),
            icon = "bookmark",
            accent = false,
            action_id = "mark_b",
        })
    else
        tntron.ui.clear_player_widget()
    end

    tntron.ui.register_sidebar({
        id = "section-loop",
        label = "A-B Loop",
        icon = "bookmark",
        widgets = {
            tntron.ui.text("Track: " .. cur_track, true),
            tntron.ui.heading(status),
            tntron.ui.row({
                tntron.ui.button("mark_a", "Mark A", { icon = "bookmark", active = st.a ~= nil }),
                tntron.ui.button("mark_b", "Mark B", { icon = "bookmark", active = st.b ~= nil }),
                tntron.ui.button("jump_a", "Jump to A", { icon = "play" }),
                tntron.ui.button("clear",  "Clear", { icon = "close" }),
            }),
            tntron.ui.toggle("toggle_loop", "Loop Active", st.enabled),
            tntron.ui.row({
                tntron.ui.button("save_track", "Save for Track", { icon = "star", accent = true }),
                tntron.ui.button("load_track", "Load Saved", { icon = "refresh" }),
            }),
        },
    })
end

function on_load(ctx)
    local pb = tntron.playback.get()
    st.track = pb and pb.track_path or ""
    render(pb)
end

function on_event(_ctx, event_name)
    if event_name == "trackChange" then
        local pb = tntron.playback.get()
        st.track = pb and pb.track_path or ""
        local saved = tntron.storage.get("loops", {})
        local entry = (type(saved) == "table" and st.track ~= "") and saved[st.track] or nil
        if type(entry) == "table" then
            st.a = tonumber(entry.a)
            st.b = tonumber(entry.b)
        else
            st.a = nil
            st.b = nil
        end
        render(pb)
    end
end

function on_tick(_ctx)
    local pb = tntron.playback.get()
    if pb and pb.playing and is_looping() and pb.position_sec >= st.b then
        tntron.playback.seek(st.a)
    end
end

function on_action(_ctx, action_id, val)
    local pb = tntron.playback.get()
    local pos = pb and pb.position_sec or 0
    if action_id == "mark_a" then
        st.a = pos
        if st.b and st.b <= st.a then st.b = nil end
        tntron.ui.toast("Marked A at " .. fmt(st.a))
    elseif action_id == "mark_b" then
        if st.a and pos > st.a + 0.3 then
            st.b = pos
            st.enabled = true
            tntron.playback.seek(st.a)
            tntron.ui.toast(string.format("A-B loop %s -> %s", fmt(st.a), fmt(st.b)))
        else
            tntron.ui.toast("Mark B must be after Mark A")
        end
    elseif action_id == "jump_a" then
        if st.a then tntron.playback.seek(st.a) end
    elseif action_id == "clear" then
        st.a = nil
        st.b = nil
    elseif action_id == "toggle_loop" then
        st.enabled = (val == true)
    elseif action_id == "save_track" then
        if st.track ~= "" and st.a and st.b then
            local saved = tntron.storage.get("loops", {})
            if type(saved) ~= "table" then saved = {} end
            saved[st.track] = { a = st.a, b = st.b }
            tntron.storage.set("loops", saved)
            tntron.ui.toast("Saved A-B loop for current track")
        end
    elseif action_id == "load_track" then
        local saved = tntron.storage.get("loops", {})
        local entry = (type(saved) == "table" and st.track ~= "") and saved[st.track] or nil
        if type(entry) == "table" then
            st.a = tonumber(entry.a)
            st.b = tonumber(entry.b)
            st.enabled = true
            tntron.ui.toast("Loaded saved A-B loop")
        else
            tntron.ui.toast("No saved loop for this track")
        end
    end
    render(pb)
end
