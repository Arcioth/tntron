-- Better Visualizer (Openify 9 animated audio-reactive vector scenes)

local st = { scene = "flame", player_bar = true }

local SCENES = {
    { id = "flame",     label = "Flame" },
    { id = "snow",      label = "Snow" },
    { id = "runner",    label = "Runner" },
    { id = "boulder",   label = "Boulder" },
    { id = "bicycle",   label = "Bicycle" },
    { id = "bars",      label = "Bars" },
    { id = "wave",      label = "Wave" },
    { id = "circle",    label = "Circle" },
    { id = "particles", label = "Particles" },
}

local function sync_mini_viz()
    local mini_mode = "bars"
    if st.scene == "wave" or st.scene == "circle" or st.scene == "flame" then
        mini_mode = st.scene
    end
    tntron.ui.set_layout_config({
        show_player_viz = st.player_bar,
        player_viz_mode = mini_mode,
    })
end

local function render(pb)
    local playing = pb and pb.playing or false
    local vol = pb and pb.volume or 80
    local spd = pb and pb.speed or 1.0
    local bpm = pb and pb.bpm

    if bpm and playing then
        tntron.ui.set_player_widget({
            text = string.format("%d BPM", math.floor(bpm + 0.5)),
            icon = "wave",
            accent = true,
            action_id = "cycle_scene",
        })
    else
        tntron.ui.clear_player_widget()
    end

    local btns = {}
    for _, sc in ipairs(SCENES) do
        table.insert(btns, tntron.ui.button("sc:" .. sc.id, sc.label, { active = st.scene == sc.id }))
    end

    tntron.ui.register_sidebar({
        id = "better-viz",
        label = "Visualizer+",
        icon = "wave",
        widgets = {
            tntron.ui.heading("Stage Scene: " .. st.scene:upper()),
            tntron.ui.text(pb and pb.title ~= "" and (pb.title .. " · " .. (pb.artist or "")) or "Play any track to drive the FFT-reactive canvas.", true),
            tntron.ui.visualizer(st.scene, { playing = playing, volume = vol, speed = spd }),
            tntron.ui.row(btns),
            tntron.ui.toggle("mini_toggle", "Show real-time canvas visualizer in player bar", st.player_bar),
        },
    })
end

function on_load(_ctx)
    st.scene = tntron.storage.get("scene", "flame")
    st.player_bar = (tntron.storage.get("player_bar", true) ~= false)
    sync_mini_viz()
    render(tntron.playback.get())
end

function on_tick(_ctx)
    render(tntron.playback.get())
end

function on_action(_ctx, action_id, val)
    if action_id:sub(1, 3) == "sc:" then
        st.scene = action_id:sub(4)
        tntron.storage.set("scene", st.scene)
        sync_mini_viz()
    elseif action_id == "cycle_scene" then
        local order = {
            flame = "snow",
            snow = "runner",
            runner = "boulder",
            boulder = "bicycle",
            bicycle = "bars",
            bars = "wave",
            wave = "circle",
            circle = "particles",
            particles = "flame",
        }
        st.scene = order[st.scene] or "flame"
        tntron.storage.set("scene", st.scene)
        sync_mini_viz()
    elseif action_id == "mini_toggle" then
        st.player_bar = (val == true)
        tntron.storage.set("player_bar", st.player_bar)
        sync_mini_viz()
    end
    render(tntron.playback.get())
end
