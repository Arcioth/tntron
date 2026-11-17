use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use mlua::{Lua, LuaSerdeExt, StdLib, Table, Value as LuaValue};

/// Converts any `serde_json::Value` to Lua without emitting `lua.null` lightuserdata.
/// Null values map cleanly to `nil`, so idiomatic Lua `if obj.field then` checks never trap.
fn json_to_lua(lua: &Lua, val: &serde_json::Value) -> mlua::Result<LuaValue> {
    match val {
        serde_json::Value::Null => Ok(LuaValue::Nil),
        serde_json::Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(LuaValue::Integer(i))
            } else if let Some(u) = n.as_u64() {
                if let Ok(i) = i64::try_from(u) {
                    Ok(LuaValue::Integer(i))
                } else {
                    Ok(LuaValue::Number(u as f64))
                }
            } else {
                Ok(LuaValue::Number(n.as_f64().unwrap_or(0.0)))
            }
        }
        serde_json::Value::String(s) => Ok(LuaValue::String(lua.create_string(s)?)),
        serde_json::Value::Array(arr) => {
            let tbl = lua.create_table_with_capacity(arr.len(), 0)?;
            for (i, item) in arr.iter().enumerate() {
                tbl.set(i + 1, json_to_lua(lua, item)?)?;
            }
            Ok(LuaValue::Table(tbl))
        }
        serde_json::Value::Object(map) => {
            let tbl = lua.create_table_with_capacity(0, map.len())?;
            for (k, v) in map {
                if !v.is_null() {
                    tbl.set(k.as_str(), json_to_lua(lua, v)?)?;
                }
            }
            Ok(LuaValue::Table(tbl))
        }
    }
}

fn ser_to_lua<T: serde::Serialize>(lua: &Lua, val: &T) -> mlua::Result<LuaValue> {
    let jv = serde_json::to_value(val)
        .map_err(|e| mlua::Error::RuntimeError(format!("serialize to json: {e}")))?;
    json_to_lua(lua, &jv)
}

use crate::ext_api::{
    ExtHostCommand, ExtLibraryCtx, ExtPlaybackCtx, ExtRegisteredTheme, ExtSettingsPanel,
    ExtSidebarTab, ExtSlotBadge, ExtensionCardInfo, ExtensionManifest, API_VERSION,
    VALID_PERMISSIONS,
};
use crate::ipc::McBridgeHandle;

#[derive(Default)]
struct LuaBridgeState {
    ext_id: String,
    permissions: HashSet<String>,
    playback: Option<ExtPlaybackCtx>,
    library: ExtLibraryCtx,
    storage: HashMap<String, serde_json::Value>,
    pending_cmds: Vec<ExtHostCommand>,
    mc_bridge: Option<McBridgeHandle>,
}

impl LuaBridgeState {
    fn has_perm(&self, perm: &str) -> bool {
        self.permissions.contains(perm)
    }

    fn require_perm(&self, perm: &str) -> mlua::Result<()> {
        if self.has_perm(perm) {
            Ok(())
        } else {
            Err(mlua::Error::RuntimeError(format!(
                "Extension '{}' lacks permission '{}'",
                self.ext_id, perm
            )))
        }
    }
}

type SharedBridge = Arc<Mutex<LuaBridgeState>>;

struct LoadedExt {
    manifest: ExtensionManifest,
    script_src: String,
    lua: Option<Lua>,
    bridge: Option<SharedBridge>,
    enabled: bool,
    error: Option<String>,
}

pub struct ExtensionHost {
    exts: Vec<LoadedExt>,
    pub sidebar_tabs: Vec<ExtSidebarTab>,
    pub settings_panels: Vec<ExtSettingsPanel>,
    pub themes: Vec<ExtRegisteredTheme>,
    pub topbar_slots: Vec<ExtSlotBadge>,
    pub player_widgets: Vec<ExtSlotBadge>,
    pub compact_menu: bool,
    compact_menu_owner: Option<String>,
    mc_bridge: Option<McBridgeHandle>,
}

impl Default for ExtensionHost {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtensionHost {
    pub fn new() -> Self {
        Self {
            exts: Vec::new(),
            sidebar_tabs: Vec::new(),
            settings_panels: Vec::new(),
            themes: Vec::new(),
            topbar_slots: Vec::new(),
            player_widgets: Vec::new(),
            compact_menu: false,
            compact_menu_owner: None,
            mc_bridge: None,
        }
    }

    pub fn set_mc_bridge(&mut self, bridge: McBridgeHandle) {
        self.mc_bridge = Some(bridge);
    }

    pub fn validate_manifest(m: &ExtensionManifest) -> Result<(), String> {
        if m.id.trim().is_empty()
            || !m
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        {
            return Err(format!("Invalid extension id '{}'", m.id));
        }
        if m.name.trim().is_empty() || m.main.trim().is_empty() {
            return Err("Manifest requires non-empty name and main".into());
        }
        let main_path = Path::new(&m.main);
        if main_path.is_absolute()
            || m.main.contains("..")
            || main_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(format!("Manifest main '{}' escapes extension directory", m.main));
        }
        for p in &m.permissions {
            if !VALID_PERMISSIONS.contains(&p.as_str()) {
                return Err(format!("Unknown permission '{p}'"));
            }
        }
        if m.requires.contains(&m.id) {
            return Err("Extension cannot require itself".into());
        }
        Ok(())
    }

    /// Ensures `<extensions_dir>/_template/` exists with starter files and `API.md` reference.
    pub fn ensure_template_dir(dir: &Path) {
        let tpl = dir.join("_template");
        if std::fs::create_dir_all(&tpl).is_err() {
            return;
        }

        let manifest_path = tpl.join("manifest.json");
        if !manifest_path.exists() {
            let manifest = r#"{
  "id": "you.sample-extension",
  "name": "Sample SDK Extension",
  "version": "1.0.0",
  "api_version": "1.0",
  "description": "Demonstrates the tntron Lua SDK (copy folder without '_' prefix to activate).",
  "author": "You",
  "main": "main.lua",
  "permissions": ["playback:read", "ui:sidebar", "ui:settings", "storage", "notify"],
  "requires": []
}
"#;
            let _ = std::fs::write(manifest_path, manifest);
        }

        let main_path = tpl.join("main.lua");
        if !main_path.exists() {
            let script = r#"-- tntron Extension API v1.0 starter
-- Lifecycle hooks: on_load(ctx), on_unload(ctx), on_event(ctx, name, payload), on_tick(ctx), on_action(ctx, id, value)

local function render(clicks)
    tntron.ui.register_sidebar({
        id = "sample-tab",
        label = "Sample Plugin",
        icon = "puzzle",
        widgets = {
            tntron.ui.heading("Sample Lua Extension"),
            tntron.ui.text("Stored click count: " .. tostring(clicks), true),
            tntron.ui.row({
                tntron.ui.button("inc", "Increment", { icon = "plus", accent = true }),
                tntron.ui.button("reset", "Reset", { icon = "refresh" }),
            }),
        },
    })
end

function on_load(_ctx)
    local clicks = tntron.storage.get("clicks", 0)
    render(clicks)
end

function on_action(_ctx, action_id, _val)
    local clicks = tntron.storage.get("clicks", 0)
    if action_id == "inc" then
        clicks = clicks + 1
        tntron.storage.set("clicks", clicks)
        render(clicks)
        tntron.ui.toast("Clicks: " .. tostring(clicks))
    elseif action_id == "reset" then
        tntron.storage.set("clicks", 0)
        render(0)
        tntron.ui.toast("Counter reset")
    end
end
"#;
            let _ = std::fs::write(main_path, script);
        }

        let api_doc_path = tpl.join("API.md");
        let doc = r#"# tntron Extension API (v1.0)

Place each extension in its own folder inside `~/.config/tntron/extensions/<ext-name>/` containing `manifest.json` + `main.lua`. Folders starting with `_` or `.` are skipped.

## Permissions (default-deny)
- `playback:read` — `tntron.playback.get()` (`playing`, `paused`, `stopped`, `muted`, `position_sec`, `duration_sec`, `volume`, `speed`, `shuffle`, `loop_mode`, `codec`, `title`, `artist`, `album`, `track_path`)
- `playback:control` — `toggle()`, `play()`, `pause()`, `next()`, `prev()`, `play_path(path_or_url)`, `set_mute(bool)`, `seek(sec)`, `seek_relative(delta)`, `set_volume(0..100)`, `set_speed(0.25..3.0)`
- `queue:control` — `tntron.queue.list()`, `tntron.queue.add(path_or_url)`, `tntron.queue.clear()`
- `library:read` — `tntron.library.current_folder()`, `all_tracks()`, `folders()`, `favorites()`, `rescan()`, `tntron.lyrics.load(path, title, artist, dur)`
- `audio:dsp` — `tntron.audio.set_filter(mpv_af)`, `tntron.audio.clear_filter()` (per-extension composed stage)
- `fs:write` — `tntron.fs.create_folder(name)`, `delete_folder(path)`, `copy_to_folder(track, folder)`, `delete_file(path)` (jailed inside music root)
- `ui:sidebar` — `tntron.ui.register_sidebar({ id, label, icon, widgets })`, `unregister_sidebar(id)`, `set_compact_menu(bool)`
- `ui:settings` — `tntron.ui.register_settings({ id?, title, widgets })`, `set_density("Compact"|"Normal"|"Large")`, `set_layout("table"|"cards"|"gallery"|"minimal")`, `set_layout_config(patch)`, `set_reactive_config(patch)`, `set_icon_config(patch)`, `set_button_config(patch)`, `set_logo_config(patch)`, `set_transparency_config(patch)`, `set_popup_config(patch)`, `copy_clipboard(str)`, `request_clipboard_paste()`, `set_compact_menu(bool)`, `tntron.extensions.set_all(bool)`
- `ui:topbar` — `tntron.ui.set_topbar_slot({ text, icon?, accent?, action_id? })`, `clear_topbar_slot()`
- `ui:player_widget` — `tntron.ui.set_player_widget({ text, icon?, accent?, action_id? })`, `clear_player_widget()`
- `themes:register` — `tntron.themes.register({ slug, label, base, sidebar, main, card, card_hover, text, text_sub, accent, logo_color? })`, `tntron.themes.set_active(slug)`, `tntron.ui.set_logo_config({ enabled?, mode?, custom_hex?, custom_path?, frame_style?, size_px?, beat_pulse? })`
- `storage` — `tntron.storage.get(key, default)`, `tntron.storage.set(key, val)`
- `notify` — `tntron.ui.toast(message)`, `tntron.notify.desktop(summary, body)`
- `network` — `tntron.net.fetch_rss(url)`, `tntron.net.yt_search(query)`, `tntron.net.yt_play(url, title?)`, `tntron.net.yt_download(url)` (requires `fs:write` too)

## Fonticon Names
`home`, `settings`, `library`, `music`, `disc`, `search`, `folder`, `star`, `play`, `pause`, `prev`, `next`, `shuffle`, `repeat`, `volume`, `mute`, `queue`, `plus`, `close`, `refresh`, `sliders`, `puzzle`, `clock`, `alert`, `bookmark`, `bell`, `download`, `rss`, `lyrics`, `wave`, `palette`, `globe`, `headphones`, `layers`, `chart`

## UI Builders (`tntron.ui`)
`heading(text)`, `text(text, muted?)`, `badge(text, opts?)`, `row(children)`, `button(id, label, { icon?, accent?, active? })`, `toggle(id, label, value)`, `slider(id, label, min, max, step, value, unit?)`, `progress(label, value, max)`, `input(id, label, value?, { placeholder?, submit_action?, submit_label? })`, `visualizer(mode, { playing?, volume?, speed? })`

## Lifecycle Hooks
`on_load(ctx)`, `on_unload(ctx)`, `on_tick(ctx)` (1 Hz), `on_action(ctx, action_id, value)`, `on_event(ctx, event_name, payload)` (`"trackChange"`, `"playbackStateChange"`, `"lyricsLoaded"`, `"rssFetched"`, `"ytSearch"`, `"ytPlaying"`, `"ytDownloaded"`)
"#;
        let _ = std::fs::write(api_doc_path, doc);
    }

    /// Scans `dir` for `<ext>/manifest.json` + `<main>.lua`, skipping `_` / `.` folders.
    pub fn discover(&mut self, dir: &Path) {
        self.exts.clear();
        self.clear_all_registrations();
        Self::ensure_template_dir(dir);

        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        let mut folders: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                !name.starts_with('_') && !name.starts_with('.') && e.path().is_dir()
            })
            .collect();
        folders.sort_by_key(|e| e.file_name());

        for entry in folders {
            let folder = entry.path();
            let manifest_path = folder.join("manifest.json");
            let Ok(raw_manifest) = std::fs::read_to_string(&manifest_path) else {
                continue;
            };
            let manifest: ExtensionManifest = match serde_json::from_str(&raw_manifest) {
                Ok(m) => m,
                Err(e) => {
                    let fallback_id = folder
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "invalid".into());
                    self.exts.push(LoadedExt {
                        manifest: ExtensionManifest {
                            id: fallback_id.clone(),
                            name: fallback_id,
                            version: "0.0.0".into(),
                            description: "Manifest parse failed".into(),
                            author: String::new(),
                            main: "main.lua".into(),
                            api_version: API_VERSION.into(),
                            permissions: Vec::new(),
                            requires: Vec::new(),
                        },
                        script_src: String::new(),
                        lua: None,
                        bridge: None,
                        enabled: false,
                        error: Some(format!("Invalid manifest.json: {e}")),
                    });
                    continue;
                }
            };

            if let Err(err) = Self::validate_manifest(&manifest) {
                self.exts.push(LoadedExt {
                    manifest,
                    script_src: String::new(),
                    lua: None,
                    bridge: None,
                    enabled: false,
                    error: Some(err),
                });
                continue;
            }

            let script_path = folder.join(&manifest.main);
            let canon_folder = folder.canonicalize().unwrap_or_else(|_| folder.clone());
            let canon_script = script_path.canonicalize();
            let inside_folder = canon_script
                .as_ref()
                .is_ok_and(|p| p.starts_with(&canon_folder) && p.is_file());
            if !inside_folder {
                self.exts.push(LoadedExt {
                    manifest,
                    script_src: String::new(),
                    lua: None,
                    bridge: None,
                    enabled: false,
                    error: Some("Entry script not found inside extension directory".into()),
                });
                continue;
            }

            match std::fs::read_to_string(&script_path) {
                Ok(script_src) => {
                    self.exts.push(LoadedExt {
                        manifest,
                        script_src,
                        lua: None,
                        bridge: None,
                        enabled: false,
                        error: None,
                    });
                }
                Err(e) => {
                    self.exts.push(LoadedExt {
                        manifest,
                        script_src: String::new(),
                        lua: None,
                        bridge: None,
                        enabled: false,
                        error: Some(format!("Cannot read {}: {e}", script_path.display())),
                    });
                }
            }
        }
    }

    fn clear_all_registrations(&mut self) {
        self.sidebar_tabs.clear();
        self.settings_panels.clear();
        self.themes.clear();
        self.topbar_slots.clear();
        self.player_widgets.clear();
        self.compact_menu = false;
        self.compact_menu_owner = None;
    }

    fn clear_for(&mut self, ext_id: &str) {
        self.sidebar_tabs.retain(|t| t.ext_id != ext_id);
        self.settings_panels.retain(|p| p.ext_id != ext_id);
        self.themes.retain(|t| t.ext_id != ext_id);
        self.topbar_slots.retain(|s| s.ext_id != ext_id);
        self.player_widgets.retain(|w| w.ext_id != ext_id);
        if self.compact_menu_owner.as_deref() == Some(ext_id) {
            self.compact_menu = false;
            self.compact_menu_owner = None;
        }
    }

    pub fn enabled_ids(&self) -> Vec<String> {
        self.exts
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.manifest.id.clone())
            .collect()
    }

    pub fn has_enabled(&self) -> bool {
        self.exts.iter().any(|e| e.enabled)
    }

    pub fn used_by(&self, id: &str) -> Vec<String> {
        self.exts
            .iter()
            .filter(|e| e.manifest.requires.iter().any(|r| r == id))
            .map(|e| e.manifest.id.clone())
            .collect()
    }

    pub fn cards(&self) -> Vec<ExtensionCardInfo> {
        self.exts
            .iter()
            .map(|e| ExtensionCardInfo {
                id: e.manifest.id.clone(),
                name: e.manifest.name.clone(),
                version: e.manifest.version.clone(),
                description: e.manifest.description.clone(),
                author: e.manifest.author.clone(),
                permissions: e.manifest.permissions.clone(),
                requires: e.manifest.requires.clone(),
                used_by: self.used_by(&e.manifest.id),
                enabled: e.enabled,
                error: e.error.clone(),
            })
            .collect()
    }

    pub fn enable_saved(
        &mut self,
        enabled_ids: &[String],
        playback: &ExtPlaybackCtx,
        library: &ExtLibraryCtx,
        storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
    ) -> Vec<(String, ExtHostCommand)> {
        let mut all_cmds = Vec::new();
        for id in enabled_ids {
            all_cmds.extend(self.enable(id, playback, library, storage_map));
        }
        all_cmds
    }

    pub fn toggle(
        &mut self,
        id: &str,
        playback: &ExtPlaybackCtx,
        library: &ExtLibraryCtx,
        storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
    ) -> (bool, Vec<String>, Vec<(String, ExtHostCommand)>) {
        let currently = self
            .exts
            .iter()
            .find(|e| e.manifest.id == id)
            .map(|e| e.enabled)
            .unwrap_or(false);
        if currently {
            let disabled = self.disable(id, playback, library, storage_map);
            (false, disabled, Vec::new())
        } else {
            let cmds = self.enable(id, playback, library, storage_map);
            let now_on = self
                .exts
                .iter()
                .find(|e| e.manifest.id == id)
                .map(|e| e.enabled)
                .unwrap_or(false);
            (now_on, Vec::new(), cmds)
        }
    }

    pub fn enable(
        &mut self,
        id: &str,
        playback: &ExtPlaybackCtx,
        library: &ExtLibraryCtx,
        storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
    ) -> Vec<(String, ExtHostCommand)> {
        let mut visited = HashSet::new();
        let mut out = Vec::new();
        self.enable_dfs(id, &mut visited, playback, library, storage_map, &mut out);
        out
    }

    fn enable_dfs(
        &mut self,
        id: &str,
        visited: &mut HashSet<String>,
        playback: &ExtPlaybackCtx,
        library: &ExtLibraryCtx,
        storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
        out: &mut Vec<(String, ExtHostCommand)>,
    ) {
        if !visited.insert(id.to_string()) {
            return;
        }
        let Some(idx) = self.exts.iter().position(|e| e.manifest.id == id) else {
            return;
        };
        let reqs = self.exts[idx].manifest.requires.clone();
        for req in &reqs {
            if !self.exts.iter().any(|e| e.manifest.id == *req) {
                self.exts[idx].error = Some(format!("Missing dependency '{req}'"));
                return;
            }
            self.enable_dfs(req, visited, playback, library, storage_map, out);
            let dep_ok = self
                .exts
                .iter()
                .find(|e| e.manifest.id == *req)
                .is_some_and(|e| e.enabled);
            if !dep_ok {
                self.exts[idx].error = Some(format!("Dependency '{req}' failed to load"));
                return;
            }
        }

        if self.exts[idx].enabled || self.exts[idx].script_src.is_empty() {
            return;
        }

        let bridge = Arc::new(Mutex::new(LuaBridgeState {
            ext_id: self.exts[idx].manifest.id.clone(),
            permissions: self.exts[idx].manifest.permissions.iter().cloned().collect(),
            playback: if self.exts[idx].manifest.permissions.iter().any(|p| p == "playback:read") {
                Some(playback.clone())
            } else {
                None
            },
            library: if self.exts[idx].manifest.permissions.iter().any(|p| p == "library:read") {
                library.clone()
            } else {
                ExtLibraryCtx::default()
            },
            storage: storage_map.get(id).cloned().unwrap_or_default(),
            pending_cmds: Vec::new(),
            mc_bridge: self.mc_bridge.clone(),
        }));

        let lua = match create_sandboxed_lua(&self.exts[idx].script_src, &bridge) {
            Ok(l) => l,
            Err(e) => {
                self.exts[idx].error = Some(e);
                return;
            }
        };

        let ctx_val = build_ctx_json(&self.exts[idx].manifest, playback, storage_map);
        match invoke_lua_hook(&lua, &bridge, "on_load", &[ctx_val]) {
            Ok(cmds) => {
                self.exts[idx].lua = Some(lua);
                self.exts[idx].bridge = Some(bridge);
                self.exts[idx].enabled = true;
                self.exts[idx].error = None;
                let permitted = self.apply_commands(id, cmds);
                for c in permitted {
                    out.push((id.to_string(), c));
                }
            }
            Err(e) => {
                self.exts[idx].error = Some(e);
            }
        }
    }

    pub fn disable(
        &mut self,
        id: &str,
        playback: &ExtPlaybackCtx,
        library: &ExtLibraryCtx,
        storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
    ) -> Vec<String> {
        let mut visited = HashSet::new();
        let mut disabled = Vec::new();
        self.disable_dfs(id, &mut visited, playback, library, storage_map, &mut disabled);
        disabled
    }

    fn disable_dfs(
        &mut self,
        id: &str,
        visited: &mut HashSet<String>,
        playback: &ExtPlaybackCtx,
        library: &ExtLibraryCtx,
        storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
        disabled: &mut Vec<String>,
    ) {
        if !visited.insert(id.to_string()) {
            return;
        }
        for ub in self.used_by(id) {
            self.disable_dfs(&ub, visited, playback, library, storage_map, disabled);
        }
        if let Some(ext) = self.exts.iter_mut().find(|e| e.manifest.id == id) {
            if ext.enabled {
                if let (Some(lua), Some(bridge)) = (&ext.lua, &ext.bridge) {
                    sync_bridge(bridge, &ext.manifest, playback, library, storage_map);
                    let ctx_val = build_ctx_json(&ext.manifest, playback, storage_map);
                    let _ = invoke_lua_hook(lua, bridge, "on_unload", &[ctx_val]);
                }
                ext.enabled = false;
                ext.lua = None;
                ext.bridge = None;
                disabled.push(id.to_string());
            }
        }
        self.clear_for(id);
    }

    pub fn dispatch_event(
        &mut self,
        event_name: &str,
        playback: &ExtPlaybackCtx,
        library: &ExtLibraryCtx,
        storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
    ) -> Vec<(String, ExtHostCommand)> {
        self.dispatch_hook_all("on_event", playback, library, storage_map, Some(event_name))
    }

    pub fn dispatch_event_to_ext(
        &mut self,
        ext_id: &str,
        event_name: &str,
        payload: serde_json::Value,
        playback: &ExtPlaybackCtx,
        library: &ExtLibraryCtx,
        storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
    ) -> Vec<(String, ExtHostCommand)> {
        let mut results = Vec::new();
        let Some(ext) = self.exts.iter_mut().find(|e| e.manifest.id == ext_id && e.enabled) else {
            return results;
        };
        let (Some(lua), Some(bridge)) = (&ext.lua, &ext.bridge) else {
            return results;
        };
        if !lua_has_fn(lua, "on_event") {
            return results;
        }
        sync_bridge(bridge, &ext.manifest, playback, library, storage_map);
        let ctx_val = build_ctx_json(&ext.manifest, playback, storage_map);
        match invoke_lua_hook(
            lua,
            bridge,
            "on_event",
            &[ctx_val, serde_json::Value::String(event_name.to_string()), payload],
        ) {
            Ok(cmds) => {
                for c in self.apply_commands(ext_id, cmds) {
                    results.push((ext_id.to_string(), c));
                }
            }
            Err(e) => {
                ext.error = Some(e);
            }
        }
        results
    }

    pub fn dispatch_tick(
        &mut self,
        playback: &ExtPlaybackCtx,
        library: &ExtLibraryCtx,
        storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
    ) -> Vec<(String, ExtHostCommand)> {
        self.dispatch_hook_all("on_tick", playback, library, storage_map, None)
    }

    fn dispatch_hook_all(
        &mut self,
        hook_name: &str,
        playback: &ExtPlaybackCtx,
        library: &ExtLibraryCtx,
        storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
        event_name: Option<&str>,
    ) -> Vec<(String, ExtHostCommand)> {
        let mut results = Vec::new();
        let ids_snapshot: Vec<String> = self
            .exts
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.manifest.id.clone())
            .collect();

        for id in ids_snapshot {
            let Some(ext) = self.exts.iter_mut().find(|e| e.manifest.id == id) else {
                continue;
            };
            let (Some(lua), Some(bridge)) = (&ext.lua, &ext.bridge) else {
                continue;
            };
            if !lua_has_fn(lua, hook_name) {
                continue;
            }
            sync_bridge(bridge, &ext.manifest, playback, library, storage_map);
            let ctx_val = build_ctx_json(&ext.manifest, playback, storage_map);
            let call_res = match event_name {
                Some(ev) => invoke_lua_hook(
                    lua,
                    bridge,
                    hook_name,
                    &[ctx_val, serde_json::Value::String(ev.to_string())],
                ),
                None => invoke_lua_hook(lua, bridge, hook_name, &[ctx_val]),
            };
            match call_res {
                Ok(cmds) => {
                    for c in self.apply_commands(&id, cmds) {
                        results.push((id.clone(), c));
                    }
                }
                Err(e) => {
                    ext.error = Some(e);
                }
            }
        }
        results
    }

    pub fn dispatch_action(
        &mut self,
        ext_id: &str,
        action_id: &str,
        value: serde_json::Value,
        playback: &ExtPlaybackCtx,
        library: &ExtLibraryCtx,
        storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
    ) -> Vec<(String, ExtHostCommand)> {
        let mut results = Vec::new();
        let Some(ext) = self.exts.iter_mut().find(|e| e.manifest.id == ext_id && e.enabled) else {
            return results;
        };
        let (Some(lua), Some(bridge)) = (&ext.lua, &ext.bridge) else {
            return results;
        };
        sync_bridge(bridge, &ext.manifest, playback, library, storage_map);
        let ctx_val = build_ctx_json(&ext.manifest, playback, storage_map);
        match invoke_lua_hook(
            lua,
            bridge,
            "on_action",
            &[ctx_val, serde_json::Value::String(action_id.to_string()), value],
        ) {
            Ok(cmds) => {
                for c in self.apply_commands(ext_id, cmds) {
                    results.push((ext_id.to_string(), c));
                }
            }
            Err(e) => {
                ext.error = Some(e);
            }
        }
        results
    }

    /// Applies UI/Theme host registrations directly and returns capability-checked runtime commands.
    fn apply_commands(&mut self, ext_id: &str, cmds: Vec<ExtHostCommand>) -> Vec<ExtHostCommand> {
        let perms: HashSet<String> = self
            .exts
            .iter()
            .find(|e| e.manifest.id == ext_id)
            .map(|e| e.manifest.permissions.iter().cloned().collect())
            .unwrap_or_default();

        let mut runtime_out = Vec::new();
        for cmd in cmds {
            match cmd {
                ExtHostCommand::RegisterSidebar {
                    id,
                    label,
                    icon,
                    widgets,
                } if perms.contains("ui:sidebar") => {
                    let tab = ExtSidebarTab {
                        ext_id: ext_id.to_string(),
                        id: id.clone(),
                        label,
                        icon,
                        widgets,
                    };
                    if let Some(existing) = self
                        .sidebar_tabs
                        .iter_mut()
                        .find(|t| t.ext_id == ext_id && t.id == id)
                    {
                        *existing = tab;
                    } else {
                        self.sidebar_tabs.push(tab);
                        self.sidebar_tabs.sort_by(|a, b| {
                            a.ext_id.cmp(&b.ext_id).then_with(|| a.id.cmp(&b.id))
                        });
                    }
                }
                ExtHostCommand::UnregisterSidebar { id } if perms.contains("ui:sidebar") => {
                    self.sidebar_tabs
                        .retain(|t| !(t.ext_id == ext_id && t.id == id));
                }
                ExtHostCommand::RegisterSettings { id, title, widgets }
                    if perms.contains("ui:settings") =>
                {
                    let panel_id = if id.is_empty() {
                        "default".to_string()
                    } else {
                        id
                    };
                    let panel = ExtSettingsPanel {
                        ext_id: ext_id.to_string(),
                        id: panel_id.clone(),
                        title,
                        widgets,
                    };
                    if let Some(existing) = self
                        .settings_panels
                        .iter_mut()
                        .find(|p| p.ext_id == ext_id && p.id == panel_id)
                    {
                        *existing = panel;
                    } else {
                        self.settings_panels.push(panel);
                        self.settings_panels.sort_by(|a, b| {
                            a.ext_id.cmp(&b.ext_id).then_with(|| a.id.cmp(&b.id))
                        });
                    }
                }
                ExtHostCommand::SetTopbarSlot {
                    text,
                    icon,
                    accent,
                    action_id,
                } if perms.contains("ui:topbar") => {
                    self.topbar_slots.retain(|s| s.ext_id != ext_id);
                    self.topbar_slots.push(ExtSlotBadge {
                        ext_id: ext_id.to_string(),
                        text,
                        icon,
                        accent,
                        action_id,
                    });
                }
                ExtHostCommand::ClearTopbarSlot if perms.contains("ui:topbar") => {
                    self.topbar_slots.retain(|s| s.ext_id != ext_id);
                }
                ExtHostCommand::SetPlayerWidget {
                    text,
                    icon,
                    accent,
                    action_id,
                } if perms.contains("ui:player_widget") => {
                    self.player_widgets.retain(|w| w.ext_id != ext_id);
                    self.player_widgets.push(ExtSlotBadge {
                        ext_id: ext_id.to_string(),
                        text,
                        icon,
                        accent,
                        action_id,
                    });
                }
                ExtHostCommand::ClearPlayerWidget if perms.contains("ui:player_widget") => {
                    self.player_widgets.retain(|w| w.ext_id != ext_id);
                }
                ExtHostCommand::RegisterTheme {
                    slug,
                    label,
                    base,
                    sidebar,
                    main,
                    card,
                    card_hover,
                    text,
                    text_sub,
                    accent,
                    logo_color,
                } if perms.contains("themes:register") => {
                    self.themes.retain(|t| t.slug != slug);
                    self.themes.push(ExtRegisteredTheme {
                        ext_id: ext_id.to_string(),
                        slug,
                        label,
                        base,
                        sidebar,
                        main,
                        card,
                        card_hover,
                        text,
                        text_sub,
                        accent,
                        logo_color,
                    });
                }
                ExtHostCommand::SetAudioFilter { .. }
                | ExtHostCommand::ClearAudioFilter
                | ExtHostCommand::AudioSetSpatial { .. }
                | ExtHostCommand::AudioSetPreset { .. }
                    if perms.contains("audio:dsp") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::AudioSetPitch { .. }
                    if perms.contains("playback:control") || perms.contains("audio:dsp") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::PlaybackToggle
                | ExtHostCommand::PlaybackSetPaused { .. }
                | ExtHostCommand::PlaybackPlayPath { .. }
                | ExtHostCommand::PlaybackSetMute { .. }
                | ExtHostCommand::PlaybackNext
                | ExtHostCommand::PlaybackPrev
                | ExtHostCommand::Seek { .. }
                | ExtHostCommand::SeekRelative { .. }
                | ExtHostCommand::SetVolume { .. }
                | ExtHostCommand::SetSpeed { .. }
                    if perms.contains("playback:control") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::QueueAdd { .. } | ExtHostCommand::QueueClear
                    if perms.contains("queue:control") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::ThemesSetActive { .. } if perms.contains("themes:register") => {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::UiSetDensity { .. } if perms.contains("ui:settings") => {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::UiSetLayout { .. }
                | ExtHostCommand::UiSetLayoutConfig { .. }
                | ExtHostCommand::UiSetReactiveConfig { .. }
                | ExtHostCommand::UiSetIconConfig { .. }
                | ExtHostCommand::UiSetButtonConfig { .. }
                | ExtHostCommand::UiSetLogoConfig { .. }
                | ExtHostCommand::UiSetDiscordRpcConfig { .. }
                | ExtHostCommand::UiSetTransparencyConfig { .. }
                | ExtHostCommand::UiSetPopupConfig { .. }
                | ExtHostCommand::UiCopyClipboard { .. }
                | ExtHostCommand::UiRequestClipboardPaste
                | ExtHostCommand::PopupLaunch
                | ExtHostCommand::PopupClose
                    if perms.contains("ui:settings") || perms.contains("ui:sidebar") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::UiOpenSidebar { .. }
                    if perms.contains("ui:sidebar") || perms.contains("ui:topbar") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::UiSetCompactMenu { compact }
                    if perms.contains("ui:sidebar") || perms.contains("ui:settings") =>
                {
                    self.compact_menu = compact;
                    self.compact_menu_owner = if compact {
                        Some(ext_id.to_string())
                    } else {
                        None
                    };
                }
                ExtHostCommand::ExtensionsSetAll { .. } if perms.contains("ui:settings") => {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::LibraryRescan
                | ExtHostCommand::LibraryOpenFolder { .. }
                    if perms.contains("library:read") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::LibraryPlayFolder { .. }
                    if perms.contains("playback:control") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::HwDaeboardColor { .. }
                | ExtHostCommand::HwDaeboardBrightness { .. }
                | ExtHostCommand::HwDaeboardStart
                | ExtHostCommand::HwDaeboardStop
                | ExtHostCommand::HwDaetronStart
                | ExtHostCommand::HwDaetronReload
                    if perms.contains("hw:control") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::McSetConfig { .. }
                | ExtHostCommand::McSendNotice { .. }
                    if perms.contains("mc:link") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::FsCreateFolder { .. }
                | ExtHostCommand::FsDeleteFolder { .. }
                | ExtHostCommand::FsCopyTrackToFolder { .. }
                | ExtHostCommand::FsDeleteTrack { .. }
                    if perms.contains("fs:write") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::NotifyDesktop { .. } if perms.contains("notify") => {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::LyricsLoad { .. } if perms.contains("library:read") => {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::LyricsTranslate { .. }
                | ExtHostCommand::NetFetchRss { .. }
                | ExtHostCommand::NetYtSearch { .. }
                | ExtHostCommand::NetYtPlay { .. }
                    if perms.contains("network") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::NetYtDownload { .. }
                    if perms.contains("network") && perms.contains("fs:write") =>
                {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::StorageSet { .. } if perms.contains("storage") => {
                    runtime_out.push(cmd);
                }
                ExtHostCommand::Toast { .. }
                    if perms.contains("notify")
                        || perms.contains("ui:sidebar")
                        || perms.contains("ui:settings")
                        || perms.contains("ui:topbar")
                        || perms.contains("ui:player_widget") =>
                {
                    runtime_out.push(cmd);
                }
                _ => {}
            }
        }
        runtime_out
    }
}

fn sync_bridge(
    bridge: &SharedBridge,
    manifest: &ExtensionManifest,
    playback: &ExtPlaybackCtx,
    library: &ExtLibraryCtx,
    storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
) {
    if let Ok(mut b) = bridge.lock() {
        b.playback = if b.has_perm("playback:read") {
            Some(playback.clone())
        } else {
            None
        };
        b.library = if b.has_perm("library:read") {
            library.clone()
        } else {
            ExtLibraryCtx::default()
        };
        if let Some(st) = storage_map.get(&manifest.id) {
            b.storage = st.clone();
        }
        b.pending_cmds.clear();
    }
}

fn create_sandboxed_lua(script_src: &str, bridge: &SharedBridge) -> Result<Lua, String> {
    let stdlib = StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8;
    let lua = Lua::new_with(stdlib, mlua::LuaOptions::default())
        .map_err(|e| format!("Lua init failed: {e}"))?;

    install_sdk(&lua, bridge).map_err(|e| format!("SDK bind failed: {e}"))?;

    lua.load(script_src)
        .exec()
        .map_err(|e| format!("Lua syntax/load error: {e}"))?;

    Ok(lua)
}

fn install_sdk(lua: &Lua, bridge: &SharedBridge) -> mlua::Result<()> {
    let root = lua.create_table()?;
    root.set("api_version", API_VERSION)?;

    {
        let br = Arc::clone(bridge);
        root.set(
            "has_permission",
            lua.create_function(move |_, perm: String| {
                let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                Ok(b.has_perm(&perm))
            })?,
        )?;
    }

    // tntron.playback
    let pb = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        pb.set(
            "get",
            lua.create_function(move |lua, ()| {
                let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("playback:read")?;
                ser_to_lua(lua, &b.playback)
            })?,
        )?;
    }
    macro_rules! push_pb_fn {
        ($name:expr, $cmd:expr) => {{
            let br = Arc::clone(bridge);
            pb.set(
                $name,
                lua.create_function(move |_, ()| {
                    let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                    b.require_perm("playback:control")?;
                    b.pending_cmds.push($cmd);
                    Ok(())
                })?,
            )?;
        }};
    }
    push_pb_fn!("toggle", ExtHostCommand::PlaybackToggle);
    push_pb_fn!("play", ExtHostCommand::PlaybackSetPaused { paused: false });
    push_pb_fn!("pause", ExtHostCommand::PlaybackSetPaused { paused: true });
    push_pb_fn!("next", ExtHostCommand::PlaybackNext);
    push_pb_fn!("prev", ExtHostCommand::PlaybackPrev);
    {
        let br = Arc::clone(bridge);
        pb.set(
            "seek",
            lua.create_function(move |_, seconds: f64| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("playback:control")?;
                b.pending_cmds.push(ExtHostCommand::Seek { seconds });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        pb.set(
            "seek_relative",
            lua.create_function(move |_, delta: f64| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("playback:control")?;
                b.pending_cmds.push(ExtHostCommand::SeekRelative { delta });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        pb.set(
            "set_volume",
            lua.create_function(move |_, volume: f64| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("playback:control")?;
                b.pending_cmds.push(ExtHostCommand::SetVolume { volume });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        pb.set(
            "set_speed",
            lua.create_function(move |_, speed: f64| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("playback:control")?;
                b.pending_cmds.push(ExtHostCommand::SetSpeed { speed });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        pb.set(
            "play_path",
            lua.create_function(move |_, path: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("playback:control")?;
                b.pending_cmds.push(ExtHostCommand::PlaybackPlayPath { path });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        pb.set(
            "set_mute",
            lua.create_function(move |_, muted: bool| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("playback:control")?;
                b.pending_cmds.push(ExtHostCommand::PlaybackSetMute { muted });
                Ok(())
            })?,
        )?;
    }
    root.set("playback", pb)?;

    // tntron.queue
    let q = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        q.set(
            "list",
            lua.create_function(move |lua, ()| {
                let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("library:read")?;
                ser_to_lua(lua, &b.library.queue)
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        q.set(
            "add",
            lua.create_function(move |_, path: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("queue:control")?;
                b.pending_cmds.push(ExtHostCommand::QueueAdd { path });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        q.set(
            "clear",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("queue:control")?;
                b.pending_cmds.push(ExtHostCommand::QueueClear);
                Ok(())
            })?,
        )?;
    }
    root.set("queue", q)?;

    // tntron.library
    let lib = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        lib.set(
            "current_folder",
            lua.create_function(move |lua, ()| {
                let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("library:read")?;
                let tbl = lua.create_table()?;
                tbl.set("path", b.library.current_dir.clone())?;
                tbl.set("tracks", ser_to_lua(lua, &b.library.tracks)?)?;
                Ok(tbl)
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        lib.set(
            "favorites",
            lua.create_function(move |lua, ()| {
                let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("library:read")?;
                ser_to_lua(lua, &b.library.favorites)
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        lib.set(
            "all_tracks",
            lua.create_function(move |lua, ()| {
                let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("library:read")?;
                ser_to_lua(lua, &b.library.library_tracks)
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        lib.set(
            "folders",
            lua.create_function(move |lua, ()| {
                let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("library:read")?;
                ser_to_lua(lua, &b.library.folders)
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        lib.set(
            "rescan",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("library:read")?;
                b.pending_cmds.push(ExtHostCommand::LibraryRescan);
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        lib.set(
            "open_folder",
            lua.create_function(move |_, path: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("library:read")?;
                b.pending_cmds.push(ExtHostCommand::LibraryOpenFolder { path });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        lib.set(
            "play_folder",
            lua.create_function(move |_, (path, shuffle): (String, Option<bool>)| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("playback:control")?;
                b.pending_cmds.push(ExtHostCommand::LibraryPlayFolder {
                    path,
                    shuffle: shuffle.unwrap_or(false),
                });
                Ok(())
            })?,
        )?;
    }
    root.set("library", lib)?;

    // tntron.fs
    let fs_tbl = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        fs_tbl.set(
            "create_folder",
            lua.create_function(move |_, name: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("fs:write")?;
                b.pending_cmds.push(ExtHostCommand::FsCreateFolder { name });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        fs_tbl.set(
            "delete_folder",
            lua.create_function(move |_, path: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("fs:write")?;
                b.pending_cmds.push(ExtHostCommand::FsDeleteFolder { path });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        fs_tbl.set(
            "copy_to_folder",
            lua.create_function(move |_, (track_path, folder_path): (String, String)| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("fs:write")?;
                b.pending_cmds.push(ExtHostCommand::FsCopyTrackToFolder {
                    track_path,
                    folder_path,
                });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        fs_tbl.set(
            "delete_file",
            lua.create_function(move |_, path: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("fs:write")?;
                b.pending_cmds.push(ExtHostCommand::FsDeleteTrack { path });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        fs_tbl.set(
            "save_tnt",
            lua.create_function(
                move |lua, (name_or_path, tbl, fmt): (String, LuaValue, Option<String>)| {
                    let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                    if !(b.has_perm("storage") || b.has_perm("fs:write")) {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Extension '{}' lacks permission 'storage' or 'fs:write'",
                            b.ext_id
                        )));
                    }
                    drop(b);
                    let cfg: serde_json::Value = lua.from_value(tbl)?;
                    let base_dir = crate::storage::default_tnt_dir();
                    let tnt_fmt = fmt.as_deref().unwrap_or("tntron-reactive-v1");
                    match crate::storage::save_tnt_file_with_format(
                        &name_or_path,
                        tnt_fmt,
                        cfg,
                        &base_dir,
                    ) {
                        Ok(path) => Ok((Some(path.display().to_string()), Option::<String>::None)),
                        Err(e) => Ok((None, Some(e))),
                    }
                },
            )?,
        )?;
    }
    {
        fs_tbl.set(
            "format_tnt_share",
            lua.create_function(
                move |lua, (tbl, fmt, name): (LuaValue, Option<String>, Option<String>)| {
                    let cfg: serde_json::Value = lua.from_value(tbl)?;
                    let tnt_fmt = fmt.as_deref().unwrap_or("tntron-reactive-v1");
                    let tnt_name = name.as_deref().unwrap_or("shared_preset");
                    match crate::storage::format_tnt_share_json(tnt_fmt, tnt_name, cfg) {
                        Ok(s) => Ok((Some(s), Option::<String>::None)),
                        Err(e) => Ok((None, Some(e))),
                    }
                },
            )?,
        )?;
    }
    {
        fs_tbl.set(
            "parse_tnt_share",
            lua.create_function(move |lua, raw: String| {
                match crate::storage::parse_tnt_share_json(&raw) {
                    Ok(val) => {
                        let lv = json_to_lua(lua, &val)?;
                        Ok((lv, Option::<String>::None))
                    }
                    Err(e) => Ok((LuaValue::Nil, Some(e))),
                }
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        fs_tbl.set(
            "load_tnt",
            lua.create_function(move |lua, name_or_path: String| {
                let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("storage") || b.has_perm("fs:write")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'storage' or 'fs:write'",
                        b.ext_id
                    )));
                }
                drop(b);
                let base_dir = crate::storage::default_tnt_dir();
                match crate::storage::load_tnt_file(&name_or_path, &base_dir) {
                    Ok(val) => {
                        let lv = json_to_lua(lua, &val)?;
                        Ok((lv, Option::<String>::None))
                    }
                    Err(e) => Ok((LuaValue::Nil, Some(e))),
                }
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        fs_tbl.set(
            "list_tnt",
            lua.create_function(move |lua, ()| {
                let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("storage") || b.has_perm("fs:write")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'storage' or 'fs:write'",
                        b.ext_id
                    )));
                }
                drop(b);
                let base_dir = crate::storage::default_tnt_dir();
                let list = crate::storage::list_tnt_files(&base_dir);
                let arr = lua.create_table()?;
                for (i, (name, path)) in list.into_iter().enumerate() {
                    let row = lua.create_table()?;
                    row.set("name", name)?;
                    row.set("path", path.display().to_string())?;
                    arr.set(i + 1, row)?;
                }
                Ok(arr)
            })?,
        )?;
    }
    root.set("fs", fs_tbl)?;

    // tntron.extensions
    let exts_tbl = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        exts_tbl.set(
            "set_all",
            lua.create_function(move |_, enabled: bool| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("ui:settings")?;
                b.pending_cmds.push(ExtHostCommand::ExtensionsSetAll { enabled });
                Ok(())
            })?,
        )?;
    }
    root.set("extensions", exts_tbl)?;

    // tntron.audio
    let audio = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        audio.set(
            "set_filter",
            lua.create_function(move |_, filter: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("audio:dsp")?;
                b.pending_cmds.push(ExtHostCommand::SetAudioFilter { filter });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        audio.set(
            "clear_filter",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("audio:dsp")?;
                b.pending_cmds.push(ExtHostCommand::ClearAudioFilter);
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        audio.set(
            "set_spatial",
            lua.create_function(move |_, (enabled, orbit_hz): (bool, Option<f64>)| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("audio:dsp")?;
                b.pending_cmds.push(ExtHostCommand::AudioSetSpatial {
                    enabled,
                    orbit_hz: orbit_hz.unwrap_or(0.18),
                });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        audio.set(
            "set_pitch",
            lua.create_function(
                move |_, (pitch_st, link_pitch, preserve_formant): (f64, Option<bool>, Option<bool>)| {
                    let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                    if !(b.has_perm("playback:control") || b.has_perm("audio:dsp")) {
                        return Err(mlua::Error::RuntimeError(format!(
                            "Extension '{}' lacks permission 'playback:control'",
                            b.ext_id
                        )));
                    }
                    b.pending_cmds.push(ExtHostCommand::AudioSetPitch {
                        pitch_st: pitch_st.clamp(-12.0, 12.0),
                        link_pitch,
                        preserve_formant,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        audio.set(
            "set_preset",
            lua.create_function(move |_, preset: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("audio:dsp")?;
                b.pending_cmds.push(ExtHostCommand::AudioSetPreset { preset });
                Ok(())
            })?,
        )?;
    }
    root.set("audio", audio)?;

    // tntron.mc (Minecraft Fabric 26.2 Link)
    let mc = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        mc.set(
            "status",
            lua.create_function(move |lua, ()| {
                let maybe_handle = {
                    let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                    b.require_perm("mc:link")?;
                    b.mc_bridge.clone()
                };
                if let Some(handle) = maybe_handle {
                    let cfg = handle.config_snapshot();
                    let snap = handle.telemetry_snapshot();
                    let payload = serde_json::json!({
                        "enabled": cfg.enabled,
                        "tcp_enabled": cfg.tcp_enabled,
                        "tcp_host": cfg.tcp_host,
                        "tcp_port": cfg.tcp_port,
                        "connected": snap.connected,
                        "clients_count": snap.clients_count,
                        "last_seen_epoch": snap.last_seen_epoch,
                        "commands_received": snap.commands_received,
                        "player_name": snap.telemetry.player_name,
                        "dimension": snap.telemetry.dimension,
                        "biome": snap.telemetry.biome,
                        "health": snap.telemetry.health,
                        "max_health": snap.telemetry.max_health,
                        "in_combat": snap.telemetry.in_combat,
                        "boss_active": snap.telemetry.boss_active,
                        "boss_name": snap.telemetry.boss_name,
                        "paused": snap.telemetry.paused,
                        "in_world": snap.telemetry.in_world,
                        "weather": snap.telemetry.weather,
                        "underwater": snap.telemetry.underwater,
                        "speed_bps": snap.telemetry.speed_bps,
                        "tnt_nearby": snap.telemetry.tnt_nearby,
                    });
                    json_to_lua(lua, &payload)
                } else {
                    let payload = serde_json::json!({
                        "enabled": false,
                        "tcp_enabled": true,
                        "tcp_host": "127.0.0.1",
                        "tcp_port": 26262,
                        "connected": false,
                        "clients_count": 0,
                        "commands_received": 0,
                    });
                    json_to_lua(lua, &payload)
                }
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        mc.set(
            "set_config",
            lua.create_function(move |lua, spec: LuaValue| {
                let patch: serde_json::Value = lua.from_value(spec)?;
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("mc:link")?;
                b.pending_cmds.push(ExtHostCommand::McSetConfig { patch });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        mc.set(
            "send_notice",
            lua.create_function(move |_, (kind, title, body): (String, String, String)| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("mc:link")?;
                b.pending_cmds.push(ExtHostCommand::McSendNotice { kind, title, body });
                Ok(())
            })?,
        )?;
    }
    root.set("mc", mc)?;

    // tntron.hw (daeboard + daetron integration)
    let hw = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        hw.set(
            "daeboard_ping",
            lua.create_function(move |lua, ()| {
                {
                    let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                    b.require_perm("hw:control")?;
                }
                let t = lua.create_table()?;
                if let Some(bin) = crate::hardware::daeboard_binary_path() {
                    t.set("bin_path", bin.display().to_string())?;
                }
                if !std::path::Path::new(crate::hardware::DAEBOARD_SOCK_PATH).exists() {
                    t.set("connected", false)?;
                    t.set("reply", "socket not present")?;
                    return Ok(t);
                }
                match crate::hardware::daeboard_send_sync("ping") {
                    Ok(reply) => {
                        t.set("connected", reply.eq_ignore_ascii_case("pong"))?;
                        t.set("reply", reply)?;
                    }
                    Err(e) => {
                        t.set("connected", false)?;
                        t.set("reply", e)?;
                    }
                }
                Ok(t)
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        hw.set(
            "daeboard_color",
            lua.create_function(move |_, hex: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("hw:control")?;
                b.pending_cmds.push(ExtHostCommand::HwDaeboardColor { hex });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        hw.set(
            "daeboard_brightness",
            lua.create_function(move |_, level: u8| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("hw:control")?;
                b.pending_cmds.push(ExtHostCommand::HwDaeboardBrightness {
                    level: level.min(3),
                });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        hw.set(
            "daetron_status",
            lua.create_function(move |lua, rules_dir: Option<String>| {
                {
                    let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                    b.require_perm("hw:control")?;
                }
                let st = crate::hardware::daetron_probe(rules_dir.as_deref().map(Path::new));
                ser_to_lua(lua, &st)
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        hw.set(
            "daeboard_start",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("hw:control")?;
                b.pending_cmds.push(ExtHostCommand::HwDaeboardStart);
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        hw.set(
            "daeboard_stop",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("hw:control")?;
                b.pending_cmds.push(ExtHostCommand::HwDaeboardStop);
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        hw.set(
            "daetron_start",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("hw:control")?;
                b.pending_cmds.push(ExtHostCommand::HwDaetronStart);
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        hw.set(
            "daetron_reload",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("hw:control")?;
                b.pending_cmds.push(ExtHostCommand::HwDaetronReload);
                Ok(())
            })?,
        )?;
    }
    root.set("hw", hw)?;

    // tntron.themes
    let themes = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        themes.set(
            "register",
            lua.create_function(move |lua, spec: LuaValue| {
                let mut v: serde_json::Value = lua.from_value(spec)?;
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("action".into(), serde_json::json!("register_theme"));
                }
                let cmd: ExtHostCommand = serde_json::from_value(v)
                    .map_err(|e| mlua::Error::RuntimeError(format!("register theme: {e}")))?;
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("themes:register")?;
                b.pending_cmds.push(cmd);
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        themes.set(
            "set_active",
            lua.create_function(move |_, slug: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("themes:register")?;
                b.pending_cmds.push(ExtHostCommand::ThemesSetActive { slug });
                Ok(())
            })?,
        )?;
    }
    root.set("themes", themes)?;

    // tntron.notify
    let notify = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        notify.set(
            "desktop",
            lua.create_function(move |_, (summary, body): (String, String)| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("notify")?;
                b.pending_cmds.push(ExtHostCommand::NotifyDesktop { summary, body });
                Ok(())
            })?,
        )?;
    }
    root.set("notify", notify)?;

    // tntron.lyrics
    let lyrics = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        lyrics.set(
            "load",
            lua.create_function(
                move |_, (path, title, artist, duration_sec, provider): (String, String, String, f64, Option<String>)| {
                    let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                    b.require_perm("library:read")?;
                    b.pending_cmds.push(ExtHostCommand::LyricsLoad {
                        path,
                        title,
                        artist,
                        duration_sec,
                        provider,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        lyrics.set(
            "translate",
            lua.create_function(
                move |_, (path, lines, target_lang): (String, Vec<String>, Option<String>)| {
                    let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                    b.require_perm("network")?;
                    b.pending_cmds.push(ExtHostCommand::LyricsTranslate {
                        path,
                        target_lang: target_lang.unwrap_or_else(|| "en".to_string()),
                        lines,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    root.set("lyrics", lyrics)?;

    // tntron.net
    let net = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        net.set(
            "fetch_rss",
            lua.create_function(move |_, url: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("network")?;
                b.pending_cmds.push(ExtHostCommand::NetFetchRss { url });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        net.set(
            "yt_search",
            lua.create_function(move |_, query: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("network")?;
                b.pending_cmds.push(ExtHostCommand::NetYtSearch { query });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        net.set(
            "yt_play",
            lua.create_function(move |_, (url, title): (String, Option<String>)| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("network")?;
                b.pending_cmds.push(ExtHostCommand::NetYtPlay { url, title });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        net.set(
            "yt_download",
            lua.create_function(move |_, url: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("network")?;
                b.require_perm("fs:write")?;
                b.pending_cmds.push(ExtHostCommand::NetYtDownload { url });
                Ok(())
            })?,
        )?;
    }
    root.set("net", net)?;

    // tntron.storage
    let storage = lua.create_table()?;
    {
        let br = Arc::clone(bridge);
        storage.set(
            "get",
            lua.create_function(move |lua, (key, default): (String, Option<LuaValue>)| {
                let b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("storage")?;
                match b.storage.get(&key) {
                    Some(v) => json_to_lua(lua, v),
                    None => Ok(default.unwrap_or(LuaValue::Nil)),
                }
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        storage.set(
            "set",
            lua.create_function(move |lua, (key, val): (String, LuaValue)| {
                let json_val: serde_json::Value = lua.from_value(val)?;
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("storage")?;
                b.storage.insert(key.clone(), json_val.clone());
                b.pending_cmds.push(ExtHostCommand::StorageSet {
                    key,
                    value: json_val,
                });
                Ok(())
            })?,
        )?;
    }
    root.set("storage", storage)?;

    // tntron.ui
    let ui = lua.create_table()?;
    bind_ui_sdk(lua, &ui, bridge)?;
    root.set("ui", ui)?;

    lua.globals().set("tntron", root)?;
    Ok(())
}

fn bind_ui_sdk(lua: &Lua, ui: &Table, bridge: &SharedBridge) -> mlua::Result<()> {
    macro_rules! push_ui_spec {
        ($fn_name:expr, $action_name:expr, $perm:expr) => {{
            let br = Arc::clone(bridge);
            ui.set(
                $fn_name,
                lua.create_function(move |lua, spec: LuaValue| {
                    let mut v: serde_json::Value = lua.from_value(spec)?;
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("action".into(), serde_json::json!($action_name));
                    }
                    let cmd: ExtHostCommand = serde_json::from_value(v)
                        .map_err(|e| mlua::Error::RuntimeError(format!("{}: {e}", $fn_name)))?;
                    let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                    b.require_perm($perm)?;
                    b.pending_cmds.push(cmd);
                    Ok(())
                })?,
            )?;
        }};
    }

    push_ui_spec!("register_sidebar", "register_sidebar", "ui:sidebar");
    push_ui_spec!("register_settings", "register_settings", "ui:settings");
    push_ui_spec!("set_topbar_slot", "set_topbar_slot", "ui:topbar");
    push_ui_spec!("set_player_widget", "set_player_widget", "ui:player_widget");

    {
        let br = Arc::clone(bridge);
        ui.set(
            "unregister_sidebar",
            lua.create_function(move |_, id: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("ui:sidebar")?;
                b.pending_cmds.push(ExtHostCommand::UnregisterSidebar { id });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "clear_topbar_slot",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("ui:topbar")?;
                b.pending_cmds.push(ExtHostCommand::ClearTopbarSlot);
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "clear_player_widget",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("ui:player_widget")?;
                b.pending_cmds.push(ExtHostCommand::ClearPlayerWidget);
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "set_compact_menu",
            lua.create_function(move |_, compact: bool| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:sidebar") || b.has_perm("ui:settings")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:sidebar'",
                        b.ext_id
                    )));
                }
                b.pending_cmds
                    .push(ExtHostCommand::UiSetCompactMenu { compact });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "set_layout",
            lua.create_function(move |_, layout: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:settings'",
                        b.ext_id
                    )));
                }
                b.pending_cmds.push(ExtHostCommand::UiSetLayout { layout });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "set_layout_config",
            lua.create_function(move |lua, patch_tbl: LuaValue| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:settings'",
                        b.ext_id
                    )));
                }
                let patch: serde_json::Value = lua.from_value(patch_tbl)?;
                b.pending_cmds.push(ExtHostCommand::UiSetLayoutConfig { patch });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "set_reactive_config",
            lua.create_function(move |lua, patch_tbl: LuaValue| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:sidebar'",
                        b.ext_id
                    )));
                }
                let patch: serde_json::Value = lua.from_value(patch_tbl)?;
                b.pending_cmds.push(ExtHostCommand::UiSetReactiveConfig { patch });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "set_icon_config",
            lua.create_function(move |lua, patch_tbl: LuaValue| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:sidebar'",
                        b.ext_id
                    )));
                }
                let patch: serde_json::Value = lua.from_value(patch_tbl)?;
                b.pending_cmds.push(ExtHostCommand::UiSetIconConfig { patch });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "set_button_config",
            lua.create_function(move |lua, patch_tbl: LuaValue| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:sidebar'",
                        b.ext_id
                    )));
                }
                let patch: serde_json::Value = lua.from_value(patch_tbl)?;
                b.pending_cmds.push(ExtHostCommand::UiSetButtonConfig { patch });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "set_logo_config",
            lua.create_function(move |lua, patch_tbl: LuaValue| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings")
                    || b.has_perm("ui:sidebar")
                    || b.has_perm("themes:register"))
                {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:settings'",
                        b.ext_id
                    )));
                }
                let patch: serde_json::Value = lua.from_value(patch_tbl)?;
                b.pending_cmds.push(ExtHostCommand::UiSetLogoConfig { patch });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "set_discord_rpc_config",
            lua.create_function(move |lua, patch_tbl: LuaValue| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:settings'",
                        b.ext_id
                    )));
                }
                let patch: serde_json::Value = lua.from_value(patch_tbl)?;
                b.pending_cmds
                    .push(ExtHostCommand::UiSetDiscordRpcConfig { patch });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "copy_clipboard",
            lua.create_function(move |_, text: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:sidebar'",
                        b.ext_id
                    )));
                }
                b.pending_cmds.push(ExtHostCommand::UiCopyClipboard { text });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "paste_clipboard",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:sidebar'",
                        b.ext_id
                    )));
                }
                b.pending_cmds.push(ExtHostCommand::UiRequestClipboardPaste);
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "set_transparency_config",
            lua.create_function(move |lua, patch_tbl: LuaValue| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:sidebar'",
                        b.ext_id
                    )));
                }
                let patch: serde_json::Value = lua.from_value(patch_tbl)?;
                b.pending_cmds.push(ExtHostCommand::UiSetTransparencyConfig { patch });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "set_popup_config",
            lua.create_function(move |lua, patch_tbl: LuaValue| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:sidebar'",
                        b.ext_id
                    )));
                }
                let patch: serde_json::Value = lua.from_value(patch_tbl)?;
                b.pending_cmds.push(ExtHostCommand::UiSetPopupConfig { patch });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "launch_popup",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:sidebar'",
                        b.ext_id
                    )));
                }
                b.pending_cmds.push(ExtHostCommand::PopupLaunch);
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "close_popup",
            lua.create_function(move |_, ()| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:settings") || b.has_perm("ui:sidebar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:sidebar'",
                        b.ext_id
                    )));
                }
                b.pending_cmds.push(ExtHostCommand::PopupClose);
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "open_sidebar",
            lua.create_function(move |_, tab_id: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("ui:sidebar") || b.has_perm("ui:topbar")) {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'ui:sidebar'",
                        b.ext_id
                    )));
                }
                b.pending_cmds.push(ExtHostCommand::UiOpenSidebar { tab_id });
                Ok(())
            })?,
        )?;
    }
    {
        let br = Arc::clone(bridge);
        ui.set(
            "toast",
            lua.create_function(move |_, message: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                if !(b.has_perm("notify")
                    || b.has_perm("ui:sidebar")
                    || b.has_perm("ui:settings")
                    || b.has_perm("ui:topbar")
                    || b.has_perm("ui:player_widget"))
                {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Extension '{}' lacks permission 'notify'",
                        b.ext_id
                    )));
                }
                b.pending_cmds.push(ExtHostCommand::Toast { message });
                Ok(())
            })?,
        )?;
    }

    // Declarative UI node constructors (pure JSON-compatible Lua tables)
    ui.set(
        "heading",
        lua.create_function(|lua, text: String| {
            let t = lua.create_table()?;
            t.set("type", "heading")?;
            t.set("text", text)?;
            Ok(t)
        })?,
    )?;
    ui.set(
        "text",
        lua.create_function(|lua, (text, muted): (String, Option<bool>)| {
            let t = lua.create_table()?;
            t.set("type", "text")?;
            t.set("text", text)?;
            t.set("muted", muted.unwrap_or(false))?;
            Ok(t)
        })?,
    )?;
    ui.set(
        "badge",
        lua.create_function(|lua, (text, opts): (String, Option<Table>)| {
            let t = lua.create_table()?;
            t.set("type", "badge")?;
            t.set("text", text)?;
            if let Some(o) = opts {
                if let Ok(icon) = o.get::<String>("icon") {
                    t.set("icon", icon)?;
                }
                if let Ok(acc) = o.get::<bool>("accent") {
                    t.set("accent", acc)?;
                }
            }
            Ok(t)
        })?,
    )?;
    ui.set(
        "row",
        lua.create_function(|lua, children: Table| {
            let t = lua.create_table()?;
            t.set("type", "row")?;
            t.set("children", children)?;
            Ok(t)
        })?,
    )?;
    ui.set(
        "button",
        lua.create_function(|lua, (id, label, opts): (String, String, Option<Table>)| {
            let t = lua.create_table()?;
            t.set("type", "button")?;
            t.set("id", id)?;
            t.set("label", label)?;
            if let Some(o) = opts {
                if let Ok(icon) = o.get::<String>("icon") {
                    t.set("icon", icon)?;
                }
                if let Ok(acc) = o.get::<bool>("accent") {
                    t.set("accent", acc)?;
                }
                if let Ok(act) = o.get::<bool>("active") {
                    t.set("active", act)?;
                }
            }
            Ok(t)
        })?,
    )?;
    ui.set(
        "toggle",
        lua.create_function(|lua, (id, label, value): (String, String, bool)| {
            let t = lua.create_table()?;
            t.set("type", "toggle")?;
            t.set("id", id)?;
            t.set("label", label)?;
            t.set("value", value)?;
            Ok(t)
        })?,
    )?;
    ui.set(
        "slider",
        lua.create_function(
            |lua,
             (id, label, min, max, step, value, unit): (
                String,
                String,
                f64,
                f64,
                f64,
                f64,
                Option<String>,
            )| {
                let t = lua.create_table()?;
                t.set("type", "slider")?;
                t.set("id", id)?;
                t.set("label", label)?;
                t.set("min", min)?;
                t.set("max", max)?;
                t.set("step", step)?;
                t.set("value", value)?;
                t.set("unit", unit.unwrap_or_default())?;
                Ok(t)
            },
        )?,
    )?;
    {
        let br = Arc::clone(bridge);
        ui.set(
            "set_density",
            lua.create_function(move |_, density: String| {
                let mut b = br.lock().map_err(|_| mlua::Error::RuntimeError("lock".into()))?;
                b.require_perm("ui:settings")?;
                b.pending_cmds.push(ExtHostCommand::UiSetDensity { density });
                Ok(())
            })?,
        )?;
    }
    ui.set(
        "progress",
        lua.create_function(|lua, (label, value, max): (String, f64, f64)| {
            let t = lua.create_table()?;
            t.set("type", "progress")?;
            t.set("label", label)?;
            t.set("value", value)?;
            t.set("max", max)?;
            Ok(t)
        })?,
    )?;
    ui.set(
        "input",
        lua.create_function(
            |lua,
             (id, label, value, opts): (
                String,
                String,
                Option<String>,
                Option<Table>,
            )| {
                let t = lua.create_table()?;
                t.set("type", "input")?;
                t.set("id", id)?;
                t.set("label", label)?;
                t.set("value", value.unwrap_or_default())?;
                if let Some(o) = opts {
                    if let Ok(ph) = o.get::<String>("placeholder") {
                        t.set("placeholder", ph)?;
                    }
                    if let Ok(sa) = o.get::<String>("submit_action") {
                        t.set("submit_action", sa)?;
                    }
                    if let Ok(sl) = o.get::<String>("submit_label") {
                        t.set("submit_label", sl)?;
                    }
                }
                Ok(t)
            },
        )?,
    )?;
    ui.set(
        "visualizer",
        lua.create_function(|lua, (mode, opts): (String, Option<Table>)| {
            let t = lua.create_table()?;
            t.set("type", "visualizer")?;
            t.set("mode", mode)?;
            if let Some(o) = opts {
                if let Ok(playing) = o.get::<bool>("playing") {
                    t.set("playing", playing)?;
                }
                if let Ok(vol) = o.get::<f64>("volume") {
                    t.set("volume", vol)?;
                }
                if let Ok(spd) = o.get::<f64>("speed") {
                    t.set("speed", spd)?;
                }
            }
            Ok(t)
        })?,
    )?;
    ui.set(
        "grid",
        lua.create_function(|lua, (columns, children): (usize, Table)| {
            let t = lua.create_table()?;
            t.set("type", "grid")?;
            t.set("columns", columns.clamp(1, 6))?;
            t.set("children", children)?;
            Ok(t)
        })?,
    )?;
    ui.set(
        "card",
        lua.create_function(|lua, opts: Table| {
            let t = lua.create_table()?;
            t.set("type", "card")?;
            if let Ok(id) = opts.get::<String>("id") {
                t.set("id", id)?;
            }
            let title = opts.get::<String>("title").unwrap_or_default();
            t.set("title", title)?;
            if let Ok(sub) = opts.get::<String>("subtitle") {
                t.set("subtitle", sub)?;
            }
            if let Ok(eb) = opts.get::<String>("eyebrow") {
                t.set("eyebrow", eb)?;
            }
            if let Ok(ic) = opts.get::<String>("icon") {
                t.set("icon", ic)?;
            }
            if let Ok(bg) = opts.get::<String>("badge") {
                t.set("badge", bg)?;
            }
            if let Ok(var) = opts.get::<String>("variant") {
                t.set("variant", var)?;
            }
            t.set("accent", opts.get::<bool>("accent").unwrap_or(false))?;
            t.set("active", opts.get::<bool>("active").unwrap_or(false))?;
            Ok(t)
        })?,
    )?;
    Ok(())
}

fn build_ctx_json(
    manifest: &ExtensionManifest,
    playback: &ExtPlaybackCtx,
    storage_map: &HashMap<String, HashMap<String, serde_json::Value>>,
) -> serde_json::Value {
    let can_read_playback = manifest.permissions.iter().any(|p| p == "playback:read");
    let can_storage = manifest.permissions.iter().any(|p| p == "storage");
    let now_sec = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    serde_json::json!({
        "id": manifest.id,
        "api_version": API_VERSION,
        "now_sec": now_sec,
        "local_hour": current_local_hour(),
        "playback": if can_read_playback { serde_json::to_value(playback).ok() } else { None },
        "storage": if can_storage {
            storage_map.get(&manifest.id).cloned().unwrap_or_default()
        } else {
            HashMap::new()
        },
    })
}

fn current_local_hour() -> u32 {
    unsafe {
        let mut t: libc::time_t = 0;
        libc::time(&raw mut t);
        let mut tm: libc::tm = std::mem::zeroed();
        if !libc::localtime_r(&raw const t, &raw mut tm).is_null() {
            return (tm.tm_hour.clamp(0, 23)) as u32;
        }
    }
    12
}

fn lua_has_fn(lua: &Lua, name: &str) -> bool {
    matches!(lua.globals().get::<LuaValue>(name), Ok(LuaValue::Function(_)))
}

fn invoke_lua_hook(
    lua: &Lua,
    bridge: &SharedBridge,
    fn_name: &str,
    args: &[serde_json::Value],
) -> Result<Vec<ExtHostCommand>, String> {
    let globals = lua.globals();
    let Ok(func) = globals.get::<mlua::Function>(fn_name) else {
        return Ok(Vec::new());
    };

    let mut lua_args = mlua::MultiValue::new();
    for a in args {
        let v = json_to_lua(lua, a).map_err(|e| format!("arg encode: {e}"))?;
        lua_args.push_back(v);
    }

    let ret: LuaValue = func
        .call(lua_args)
        .map_err(|e| format!("{fn_name}() failed: {e}"))?;

    let mut cmds = if let Ok(mut b) = bridge.lock() {
        std::mem::take(&mut b.pending_cmds)
    } else {
        Vec::new()
    };

    if matches!(ret, LuaValue::Nil) {
        return Ok(cmds);
    }

    let json_ret: serde_json::Value = lua
        .from_value(ret)
        .map_err(|e| format!("{fn_name}() return decode: {e}"))?;

    if let Some(arr) = json_ret.as_array() {
        for item in arr {
            match serde_json::from_value::<ExtHostCommand>(item.clone()) {
                Ok(cmd) => cmds.push(cmd),
                Err(e) => return Err(format!("{fn_name}() invalid host action: {e}")),
            }
        }
    }

    Ok(cmds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ExtTrackSummary;

    fn test_pb() -> ExtPlaybackCtx {
        ExtPlaybackCtx {
            playing: true,
            paused: false,
            stopped: false,
            muted: false,
            position_sec: 12.0,
            duration_sec: 180.0,
            volume: 85.0,
            speed: 1.0,
            pitch_st: 0.0,
            link_pitch: false,
            preserve_formant: true,
            spatial_on: false,
            spatial_hz: 0.18,
            shuffle: false,
            loop_mode: "All".into(),
            codec: Some("mp3".into()),
            title: "Night Drive".into(),
            artist: "Synth".into(),
            album: "Horizon".into(),
            track_path: "/tmp/night.mp3".into(),
            lyric: "Neon lights".into(),
            bpm: Some(128.0),
            energy: 0.68,
            peak_db: -2.4,
            bitrate_kbps: Some(320),
            sample_rate_hz: Some(44100),
            channels: Some(2),
            beat_pulse: 0.5,
            bands: vec![0.4; 16],
        }
    }

    #[test]
    fn test_sdk_imperative_ui_storage_tick_and_default_deny() {
        let root = std::env::temp_dir().join("tntron_ext_sdk_test");
        let _ = std::fs::remove_dir_all(&root);
        let ext_dir = root.join("sdk-test");
        std::fs::create_dir_all(&ext_dir).unwrap();

        let manifest = r#"{
          "id": "user.eq-probe",
          "name": "EQ Probe",
          "version": "1.0.0",
          "author": "Tester",
          "main": "main.lua",
          "permissions": ["playback:read", "audio:dsp", "ui:sidebar", "ui:settings", "ui:topbar", "storage", "notify"]
        }"#;
        let script = r#"
function on_load(_ctx)
    local boost = tntron.storage.get("boost", 0)
    tntron.ui.register_sidebar({
        id = "eq-tab",
        label = "EQ Probe",
        icon = "sliders",
        widgets = {
            tntron.ui.heading("Equalizer"),
            tntron.ui.row({
                tntron.ui.button("preset_flat", "Flat", { active = boost == 0 }),
                tntron.ui.button("preset_bass", "Bass+", { accent = true, active = boost > 0 }),
            }),
            tntron.ui.slider("gain", "Bass", -12, 12, 1, boost, "dB"),
        }
    })
    tntron.ui.register_settings({
        title = "EQ Probe Panel",
        widgets = { tntron.ui.progress("Load", 50, 100) }
    })
    tntron.ui.set_topbar_slot({ text = "EQ Ready", icon = "sliders", accent = true, action_id = "quick_toggle" })
end

function on_action(_ctx, action_id, val)
    if action_id == "preset_bass" then
        tntron.storage.set("boost", 6)
        tntron.audio.set_filter("equalizer=f=100:width_type=h:w=100:g=6")
        tntron.ui.toast("Bass +6 dB")
        -- Should fail with permission error because queue:control was not granted:
        local ok, _ = pcall(function() tntron.queue.clear() end)
        assert(not ok, "expected permission error for queue:control")
    elseif action_id == "gain" then
        tntron.storage.set("boost", val)
    end
end

function on_tick(_ctx)
    local pb = tntron.playback.get()
    if pb and pb.playing then
        tntron.ui.set_topbar_slot({ text = "EQ " .. tostring(math.floor(pb.position_sec)) .. "s", icon = "sliders" })
    end
end
"#;
        std::fs::write(ext_dir.join("manifest.json"), manifest).unwrap();
        std::fs::write(ext_dir.join("main.lua"), script).unwrap();

        let mut host = ExtensionHost::new();
        crate::bundled_extensions::seed_missing_into(&root);
        host.discover(&root);
        assert!(root.join("_template/API.md").exists());
        assert!(host.cards().iter().any(|c| c.id == "user.eq-probe"));

        let pb = test_pb();
        let lib = ExtLibraryCtx::default();
        let mut storage: HashMap<String, HashMap<String, serde_json::Value>> = HashMap::new();

        let (on, _, _) = host.toggle("user.eq-probe", &pb, &lib, &storage);
        assert!(on);
        assert_eq!(host.sidebar_tabs.len(), 1);
        assert_eq!(host.settings_panels.len(), 1);
        assert_eq!(host.topbar_slots.len(), 1);
        assert_eq!(host.topbar_slots[0].action_id.as_deref(), Some("quick_toggle"));

        let action_cmds = host.dispatch_action(
            "user.eq-probe",
            "preset_bass",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(action_cmds.iter().any(|(_, c)| matches!(c, ExtHostCommand::SetAudioFilter { .. })));
        assert!(action_cmds.iter().any(|(_, c)| matches!(c, ExtHostCommand::StorageSet { key, .. } if key == "boost")));
        assert!(action_cmds.iter().any(|(_, c)| matches!(c, ExtHostCommand::Toast { .. })));
        assert!(!action_cmds.iter().any(|(_, c)| matches!(c, ExtHostCommand::QueueClear)));

        storage
            .entry("user.eq-probe".into())
            .or_default()
            .insert("boost".into(), serde_json::json!(6));

        let _ = host.dispatch_tick(&pb, &lib, &storage);
        assert_eq!(host.topbar_slots[0].text, "EQ 12s");

        let (off, disabled, _) = host.toggle("user.eq-probe", &pb, &lib, &storage);
        assert!(!off);
        assert_eq!(disabled, vec!["user.eq-probe".to_string()]);
        assert!(host.sidebar_tabs.is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_manifest_blocks_path_traversal() {
        let m = ExtensionManifest {
            id: "evil.ext".into(),
            name: "Evil".into(),
            version: "1.0.0".into(),
            description: String::new(),
            author: String::new(),
            main: "../../etc/passwd".into(),
            api_version: "1.0".into(),
            permissions: Vec::new(),
            requires: Vec::new(),
        };
        assert!(ExtensionHost::validate_manifest(&m).is_err());
    }

    #[test]
    fn test_all_29_bundled_extensions_load_tick_action_and_cascade() {
        let root = std::env::temp_dir().join("tntron_bundled_29_test");
        let _ = std::fs::remove_dir_all(&root);

        let mut host = ExtensionHost::new();
        crate::bundled_extensions::seed_missing_into(&root);
        host.discover(&root);

        let cards = host.cards();
        assert_eq!(
            cards.len(),
            crate::bundled_extensions::BUNDLED_EXTENSIONS.len(),
            "expected all bundled extensions"
        );
        for c in &cards {
            assert!(c.error.is_none(), "manifest error in {}: {:?}", c.id, c.error);
        }

        let idle_pb = ExtPlaybackCtx {
            playing: false,
            paused: false,
            stopped: true,
            muted: false,
            position_sec: 0.0,
            duration_sec: 0.0,
            volume: 100.0,
            speed: 1.0,
            pitch_st: 0.0,
            link_pitch: false,
            preserve_formant: true,
            spatial_on: false,
            spatial_hz: 0.18,
            shuffle: false,
            loop_mode: "Off".into(),
            codec: None,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            track_path: String::new(),
            lyric: String::new(),
            bpm: None,
            energy: 0.0,
            peak_db: -96.0,
            bitrate_kbps: None,
            sample_rate_hz: None,
            channels: None,
            beat_pulse: 0.0,
            bands: vec![0.0; 16],
        };
        let pb = test_pb();
        let lib = ExtLibraryCtx {
            current_dir: "/tmp/music".into(),
            tracks: vec![ExtTrackSummary {
                title: "Night Drive".into(),
                artist: "Synth".into(),
                album: "Horizon".into(),
                folder: "music".into(),
                path: "/tmp/night.mp3".into(),
                duration_sec: 180.0,
                is_favorite: true,
                bpm: Some(128.0),
                bitrate_kbps: Some(320),
                sample_rate_hz: Some(44100),
                channels: Some(2),
            }],
            library_tracks: vec![ExtTrackSummary {
                title: "Night Drive".into(),
                artist: "Synth".into(),
                album: "Horizon".into(),
                folder: "music".into(),
                path: "/tmp/night.mp3".into(),
                duration_sec: 180.0,
                is_favorite: true,
                bpm: Some(128.0),
                bitrate_kbps: Some(320),
                sample_rate_hz: Some(44100),
                channels: Some(2),
            }],
            favorites: vec!["/tmp/night.mp3".into()],
            folders: Vec::new(),
            queue: Vec::new(),
        };
        let storage: HashMap<String, HashMap<String, serde_json::Value>> = HashMap::new();

        // Enable every single one of the 25 extensions under idle startup (no track, codec=None)
        let all_ids: Vec<String> = host.cards().into_iter().map(|c| c.id).collect();
        for id in &all_ids {
            let _ = host.enable(id, &idle_pb, &lib, &storage);
        }
        let _ = host.dispatch_tick(&idle_pb, &lib, &storage);

        for c in host.cards() {
            assert!(c.enabled, "extension {} failed to enable: {:?}", c.id, c.error);
            assert!(c.error.is_none(), "runtime error on load for {}: {:?}", c.id, c.error);
        }

        // Theme packs registered all 19 themes (4 neon + 9 festival + 6 dev)
        assert_eq!(host.themes.len(), 19, "expected 19 registered extension themes");

        // Fire lifecycle events and 1Hz ticks across all 25 active extensions
        let _ = host.dispatch_event("trackChange", &pb, &lib, &storage);
        let _ = host.dispatch_event("playbackStateChange", &pb, &lib, &storage);
        let _ = host.dispatch_tick(&pb, &lib, &storage);

        // Exercise representative interactive actions across extensions
        let eq_cmds = host.dispatch_action(
            "tunotron.equalizer",
            "preset:bass",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(eq_cmds
            .iter()
            .any(|(_, c)| matches!(c, ExtHostCommand::SetAudioFilter { filter } if filter.contains("equalizer="))));

        let spatial_cmds = host.dispatch_action(
            "tunotron.spatial-audio",
            "mode:8d",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(spatial_cmds
            .iter()
            .any(|(_, c)| matches!(c, ExtHostCommand::SetAudioFilter { filter } if filter.contains("apulsator="))));

        let speed_cmds = host.dispatch_action(
            "tunotron.better-speed",
            "preset:nightcore",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(speed_cmds
            .iter()
            .any(|(_, c)| matches!(c, ExtHostCommand::SetSpeed { speed } if (*speed - 1.25).abs() < 0.01)));
        assert!(speed_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::AudioSetPitch {
                pitch_st,
                link_pitch: Some(true),
                ..
            } if *pitch_st > 3.0
        )));

        let sleep_cmds = host.dispatch_action(
            "tunotron.sleep-timer",
            "set:15",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(sleep_cmds
            .iter()
            .any(|(_, c)| matches!(c, ExtHostCommand::Toast { .. })));

        let folder_cmds = host.dispatch_action(
            "openify.better-folders",
            "create_folder",
            serde_json::json!("SynthMix"),
            &pb,
            &lib,
            &storage,
        );
        assert!(folder_cmds
            .iter()
            .any(|(_, c)| matches!(c, ExtHostCommand::FsCreateFolder { name } if name == "SynthMix")));

        let rss_cmds = host.dispatch_action(
            "tunotron.rss-feeds",
            "fetch_url",
            serde_json::json!("https://example.com/feed.xml"),
            &pb,
            &lib,
            &storage,
        );
        assert!(rss_cmds
            .iter()
            .any(|(_, c)| matches!(c, ExtHostCommand::NetFetchRss { .. })));

        let yt_cmds = host.dispatch_action(
            "openify.yt-dlp",
            "search",
            serde_json::json!("synthwave"),
            &pb,
            &lib,
            &storage,
        );
        assert!(yt_cmds
            .iter()
            .any(|(_, c)| matches!(c, ExtHostCommand::NetYtSearch { .. })));

        let lib_cmds = host.dispatch_action(
            "openify.better-library",
            "play:/tmp/music",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(lib_cmds
            .iter()
            .any(|(_, c)| matches!(c, ExtHostCommand::LibraryPlayFolder { path, shuffle: false } if path == "/tmp/music")));

        let details_cmds = host.dispatch_action(
            "openify.song-details",
            "bind_spatial",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(details_cmds
            .iter()
            .any(|(_, c)| matches!(c, ExtHostCommand::AudioSetSpatial { enabled: true, .. })));

        let hw_cmds = host.dispatch_action(
            "openify.daetron-daeboard",
            "apply_hex",
            serde_json::json!("ff00aa"),
            &pb,
            &lib,
            &storage,
        );
        assert!(hw_cmds
            .iter()
            .any(|(_, c)| matches!(c, ExtHostCommand::StorageSet { key, .. } if key == "color_hex")));

        let reactive_cmds = host.dispatch_action(
            "tntron.reactive-ui",
            "eqs:mirrored",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(reactive_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::UiSetReactiveConfig { patch }
                if patch.get("eq_style").and_then(|v| v.as_str()) == Some("mirrored")
        )));

        let icon_cmds = host.dispatch_action(
            "tntron.icon-packs",
            "pack:material",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(icon_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::UiSetIconConfig { patch }
                if patch.get("pack").and_then(|v| v.as_str()) == Some("material")
        )));

        let trans_cmds = host.dispatch_action(
            "tntron.transparency",
            "pre:glass",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(trans_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::UiSetTransparencyConfig { patch }
                if patch.get("enabled").and_then(|v| v.as_bool()) == Some(true)
        )));

        let popup_cmds = host.dispatch_action(
            "tntron.popup-player",
            "term:kitty",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(popup_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::UiSetPopupConfig { patch }
                if patch.get("terminal").and_then(|v| v.as_str()) == Some("kitty")
        )));

        let btn_cmds = host.dispatch_action(
            "tntron.button-packs",
            "pre:cyberpunk",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(btn_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::UiSetButtonConfig { patch }
                if patch.get("preset").and_then(|v| v.as_str()) == Some("cyberpunk")
        )));
        let btn_share = host.dispatch_action(
            "tntron.button-packs",
            "copy_share",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(btn_share.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::UiCopyClipboard { text } if text.contains("tntron-buttons-v1")
        )));

        let rui_cmds = host.dispatch_action(
            "tntron.reactive-ui",
            "glw:spiral",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(rui_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::UiSetReactiveConfig { patch }
                if patch.get("glow_particle_style").and_then(|v| v.as_str()) == Some("spiral")
        )));

        assert!(host.themes.iter().any(|t| {
            t.slug == "catppuccin" && t.logo_color.as_deref() == Some("#cba6f7")
        }));
        let logo_cmds = host.dispatch_action(
            "tntron.better-themes",
            "logo_frame:glow",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(logo_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::UiSetLogoConfig { patch }
                if patch.get("frame_style").and_then(|v| v.as_str()) == Some("glow")
        )));

        assert!(host.compact_menu, "openify.compact-menu should toggle compact_menu");
        let _ = host.dispatch_action(
            "openify.compact-menu",
            "toggle",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(!host.compact_menu, "compact-menu toggle should disable compact_menu");

        // Targeted async callbacks (lyrics, translation, rss, ytdlp)
        let _ = host.dispatch_event_to_ext(
            "openify.lyrics",
            "lyricsLoaded",
            serde_json::json!({
                "path": "/tmp/night.mp3",
                "source": "Local .lrc",
                "lines": [{ "time_sec": 5.0, "text": "Neon lights" }]
            }),
            &pb,
            &lib,
            &storage,
        );
        let tr_cmds = host.dispatch_action(
            "openify.lyrics",
            "toggle_trans",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(tr_cmds
            .iter()
            .any(|(_, c)| matches!(c, ExtHostCommand::LyricsTranslate { target_lang, .. } if target_lang == "en")));
        let _ = host.dispatch_event_to_ext(
            "openify.lyrics",
            "lyricsTranslated",
            serde_json::json!({
                "path": "/tmp/night.mp3",
                "target_lang": "en",
                "detected_lang": "en",
                "provider": "Google Translate",
                "translations": ["Neon lights"]
            }),
            &pb,
            &lib,
            &storage,
        );
        let _ = host.dispatch_event_to_ext(
            "tunotron.rss-feeds",
            "rssFetched",
            serde_json::json!({
                "ok": true,
                "url": "https://example.com/feed.xml",
                "title": "Test Cast",
                "episodes": [{ "title": "Ep 1", "audio_url": "https://example.com/1.mp3", "duration": "10:00", "published": "" }]
            }),
            &pb,
            &lib,
            &storage,
        );

        for c in host.cards() {
            assert!(c.error.is_none(), "post-action error in {}: {:?}", c.id, c.error);
        }

        // Verify dependency disable cascades cleanly
        let disabled_lrc = host.disable("openify.lyrics", &pb, &lib, &storage);
        assert!(disabled_lrc.contains(&"openify.better-lyrics".to_string()));
        assert!(disabled_lrc.contains(&"openify.lyrics".to_string()));

        let disabled_viz = host.disable("openify.visualizer", &pb, &lib, &storage);
        assert!(disabled_viz.contains(&"openify.better-visualizer".to_string()));
        assert!(disabled_viz.contains(&"openify.visualizer".to_string()));

        let disabled_ui = host.disable("openify.better-ui", &pb, &lib, &storage);
        assert!(disabled_ui.contains(&"openify.better-library".to_string()));
        assert!(disabled_ui.contains(&"openify.better-ui".to_string()));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_minecraft_link_extension_telemetry_and_actions() {
        let dir = std::env::temp_dir().join(format!(
            "tntron_mc_link_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        crate::bundled_extensions::seed_missing_into(&dir);

        let bridge = McBridgeHandle::new(crate::storage::MinecraftLinkConfig::default());
        let mut host = ExtensionHost::new();
        host.set_mc_bridge(bridge.clone());
        host.discover(&dir);

        let pb = test_pb();
        let lib = ExtLibraryCtx::default();
        let storage: HashMap<String, HashMap<String, serde_json::Value>> = HashMap::new();

        let load_cmds = host.enable("tntron.minecraft-link", &pb, &lib, &storage);
        assert!(load_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::McSetConfig { patch }
                if patch.get("enabled").and_then(|v| v.as_bool()) == Some(true)
        )));

        let action_cmds = host.dispatch_action(
            "tntron.minecraft-link",
            "combat_boost",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(action_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::StorageSet { key, .. } if key == "mc_link_cfg"
        )));

        let ev_cmds = host.dispatch_event_to_ext(
            "tntron.minecraft-link",
            "mcTelemetry",
            serde_json::json!({
                "player_name": "Steve",
                "dimension": "minecraft:the_nether",
                "in_combat": true,
                "boss_active": false,
                "paused": false,
                "in_world": true,
                "underwater": false,
                "tnt_nearby": false
            }),
            &pb,
            &lib,
            &storage,
        );
        assert!(ev_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::AudioSetPreset { preset } if preset == "bass"
        )));

        let notice_cmds = host.dispatch_action(
            "tntron.minecraft-link",
            "test_toast",
            serde_json::Value::Bool(true),
            &pb,
            &lib,
            &storage,
        );
        assert!(notice_cmds.iter().any(|(_, c)| matches!(
            c,
            ExtHostCommand::McSendNotice { kind, .. } if kind == "toast"
        )));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
