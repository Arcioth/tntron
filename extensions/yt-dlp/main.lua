-- YouTube Music via yt-dlp (Openify yt-dlp + Tunotron ytdlp.lua)

local st = {
    query = "",
    searching = false,
    status = "Search YouTube or paste a video URL to stream or download into your current music folder.",
    results = {},
}

local function render()
    local cur = tntron.library.current_folder() or { path = "" }
    local widgets = {
        tntron.ui.input("yt_query", "YouTube Search / URL", st.query, {
            placeholder = "Artist, song name, or https://youtube.com/...",
            submit_action = "search",
            submit_label = st.searching and "Searching..." or "Search",
        }),
        tntron.ui.text("Download target: " .. tostring(cur.path), true),
        tntron.ui.text(st.status, true),
    }

    if #st.results > 0 then
        table.insert(widgets, tntron.ui.heading(string.format("Results (%d)", #st.results)))
        for i, item in ipairs(st.results) do
            local lbl = string.format("%s  (%s · %s)", item.title or "Track", item.channel or "YouTube", item.duration or "--:--")
            table.insert(widgets, tntron.ui.row({
                tntron.ui.button("play:" .. tostring(i), lbl, { icon = "play", accent = true }),
                tntron.ui.button("dl:" .. tostring(i), "Download", { icon = "download" }),
            }))
        end
    end

    tntron.ui.register_sidebar({
        id = "yt-dlp",
        label = "YouTube",
        icon = "globe",
        widgets = widgets,
    })
end

function on_load(_ctx)
    render()
end

function on_event(_ctx, event_name, payload)
    if event_name == "ytSearch" and type(payload) == "table" then
        st.searching = false
        if payload.ok then
            st.results = type(payload.results) == "table" and payload.results or {}
            st.status = string.format("Found %d result(s) for '%s'", #st.results, tostring(payload.query or ""))
        else
            st.status = "Search failed: " .. tostring(payload.error or "yt-dlp error")
            tntron.ui.toast(st.status)
        end
        render()
    elseif event_name == "ytPlaying" and type(payload) == "table" then
        if payload.ok then
            st.status = "Streaming: " .. tostring(payload.title or "YouTube Audio")
            tntron.ui.toast(st.status)
        else
            st.status = "Stream failed: " .. tostring(payload.error or "yt-dlp error")
            tntron.ui.toast(st.status)
        end
        render()
    elseif event_name == "ytDownloaded" and type(payload) == "table" then
        if payload.ok then
            st.status = tostring(payload.message or "Audio downloaded into music library")
        else
            st.status = "Download failed: " .. tostring(payload.error or "yt-dlp error")
        end
        tntron.ui.toast(st.status)
        render()
    end
end

function on_action(_ctx, action_id, val)
    if action_id == "search" then
        local q = type(val) == "string" and (val:match("^%s*(.-)%s*$") or "") or ""
        if q == "" then
            tntron.ui.toast("Enter a YouTube search term or URL")
            return
        end
        st.query = q
        st.searching = true
        st.status = "Searching YouTube via yt-dlp..."
        tntron.net.yt_search(q)
    elseif action_id:sub(1, 5) == "play:" then
        local idx = tonumber(action_id:sub(6))
        local item = idx and st.results[idx]
        if item and item.url then
            st.status = "Resolving audio stream for " .. tostring(item.title) .. "..."
            tntron.net.yt_play(item.url, item.title)
        end
    elseif action_id:sub(1, 3) == "dl:" then
        local idx = tonumber(action_id:sub(4))
        local item = idx and st.results[idx]
        if item and item.url then
            st.status = "Downloading opus audio for " .. tostring(item.title) .. "..."
            tntron.net.yt_download(item.url)
        end
    end
    render()
end
