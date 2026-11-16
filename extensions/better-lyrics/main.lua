-- Better Lyrics (Sync+, multi-provider source, translation, and player bar lyrics)

local st = {
    track_path = "",
    offset = 0.0,
    skip_lines = 0,
    lines = {},
    source = "Waiting",
    provider = "auto",
    show_bar = true,
    translate_on = false,
    target_lang = "en",
    translating = false,
    translations = {},
    detected_lang = nil,
}

local PROVIDERS = {
    { id = "auto",          label = "Auto" },
    { id = "lrclib",        label = "LRCLIB" },
    { id = "lrclib_search", label = "Search" },
    { id = "netease",       label = "NetEase" },
    { id = "kugou",         label = "Kugou" },
    { id = "lyrics_ovh",    label = "Lyrics.ovh" },
}

local LANGS = { "en", "tr", "es", "fr", "de", "ja", "ko", "ru" }

local function get_saved_num(map_key, path, default_val)
    if path == "" then return default_val end
    local tbl = tntron.storage.get(map_key, {})
    if type(tbl) == "table" and tbl[path] ~= nil then
        return tonumber(tbl[path]) or default_val
    end
    return default_val
end

local function save_map_num(map_key, path, val)
    if path == "" then return end
    local tbl = tntron.storage.get(map_key, {})
    if type(tbl) ~= "table" then tbl = {} end
    tbl[path] = val
    tntron.storage.set(map_key, tbl)
end

local function active_index(pb)
    if not pb or #st.lines == 0 then return nil end
    local adj = (pb.position_sec or 0) + st.offset
    local base_idx = nil
    for i, l in ipairs(st.lines) do
        if (tonumber(l.time_sec) or 0) <= (adj + 0.1) then
            base_idx = i
        else
            break
        end
    end
    if not base_idx then return nil end
    return math.max(1, math.min(#st.lines, base_idx + st.skip_lines))
end

local function trigger_translate()
    if not st.translate_on or #st.lines == 0 or st.track_path == "" then
        return
    end
    local raw = {}
    for _, l in ipairs(st.lines) do
        table.insert(raw, tostring(l.text or ""))
    end
    st.translating = true
    st.translations = {}
    tntron.lyrics.translate(st.track_path, raw, st.target_lang)
end

local function render(pb)
    local act = active_index(pb)
    local cur_line = act and st.lines[act] and st.lines[act].text or nil
    local cur_tr = act and st.translations[act] or nil

    if st.show_bar and cur_line and cur_line ~= "" then
        local bar_txt = cur_line
        if st.translate_on and cur_tr and cur_tr ~= "" and cur_tr ~= cur_line then
            bar_txt = cur_tr
        end
        local short = #bar_txt > 42 and (bar_txt:sub(1, 39) .. "...") or bar_txt
        tntron.ui.set_player_widget({
            text = short,
            icon = "lyrics",
            accent = true,
            action_id = "nudge_back",
        })
    else
        tntron.ui.clear_player_widget()
    end

    local prov_btns = {}
    for _, p in ipairs(PROVIDERS) do
        table.insert(prov_btns, tntron.ui.button("prov:" .. p.id, p.label, { active = st.provider == p.id }))
    end

    local tr_row = {
        tntron.ui.button("toggle_trans", st.translate_on and "Translate: ON" or "Translate: OFF", {
            icon = "globe",
            active = st.translate_on,
            accent = st.translate_on,
        }),
    }
    for _, code in ipairs(LANGS) do
        table.insert(tr_row, tntron.ui.button("lang:" .. code, code:upper(), { active = st.target_lang == code }))
    end
    if st.translate_on then
        local lbl = st.translating and "Translating..."
            or (st.detected_lang and string.format("%s->%s", st.detected_lang:upper(), st.target_lang:upper()))
            or st.target_lang:upper()
        table.insert(tr_row, tntron.ui.badge(lbl, { icon = "globe", accent = #st.translations > 0 }))
    end

    local wid = {
        tntron.ui.heading(pb and pb.title ~= "" and pb.title or "Better Lyrics"),
        tntron.ui.row({
            tntron.ui.badge("Source: " .. tostring(st.source), { icon = "lyrics", accent = #st.lines > 0 }),
            tntron.ui.button("resync", "Sync Now", { icon = "refresh", accent = true }),
        }),
        tntron.ui.row(prov_btns),
        tntron.ui.row(tr_row),
        tntron.ui.slider("offset", "Timing Offset", -5.0, 5.0, 0.25, st.offset, "s"),
        tntron.ui.row({
            tntron.ui.button("nudge_back", "-0.5s", { icon = "prev" }),
            tntron.ui.button("nudge_zero", "0.0s", { active = math.abs(st.offset) < 0.01 }),
            tntron.ui.button("nudge_fwd",  "+0.5s", { icon = "next" }),
            tntron.ui.button("skip_prev",  "Skip -1"),
            tntron.ui.button("skip_zero",  "Skip: " .. tostring(st.skip_lines), { active = st.skip_lines ~= 0 }),
            tntron.ui.button("skip_next",  "Skip +1"),
        }),
        tntron.ui.toggle("bar_toggle", "Show live lyric line in player bar", st.show_bar),
    }

    if #st.lines == 0 then
        table.insert(wid, tntron.ui.text("Waiting for synced lyrics (.lrc / LRCLIB / NetEase / Kugou)...", true))
    else
        local center = act or 1
        local lo = math.max(1, center - 4)
        local hi = math.min(#st.lines, lo + 9)
        for i = lo, hi do
            local line_txt = st.lines[i].text or ""
            if line_txt ~= "" then
                if i == act then
                    table.insert(wid, tntron.ui.heading(">> " .. line_txt))
                else
                    table.insert(wid, tntron.ui.text(line_txt, true))
                end
                if st.translate_on then
                    local tr = st.translations[i]
                    if tr and tr ~= "" and tr ~= line_txt then
                        table.insert(wid, tntron.ui.text("   -> " .. tr, i ~= act))
                    end
                end
            end
        end
    end

    tntron.ui.register_sidebar({
        id = "better-lyrics",
        label = "Lyrics Sync+",
        icon = "lyrics",
        widgets = wid,
    })
end

local function request_lines(pb)
    if not pb or pb.track_path == "" then
        st.track_path = ""
        st.offset = 0.0
        st.skip_lines = 0
        st.lines = {}
        st.translations = {}
        st.source = "Idle"
        render(pb)
        return
    end
    st.track_path = pb.track_path
    st.offset = get_saved_num("offsets", pb.track_path, 0.0)
    st.skip_lines = math.floor(get_saved_num("skips", pb.track_path, 0))
    st.lines = {}
    st.translations = {}
    st.translating = false
    st.detected_lang = nil
    st.source = "Searching (" .. st.provider .. ")..."
    tntron.lyrics.load(pb.track_path, pb.title, pb.artist, pb.duration_sec or 0, st.provider)
    render(pb)
end

function on_load(_ctx)
    st.show_bar = (tntron.storage.get("show_bar", true) ~= false)
    st.provider = tntron.storage.get("provider", "auto")
    st.translate_on = (tntron.storage.get("translate_on", false) == true)
    st.target_lang = tntron.storage.get("target_lang", "en")
    request_lines(tntron.playback.get())
end

function on_event(_ctx, event_name, payload)
    local pb = tntron.playback.get()
    if event_name == "trackChange" then
        request_lines(pb)
    elseif event_name == "lyricsLoaded" and type(payload) == "table" then
        st.source = payload.source or "Loaded"
        st.lines = type(payload.lines) == "table" and payload.lines or {}
        trigger_translate()
        render(pb)
    elseif event_name == "lyricsTranslated" and type(payload) == "table" then
        if payload.path == st.track_path and payload.target_lang == st.target_lang then
            st.translating = false
            st.detected_lang = payload.detected_lang
            st.translations = type(payload.translations) == "table" and payload.translations or {}
            render(pb)
        end
    end
end

function on_tick(_ctx)
    render(tntron.playback.get())
end

function on_action(_ctx, action_id, val)
    local pb = tntron.playback.get()
    if action_id == "offset" then
        st.offset = tonumber(val) or 0.0
        save_map_num("offsets", st.track_path, st.offset)
    elseif action_id:sub(1, 5) == "prov:" then
        st.provider = action_id:sub(6)
        tntron.storage.set("provider", st.provider)
        request_lines(pb)
        return
    elseif action_id == "toggle_trans" then
        st.translate_on = not st.translate_on
        tntron.storage.set("translate_on", st.translate_on)
        trigger_translate()
    elseif action_id:sub(1, 5) == "lang:" then
        st.target_lang = action_id:sub(6)
        st.translate_on = true
        tntron.storage.set("target_lang", st.target_lang)
        tntron.storage.set("translate_on", true)
        trigger_translate()
    elseif action_id == "nudge_back" then
        st.offset = math.max(-5.0, st.offset - 0.5)
        save_map_num("offsets", st.track_path, st.offset)
    elseif action_id == "nudge_fwd" then
        st.offset = math.min(5.0, st.offset + 0.5)
        save_map_num("offsets", st.track_path, st.offset)
    elseif action_id == "nudge_zero" then
        st.offset = 0.0
        save_map_num("offsets", st.track_path, 0.0)
    elseif action_id == "skip_prev" then
        st.skip_lines = math.max(-10, st.skip_lines - 1)
        save_map_num("skips", st.track_path, st.skip_lines)
    elseif action_id == "skip_next" then
        st.skip_lines = math.min(10, st.skip_lines + 1)
        save_map_num("skips", st.track_path, st.skip_lines)
    elseif action_id == "skip_zero" then
        st.skip_lines = 0
        save_map_num("skips", st.track_path, 0)
    elseif action_id == "resync" then
        request_lines(pb)
        return
    elseif action_id == "bar_toggle" then
        st.show_bar = (val == true)
        tntron.storage.set("show_bar", st.show_bar)
    end
    render(pb)
end
