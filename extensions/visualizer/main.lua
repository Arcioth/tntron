-- Visualizer (Openify visualizer + live FFT spectrum telemetry)

local st = { mode = "bars", topbar = false }

local function render(pb)
    local playing = pb and pb.playing or false
    local vol = pb and pb.volume or 80
    local spd = pb and pb.speed or 1.0
    local bpm_txt = (pb and pb.bpm) and string.format("%d BPM", math.floor(pb.bpm + 0.5)) or "Live"

    if st.topbar then
        local peak = math.floor(((pb and pb.energy) or 0) * 100)
        tntron.ui.set_topbar_slot({
            text = playing and string.format("VIZ %s · %s (%d%%)", st.mode:upper(), bpm_txt, peak) or "Visualizer",
            icon = "wave",
            accent = playing,
            action_id = "cycle_mode",
        })
    else
        tntron.ui.clear_topbar_slot()
    end

    local master_pct = playing and math.min(100, math.floor(((pb and pb.energy) or 0.4) * 165)) or 0
    tntron.ui.register_sidebar({
        id = "visualizer-main",
        label = "Visualizer",
        icon = "wave",
        widgets = {
            tntron.ui.heading(pb and pb.title ~= "" and pb.title or "Audio Spectrum Visualizer"),
            tntron.ui.text(playing and ("Mode: " .. st.mode:upper() .. " · " .. bpm_txt) or "Idle (play audio to animate)", true),
            tntron.ui.visualizer(st.mode, { playing = playing, volume = vol, speed = spd }),
            tntron.ui.progress("Signal Energy", master_pct, 100),
            tntron.ui.row({
                tntron.ui.button("mode:bars",      "Bars",      { icon = "bars",  active = st.mode == "bars" }),
                tntron.ui.button("mode:wave",      "Wave",      { icon = "wave",  active = st.mode == "wave" }),
                tntron.ui.button("mode:circle",    "Circle",    { icon = "disc",  active = st.mode == "circle" }),
                tntron.ui.button("mode:particles", "Particles", { icon = "music", active = st.mode == "particles" }),
            }),
            tntron.ui.toggle("topbar_toggle", "Show live visualizer badge in top bar", st.topbar),
        },
    })
end

function on_load(_ctx)
    st.mode = tntron.storage.get("mode", "bars")
    st.topbar = (tntron.storage.get("topbar", false) == true)
    render(tntron.playback.get())
end

function on_tick(_ctx)
    render(tntron.playback.get())
end

function on_action(_ctx, action_id, val)
    if action_id:sub(1, 5) == "mode:" then
        st.mode = action_id:sub(6)
        tntron.storage.set("mode", st.mode)
    elseif action_id == "cycle_mode" then
        local order = { bars = "wave", wave = "circle", circle = "particles", particles = "bars" }
        st.mode = order[st.mode] or "bars"
        tntron.storage.set("mode", st.mode)
    elseif action_id == "topbar_toggle" then
        st.topbar = (val == true)
        tntron.storage.set("topbar", st.topbar)
    end
    render(tntron.playback.get())
end
