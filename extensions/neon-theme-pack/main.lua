-- Neon Theme Pack (ported from Openify sample-neon-theme)

function on_load(_ctx)
    local themes = {
        {
            slug = "neon-pink",
            label = "Neon Pink",
            base = "#0d0010",
            sidebar = "#0d0010",
            main = "#130018",
            card = "#1a0025",
            card_hover = "#2a0040",
            text = "#ffc0e0",
            text_sub = "#b06090",
            accent = "#ff69b4",
        },
        {
            slug = "neon-cyan",
            label = "Neon Cyan",
            base = "#000d0d",
            sidebar = "#000d0d",
            main = "#001318",
            card = "#001a25",
            card_hover = "#002a40",
            text = "#c0ffff",
            text_sub = "#609090",
            accent = "#00ffff",
        },
        {
            slug = "twin",
            label = "Twin",
            base = "#0a0612",
            sidebar = "#0e0818",
            main = "#110a1e",
            card = "#1a1030",
            card_hover = "#251845",
            text = "#e0d0ff",
            text_sub = "#8a70b0",
            accent = "#b48eff",
        },
        {
            slug = "vegza",
            label = "Vegza",
            base = "#060d05",
            sidebar = "#081008",
            main = "#0a140a",
            card = "#122012",
            card_hover = "#1a3018",
            text = "#d0ffc8",
            text_sub = "#6a9a60",
            accent = "#7cff6b",
        },
    }

    for _, t in ipairs(themes) do
        tntron.themes.register(t)
    end
end
