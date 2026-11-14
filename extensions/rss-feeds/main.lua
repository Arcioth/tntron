-- Podcast & RSS Audio Feeds (ported from Tunotron rss_feeds.lua)

local DEFAULT_FEEDS = {
    "https://feeds.npr.org/510289/podcast.xml",
    "https://feeds.simplecast.com/54nAGcIl",
}

local st = {
    feeds = {},
    current_feed = nil,
    loading = false,
    status = "Enter or select a podcast RSS URL to fetch episodes.",
}

local function render()
    local saved_btns = {}
    for i, url in ipairs(st.feeds) do
        local short = url:gsub("^https?://", "")
        if #short > 28 then short = short:sub(1, 25) .. "..." end
        table.insert(saved_btns, tntron.ui.button("fetch_saved:" .. tostring(i), short, {
            icon = "rss",
            active = st.current_feed and st.current_feed.url == url,
        }))
    end

    if st.current_feed and st.current_feed.title then
        tntron.ui.set_topbar_slot({
            text = "RSS: " .. st.current_feed.title,
            icon = "rss",
            accent = true,
        })
    else
        tntron.ui.clear_topbar_slot()
    end

    local widgets = {
        tntron.ui.input("rss_url", "Subscribe RSS URL", "", {
            placeholder = "https://example.com/podcast.xml",
            submit_action = "fetch_url",
            submit_label = st.loading and "Fetching..." or "Fetch",
        }),
        tntron.ui.heading("Subscribed Feeds"),
        tntron.ui.row(saved_btns),
        tntron.ui.text(st.status, true),
    }

    if st.current_feed and type(st.current_feed.episodes) == "table" then
        local eps = st.current_feed.episodes
        table.insert(widgets, tntron.ui.heading(string.format("%s (%d episodes)", st.current_feed.title or "Feed", #eps)))
        for i = 1, math.min(#eps, 12) do
            local ep = eps[i]
            table.insert(widgets, tntron.ui.row({
                tntron.ui.button("play_ep:" .. tostring(i), ep.title or "Episode", { icon = "play" }),
                tntron.ui.button("q_ep:" .. tostring(i), "+Queue", { icon = "plus" }),
            }))
        end
    end

    tntron.ui.register_sidebar({
        id = "rss-podcasts",
        label = "Podcasts",
        icon = "rss",
        widgets = widgets,
    })
end

function on_load(_ctx)
    local saved = tntron.storage.get("feeds", nil)
    if type(saved) == "table" and #saved > 0 then
        st.feeds = saved
    else
        st.feeds = DEFAULT_FEEDS
        tntron.storage.set("feeds", st.feeds)
    end
    render()
end

function on_event(_ctx, event_name, payload)
    if event_name == "rssFetched" and type(payload) == "table" then
        st.loading = false
        if payload.ok then
            st.current_feed = payload
            local count = type(payload.episodes) == "table" and #payload.episodes or 0
            st.status = string.format("Loaded %d playable audio episode(s) from %s", count, payload.title or payload.url or "")
            tntron.ui.toast("RSS loaded: " .. tostring(payload.title or "Podcast"))
        else
            st.status = "RSS error: " .. tostring(payload.error or "Unknown failure")
            tntron.ui.toast(st.status)
        end
        render()
    end
end

function on_action(_ctx, action_id, val)
    if action_id == "fetch_url" then
        local url = type(val) == "string" and (val:match("^%s*(.-)%s*$") or "") or ""
        if url == "" then
            tntron.ui.toast("Enter an RSS feed URL")
            return
        end
        local seen = false
        for _, u in ipairs(st.feeds) do if u == url then seen = true end end
        if not seen then
            table.insert(st.feeds, 1, url)
            tntron.storage.set("feeds", st.feeds)
        end
        st.loading = true
        st.status = "Fetching " .. url .. "..."
        tntron.net.fetch_rss(url)
    elseif action_id:sub(1, 12) == "fetch_saved:" then
        local idx = tonumber(action_id:sub(13))
        local url = idx and st.feeds[idx]
        if url then
            st.loading = true
            st.status = "Fetching " .. url .. "..."
            tntron.net.fetch_rss(url)
        end
    elseif action_id:sub(1, 8) == "play_ep:" then
        local idx = tonumber(action_id:sub(9))
        local ep = idx and st.current_feed and st.current_feed.episodes and st.current_feed.episodes[idx]
        if ep and ep.audio_url then
            tntron.playback.play_path(ep.audio_url)
            tntron.ui.toast("Streaming: " .. tostring(ep.title))
        end
    elseif action_id:sub(1, 5) == "q_ep:" then
        local idx = tonumber(action_id:sub(6))
        local ep = idx and st.current_feed and st.current_feed.episodes and st.current_feed.episodes[idx]
        if ep and ep.audio_url then
            tntron.queue.add(ep.audio_url)
            tntron.ui.toast("Queued episode")
        end
    end
    render()
end
