-- Discord Rich Presence Customizer (syncs with native Settings -> Discord Rich Presence & .tnt)

local DEFAULTS = {
    enabled = true,
    client_id = "1306739184569253928",
    activity_type = 2,
    show_title = true,
    show_artist = true,
    show_album = true,
    show_timestamps = true,
    timestamp_mode = "remaining",
    show_paused = true,
    clear_on_pause = false,
    show_codec = true,
    show_bpm = true,
    show_speed_pitch = true,
    show_loop_shuffle = true,
    show_queue_info = true,
    show_idle = true,
    idle_text = "Browsing library · Idle",
    details_mode = "auto",
    state_mode = "auto",
    custom_details = "{title} · {bpm}",
    custom_state = "{artist} · {album} · {codec}",
    large_image_key = "tntron_logo",
    large_text = "{album} · {codec}",
    small_image_key = "tntron_play",
    small_text = "{state} · {speed}",
    show_button = true,
    button_label = "tntron on GitHub",
    button_url = "https://github.com/Arcioth/tntron",
    privacy_mode = false,
}

local st = { cfg = {} }

local function copy_cfg(src)
    local out = {}
    for k, v in pairs(DEFAULTS) do
        if type(src) == "table" and src[k] ~= nil then
            out[k] = src[k]
        else
            out[k] = v
        end
    end
    return out
end

local function push_cfg()
    tntron.storage.set("discord_rpc_cfg", st.cfg)
    tntron.ui.set_discord_rpc_config(st.cfg)
end

local function render()
    local widgets = {
        tntron.ui.heading("Discord Rich Presence Controls"),
        tntron.ui.text(
            "Configure every field sent over Discord/Vesktop IPC (discord-ipc-0..9). Also editable natively in Settings -> Discord Rich Presence.",
            true
        ),
        tntron.ui.row({
            tntron.ui.toggle("enabled", "Enable Discord Rich Presence", st.cfg.enabled),
            tntron.ui.toggle("privacy_mode", "Privacy Mode", st.cfg.privacy_mode),
            tntron.ui.button("reset", "Reset Defaults", { icon = "refresh" }),
        }),
        tntron.ui.row({
            tntron.ui.button("type:2", "Listening", { active = st.cfg.activity_type == 2 }),
            tntron.ui.button("type:0", "Playing",   { active = st.cfg.activity_type == 0 }),
            tntron.ui.button("type:3", "Watching",  { active = st.cfg.activity_type == 3 }),
            tntron.ui.button("tm:remaining", "Remaining Timer", { active = st.cfg.timestamp_mode == "remaining" }),
            tntron.ui.button("tm:elapsed",   "Elapsed Timer",   { active = st.cfg.timestamp_mode == "elapsed" }),
            tntron.ui.button("tm:none",      "No Timer",        { active = st.cfg.timestamp_mode == "none" }),
        }),
        tntron.ui.heading("Visible Details"),
        tntron.ui.row({
            tntron.ui.toggle("show_title",        "Title",         st.cfg.show_title),
            tntron.ui.toggle("show_artist",       "Artist",        st.cfg.show_artist),
            tntron.ui.toggle("show_album",        "Album",         st.cfg.show_album),
            tntron.ui.toggle("show_timestamps",   "Timestamps",    st.cfg.show_timestamps),
            tntron.ui.toggle("show_paused",       "Paused Badge",  st.cfg.show_paused),
            tntron.ui.toggle("clear_on_pause",    "Clear on Pause",st.cfg.clear_on_pause),
            tntron.ui.toggle("show_codec",        "Codec / Rate",  st.cfg.show_codec),
            tntron.ui.toggle("show_bpm",          "BPM",           st.cfg.show_bpm),
            tntron.ui.toggle("show_speed_pitch",  "Speed / Pitch", st.cfg.show_speed_pitch),
            tntron.ui.toggle("show_loop_shuffle", "Loop / Shuffle",st.cfg.show_loop_shuffle),
            tntron.ui.toggle("show_queue_info",   "Queue / Track", st.cfg.show_queue_info),
            tntron.ui.toggle("show_idle",         "Idle Status",   st.cfg.show_idle),
            tntron.ui.toggle("show_button",       "Action Button", st.cfg.show_button),
        }),
        tntron.ui.input("client_id", "Discord App Client ID", st.cfg.client_id, {
            placeholder = "1306739184569253928",
            submit_action = "set_client_id",
            submit_label = "Apply",
        }),
        tntron.ui.input("custom_details", "Custom Details Template", st.cfg.custom_details, {
            placeholder = "{title} · {bpm}",
            submit_action = "set_custom_details",
            submit_label = "Use Custom",
        }),
        tntron.ui.input("custom_state", "Custom State Template", st.cfg.custom_state, {
            placeholder = "{artist} · {album} · {codec}",
            submit_action = "set_custom_state",
            submit_label = "Use Custom",
        }),
        tntron.ui.input("idle_text", "Idle Status Text", st.cfg.idle_text, {
            placeholder = "Browsing library · Idle",
            submit_action = "set_idle_text",
            submit_label = "Save",
        }),
    }

    tntron.ui.register_settings({
        id = "discord-rpc",
        title = "Discord Rich Presence",
        widgets = widgets,
    })
    tntron.ui.register_sidebar({
        id = "discord-rpc",
        label = "Discord",
        icon = "globe",
        widgets = widgets,
    })
end

function on_load(_ctx)
    st.cfg = copy_cfg(tntron.storage.get("discord_rpc_cfg", nil))
    push_cfg()
    render()
end

function on_unload(_ctx)
    tntron.ui.unregister_sidebar("discord-rpc")
end

function on_action(_ctx, action_id, val)
    local bool_keys = {
        enabled = true, privacy_mode = true, show_title = true, show_artist = true,
        show_album = true, show_timestamps = true, show_paused = true, clear_on_pause = true,
        show_codec = true, show_bpm = true, show_speed_pitch = true, show_loop_shuffle = true,
        show_queue_info = true, show_idle = true, show_button = true,
    }
    if bool_keys[action_id] then
        st.cfg[action_id] = (val == true)
    elseif action_id == "reset" then
        st.cfg = copy_cfg(nil)
        tntron.ui.toast("Discord RPC defaults restored")
    elseif action_id == "set_client_id" and type(val) == "string" and val ~= "" then
        st.cfg.client_id = val
        tntron.ui.toast("Updated Discord Client ID")
    elseif action_id == "set_custom_details" and type(val) == "string" then
        st.cfg.custom_details = val
        st.cfg.details_mode = "custom"
    elseif action_id == "set_custom_state" and type(val) == "string" then
        st.cfg.custom_state = val
        st.cfg.state_mode = "custom"
    elseif action_id == "set_idle_text" and type(val) == "string" then
        st.cfg.idle_text = val
    else
        local t = action_id:match("^type:(%d+)$")
        local tm = action_id:match("^tm:(.+)$")
        if t then st.cfg.activity_type = tonumber(t) or 2 end
        if tm then st.cfg.timestamp_mode = tm end
    end
    push_cfg()
    render()
end
