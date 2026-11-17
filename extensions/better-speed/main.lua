-- Better Speed & Pitch Studio (Musicolet-style Tempo & Pitch Engine)

local st = {
    enabled = true,
    speed = 1.00,
    pitch_st = 0.0,
    link_pitch = false,
    preserve_formant = true,
    reverb = 0.00,
    preset_draft = "",
    custom_presets = {},
}

local function round_to(val, decimals)
    local mult = 10 ^ decimals
    return math.floor(val * mult + 0.5) / mult
end

local function clamp(val, lo, hi, decimals)
    if val < lo then val = lo end
    if val > hi then val = hi end
    return round_to(val, decimals)
end

local function log2(x)
    return math.log(x) / math.log(2)
end

local function speed_to_semitones(spd)
    return clamp(12.0 * log2(math.max(0.25, spd)), -12.0, 12.0, 1)
end

local function semitones_to_speed(semi)
    return clamp(2 ^ (semi / 12.0), 0.25, 3.00, 2)
end

local function effective_pitch_ratio()
    return 2 ^ (st.pitch_st / 12.0)
end

local function build_filter()
    if not st.enabled then return "" end
    local stages = {}
    -- Core engine natively handles Vinyl Link via audio-pitch-correction=no
    -- and independent pitch shifting via rubberband; extension adds reverb.
    if st.reverb >= 0.05 then
        table.insert(stages, string.format("aecho=0.80:0.88:55:%.2f", st.reverb))
    end
    return table.concat(stages, ",")
end

local function status_summary()
    if not st.enabled then
        return "Bypassed (playing at 1.00x / 0.0 st)"
    end
    local ratio = effective_pitch_ratio()
    local cents = math.floor(st.pitch_st * 100 + (st.pitch_st >= 0 and 0.5 or -0.5))
    local mode = st.link_pitch and "Vinyl Linked" or "Independent"
    return string.format(
        "%.2fx speed  |  %+.1f st (%+d cents, %.3fx)  |  %s",
        st.speed, st.pitch_st, cents, ratio, mode
    )
end

local function is_modified()
    return math.abs(st.speed - 1.00) >= 0.005
        or math.abs(st.pitch_st) >= 0.05
        or st.reverb >= 0.05
end

local function sync_audio_and_ui()
    local target_speed = st.enabled and st.speed or 1.00
    local target_pitch = st.enabled and st.pitch_st or 0.0
    local target_link = st.enabled and st.link_pitch or false
    tntron.audio.set_pitch(target_pitch, target_link, st.preserve_formant)
    tntron.playback.set_speed(target_speed)

    local af = build_filter()
    if af == "" then
        tntron.audio.clear_filter()
    else
        tntron.audio.set_filter(af)
    end

    tntron.ui.clear_player_widget()

    tntron.ui.register_sidebar({
        id = "better-speed",
        label = "Speed & Pitch",
        icon = "sliders",
        widgets = {
            tntron.ui.toggle("enable", "Enable Speed & Pitch", st.enabled),
            tntron.ui.toggle("link_pitch", "Link Speed & Pitch (Vinyl / Tape)", st.link_pitch),
            tntron.ui.toggle("preserve_formant", "Preserve Vocal Formants", st.preserve_formant),
            tntron.ui.text(status_summary(), true),

            tntron.ui.slider("speed", "Playback Speed", 0.25, 3.00, 0.01, st.speed, "x"),
            tntron.ui.row({
                tntron.ui.button("spd_nudge:-0.10", "-0.10", {}),
                tntron.ui.button("spd_nudge:-0.05", "-0.05", {}),
                tntron.ui.button("spd_nudge:-0.01", "-0.01", {}),
                tntron.ui.button("spd_set:1.00",    "1.00x", { active = math.abs(st.speed - 1.00) < 0.005 }),
                tntron.ui.button("spd_nudge:0.01",  "+0.01", {}),
                tntron.ui.button("spd_nudge:0.05",  "+0.05", {}),
                tntron.ui.button("spd_nudge:0.10",  "+0.10", {}),
            }),
            tntron.ui.row({
                tntron.ui.button("spd_set:0.50", "0.50x", { active = math.abs(st.speed - 0.50) < 0.005 }),
                tntron.ui.button("spd_set:0.75", "0.75x", { active = math.abs(st.speed - 0.75) < 0.005 }),
                tntron.ui.button("spd_set:1.25", "1.25x", { active = math.abs(st.speed - 1.25) < 0.005 }),
                tntron.ui.button("spd_set:1.50", "1.50x", { active = math.abs(st.speed - 1.50) < 0.005 }),
                tntron.ui.button("spd_set:2.00", "2.00x", { active = math.abs(st.speed - 2.00) < 0.005 }),
            }),

            tntron.ui.slider("pitch", "Pitch Shift (Semitones)", -12.0, 12.0, 0.1, st.pitch_st, "st"),
            tntron.ui.row({
                tntron.ui.button("pitch_nudge:-2.0", "-2st",   {}),
                tntron.ui.button("pitch_nudge:-1.0", "-1st",   {}),
                tntron.ui.button("pitch_nudge:-0.5", "-0.5st", {}),
                tntron.ui.button("pitch_set:0.0",    "0st",    { active = math.abs(st.pitch_st) < 0.05 }),
                tntron.ui.button("pitch_nudge:0.5",  "+0.5st", {}),
                tntron.ui.button("pitch_nudge:1.0",  "+1st",   {}),
                tntron.ui.button("pitch_nudge:2.0",  "+2st",   {}),
            }),

            tntron.ui.slider("reverb", "Hall / Slowed Reverb", 0.00, 0.60, 0.05, st.reverb, ""),
            tntron.ui.row({
                tntron.ui.button("preset:normal",    "Normal",        { active = not is_modified() }),
                tntron.ui.button("preset:practice",  "Practice .75x", {}),
                tntron.ui.button("preset:nightcore", "Nightcore",     {}),
                tntron.ui.button("preset:daycore",   "Daycore",       {}),
                tntron.ui.button("preset:slowed",    "Slowed+Reverb", {}),
            }),
            tntron.ui.heading("Saved Speed Presets"),
            tntron.ui.input("preset_draft", "Save Current Settings", st.preset_draft, {
                placeholder = "Preset name (e.g. Vocal Practice)...",
                submit_action = "save_preset",
                submit_label = "Save Preset",
            }),
        },
    })

    if #st.custom_presets > 0 then
        local rows = {
            tntron.ui.toggle("enable", "Enable Speed & Pitch", st.enabled),
            tntron.ui.toggle("link_pitch", "Link Speed & Pitch (Vinyl / Tape)", st.link_pitch),
            tntron.ui.toggle("preserve_formant", "Preserve Vocal Formants", st.preserve_formant),
            tntron.ui.text(status_summary(), true),
            tntron.ui.slider("speed", "Playback Speed", 0.25, 3.00, 0.01, st.speed, "x"),
            tntron.ui.row({
                tntron.ui.button("spd_nudge:-0.10", "-0.10", {}),
                tntron.ui.button("spd_nudge:-0.05", "-0.05", {}),
                tntron.ui.button("spd_nudge:-0.01", "-0.01", {}),
                tntron.ui.button("spd_set:1.00",    "1.00x", { active = math.abs(st.speed - 1.00) < 0.005 }),
                tntron.ui.button("spd_nudge:0.01",  "+0.01", {}),
                tntron.ui.button("spd_nudge:0.05",  "+0.05", {}),
                tntron.ui.button("spd_nudge:0.10",  "+0.10", {}),
            }),
            tntron.ui.row({
                tntron.ui.button("spd_set:0.50", "0.50x", { active = math.abs(st.speed - 0.50) < 0.005 }),
                tntron.ui.button("spd_set:0.75", "0.75x", { active = math.abs(st.speed - 0.75) < 0.005 }),
                tntron.ui.button("spd_set:1.25", "1.25x", { active = math.abs(st.speed - 1.25) < 0.005 }),
                tntron.ui.button("spd_set:1.50", "1.50x", { active = math.abs(st.speed - 1.50) < 0.005 }),
                tntron.ui.button("spd_set:2.00", "2.00x", { active = math.abs(st.speed - 2.00) < 0.005 }),
            }),
            tntron.ui.slider("pitch", "Pitch Shift (Semitones)", -12.0, 12.0, 0.1, st.pitch_st, "st"),
            tntron.ui.row({
                tntron.ui.button("pitch_nudge:-2.0", "-2st",   {}),
                tntron.ui.button("pitch_nudge:-1.0", "-1st",   {}),
                tntron.ui.button("pitch_nudge:-0.5", "-0.5st", {}),
                tntron.ui.button("pitch_set:0.0",    "0st",    { active = math.abs(st.pitch_st) < 0.05 }),
                tntron.ui.button("pitch_nudge:0.5",  "+0.5st", {}),
                tntron.ui.button("pitch_nudge:1.0",  "+1st",   {}),
                tntron.ui.button("pitch_nudge:2.0",  "+2st",   {}),
            }),
            tntron.ui.slider("reverb", "Hall / Slowed Reverb", 0.00, 0.60, 0.05, st.reverb, ""),
            tntron.ui.row({
                tntron.ui.button("preset:normal",    "Normal",        { active = not is_modified() }),
                tntron.ui.button("preset:practice",  "Practice .75x", {}),
                tntron.ui.button("preset:nightcore", "Nightcore",     {}),
                tntron.ui.button("preset:daycore",   "Daycore",       {}),
                tntron.ui.button("preset:slowed",    "Slowed+Reverb", {}),
            }),
            tntron.ui.heading("Saved Speed Presets"),
            tntron.ui.input("preset_draft", "Save Current Settings", st.preset_draft, {
                placeholder = "Preset name (e.g. Vocal Practice)...",
                submit_action = "save_preset",
                submit_label = "Save Preset",
            }),
        }
        for i, p in ipairs(st.custom_presets) do
            table.insert(rows, tntron.ui.row({
                tntron.ui.button("cpreset:" .. tostring(i), string.format("%s (%.2fx / %+.1fst)", p.name, p.speed, p.pitch_st), { icon = "play" }),
                tntron.ui.button("cdel:" .. tostring(i), "Delete", { icon = "close" }),
            }))
        end
        tntron.ui.register_sidebar({
            id = "better-speed",
            label = "Speed & Pitch",
            icon = "sliders",
            widgets = rows,
        })
    end
end

function on_load(_ctx)
    local saved = tntron.storage.get("speed_state", nil)
    if type(saved) == "table" then
        st.enabled = (saved.enabled ~= false)
        st.speed = clamp(tonumber(saved.speed) or 1.00, 0.25, 3.00, 2)
        st.pitch_st = clamp(tonumber(saved.pitch_st) or 0.0, -12.0, 12.0, 1)
        st.link_pitch = (saved.link_pitch == true)
        st.preserve_formant = (saved.preserve_formant ~= false)
        st.reverb = clamp(tonumber(saved.reverb) or 0.00, 0.00, 0.60, 2)
        if type(saved.custom_presets) == "table" then
            st.custom_presets = saved.custom_presets
        end
    end
    sync_audio_and_ui()
end

function on_unload(_ctx)
    tntron.audio.clear_filter()
    tntron.playback.set_speed(1.00)
end

local function apply_speed(new_speed)
    st.speed = clamp(new_speed, 0.25, 3.00, 2)
    st.enabled = true
    if st.link_pitch then
        st.pitch_st = speed_to_semitones(st.speed)
    end
end

local function apply_pitch(new_pitch)
    st.pitch_st = clamp(new_pitch, -12.0, 12.0, 1)
    st.enabled = true
    if st.link_pitch then
        st.speed = semitones_to_speed(st.pitch_st)
    end
end

function on_action(_ctx, action_id, val)
    if action_id == "enable" then
        st.enabled = (val == true)
    elseif action_id == "quick_toggle" then
        st.enabled = not st.enabled
        tntron.ui.toast(st.enabled and status_summary() or "Speed & Pitch bypassed")
    elseif action_id == "link_pitch" then
        st.link_pitch = (val == true)
        if st.link_pitch then
            st.pitch_st = speed_to_semitones(st.speed)
        end
    elseif action_id == "preserve_formant" then
        st.preserve_formant = (val == true)
    elseif action_id == "speed" then
        apply_speed(tonumber(val) or st.speed)
    elseif action_id:sub(1, 10) == "spd_nudge:" then
        apply_speed(st.speed + (tonumber(action_id:sub(11)) or 0))
    elseif action_id:sub(1, 8) == "spd_set:" then
        apply_speed(tonumber(action_id:sub(9)) or 1.00)
    elseif action_id == "pitch" then
        apply_pitch(tonumber(val) or st.pitch_st)
    elseif action_id:sub(1, 12) == "pitch_nudge:" then
        apply_pitch(st.pitch_st + (tonumber(action_id:sub(13)) or 0))
    elseif action_id:sub(1, 10) == "pitch_set:" then
        apply_pitch(tonumber(action_id:sub(11)) or 0.0)
    elseif action_id == "reverb" then
        st.reverb = clamp(tonumber(val) or st.reverb, 0.00, 0.60, 2)
        st.enabled = true
    elseif action_id == "preset:normal" then
        st.enabled = true
        st.speed = 1.00
        st.pitch_st = 0.0
        st.link_pitch = false
        st.preserve_formant = true
        st.reverb = 0.00
    elseif action_id == "preset:practice" then
        st.enabled = true
        st.speed = 0.75
        st.pitch_st = 0.0
        st.link_pitch = false
        st.preserve_formant = true
        st.reverb = 0.00
    elseif action_id == "preset:nightcore" then
        st.enabled = true
        st.speed = 1.25
        st.pitch_st = speed_to_semitones(1.25)
        st.link_pitch = true
        st.preserve_formant = false
        st.reverb = 0.00
    elseif action_id == "preset:daycore" then
        st.enabled = true
        st.speed = 0.82
        st.pitch_st = speed_to_semitones(0.82)
        st.link_pitch = true
        st.preserve_formant = true
        st.reverb = 0.15
    elseif action_id == "preset:slowed" then
        st.enabled = true
        st.speed = 0.85
        st.pitch_st = speed_to_semitones(0.85)
        st.link_pitch = true
        st.preserve_formant = true
        st.reverb = 0.35
    elseif action_id == "save_preset" then
        local name = type(val) == "string" and val:match("^%s*(.-)%s*$") or ""
        if name ~= "" then
            local next_list = {}
            for _, old in ipairs(st.custom_presets) do
                if old.name ~= name then table.insert(next_list, old) end
            end
            table.insert(next_list, {
                name = name,
                speed = st.speed,
                pitch_st = st.pitch_st,
                link_pitch = st.link_pitch,
                preserve_formant = st.preserve_formant,
                reverb = st.reverb,
            })
            st.custom_presets = next_list
            st.preset_draft = ""
            tntron.ui.toast("Saved preset: " .. name)
        end
    elseif action_id:sub(1, 8) == "cpreset:" then
        local idx = tonumber(action_id:sub(9))
        local p = idx and st.custom_presets[idx]
        if p then
            st.enabled = true
            st.speed = clamp(tonumber(p.speed) or 1.0, 0.25, 3.00, 2)
            st.pitch_st = clamp(tonumber(p.pitch_st) or 0.0, -12.0, 12.0, 1)
            st.link_pitch = (p.link_pitch == true)
            st.preserve_formant = (p.preserve_formant ~= false)
            st.reverb = clamp(tonumber(p.reverb) or 0.0, 0.0, 0.60, 2)
            tntron.ui.toast("Preset: " .. p.name)
        end
    elseif action_id:sub(1, 5) == "cdel:" then
        local idx = tonumber(action_id:sub(6))
        if idx and st.custom_presets[idx] then
            local rm = st.custom_presets[idx].name
            table.remove(st.custom_presets, idx)
            tntron.ui.toast("Deleted preset: " .. rm)
        end
    end

    tntron.storage.set("speed_state", st)
    sync_audio_and_ui()
end
