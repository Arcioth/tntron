-- Folder-based Playlists Manager (Folders ARE Playlists)

local st = { selected_path = "", confirm_delete = false }

local function find_folder(folders, path)
    for _, f in ipairs(folders) do
        if f.path == path then return f end
    end
    return folders[1]
end

local function tracks_in_folder(all_tracks, _folder_name, folder_path)
    local out = {}
    local prefix = folder_path .. "/"
    for _, t in ipairs(all_tracks) do
        if type(t.path) == "string" and t.path:sub(1, #prefix) == prefix then
            table.insert(out, t)
        end
    end
    return out
end

local function render()
    local folders = tntron.library.folders() or {}
    local all = tntron.library.all_tracks() or {}
    local sel = find_folder(folders, st.selected_path)
    if sel then
        st.selected_path = sel.path
    else
        st.selected_path = ""
    end

    local names_row = {}
    for _, f in ipairs(folders) do
        local label = string.format("%s (%d)", f.name, tonumber(f.track_count) or 0)
        table.insert(names_row, tntron.ui.button("sel:" .. f.path, label, {
            icon = "folder",
            active = (sel ~= nil and f.path == sel.path),
        }))
    end

    local widgets = {
        tntron.ui.input("new_pl", "New Playlist (Folder)", "", {
            placeholder = "Playlist folder name...",
            submit_action = "create_pl",
            submit_label = "Create",
        }),
        tntron.ui.heading(string.format("Folder Playlists (%d)", #folders)),
    }

    if #names_row == 0 then
        table.insert(widgets, tntron.ui.text("No playlist folders in music root yet. Create one above.", true))
    else
        table.insert(widgets, tntron.ui.row(names_row))
    end

    if sel then
        local items = tracks_in_folder(all, sel.name, sel.path)
        table.insert(widgets, tntron.ui.heading(string.format("%s (%d cached / %d files)", sel.name, #items, tonumber(sel.track_count) or 0)))

        local actions = {
            tntron.ui.button("play_pl", "Play Playlist", { icon = "play", accent = true }),
            tntron.ui.button("add_cur", "Add Current Track", { icon = "plus" }),
            tntron.ui.button("rescan",  "Refresh", { icon = "refresh" }),
        }
        if st.confirm_delete then
            table.insert(actions, tntron.ui.button("confirm_del_pl", "Confirm Remove Folder", { icon = "close", accent = true }))
            table.insert(actions, tntron.ui.button("cancel_del_pl", "Cancel", {}))
        else
            table.insert(actions, tntron.ui.button("ask_del_pl", "Remove Playlist", { icon = "close" }))
        end
        table.insert(widgets, tntron.ui.row(actions))

        if #items == 0 then
            table.insert(widgets, tntron.ui.text("Open this folder in Library or play songs to populate track list.", true))
        else
            for i, item in ipairs(items) do
                table.insert(widgets, tntron.ui.row({
                    tntron.ui.button("play_path:" .. item.path, string.format("%d. %s - %s", i, item.title or "Track", item.artist or "Unknown"), { icon = "play" }),
                    tntron.ui.button("rm_file:" .. item.path, "Remove", { icon = "close" }),
                }))
            end
        end
    end

    tntron.ui.register_sidebar({
        id = "playlists-view",
        label = "Playlists",
        icon = "folder",
        widgets = widgets,
    })
end

function on_load(_ctx)
    st.selected_path = tntron.storage.get("selected_path", "")
    render()
end

function on_event(_ctx, _ev)
    render()
end

function on_action(_ctx, action_id, val)
    local folders = tntron.library.folders() or {}
    local sel = find_folder(folders, st.selected_path)

    if action_id == "create_pl" then
        local name = type(val) == "string" and (val:match("^%s*(.-)%s*$") or "") or ""
        if name ~= "" then
            tntron.fs.create_folder(name)
            st.confirm_delete = false
        end
    elseif action_id:sub(1, 4) == "sel:" then
        st.selected_path = action_id:sub(5)
        st.confirm_delete = false
        tntron.storage.set("selected_path", st.selected_path)
    elseif action_id == "add_cur" and sel then
        local pb = tntron.playback.get()
        if pb and pb.track_path ~= "" then
            tntron.fs.copy_to_folder(pb.track_path, sel.path)
        else
            tntron.ui.toast("Play a song first to add it to " .. sel.name)
        end
    elseif action_id == "play_pl" and sel then
        local all = tntron.library.all_tracks() or {}
        local items = tracks_in_folder(all, sel.name, sel.path)
        if #items > 0 then
            tntron.queue.clear()
            tntron.playback.play_path(items[1].path)
            for i = 2, #items do
                tntron.queue.add(items[i].path)
            end
            tntron.ui.toast(string.format("Playing playlist folder %s (%d)", sel.name, #items))
        else
            tntron.ui.toast("Open folder '" .. sel.name .. "' once to load its tracks")
        end
    elseif action_id == "ask_del_pl" then
        st.confirm_delete = true
    elseif action_id == "cancel_del_pl" then
        st.confirm_delete = false
    elseif action_id == "confirm_del_pl" and sel then
        st.confirm_delete = false
        tntron.fs.delete_folder(sel.path)
        st.selected_path = ""
    elseif action_id == "rescan" then
        tntron.library.rescan()
    elseif action_id:sub(1, 10) == "play_path:" then
        tntron.playback.play_path(action_id:sub(11))
    elseif action_id:sub(1, 8) == "rm_file:" then
        tntron.fs.delete_file(action_id:sub(9))
    end
    render()
end
