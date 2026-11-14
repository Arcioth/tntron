-- Song Details & Reactive Bindings Extension
-- Extracts BPM / Energy / Peak dBFS / Codec details and binds them to Spatial 8D Orbit or Daeboard RGB.

local st = {
    bind_spatial = false,
    spatial_div = 0.125, -- 1/8th of beat frequency
    bind_daeboard = false,
    manual_bpm = nil,
    tap_times = {},
    last_track = "",
    last_spatial_active = false,
    last_spatial_hz = 0,
    hw_ok = false,
}

local function active_bpm(pb)
    if st.manual_bpm then return st.manual_bpm end
    if pb and pb.bpm then return pb.bpm end
    return nil
end

local function apply_bindings(pb)
    if not pb then return end
    local bpm = active_bpm(pb)

    if st.bind_spatial and bpm and bpm > 20 then
        local effective_bpm = bpm * (pb.speed or 1.0)
        local orbit_hz = math.floor(math.max(0.05, math.min(1.5, (effective_bpm / 60.0) * st.spatial_div)) * 100 + 0.5) / 100
        if not st.last_spatial_active or math.abs(orbit_hz - st.last_spatial_hz) >= 0.01 then
            st.last_spatial_active = true
            st.last_spatial_hz = orbit_hz
            tntron.audio.set_spatial(true, orbit_hz)
        end
    end

    if st.bind_daeboard and st.hw_ok and pb.playing then
        local energy = tonumber(pb.energy) or 0.0
        local beat = tonumber(pb.beat_pulse) or 0.0
        local hex = "14b8a6"
        if bpm then
            if bpm >= 134 then
                hex = "ef4444"
            elseif bpm >= 105 then
                hex = "a855f7"
            end
        end
        local bri = 1
        if beat > 0.30 or energy > 0.45 then
            bri = 3
        elseif beat > 0.10 or energy > 0.20 then
            bri = 2
        end
        tntron.hw.daeboard_color(hex)
        tntron.hw.daeboard_brightness(bri)
    end
end

local function render(ctx)
    local pb = tntron.playback.get()
    local bpm = active_bpm(pb)
    local energy_pct = pb and math.min(100, math.floor((tonumber(pb.energy) or 0) * 180)) or 0
    local peak_str = pb and pb.peak_db and pb.peak_db > -90 and string.format("%.1f dBFS", pb.peak_db) or "—"
    local bpm_str = bpm and string.format("%.1f BPM", bpm) or "Analyzing..."
    local beat_ms = (bpm and bpm > 10) and string.format("%d ms", math.floor(60000.0 / bpm + 0.5)) or "—"
    local orbit_preview = (bpm and bpm > 10)
        and string.format("%.2f Hz", math.max(0.05, math.min(1.5, (bpm * ((pb and pb.speed) or 1.0) / 60.0) * st.spatial_div)))
        or "—"

    tntron.ui.clear_player_widget()

    local codec_str = (pb and pb.codec) and pb.codec:upper() or "—"
    local br_str = (pb and pb.bitrate_kbps) and (tostring(pb.bitrate_kbps) .. " kbps") or "—"
    local sr_str = (pb and pb.sample_rate_hz) and string.format("%.1f kHz", pb.sample_rate_hz / 1000.0) or "—"
    local ch_str = (pb and pb.channels) and (tostring(pb.channels) .. "ch") or "—"
    local lyric_str = (pb and pb.lyric and pb.lyric ~= "") and pb.lyric or "—"

    tntron.ui.register_sidebar({
        id = "song-details",
        label = "BPM & Info",
        icon = "gauge",
        widgets = {
            tntron.ui.heading((pb and pb.title ~= "") and pb.title or "No Track Playing"),
            tntron.ui.text((pb and pb.artist ~= "") and (pb.artist .. " · " .. (pb.album or "")) or "Play any track to inspect acoustic telemetry.", true),
            tntron.ui.row({
                tntron.ui.badge("Tempo: " .. bpm_str, { icon = "clock", accent = bpm ~= nil }),
                tntron.ui.badge("Beat: " .. beat_ms, {}),
                tntron.ui.badge("Peak: " .. peak_str, {}),
            }),
            tntron.ui.progress("Track Energy", energy_pct, 100),
            tntron.ui.text(string.format("Stream: %s  |  %s  |  %s  |  %s", codec_str, br_str, sr_str, ch_str), true),
            tntron.ui.text("Current Lyric: " .. lyric_str, true),
            tntron.ui.heading("Tap Tempo / BPM Override"),
            tntron.ui.row({
                tntron.ui.button("tap_bpm", "Tap Beat", { icon = "play", accent = true }),
                tntron.ui.button("clear_bpm", "Auto BPM", { active = st.manual_bpm == nil }),
            }),
            tntron.ui.heading("Reactive Output Bindings"),
            tntron.ui.toggle("bind_spatial", "Bind BPM -> 8D Spatial Orbit (" .. orbit_preview .. ")", st.bind_spatial),
            tntron.ui.row({
                tntron.ui.button("div:0.0625", "1/16 Beat", { active = math.abs(st.spatial_div - 0.0625) < 0.005 }),
                tntron.ui.button("div:0.125",  "1/8 Beat",  { active = math.abs(st.spatial_div - 0.125) < 0.005 }),
                tntron.ui.button("div:0.25",   "1/4 Beat",  { active = math.abs(st.spatial_div - 0.25) < 0.005 }),
            }),
            tntron.ui.toggle("bind_daeboard", "Bind Energy & BPM -> Daeboard RGB", st.bind_daeboard),
        },
    })
end

function on_load(ctx)
    st.bind_spatial = (tntron.storage.get("bind_spatial", false) == true)
    st.bind_daeboard = (tntron.storage.get("bind_daeboard", false) == true)
    st.spatial_div = tonumber(tntron.storage.get("spatial_div", 0.125)) or 0.125
    local ping = tntron.hw.daeboard_ping()
    st.hw_ok = (ping and ping.connected == true)
    local pb = tntron.playback.get()
    if pb then st.last_track = pb.track_path or "" end
    apply_bindings(pb)
    render(ctx)
end

function on_tick(ctx)
    local pb = tntron.playback.get()
    apply_bindings(pb)
    render(ctx)
end

function on_event(ctx, event_name)
    if event_name == "trackChange" then
        local pb = tntron.playback.get()
        local tp = pb and pb.track_path or ""
        if tp ~= st.last_track then
            st.last_track = tp
            st.manual_bpm = nil
            st.tap_times = {}
        end
        apply_bindings(pb)
        render(ctx)
    end
end

function on_action(ctx, action_id, val)
    local pb = tntron.playback.get()
    if action_id == "tap_bpm" then
        local now = (ctx and ctx.now_sec) or 0
        if #st.tap_times > 0 and (now - st.tap_times[#st.tap_times]) > 3 then
            st.tap_times = {}
        end
        table.insert(st.tap_times, now)
        if #st.tap_times >= 2 then
            local span = st.tap_times[#st.tap_times] - st.tap_times[1]
            if span > 0.2 then
                local est = ((#st.tap_times - 1) / span) * 60.0
                if est >= 40 and est <= 240 then
                    st.manual_bpm = math.floor(est + 0.5)
                    tntron.ui.toast(string.format("Tap Tempo: %d BPM", st.manual_bpm))
                end
            end
        end
    elseif action_id == "clear_bpm" then
        st.manual_bpm = nil
        st.tap_times = {}
    elseif action_id == "bind_spatial" then
        st.bind_spatial = (val == true)
        tntron.storage.set("bind_spatial", st.bind_spatial)
        if not st.bind_spatial then
            tntron.audio.set_spatial(false, 0.18)
        end
    elseif action_id:sub(1, 4) == "div:" then
        st.spatial_div = tonumber(action_id:sub(5)) or 0.125
        tntron.storage.set("spatial_div", st.spatial_div)
    elseif action_id == "bind_daeboard" then
        st.bind_daeboard = (val == true)
        tntron.storage.set("bind_daeboard", st.bind_daeboard)
        local ping = tntron.hw.daeboard_ping()
        st.hw_ok = (ping and ping.connected == true)
        if st.bind_daeboard and not st.hw_ok then
            tntron.ui.toast("Daeboard socket not active")
        end
    end
    apply_bindings(pb)
    render(ctx)
end
