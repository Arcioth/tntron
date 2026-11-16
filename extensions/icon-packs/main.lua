-- Icon Packs & Studio (12 bundled packs + TTF/OTF font importing + quick glyph swatches + per-slot overrides)

local PACKS = {
    { id = "nerd",        label = "Nerd Classic" },
    { id = "material",    label = "Material" },
    { id = "octicons",    label = "Octicons" },
    { id = "codicons",    label = "Codicons" },
    { id = "fontawesome", label = "FontAwesome" },
    { id = "weather",     label = "Weather" },
    { id = "emoji",       label = "Emoji" },
    { id = "arcade",      label = "Arcade" },
    { id = "kawaii",      label = "Kawaii" },
    { id = "cyber",       label = "Cyber" },
    { id = "minimal",     label = "Minimal" },
    { id = "ascii",       label = "ASCII CLI" },
}

local QUICK_GLYPHS = {
    "★", "♡", "✿", "♪", "♫", "✦", "⚡", "❄",
    "🔥", "🎧", "💎", "🌊", "🌸", "⌘", "▶", "❚❚",
    "◉", "🏠", "⚙️", "🎨", "▶▶", "◀◀", "🔀", "🔁",
}

local SLOTS = {
    "home", "settings", "library", "music", "disc", "folder",
    "play", "pause", "prev", "next", "shuffle", "repeat", "repeat_once",
    "volume", "mute", "queue", "bars", "star", "star_outline", "search",
    "lyrics", "wave", "palette", "sliders", "headphones", "keyboard",
    "bell", "clock", "download", "rss", "globe", "layers",
    "chart", "gauge", "plus", "close", "refresh", "arrow_up", "alert", "bookmark",
}

local DEFAULTS = {
    pack = "nerd",
    scale = 1.0,
    tint_mode = "theme",
    custom_hex = "",
    overrides = {},
    font_path = "",
    font_family = "",
}

local st = {
    cfg = {},
    selected_slot = "play",
    glyph_draft = "",
    hex_draft = "",
    font_draft = "",
    family_draft = "",
}

local function copy_cfg(src)
    local out = {
        pack = (type(src) == "table" and src.pack) or DEFAULTS.pack,
        scale = (type(src) == "table" and tonumber(src.scale)) or DEFAULTS.scale,
        tint_mode = (type(src) == "table" and src.tint_mode) or DEFAULTS.tint_mode,
        custom_hex = (type(src) == "table" and src.custom_hex) or "",
        font_path = (type(src) == "table" and src.font_path) or "",
        font_family = (type(src) == "table" and src.font_family) or "",
        overrides = {},
    }
    if type(src) == "table" and type(src.overrides) == "table" then
        for k, v in pairs(src.overrides) do
            out.overrides[tostring(k)] = tostring(v)
        end
    end
    return out
end

local function push_cfg()
    tntron.storage.set("icon_pack_cfg", st.cfg)
    tntron.ui.set_icon_config(st.cfg)
end

local function count_overrides()
    local n = 0
    for _ in pairs(st.cfg.overrides) do n = n + 1 end
    return n
end

local function render()
    local pack_btns = {}
    for _, p in ipairs(PACKS) do
        table.insert(pack_btns, tntron.ui.button("pack:" .. p.id, p.label, { active = st.cfg.pack == p.id }))
    end

    local slot_btns = {}
    for _, s in ipairs(SLOTS) do
        local has_ov = st.cfg.overrides[s] ~= nil
        local lbl = has_ov and (s .. " *") or s
        table.insert(slot_btns, tntron.ui.button("slot:" .. s, lbl, {
            active = st.selected_slot == s,
            accent = has_ov,
        }))
    end

    local swatch_btns = {}
    for _, g in ipairs(QUICK_GLYPHS) do
        table.insert(swatch_btns, tntron.ui.button("qgl:" .. g, g, {}))
    end

    local cur_ov = st.cfg.overrides[st.selected_slot]
    local font_status = (st.cfg.font_path and st.cfg.font_path ~= "")
        and ("Imported Font: " .. st.cfg.font_path)
        or "Built-in NerdFontPropo.ttf"
    local widgets = {
        tntron.ui.row({
            tntron.ui.badge("Active Pack: " .. st.cfg.pack:upper(), { icon = st.selected_slot, accent = true }),
            tntron.ui.badge(string.format("%d custom overrides", count_overrides()), { icon = "sliders" }),
            tntron.ui.button("reset_all", "Reset All", { icon = "refresh" }),
        }),
        tntron.ui.heading("Bundled Icon Packs (12 Packs)"),
        tntron.ui.row(pack_btns),

        tntron.ui.heading("Import Custom Icon Font (.ttf / .otf)"),
        tntron.ui.text(font_status .. " — Import any icon font (Nerd Font, IcoMoon, RemixIcon, Material, Feather) and enter its codepoints below.", true),
        tntron.ui.input("font_path", "Font File Path (~/.local/share/fonts/custom.ttf)", st.font_draft, {
            placeholder = "/usr/share/fonts/.../Symbols.ttf or ~/Downloads/icons.ttf",
            submit_action = "import_font",
            submit_label = "Import Font",
        }),
        tntron.ui.input("font_family", "Optional Font Family Override (auto-detected if blank)", st.family_draft, {
            placeholder = "Symbols Nerd Font Mono",
            submit_action = "set_family",
            submit_label = "Set Family",
        }),
        tntron.ui.row({
            tntron.ui.button("clear_font", "Revert to Built-in Font", { icon = "close" }),
        }),

        tntron.ui.heading("Size & Tint"),
        tntron.ui.slider("scale", "Icon Scale", 0.75, 1.45, 0.05, st.cfg.scale, "x"),
        tntron.ui.row({
            tntron.ui.button("tint:theme",  "Theme Colors", { active = st.cfg.tint_mode == "theme" }),
            tntron.ui.button("tint:custom", "Custom Tint",  { active = st.cfg.tint_mode == "custom" }),
        }),
        tntron.ui.input("custom_hex", "Custom Icon Color (#RRGGBB)", st.hex_draft, {
            placeholder = "#38bdf8",
            submit_action = "apply_hex",
            submit_label = "Set Color",
        }),

        tntron.ui.heading("Customize Any Icon Slot"),
        tntron.ui.text(string.format(
            "Selected slot: '%s'%s. Pick a bundled glyph below or type any glyph, emoji, ASCII, or hex codepoint (U+f005 / f005).",
            st.selected_slot,
            cur_ov and (" (override: " .. cur_ov .. ")") or ""
        ), true),
        tntron.ui.row(swatch_btns),
        tntron.ui.input("glyph_input", "Glyph for '" .. st.selected_slot .. "'", st.glyph_draft, {
            placeholder = "★ or |> or U+f005",
            submit_action = "set_glyph",
            submit_label = "Apply Icon",
        }),
        tntron.ui.row({
            tntron.ui.button("clear_slot", "Clear '" .. st.selected_slot .. "' Override", { icon = "close" }),
            tntron.ui.button("clear_overrides", "Clear All Overrides"),
        }),
        tntron.ui.row(slot_btns),
    }

    tntron.ui.register_sidebar({
        id = "icon-packs",
        label = "Icons",
        icon = "star",
        widgets = widgets,
    })
end

function on_load(_ctx)
    st.cfg = copy_cfg(tntron.storage.get("icon_pack_cfg", nil))
    st.hex_draft = st.cfg.custom_hex or ""
    st.font_draft = st.cfg.font_path or ""
    st.family_draft = st.cfg.font_family or ""
    st.glyph_draft = st.cfg.overrides[st.selected_slot] or ""
    push_cfg()
    render()
end

function on_unload(_ctx)
    tntron.ui.set_icon_config(copy_cfg(nil))
end

function on_action(_ctx, action_id, val)
    if action_id:sub(1, 5) == "pack:" then
        st.cfg.pack = action_id:sub(6)
        push_cfg()
        tntron.ui.toast("Icon pack: " .. st.cfg.pack)
    elseif action_id == "scale" then
        st.cfg.scale = tonumber(val) or 1.0
        push_cfg()
    elseif action_id:sub(1, 5) == "tint:" then
        st.cfg.tint_mode = action_id:sub(6)
        push_cfg()
    elseif action_id == "apply_hex" then
        local clean = tostring(val or st.hex_draft or ""):gsub("%s+", "")
        if clean ~= "" and clean:sub(1, 1) ~= "#" then
            clean = "#" .. clean
        end
        st.hex_draft = clean
        st.cfg.custom_hex = clean
        st.cfg.tint_mode = clean ~= "" and "custom" or "theme"
        push_cfg()
    elseif action_id:sub(1, 5) == "slot:" then
        st.selected_slot = action_id:sub(6)
        st.glyph_draft = st.cfg.overrides[st.selected_slot] or ""
    elseif action_id == "set_glyph" then
        local g = tostring(val or st.glyph_draft or ""):match("^%s*(.-)%s*$")
        if g ~= "" then
            st.glyph_draft = g
            st.cfg.overrides[st.selected_slot] = g
            push_cfg()
            tntron.ui.toast("Updated '" .. st.selected_slot .. "' icon")
        end
    elseif action_id == "import_font" then
        local p = tostring(val or st.font_draft or ""):match("^%s*(.-)%s*$")
        st.font_draft = p
        st.cfg.font_path = p
        push_cfg()
        tntron.ui.toast(p ~= "" and ("Importing font: " .. p) or "Cleared custom font")
    elseif action_id == "set_family" then
        local f = tostring(val or st.family_draft or ""):match("^%s*(.-)%s*$")
        st.family_draft = f
        st.cfg.font_family = f
        push_cfg()
    elseif action_id == "clear_font" then
        st.font_draft = ""
        st.family_draft = ""
        st.cfg.font_path = ""
        st.cfg.font_family = ""
        push_cfg()
        tntron.ui.toast("Reverted to built-in icon font")
    elseif action_id:sub(1, 4) == "qgl:" then
        local g = action_id:sub(5)
        st.glyph_draft = g
        st.cfg.overrides[st.selected_slot] = g
        push_cfg()
        tntron.ui.toast("Applied '" .. g .. "' -> " .. st.selected_slot)
    elseif action_id == "clear_slot" then
        st.cfg.overrides[st.selected_slot] = nil
        st.glyph_draft = ""
        push_cfg()
    elseif action_id == "clear_overrides" then
        st.cfg.overrides = {}
        st.glyph_draft = ""
        push_cfg()
    elseif action_id == "reset_all" then
        st.cfg = copy_cfg(nil)
        st.hex_draft = ""
        st.glyph_draft = ""
        st.font_draft = ""
        st.family_draft = ""
        push_cfg()
        tntron.ui.toast("Reset icon packs to default")
    end
    render()
end
