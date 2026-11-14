-- Better Topbar (Openify better-topbar + Tunotron better_topbar.lua)

local st = { show_codec = true, show_count = true, show_played = true, played = 0 }

local function render(pb)
    tntron.ui.clear_topbar_slot()

    local codec = (pb and pb.codec and pb.codec ~= "") and pb.codec:upper() or "IDLE"
    local sr = (pb and pb.sample_rate and pb.sample_rate > 0) and (tostring(pb.sample_rate) .. " Hz") or "—"
    local ch = (pb and pb.channels and pb.channels > 0) and (tostring(pb.channels) .. "ch") or "—"
    local all = tntron.library.all_tracks() or {}
    local q = (tntron.queue and tntron.queue.list and tntron.queue.list()) or {}
    local summary = string.format(
        "Audio: %s · %s · %s   |   Session Played: %d   |   Library: %d tracks · Queue: %d",
        codec, sr, ch, st.played, #all, #q
    )

    tntron.ui.register_settings({
        id = "better-topbar",
        title = "Audio Stream & Session Telemetry",
        widgets = {
            tntron.ui.text(summary, false),
            tntron.ui.toggle("codec", "Track audio codec readout (AAC/FLAC/OPUS/MP3)", st.show_codec),
            tntron.ui.toggle("played", "Track session plays counter", st.show_played),
            tntron.ui.toggle("count", "Track loaded library & queue count", st.show_count),
        },
    })
end

function on_load(_ctx)
    st.show_codec = (tntron.storage.get("codec", true) ~= false)
    st.show_played = (tntron.storage.get("played", true) ~= false)
    st.show_count = (tntron.storage.get("count", true) ~= false)
    st.played = tonumber(tntron.storage.get("played_count", 0)) or 0
    render(tntron.playback.get())
end

function on_event(_ctx, event_name)
    if event_name == "trackChange" then
        st.played = st.played + 1
        tntron.storage.set("played_count", st.played)
    end
    render(tntron.playback.get())
end

function on_tick(_ctx)
    render(tntron.playback.get())
end

function on_action(_ctx, action_id, val)
    if action_id == "codec" then
        st.show_codec = (val == true)
        tntron.storage.set("codec", st.show_codec)
    elseif action_id == "played" then
        st.show_played = (val == true)
        tntron.storage.set("played", st.show_played)
    elseif action_id == "count" then
        st.show_count = (val == true)
        tntron.storage.set("count", st.show_count)
    end
    render(tntron.playback.get())
end
