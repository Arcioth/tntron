-- Compact Menu (ported from Openify compact-menu)

local st = { compact = true }

local function sync()
    tntron.ui.set_compact_menu(st.compact)
    tntron.ui.clear_topbar_slot()
    tntron.ui.register_settings({
        id = "compact-menu",
        title = "Compact Menu Buttons",
        widgets = {
            tntron.ui.toggle("compact_on", "Use compact icon-button grid in sidebar navigation", st.compact),
            tntron.ui.text("Replaces long sidebar rows with compact 5-across icon buttons so Playlists stay visible.", true),
        },
    })
end

function on_load(_ctx)
    st.compact = (tntron.storage.get("compact", true) ~= false)
    sync()
end

function on_unload(_ctx)
    tntron.ui.set_compact_menu(false)
    tntron.ui.clear_topbar_slot()
end

function on_action(_ctx, action_id, val)
    if action_id == "toggle" then
        st.compact = not st.compact
    elseif action_id == "compact_on" then
        st.compact = (val == true)
    end
    tntron.storage.set("compact", st.compact)
    sync()
    tntron.ui.toast(st.compact and "Compact sidebar buttons ON" or "Standard sidebar list ON")
end
