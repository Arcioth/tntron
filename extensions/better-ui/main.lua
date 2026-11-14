-- Better UI (ported from Openify better-ui)

local st = { player_pill = true }

local function render(pb)
    if st.player_pill and pb and pb.title ~= "" then
        local codec = (pb.codec and pb.codec ~= "") and pb.codec:upper() or "AUDIO"
        local state_lbl = pb.playing and "LIVE" or (pb.paused and "PAUSED" or "STOP")
        tntron.ui.set_player_widget({
            text = string.format("%s · %s", state_lbl, codec),
            icon = "music",
            accent = pb.playing,
        })
    else
        tntron.ui.clear_player_widget()
    end

    tntron.ui.register_settings({
        id = "better-ui",
        title = "Better UI Polish",
        widgets = {
            tntron.ui.toggle("pill_toggle", "Show live fidelity badge inside player bar", st.player_pill),
            tntron.ui.text("Provides shared UI polish capabilities for dependent extensions (e.g. Better Library).", true),
        },
    })
end

function on_load(_ctx)
    st.player_pill = (tntron.storage.get("player_pill", true) ~= false)
    render(tntron.playback.get())
end

function on_event(_ctx, _ev)
    render(tntron.playback.get())
end

function on_action(_ctx, action_id, val)
    if action_id == "pill_toggle" then
        st.player_pill = (val == true)
        tntron.storage.set("player_pill", st.player_pill)
    end
    render(tntron.playback.get())
end
