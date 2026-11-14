-- 3D / 8D Spatial Audio Studio (Precision Orbit Controls + Auto Track BPM Sync)

local st = {
    active = false,
    mode = "8d",
    hz = 0.20,
    sync_bpm = true,
    bpm_div = 0.125,
    amount = 0.85,
    width = 1.60,
    reverb = 0.00,
    last_bpm = nil,
    last_filter = "",
}

local function clamp(x, lo, hi)
    if x < lo then return lo end
    if x > hi then return hi end
    return math.floor(x * 100 + 0.5) / 100
end

local function lfo_for_mode(mode)
    if mode == "8d" then return "sine" end
    if mode == "7d" then return "triangle" end
    if mode == "pulse" then return "square" end
    if mode == "spiral" then return "sawup" end
    return nil
end

local function compute_bpm_hz(pb)
    local bpm = pb and tonumber(pb.bpm) or nil
    if not bpm or bpm < 20 then
        return nil, nil
    end
    local spd = (pb and tonumber(pb.speed)) or 1.0
    local hz = clamp(((bpm * spd) / 60.0) * st.bpm_div, 0.05, 2.50)
    return bpm, hz
end

local function build_filter()
    if not st.active then return "" end
    local stages = {}
    local lfo = lfo_for_mode(st.mode)
    if lfo then
        local pan_width = (st.mode == "7d") and 0.90 or 1.00
        table.insert(stages, string.format(
            "apulsator=mode=%s:hz=%.2f:amount=%.2f:width=%.2f:timing=hz",
            lfo, st.hz, st.amount, pan_width
        ))
    end
    local echo_wet = (st.mode == "hall") and math.max(0.25, st.reverb) or st.reverb
    if echo_wet >= 0.05 then
        table.insert(stages, string.format("aecho=0.80:0.88:48:%.2f", echo_wet))
    end
    table.insert(stages, string.format("extrastereo=m=%.2f", st.width))
    return table.concat(stages, ",")
end

local function orbit_period_label(bpm)
    local sec = 1.0 / math.max(0.01, st.hz)
    local base = string.format("%.2f Hz  |  1 full orbit every %.2f s", st.hz, sec)
    if bpm and bpm >= 20 then
        return string.format("%s  |  Track Tempo: %.1f BPM", base, bpm)
    end
    return base .. "  |  Track Tempo: Analyzing..."
end

local function sync_audio_and_ui(force_filter)
    local pb = tntron.playback.get()
    local bpm, synced_hz = compute_bpm_hz(pb)
    st.last_bpm = bpm
    if st.sync_bpm and synced_hz then
        st.hz = synced_hz
    end

    local af = build_filter()
    if force_filter or af ~= st.last_filter then
        st.last_filter = af
        if af == "" then
            tntron.audio.clear_filter()
            tntron.ui.clear_player_widget()
        else
            tntron.audio.set_filter(af)
            local badge = (st.mode == "hall")
                and "HALL"
                or string.format("%s %.2fHz", st.mode:upper(), st.hz)
            tntron.ui.set_player_widget({
                text = badge,
                icon = "headphones",
                accent = true,
                action_id = "quick_toggle",
            })
        end
    end

    tntron.ui.register_sidebar({
        id = "spatial-studio",
        label = "Spatial Audio",
        icon = "headphones",
        widgets = {
            tntron.ui.toggle("spatial_on", "Enable Spatial DSP", st.active),
            tntron.ui.toggle("sync_bpm", "Sync Orbit Speed to Track BPM", st.sync_bpm),
            tntron.ui.row({
                tntron.ui.button("div:0.0625", "1/16 Beat", { active = math.abs(st.bpm_div - 0.0625) < 0.005 }),
                tntron.ui.button("div:0.125",  "1/8 Beat",  { active = math.abs(st.bpm_div - 0.125) < 0.005 }),
                tntron.ui.button("div:0.25",   "1/4 Beat",  { active = math.abs(st.bpm_div - 0.25) < 0.005 }),
                tntron.ui.button("div:0.5",    "1/2 Beat",  { active = math.abs(st.bpm_div - 0.5) < 0.005 }),
                tntron.ui.button("div:1.0",    "1 Beat",    { active = math.abs(st.bpm_div - 1.0) < 0.005 }),
            }),
            tntron.ui.row({
                tntron.ui.button("mode:8d",     "8D Sine",  { active = st.mode == "8d", icon = "headphones" }),
                tntron.ui.button("mode:7d",     "7D Tri",   { active = st.mode == "7d" }),
                tntron.ui.button("mode:pulse",  "Pulse",    { active = st.mode == "pulse" }),
                tntron.ui.button("mode:spiral", "Spiral",   { active = st.mode == "spiral" }),
                tntron.ui.button("mode:hall",   "Hall",     { active = st.mode == "hall" }),
            }),
            tntron.ui.slider("hz", "Orbit Speed", 0.01, 2.50, 0.01, st.hz, "Hz"),
            tntron.ui.text(orbit_period_label(bpm), true),
            tntron.ui.row({
                tntron.ui.button("nudge:-0.05", "-0.05", {}),
                tntron.ui.button("nudge:-0.01", "-0.01", {}),
                tntron.ui.button("set_hz:0.20", "0.20Hz", { active = (not st.sync_bpm) and math.abs(st.hz - 0.20) < 0.005 }),
                tntron.ui.button("nudge:0.01",  "+0.01", {}),
                tntron.ui.button("nudge:0.05",  "+0.05", {}),
            }),
            tntron.ui.slider("amount", "Orbit Depth",   0.10, 1.00, 0.05, st.amount, "x"),
            tntron.ui.slider("width",  "Stereo Width",  1.00, 3.00, 0.05, st.width,  "x"),
            tntron.ui.slider("reverb", "Hall Ambience", 0.00, 0.60, 0.05, st.reverb, ""),
            tntron.ui.text("When Sync to BPM is enabled, orbit frequency adapts automatically to track tempo and speed.", true),
        },
    })
end

function on_load(_ctx)
    local saved = tntron.storage.get("spatial_state", nil)
    if type(saved) == "table" then
        st.active = saved.active == true
        st.mode = saved.mode or "8d"
        st.hz = clamp(tonumber(saved.hz) or 0.20, 0.01, 2.50)
        if saved.sync_bpm ~= nil then
            st.sync_bpm = (saved.sync_bpm == true)
        end
        st.bpm_div = tonumber(saved.bpm_div) or 0.125
        st.amount = clamp(tonumber(saved.amount) or 0.85, 0.10, 1.00)
        st.width = clamp(tonumber(saved.width) or 1.60, 1.00, 3.00)
        st.reverb = clamp(tonumber(saved.reverb) or 0.00, 0.00, 0.60)
    end
    sync_audio_and_ui(true)
end

function on_tick(_ctx)
    local pb = tntron.playback.get()
    local bpm, synced_hz = compute_bpm_hz(pb)
    local bpm_changed = (bpm ~= st.last_bpm)
    local hz_changed = (st.sync_bpm and synced_hz and math.abs(synced_hz - st.hz) >= 0.01)
    if bpm_changed or hz_changed then
        sync_audio_and_ui(false)
    end
end

function on_event(_ctx, event_name)
    if event_name == "trackChange" then
        sync_audio_and_ui(false)
    end
end

function on_unload(_ctx)
    tntron.audio.clear_filter()
end

function on_action(_ctx, action_id, val)
    if action_id == "spatial_on" then
        st.active = (val == true)
    elseif action_id == "sync_bpm" then
        st.sync_bpm = (val == true)
    elseif action_id:sub(1, 4) == "div:" then
        st.bpm_div = tonumber(action_id:sub(5)) or 0.125
        st.sync_bpm = true
        st.active = true
    elseif action_id == "quick_toggle" then
        st.active = not st.active
        tntron.ui.toast(st.active and ("Spatial " .. st.mode:upper() .. " ON") or "Spatial OFF")
    elseif action_id:sub(1, 5) == "mode:" then
        st.mode = action_id:sub(6)
        st.active = true
    elseif action_id == "hz" then
        st.sync_bpm = false
        st.hz = clamp(tonumber(val) or st.hz, 0.01, 2.50)
    elseif action_id:sub(1, 6) == "nudge:" then
        st.sync_bpm = false
        local d = tonumber(action_id:sub(7)) or 0
        st.hz = clamp(st.hz + d, 0.01, 2.50)
        st.active = true
    elseif action_id:sub(1, 7) == "set_hz:" then
        st.sync_bpm = false
        st.hz = clamp(tonumber(action_id:sub(8)) or 0.20, 0.01, 2.50)
        st.active = true
    elseif action_id == "amount" then
        st.amount = clamp(tonumber(val) or st.amount, 0.10, 1.00)
    elseif action_id == "width" then
        st.width = clamp(tonumber(val) or st.width, 1.00, 3.00)
    elseif action_id == "reverb" then
        st.reverb = clamp(tonumber(val) or st.reverb, 0.00, 0.60)
    end
    tntron.storage.set("spatial_state", st)
    sync_audio_and_ui(true)
end
