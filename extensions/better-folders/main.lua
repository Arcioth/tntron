-- Better Folders & Audit (Openify better-folders + Tunotron better_folders.lua)

local st = { draft = "", audit_report = nil, quick_tracks = {} }

local function basename(path)
    return path:match("([^/]+)$") or path
end

local function run_duplicate_audit(tracks)
    local seen = {}
    local dups = {}
    for _, t in ipairs(tracks) do
        local name = basename(t.path):lower()
        if seen[name] then
            table.insert(dups, name)
        else
            seen[name] = true
        end
    end
    if #dups == 0 then
        return "Audit clean: 0 duplicate filenames detected across " .. tostring(#tracks) .. " tracks."
    end
    return string.format("Found %d duplicate filename(s): %s", #dups, table.concat(dups, ", "))
end

local function render()
    local cur = tntron.library.current_folder() or { path = "", tracks = {} }
    local tracks = cur.tracks or {}
    local all = tntron.library.all_tracks() or {}

    local widgets = {
        tntron.ui.heading("Directory Operations"),
        tntron.ui.text("Current folder: " .. tostring(cur.path), true),
        tntron.ui.input("folder_name", "New Subfolder", st.draft, {
            placeholder = "Enter subfolder name...",
            submit_action = "create_folder",
            submit_label = "Create",
        }),
        tntron.ui.row({
            tntron.ui.button("audit", "Audit Duplicate Filenames", { icon = "search", accent = true }),
            tntron.ui.button("rescan", "Rescan Directory", { icon = "refresh" }),
        }),
    }

    if st.audit_report then
        table.insert(widgets, tntron.ui.text(st.audit_report, false))
    end

    table.insert(widgets, tntron.ui.heading(string.format("Tracks in Folder (%d)", #tracks)))
    st.quick_tracks = {}
    for i = 1, math.min(#tracks, 12) do
        local t = tracks[i]
        st.quick_tracks[i] = t.path
        table.insert(widgets, tntron.ui.button(
            "play:" .. tostring(i),
            string.format("%s — %s", t.title, t.artist),
            { icon = "folder" }
        ))
    end

    tntron.ui.register_sidebar({
        id = "better-folders",
        label = "Folders+",
        icon = "folder",
        widgets = widgets,
    })
end

function on_load(_ctx)
    render()
end

function on_action(_ctx, action_id, val)
    if action_id == "create_folder" then
        local name = type(val) == "string" and val or ""
        name = name:match("^%s*(.-)%s*$") or ""
        if name ~= "" then
            tntron.fs.create_folder(name)
            st.draft = ""
        else
            tntron.ui.toast("Enter a folder name first")
        end
    elseif action_id == "audit" then
        local all = tntron.library.all_tracks() or {}
        st.audit_report = run_duplicate_audit(all)
        tntron.ui.toast("Duplicate audit finished")
    elseif action_id == "rescan" then
        tntron.library.rescan()
        tntron.ui.toast("Library rescan started")
    elseif action_id:sub(1, 5) == "play:" then
        local idx = tonumber(action_id:sub(6))
        local p = idx and st.quick_tracks[idx]
        if p then tntron.playback.play_path(p) end
    end
    render()
end
