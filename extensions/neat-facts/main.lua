-- Neat Facts & Vibe Quips (720p YouTube data-saved calculator + Tunotron lore + playful vibes)

-- Minimum 720p YouTube bitrate estimate:
-- 2.5 Mbps (2500 kbps) = 0.3125 MB/s (18.75 MB/min) for local playback vs 720p video+audio.
-- When streaming audio-only (~128 kbps), net video savings = ~2.372 Mbps = 0.2965 MB/s.
local YT_720P_LOCAL_MB_PER_SEC = 0.3125
local YT_720P_STREAM_MB_PER_SEC = 0.2965

local st = {
    show_bar = true,
    periodic_toasts = false,
    interval_sec = 30,
    category = "all", -- "all" | "stats" | "lore" | "vibes" | "quotes"
    fact_idx = 1,
    tick_counter = 0,
    toast_counter = 0,
    local_sec = 0,
    stream_sec = 0,
    session_sec = 0,
    tick_flush = 0,
    current_fact = "Did you know? Tunotron was built in one day as a Rust rewrite!",
}

local LORE_FACTS = {
    "Did you know? Tunotron was built in one day as a Rust rewrite!",
    "Tune o trona deus ex machina!",
    "Your one and only music source after the apocalypse.",
    "Knock knock! Who's there? Your tunotron subscription bill, $0!",
    "Zero Electron. Zero Chromium. Pure Rust, GPU shaders, mpv, and Lua 5.4 sandboxes.",
    "Fun fact: Tunotron + mpv uses less RAM than a single YouTube homepage tab.",
    "Every extension runs inside a sandboxed Lua 5.4 VM — zero filesystem escape, all vibes.",
    "Did you know? Tunotron decodes JPEG XL (.jxl) covers natively and computes 16-band Goertzel spectrums at 25 fps.",
    "Local audio files never get hit with 'This video is unavailable in your country.'",
    "Your audio pipeline runs native — no browser tab quietly suspending your music.",
    "Vinyl Link talks straight to mpv's pitch engine for true turntable tempo-pitch coupling.",
    "Built to outlive every cloud music service shutting down next Tuesday.",
    "Apocalypse survival kit: water, flashlight, and a local music folder that needs zero Wi-Fi.",
}

local VIBE_FACTS = {
    "Hey, I see you swaying your mouse to the beat!",
    "Just Vibin'",
    "哎嘿,你想听听我最新的混音吗?",
    "Warning: Side effects of Tunotron include involuntary head-nodding and clean commits.",
    "Hydration check! Grab a sip of water before the chorus drops.",
    "Posture check! Un-shrimp your spine — the bassline demands dignity.",
    "Plot twist: Your local folder knows your taste better than any algorithm.",
    "No unskippable 30-second ads interrupted this vibe. You're welcome.",
    "Legend says looping a track 100 times unlocks its secret boss theme.",
    "Audio engineers spent 400 hours mastering this mix — zero browser compression here.",
    "Pressing Next three times in a row officially counts as cardio.",
    "Current telemetry: 90% caffeine, 10% sub-bass frequencies.",
    "Somewhere out there, a streaming subscription is sad you own your music.",
    "You didn't just play a song — you summoned a sound wave.",
    "Pro tip: Turning it up 5% empirically makes code compile faster.",
}

local SONG_QUOTES = {
    -- User favorites & Tunotron remixes
    "O-oh sometimes, I feel a good feeling...",
    "Turn down for what!",
    "Everybody, make some noise!",
    "One more time.",
    "Back with another one of those Tune oh tronning beats!",
    "Let the tunotron cannon kick it!",
    "Blow a kiss, fire a gun, we need vocatron to listen on!",
    -- Daft Punk
    "Harder, Better, Faster, Stronger — basically the Tunotron changelog.",
    "Around the world, around the world... (local playback, 0 MB streamed).",
    "We're up all night to get lucky — and to dial in the 3-band EQ.",
    "Human After All, just accelerated by Rust and wgpu.",
    "Technologic: Play it, loop it, pitch it, speed it, bass-boost it.",
    "Digital Love in crisp local audio.",
    "Give Life Back to Music.",
    -- Eminem
    "Guess who's back, back again — Tunotron's back, tell a friend!",
    "Will the real Slim Shady please press play?",
    "Snap back to reality, ope there goes gravity... (Vinyl Link: 0.85x).",
    "You only get one shot, do not miss your chance to drop the bass.",
    "Mom's spaghetti, clean playlists, zero buffering.",
    "Now this looks like a playlist for me.",
    "Look, if you had one shot, or one opportunity, to own all the music you ever wanted...",
    -- Club / EDM / Vibe Playlist Classics
    "Right about now, the funk soul brother — check it out now!",
    "Wake me up when the playlist's all over.",
    "Feel Good Inc. — now running natively on GPU shaders.",
    "Sandstorm advisory: Dududududu detected across all 16 spectrum bands.",
    "Music makes you lose control — Rust keeps memory safe while you do.",
    "Pump up the jam, pump it up, while your RAM footprint stays tiny!",
    "We came, we saw, we cranked the sub-bass.",
    "Let's get it started in here!",
    "Don't stop me now — I'm having such a good time.",
    "No sleep till... the sleep timer hits zero.",
    "Drop it like it's hot (25fps Goertzel spectrum approved).",
    "Put your hands up — localhost is live on the decks!",
}

local function fmt_int(n)
    local s = tostring(math.floor(math.max(0, tonumber(n) or 0)))
    local rev = s:reverse():gsub("(%d%d%d)", "%1,"):reverse()
    if rev:sub(1, 1) == "," then
        rev = rev:sub(2)
    end
    return rev
end

local function fmt_duration(sec)
    local s = math.floor(math.max(0, sec or 0))
    local h = math.floor(s / 3600)
    local m = math.floor((s % 3600) / 60)
    if h > 0 then
        return string.format("%dh %dm", h, m)
    elseif m > 0 then
        return string.format("%dm %02ds", m, s % 60)
    end
    return string.format("%ds", s)
end

local function calc_stats(pb)
    local life_sec = pb and tonumber(pb.lifetime_listen_sec) or 0
    local life_plays = pb and tonumber(pb.lifetime_plays) or 0
    local total_sec = math.max(life_sec, st.local_sec + st.stream_sec)
    local stream_part = math.min(st.stream_sec, total_sec)
    local local_part = math.max(0, total_sec - stream_part)

    local saved_mb = local_part * YT_720P_LOCAL_MB_PER_SEC + stream_part * YT_720P_STREAM_MB_PER_SEC
    local saved_mb_int = math.floor(saved_mb + 0.5)
    local saved_gb = saved_mb / 1024.0
    local floppies = math.max(1, math.floor(saved_mb / 1.44 + 0.5))
    local frames_skipped = math.floor(total_sec * 30)

    return {
        total_sec = total_sec,
        life_plays = life_plays,
        saved_mb = saved_mb,
        saved_mb_int = saved_mb_int,
        saved_gb = saved_gb,
        floppies = floppies,
        frames_skipped = frames_skipped,
    }
end

local function build_pool(pb)
    local stats = calc_stats(pb)
    local pool = {}

    local function add(cat, msg)
        if st.category == "all" or st.category == cat then
            table.insert(pool, msg)
        end
    end

    -- Dynamic bandwidth & usage facts (720p minimum YouTube baseline)
    local mb_label = stats.saved_mb_int >= 1 and (fmt_int(stats.saved_mb_int) .. " MB")
        or string.format("%.1f MB", stats.saved_mb)
    add("stats", string.format(
        "You saved at least %s of data since you started using Tunotron! (vs 720p YouTube)",
        mb_label
    ))
    if stats.saved_mb_int >= 10 then
        add("stats", string.format(
            "Skipping 720p YouTube video just saved ~%s floppy disks worth of bandwidth!",
            fmt_int(stats.floppies)
        ))
    end
    if stats.total_sec >= 30 then
        add("stats", string.format(
            "Your GPU dodged decoding ~%s unnecessary 720p video frames so far!",
            fmt_int(stats.frames_skipped)
        ))
        add("stats", string.format(
            "%s of pure audio played — zero buffering wheels, zero preroll ads.",
            fmt_duration(stats.total_sec)
        ))
    end
    if stats.life_plays >= 1 then
        add("stats", string.format(
            "%s tracks spun in Tunotron so far — and your library stays 100%% yours.",
            fmt_int(stats.life_plays)
        ))
    end

    -- Context-aware live audio facts
    if pb and pb.playing then
        local beat = tonumber(pb.beat_pulse) or 0
        local spd = tonumber(pb.speed) or 1.0
        local pitch = tonumber(pb.pitch_st) or 0.0
        local bpm = tonumber(pb.bpm)

        if beat > 0.50 then
            add("vibes", "Hey, I see you swaying your mouse to the beat!")
            add("vibes", "Sub-bass spike detected — your desk is officially a subwoofer now.")
        end
        if spd > 1.08 then
            add("vibes", string.format("Nightcore physics engaged — warping time at %.2fx speed!", spd))
        elseif spd < 0.92 then
            add("vibes", string.format("Slowed & cruising at %.2fx — gravity just dropped 20%%.", spd))
        end
        if math.abs(pitch) >= 0.5 then
            add("vibes", string.format("Reality shifted by %+.1f semitones — alternate-universe mix unlocked.", pitch))
        end
        if bpm and bpm >= 140 then
            add("vibes", string.format("%.0f BPM?! Hold onto your keyboard before it achieves lift-off.", bpm))
        elseif bpm and bpm > 0 and bpm <= 85 then
            add("vibes", string.format("Chill %.0f BPM cruise control — peak late-night focus zone.", bpm))
        end
    end

    for _, f in ipairs(LORE_FACTS) do
        add("lore", f)
    end
    for _, f in ipairs(SONG_QUOTES) do
        add("quotes", f)
    end
    for _, f in ipairs(VIBE_FACTS) do
        add("vibes", f)
    end

    if st.category == "all" and #pool > 4 then
        -- Interleave stats, lore, song quotes, and vibes so clicking Next Fact stays varied.
        local mixed = {}
        local stride = 7
        local used = {}
        local idx = 1
        for _ = 1, #pool do
            while used[idx] do
                idx = (idx % #pool) + 1
            end
            used[idx] = true
            table.insert(mixed, pool[idx])
            idx = ((idx - 1 + stride) % #pool) + 1
        end
        pool = mixed
    end

    if #pool == 0 then
        table.insert(pool, "Just Vibin'")
    end
    return pool
end

local function pick_next_fact(pb, advance)
    local pool = build_pool(pb)
    if advance then
        st.fact_idx = (st.fact_idx % #pool) + 1
    elseif st.fact_idx < 1 or st.fact_idx > #pool then
        st.fact_idx = 1
    end
    st.current_fact = pool[st.fact_idx]
    return st.current_fact
end

local function shorten(s, max_chars)
    local len = utf8.len(s)
    if not len or len <= max_chars then
        return s
    end
    local cut = utf8.offset(s, math.max(1, max_chars - 2))
    if not cut then
        return s
    end
    return s:sub(1, cut - 1) .. "..."
end

local function persist()
    tntron.storage.set("prefs", {
        show_bar = st.show_bar,
        periodic_toasts = st.periodic_toasts,
        interval_sec = st.interval_sec,
        category = st.category,
        local_sec = st.local_sec,
        stream_sec = st.stream_sec,
    })
end

local function render(pb)
    local stats = calc_stats(pb)

    if st.show_bar then
        tntron.ui.set_player_widget({
            text = shorten(st.current_fact, 58),
            icon = "sparkle",
            accent = false,
            action_id = "next_fact",
        })
    else
        tntron.ui.clear_player_widget()
    end

    local saved_badge = stats.saved_mb_int >= 1024
        and string.format("Saved >= %s MB (%.2f GB)", fmt_int(stats.saved_mb_int), stats.saved_gb)
        or (stats.saved_mb_int >= 1
            and string.format("Saved >= %s MB", fmt_int(stats.saved_mb_int))
            or string.format("Saved >= %.1f MB", stats.saved_mb))

    tntron.ui.register_sidebar({
        id = "neat-facts",
        label = "Neat Facts",
        icon = "sparkle",
        widgets = {
            tntron.ui.heading("Neat Fact of the Moment"),
            tntron.ui.text(st.current_fact, false),
            tntron.ui.row({
                tntron.ui.button("next_fact", "Next Fact", { icon = "sparkle", accent = true }),
                tntron.ui.button("toast_now", "Pop Toast", { icon = "bell" }),
            }),

            tntron.ui.heading("720p YouTube Data Saved"),
            tntron.ui.row({
                tntron.ui.badge(saved_badge, { icon = "sparkle", accent = true }),
                tntron.ui.badge("Listened: " .. fmt_duration(stats.total_sec), { icon = "clock" }),
            }),
            tntron.ui.row({
                tntron.ui.badge("Plays: " .. fmt_int(stats.life_plays), { icon = "music" }),
                tntron.ui.badge("Frames Dodged: " .. fmt_int(stats.frames_skipped), { icon = "eye" }),
            }),
            tntron.ui.text(
                "Baseline: YouTube 720p minimum (~2.5 Mbps / 18.75 MB per minute). Audio-only streams save ~2.37 Mbps of video payload.",
                true
            ),

            tntron.ui.heading("Display & Rotation"),
            tntron.ui.toggle("show_bar", "Show Fact Ticker in Player Bar", st.show_bar),
            tntron.ui.toggle("periodic_toasts", "Occasional Toast Quips (every 3 min)", st.periodic_toasts),
            tntron.ui.row({
                tntron.ui.button("cat:all", "All", { active = st.category == "all" }),
                tntron.ui.button("cat:stats", "Data & Stats", { active = st.category == "stats" }),
                tntron.ui.button("cat:quotes", "Song Quotes", { active = st.category == "quotes" }),
                tntron.ui.button("cat:lore", "Lore", { active = st.category == "lore" }),
                tntron.ui.button("cat:vibes", "Vibes", { active = st.category == "vibes" }),
            }),
            tntron.ui.row({
                tntron.ui.button("int:15", "15s", { active = st.interval_sec == 15 }),
                tntron.ui.button("int:30", "30s", { active = st.interval_sec == 30 }),
                tntron.ui.button("int:60", "60s", { active = st.interval_sec == 60 }),
                tntron.ui.button("int:120", "2m", { active = st.interval_sec == 120 }),
            }),
        },
    })
end

function on_load(_ctx)
    local saved = tntron.storage.get("prefs", nil)
    if type(saved) == "table" then
        if saved.show_bar ~= nil then st.show_bar = (saved.show_bar == true) end
        if saved.periodic_toasts ~= nil then st.periodic_toasts = (saved.periodic_toasts == true) end
        if tonumber(saved.interval_sec) then st.interval_sec = math.max(10, math.floor(tonumber(saved.interval_sec))) end
        if type(saved.category) == "string" then st.category = saved.category end
        st.local_sec = tonumber(saved.local_sec) or 0
        st.stream_sec = tonumber(saved.stream_sec) or 0
    end
    local pb = tntron.playback.get()
    pick_next_fact(pb, false)
    render(pb)
end

function on_unload(_ctx)
    tntron.ui.clear_player_widget()
end

function on_event(_ctx, event_name)
    if event_name == "trackChange" then
        local pb = tntron.playback.get()
        st.tick_counter = 0
        pick_next_fact(pb, true)
        render(pb)
    end
end

function on_tick(_ctx)
    local pb = tntron.playback.get()
    if pb and pb.playing then
        st.session_sec = st.session_sec + 1
        local p = tostring(pb.track_path or "")
        if p:sub(1, 7) == "http://" or p:sub(1, 8) == "https://" then
            st.stream_sec = st.stream_sec + 1
        else
            st.local_sec = st.local_sec + 1
        end
        st.tick_flush = st.tick_flush + 1
        if st.tick_flush >= 10 then
            st.tick_flush = 0
            persist()
        end
    end

    st.tick_counter = st.tick_counter + 1
    local rotated = false
    if st.tick_counter >= st.interval_sec then
        st.tick_counter = 0
        pick_next_fact(pb, true)
        rotated = true
    end

    if st.periodic_toasts and pb and pb.playing then
        st.toast_counter = st.toast_counter + 1
        if st.toast_counter >= 180 then
            st.toast_counter = 0
            tntron.ui.toast(pick_next_fact(pb, true))
            rotated = true
        end
    end

    if rotated or (pb and pb.playing and st.tick_flush == 0) then
        render(pb)
    end
end

function on_action(_ctx, action_id, val)
    local pb = tntron.playback.get()
    if action_id == "next_fact" then
        st.tick_counter = 0
        pick_next_fact(pb, true)
    elseif action_id == "toast_now" then
        tntron.ui.toast(st.current_fact)
    elseif action_id == "show_bar" then
        st.show_bar = (val == true)
    elseif action_id == "periodic_toasts" then
        st.periodic_toasts = (val == true)
        st.toast_counter = 0
    elseif action_id:sub(1, 4) == "cat:" then
        st.category = action_id:sub(5)
        st.fact_idx = 0
        pick_next_fact(pb, true)
    elseif action_id:sub(1, 4) == "int:" then
        st.interval_sec = math.max(10, math.floor(tonumber(action_id:sub(5)) or 30))
        st.tick_counter = 0
    end
    persist()
    render(pb)
end
