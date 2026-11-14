-- 10-Band Studio Equalizer (ported from Tunotron equalizer.lua)

local BANDS = {
    { key = "b60",  freq = 60,    label = "60 Hz (Sub)" },
    { key = "b170", freq = 170,   label = "170 Hz (Bass)" },
    { key = "b310", freq = 310,   label = "310 Hz (Warm)" },
    { key = "b600", freq = 600,   label = "600 Hz (Body)" },
    { key = "b1k",  freq = 1000,  label = "1 kHz (Mid)" },
    { key = "b3k",  freq = 3000,  label = "3 kHz (Vocal)" },
    { key = "b6k",  freq = 6000,  label = "6 kHz (Pres)" },
    { key = "b12k", freq = 12000, label = "12 kHz (Bril)" },
    { key = "b14k", freq = 14000, label = "14 kHz (Air)" },
    { key = "b16k", freq = 16000, label = "16 kHz (Sparkle)" },
}

local PRESETS = {
    { id = "flat",       label = "Flat",          gains = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 } },
    { id = "bass",       label = "Bass Boost",    gains = { 6, 5, 3, 1, 0, 0, 1, 2, 2, 1 } },
    { id = "vocal",      label = "Vocal Clarity", gains = { -2, -1, 0, 2, 3, 5, 4, 2, 1, 0 } },
    { id = "rock",       label = "Rock",          gains = { 4, 3, 1, -1, -1, 1, 3, 4, 3, 2 } },
    { id = "electronic", label = "Electronic",    gains = { 5, 4, 2, 0, -2, 1, 3, 5, 4, 3 } },
    { id = "jazz",       label = "Jazz",          gains = { 3, 2, 1, 2, -1, 0, 2, 3, 2, 2 } },
}

local state = { enabled = true, preset = "flat", gains = { 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 } }

local function build_filter()
    if not state.enabled then return "" end
    local parts = {}
    for i, b in ipairs(BANDS) do
        local g = tonumber(state.gains[i]) or 0
        if math.abs(g) > 0.05 then
            table.insert(parts, string.format("equalizer=f=%d:t=o:w=1.0:g=%.1f", b.freq, g))
        end
    end
    return table.concat(parts, ",")
end

local function apply_audio()
    local af = build_filter()
    if af == "" then
        tntron.audio.clear_filter()
    else
        tntron.audio.set_filter(af)
    end
end

local function save()
    tntron.storage.set("eq_state", state)
end

local function render()
    local preset_btns = {}
    for _, p in ipairs(PRESETS) do
        table.insert(preset_btns, tntron.ui.button("preset:" .. p.id, p.label, {
            active = (state.preset == p.id),
        }))
    end

    local sidebar_widgets = {
        tntron.ui.toggle("eq_enable", "Equalizer Enabled", state.enabled),
        tntron.ui.heading("Sound Coloration Presets"),
        tntron.ui.row(preset_btns),
        tntron.ui.heading("10-Band Frequency Matrix"),
    }
    for i, b in ipairs(BANDS) do
        table.insert(sidebar_widgets, tntron.ui.slider(
            "band:" .. tostring(i), b.label, -12, 12, 0.5, state.gains[i] or 0, "dB"
        ))
    end
    tntron.ui.register_sidebar({
        id = "eq-studio",
        label = "Equalizer",
        icon = "sliders",
        widgets = sidebar_widgets,
    })

    tntron.ui.register_settings({
        id = "eq-quick",
        title = "Studio Equalizer (10-Band DSP)",
        widgets = {
            tntron.ui.toggle("eq_enable", "Enable 10-Band EQ Stage", state.enabled),
            tntron.ui.row(preset_btns),
        },
    })
end

function on_load(_ctx)
    local saved = tntron.storage.get("eq_state", nil)
    if type(saved) == "table" and type(saved.gains) == "table" then
        state.enabled = (saved.enabled ~= false)
        state.preset = saved.preset or "custom"
        for i = 1, #BANDS do
            state.gains[i] = tonumber(saved.gains[i]) or 0
        end
    end
    apply_audio()
    render()
end

function on_unload(_ctx)
    tntron.audio.clear_filter()
end

function on_action(_ctx, action_id, val)
    if action_id == "eq_enable" then
        state.enabled = (val == true)
    elseif action_id:sub(1, 7) == "preset:" then
        local pid = action_id:sub(8)
        for _, p in ipairs(PRESETS) do
            if p.id == pid then
                state.preset = pid
                for i = 1, #BANDS do state.gains[i] = p.gains[i] end
                tntron.ui.toast("EQ Preset: " .. p.label)
                break
            end
        end
    elseif action_id:sub(1, 5) == "band:" then
        local idx = tonumber(action_id:sub(6))
        if idx and idx >= 1 and idx <= #BANDS then
            state.gains[idx] = tonumber(val) or 0
            state.preset = "custom"
        end
    end
    save()
    apply_audio()
    render()
end
