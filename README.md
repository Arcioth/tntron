# tntron

GPU-native GUI & CLI music workstation built with **Rust**, **iced 0.14**, and headless **mpv** — uniting **Openify**'s modern desktop layout, **Tunotron**'s sandboxed Lua extension SDK, and **Vocatron**'s headless telemetry engine.

## Architecture

- [`crates/tntron-core`](crates/tntron-core):
  - **Extension API v1.0 (`ext_api.rs`, `extensions.rs`)**: Discovers `<folder>/manifest.json` + `<main>.lua` from `~/.config/tntron/extensions/` (automatically seeding `_template/` with `manifest.json`, `main.lua`, and `API.md`). Runs each plugin in an isolated Lua 5.4 VM exposing the `tntron.*` SDK (`tntron.playback`, `tntron.queue`, `tntron.library`, `tntron.audio`, `tntron.ui`, `tntron.themes`, `tntron.storage`), default-deny permissions (`playback:read`, `playback:control`, `queue:control`, `library:read`, `audio:dsp`, `fs:write`, `ui:sidebar`, `ui:settings`, `ui:topbar`, `ui:player_widget`, `themes:register`, `storage`, `notify`, `network`), dependency cascades (`requires`/`used_by`), named multi-extension mpv `-af` stage composition, and 1 Hz `on_tick` hooks.
  - **Atomic Persistence (`storage.rs`)**: Temp-file + rename persistence (`~/.config/tntron/state.json`) for active theme, enabled extensions, per-extension storage, sort order, view density, favorites, recents, and loops.
  - **Library & Audio Core (`library/`, `clock.rs`, `audio/`)**: Jailed directory browser (`create_folder_in_jail` / `rename_in_jail`), `lofty` metadata & cover-art scanner, 10,000-track O(1) LRU cache, monotonic `PlaybackClock`, `XorShift64` shuffle, headless `mpv` JSON-IPC supervisor observing `audio-codec-name` & `audio-params`.
  - **Unix IPC (`ipc.rs`)**: `$XDG_RUNTIME_DIR/tntron.sock` serving Waybar & CLI parity commands.
- [`crates/tntron-gui`](crates/tntron-gui):
  - **Openify-Inspired Workspace (`app.rs`, `ui_widgets.rs`, `theme.rs`, `icons.rs`)**: Pure `#000000` canvas, `#121212` rounded sidebar cards (`Home`, `Settings`, dynamic extension tabs + `Library` folders), bundled **JetBrainsMono Nerd Font Propo** vector fonticons across all UI chrome (zero emojis), top pill search header, `140×140` Collection Banner, `Compact`/`Normal`/`Big` track rows, collapsible right `Queue` sidebar (`Manual Queue` + `Coming Up Next`), Openify circular theme swatches (12 built-in + extension-registered themes), clean Extension Manager & dynamic `tntron.ui` widget renderer (`Row`, `Heading`, `Text`, `Badge`, `Button`, `Toggle`, `Slider`, `Progress`), and `88px` Player Bar + telemetry footer.

## Authoring an Extension

1. Copy `~/.config/tntron/extensions/_template/` into `~/.config/tntron/extensions/my-ext/`.
2. See `~/.config/tntron/extensions/_template/API.md` for full `manifest.json` permissions, fonticon names, `tntron.*` SDK calls, `tntron.ui` declarative builders, and lifecycle hooks (`on_load`, `on_unload`, `on_event`, `on_tick`, `on_action`).
3. Click **Reload Extensions** in **Settings → Extensions** and toggle it on.

## Running on NixOS

```bash
cd /home/arcioth/Documents/tntron
nix-shell --run "cargo run -p tntron -- ~/Music/yt"
```

### Tests & Lint

```bash
nix-shell --run "cargo test --workspace && cargo clippy --workspace -- -D warnings"
```
