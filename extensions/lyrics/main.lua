-- Lyrics (Multi-provider lyrics + line-by-line translation + click-to-seek)

local st = {
    track_path = "",
    title = "",
    artist = "",
    provider = "auto",
    source = "Waiting",
    loading = false,
    error = nil,
    lines = {},
    translate_on = false,
    target_lang = "en",
    translating = false,
    trans_error = nil,
    detected_lang = nil,
    trans_provider = nil,
    translations = {},
    trans_cache = {},
}

local PROVIDERS = {
    { id = "auto",          label = "Auto" },
    { id = "lrclib",        label = "LRCLIB" },
    { id = "lrclib_search", label = "Search" },
    { id = "netease",       label = "NetEase" },
    { id = "kugou",         label = "Kugou" },
    { id = "lyrics_ovh",    label = "Lyrics.ovh" },
    { id = "local",         label = "Local" },
}

local LANGS = { "en", "tr", "es", "fr", "de", "ja", "ko", "ru", "ar" }

local function find_active_idx(lines, pos)
    local active = nil
    for i, l in ipairs(lines) do
        if (tonumber(l.time_sec) or 0) <= (pos + 0.1) then
            active = i
        else
            break
        end
    end
    return active
end

local function fmt(sec)
    local s = math.max(0, math.floor(sec or 0))
    return string.format("%02d:%02d", math.floor(s / 60), s % 60)
end

local function cache_key(path, lang)
    return (path or "") .. "|" .. (lang or "en")
end

local function request_translation_if_needed()
    if not st.translate_on or #st.lines == 0 or st.track_path == "" then
        return
    end
    local key = cache_key(st.track_path, st.target_lang)
    local cached = st.trans_cache[key]
    if cached then
        st.translations = cached.translations or {}
        st.detected_lang = cached.detected
        st.trans_provider = cached.via
        st.translating = false
        st.trans_error = nil
        return
    end
    local raw = {}
    for _, l in ipairs(st.lines) do
        table.insert(raw, tostring(l.text or ""))
    end
    st.translating = true
    st.trans_error = nil
    st.translations = {}
    tntron.lyrics.translate(st.track_path, raw, st.target_lang)
end

local function render(pb)
    local widgets = {}
    local title = pb and pb.title ~= "" and pb.title or "No track playing"
    local artist = pb and pb.artist or ""
    table.insert(widgets, tntron.ui.heading(title))
    table.insert(widgets, tntron.ui.row({
        tntron.ui.text(artist ~= "" and artist or "Select or play a track", true),
        tntron.ui.badge(st.loading and "Searching..." or st.source, { icon = "lyrics", accent = (#st.lines > 0) }),
        tntron.ui.button("reload_lrc", "Refresh", { icon = "refresh" }),
    }))

    local prov_btns = {}
    for _, p in ipairs(PROVIDERS) do
        table.insert(prov_btns, tntron.ui.button(
            "prov:" .. p.id,
            p.label,
            { active = st.provider == p.id }
        ))
    end
    table.insert(widgets, tntron.ui.row(prov_btns))

    local trans_row = {
        tntron.ui.button("toggle_trans", st.translate_on and "Translate: ON" or "Translate: OFF", {
            icon = "globe",
            active = st.translate_on,
            accent = st.translate_on,
        }),
    }
    for _, code in ipairs(LANGS) do
        table.insert(trans_row, tntron.ui.button(
            "lang:" .. code,
            code:upper(),
            { active = st.target_lang == code }
        ))
    end
    if st.translate_on then
        local status_txt = st.translating and "Translating..."
            or (st.detected_lang and string.format("%s -> %s", st.detected_lang:upper(), st.target_lang:upper()))
            or ("-> " .. st.target_lang:upper())
        table.insert(trans_row, tntron.ui.badge(status_txt, {
            icon = "globe",
            accent = (#st.translations > 0),
        }))
    end
    table.insert(widgets, tntron.ui.row(trans_row))
    if st.translate_on then
        table.insert(widgets, tntron.ui.input("lang_custom", "Lang code", st.target_lang, {
            placeholder = "e.g. pt, it, zh-CN",
            submit_action = "set_custom_lang",
            submit_label = "Use",
        }))
    end

    if st.trans_error and st.translate_on then
        table.insert(widgets, tntron.ui.text("Translation info: " .. tostring(st.trans_error), true))
    end

    if st.error and #st.lines == 0 then
        table.insert(widgets, tntron.ui.text("Lookup info: " .. tostring(st.error), true))
    elseif #st.lines == 0 then
        table.insert(widgets, tntron.ui.text("No synced lines loaded yet for this track.", true))
    else
        local pos = pb and pb.position_sec or 0
        local active = find_active_idx(st.lines, pos) or 1
        local start_i = math.max(1, active - 4)
        local end_i = math.min(#st.lines, start_i + 12)
        for i = start_i, end_i do
            local line = st.lines[i]
            local is_cur = (i == active)
            local ts = fmt(line.time_sec)
            table.insert(widgets, tntron.ui.button(
                "seek:" .. tostring(i),
                string.format("[%s] %s", ts, line.text),
                { active = is_cur, accent = is_cur }
            ))
            if st.translate_on then
                local tr = st.translations[i]
                if tr and tr ~= "" and tr ~= line.text then
                    table.insert(widgets, tntron.ui.text("   -> " .. tr, not is_cur))
                end
            end
        end
    end

    tntron.ui.register_sidebar({
        id = "lyrics-view",
        label = "Lyrics",
        icon = "lyrics",
        widgets = widgets,
    })
end

local function fetch_for_current(pb)
    if not pb or pb.track_path == "" then
        st.track_path = ""
        st.lines = {}
        st.translations = {}
        st.translating = false
        st.source = "Idle"
        render(pb)
        return
    end
    st.track_path = pb.track_path
    st.title = pb.title
    st.artist = pb.artist
    st.source = "Searching (" .. st.provider .. ")..."
    st.loading = true
    st.error = nil
    st.lines = {}
    st.translations = {}
    st.translating = false
    st.trans_error = nil
    st.detected_lang = nil
    tntron.lyrics.load(pb.track_path, pb.title, pb.artist, pb.duration_sec or 0, st.provider)
    render(pb)
end

function on_load(_ctx)
    st.provider = tntron.storage.get("provider", "auto")
    st.translate_on = (tntron.storage.get("translate_on", false) == true)
    st.target_lang = tntron.storage.get("target_lang", "en")
    fetch_for_current(tntron.playback.get())
end

function on_event(_ctx, event_name, payload)
    local pb = tntron.playback.get()
    if event_name == "trackChange" then
        fetch_for_current(pb)
    elseif event_name == "lyricsLoaded" and type(payload) == "table" then
        if payload.path == st.track_path or st.track_path == "" then
            st.loading = false
            st.source = payload.source or "Loaded"
            st.error = payload.error
            st.lines = type(payload.lines) == "table" and payload.lines or {}
            request_translation_if_needed()
            render(pb)
        end
    elseif event_name == "lyricsTranslated" and type(payload) == "table" then
        if payload.path == st.track_path and payload.target_lang == st.target_lang then
            st.translating = false
            st.trans_error = payload.error
            st.detected_lang = payload.detected_lang
            st.trans_provider = payload.provider
            local trs = type(payload.translations) == "table" and payload.translations or {}
            st.translations = trs
            if #trs > 0 and not payload.error then
                st.trans_cache[cache_key(st.track_path, st.target_lang)] = {
                    translations = trs,
                    detected = payload.detected_lang,
                    via = payload.provider,
                }
            end
            render(pb)
        end
    end
end

function on_tick(_ctx)
    if #st.lines > 0 or st.loading or st.translating then
        render(tntron.playback.get())
    end
end

function on_action(_ctx, action_id, val)
    local pb = tntron.playback.get()
    if action_id == "reload_lrc" then
        fetch_for_current(pb)
        return
    elseif action_id:sub(1, 5) == "prov:" then
        st.provider = action_id:sub(6)
        tntron.storage.set("provider", st.provider)
        fetch_for_current(pb)
        return
    elseif action_id == "toggle_trans" then
        st.translate_on = not st.translate_on
        tntron.storage.set("translate_on", st.translate_on)
        request_translation_if_needed()
    elseif action_id:sub(1, 5) == "lang:" then
        st.target_lang = action_id:sub(6)
        st.translate_on = true
        tntron.storage.set("target_lang", st.target_lang)
        tntron.storage.set("translate_on", true)
        request_translation_if_needed()
    elseif action_id == "set_custom_lang" then
        local code = tostring(val or ""):gsub("%s+", ""):lower()
        if code ~= "" then
            st.target_lang = code
            st.translate_on = true
            tntron.storage.set("target_lang", st.target_lang)
            tntron.storage.set("translate_on", true)
            request_translation_if_needed()
        end
    elseif action_id:sub(1, 5) == "seek:" then
        local idx = tonumber(action_id:sub(6))
        local line = idx and st.lines[idx]
        if line and line.time_sec then
            tntron.playback.seek(tonumber(line.time_sec) or 0)
        end
    end
    render(pb)
end
