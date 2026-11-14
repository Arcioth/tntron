-- Theme Festival (ported from Openify theme-festival)

function on_load(_ctx)
    local themes = {
        {
            slug = "sakura", label = "Sakura",
            base = "#1a0f14", sidebar = "#1a0f14", main = "#211519",
            card = "#2d1c22", card_hover = "#3d2830",
            text = "#fce4ec", text_sub = "#c48b9f", accent = "#ffb7c5",
        },
        {
            slug = "ocean-deep", label = "Ocean Deep",
            base = "#020e18", sidebar = "#020e18", main = "#041825",
            card = "#082438", card_hover = "#0c3350",
            text = "#d0ecf8", text_sub = "#5a8ca8", accent = "#00b4d8",
        },
        {
            slug = "retrowave", label = "Retrowave",
            base = "#0b0014", sidebar = "#0b0014", main = "#10001e",
            card = "#1a0830", card_hover = "#261040",
            text = "#edc4ff", text_sub = "#9a5abf", accent = "#f72585",
        },
        {
            slug = "forest", label = "Forest",
            base = "#080f0b", sidebar = "#080f0b", main = "#0e1a13",
            card = "#14261b", card_hover = "#1c3425",
            text = "#d8f3dc", text_sub = "#74a383", accent = "#52b788",
        },
        {
            slug = "sunset-beach", label = "Sunset Beach",
            base = "#1a0e08", sidebar = "#1a0e08", main = "#24140c",
            card = "#321c10", card_hover = "#44281a",
            text = "#ffe0cc", text_sub = "#b87d5e", accent = "#ff6b35",
        },
        {
            slug = "lavender-dream", label = "Lavender Dream",
            base = "#12101a", sidebar = "#12101a", main = "#1a1724",
            card = "#242030", card_hover = "#302a40",
            text = "#e8e0f0", text_sub = "#9088a8", accent = "#b4a7d6",
        },
        {
            slug = "volcano", label = "Volcano",
            base = "#0a0000", sidebar = "#0a0000", main = "#140200",
            card = "#1e0500", card_hover = "#300a00",
            text = "#ffd6cc", text_sub = "#aa6050", accent = "#dc2f02",
        },
        {
            slug = "arctic", label = "Arctic",
            base = "#e8f0f8", sidebar = "#dce8f4", main = "#f0f6fc",
            card = "#ffffff", card_hover = "#e2ecf5",
            text = "#1a2a3a", text_sub = "#5a7a9a", accent = "#3a86c8",
        },
        {
            slug = "candy-pop", label = "Candy Pop",
            base = "#fff0f3", sidebar = "#ffe5ea", main = "#fff5f7",
            card = "#ffffff", card_hover = "#ffe0e8",
            text = "#3d1525", text_sub = "#a05070", accent = "#ff85a1",
        },
    }

    for _, t in ipairs(themes) do
        tntron.themes.register(t)
    end
end
