-- Better Extensions (ported from Openify better-extensions)

local function render()
    tntron.ui.register_settings({
        id = "better-extensions",
        title = "Extension Manager Quick Controls",
        widgets = {
            tntron.ui.row({
                tntron.ui.button("enable_all", "Enable All Extensions", { icon = "puzzle", accent = true }),
                tntron.ui.button("disable_all", "Disable Others", { icon = "close" }),
            }),
            tntron.ui.text("Search, Compact/Normal/Large card sizing, and Enable/Disable All controls are active in Settings.", true),
        },
    })
end

function on_load(_ctx)
    render()
end

function on_action(_ctx, action_id, _val)
    if action_id == "enable_all" then
        tntron.extensions.set_all(true)
    elseif action_id == "disable_all" then
        tntron.extensions.set_all(false)
    end
    render()
end
