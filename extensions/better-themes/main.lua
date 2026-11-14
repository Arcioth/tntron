-- Better Themes (Tunotron developer palettes + Openify searchable/categorized theme picker)

local BUILTINS = {
    { slug = "openify",  label = "Openify" },
    { slug = "midnight", label = "Midnight" },
    { slug = "aurora",   label = "Aurora" },
    { slug = "rose",     label = "Rose" },
}

local PALETTES = {
    {
        slug = "catppuccin", label = "Catppuccin",
        base = "#11111b", sidebar = "#181825", main = "#1e1e2e",
        card = "#24273a", card_hover = "#313244",
        text = "#cdd6f4", text_sub = "#a6adc8", accent = "#cba6f7",
    },
    {
        slug = "nord", label = "Nord",
        base = "#242933", sidebar = "#2e3440", main = "#2e3440",
        card = "#3b4252", card_hover = "#434c5e",
        text = "#eceff4", text_sub = "#d8dee9", accent = "#88c0d0",
    },
    {
        slug = "dracula", label = "Dracula",
        base = "#191a21", sidebar = "#21222c", main = "#282a36",
        card = "#343746", card_hover = "#44475a",
        text = "#f8f8f2", text_sub = "#9aa2c4", accent = "#bd93f9",
    },
    {
        slug = "gruvbox", label = "Gruvbox",
        base = "#1d2021", sidebar = "#282828", main = "#282828",
        card = "#32302f", card_hover = "#3c3836",
        text = "#ebdbb2", text_sub = "#a89984", accent = "#fabd2f",
    },
    {
        slug = "tokyo-night", label = "Tokyo Night",
        base = "#16161e", sidebar = "#1a1b26", main = "#1a1b26",
        card = "#24283b", card_hover = "#292e42",
        text = "#c0caf5", text_sub = "#7aa2f7", accent = "#7dcfff",
    },
    {
        slug = "monokai", label = "Monokai",
        base = "#1e1f1c", sidebar = "#272822", main = "#272822",
        card = "#32332c", card_hover = "#3e3d32",
        text = "#f8f8f2", text_sub = "#a6a28c", accent = "#a6e22e",
    },
}

local st = { selected = "openify", filter = "" }

local function matches_filter(label)
    if st.filter == "" then return true end
    return label:lower():find(st.filter:lower(), 1, true) ~= nil
end

local function render_settings()
    local builtin_btns = {}
    for _, t in ipairs(BUILTINS) do
        if matches_filter(t.label) then
            table.insert(builtin_btns, tntron.ui.button("use:" .. t.slug, t.label, {
                icon = "palette",
                active = (st.selected == t.slug),
            }))
        end
    end

    local dev_btns = {}
    for _, t in ipairs(PALETTES) do
        if matches_filter(t.label) then
            table.insert(dev_btns, tntron.ui.button("use:" .. t.slug, t.label, {
                icon = "palette",
                active = (st.selected == t.slug),
            }))
        end
    end

    local widgets = {
        tntron.ui.input("theme_filter", "Search Themes", st.filter, {
            placeholder = "Filter e.g. Nord, Sakura, Dracula...",
            submit_action = "apply_filter",
            submit_label = "Filter",
        }),
    }

    if #builtin_btns > 0 then
        table.insert(widgets, tntron.ui.text("Built-in Themes", true))
        table.insert(widgets, tntron.ui.row(builtin_btns))
    end
    if #dev_btns > 0 then
        table.insert(widgets, tntron.ui.text("Developer Palettes", true))
        table.insert(widgets, tntron.ui.row(dev_btns))
    end
    if #builtin_btns == 0 and #dev_btns == 0 then
        table.insert(widgets, tntron.ui.row({
            tntron.ui.text("No themes match '" .. st.filter .. "'.", true),
            tntron.ui.button("clear_filter", "Clear Filter", { icon = "close" }),
        }))
    elseif st.filter ~= "" then
        table.insert(widgets, tntron.ui.button("clear_filter", "Clear Filter", { icon = "close" }))
    end

    tntron.ui.register_settings({
        id = "better-themes",
        title = "Better Themes (Categorized Switcher)",
        widgets = widgets,
    })
end

function on_load(_ctx)
    for _, t in ipairs(PALETTES) do
        tntron.themes.register(t)
    end
    st.selected = tntron.storage.get("selected_slug", "openify")
    render_settings()
end

function on_action(_ctx, action_id, val)
    if action_id == "apply_filter" then
        st.filter = type(val) == "string" and val or ""
    elseif action_id == "clear_filter" then
        st.filter = ""
    else
        local slug = action_id:match("^use:(.+)$")
        if slug then
            st.selected = slug
            tntron.themes.set_active(slug)
            tntron.storage.set("selected_slug", slug)
            tntron.ui.toast("Theme switched: " .. slug)
        end
    end
    render_settings()
end
