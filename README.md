<div align="center">

<img src="assets/icons/hicolor/scalable/apps/tntron.svg" alt="tntron icon" width="112" height="112" />
&nbsp;&nbsp;
<img src="assets/icons/tntron-logo.svg" alt="tntron logo mark" width="112" height="100" />

# tntron

**GPU-native music workstation for Linux — built with Rust, `iced 0.14`, headless `mpv`, and a sandboxed Lua 5.4 extension engine.**

[![License: MIT](https://img.shields.io/badge/License-MIT-1db954.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](Cargo.toml)
[![Nix Flake Ready](https://img.shields.io/badge/Nix-Flake%20%26%20Legacy-5277C3.svg)](flake.nix)
[![Nightly Builds](https://img.shields.io/badge/Releases-Nightly%20%2B%20GIFs-7c3aed.svg)](https://github.com/Arcioth/tntron/releases)

</div>

---

## Highlights

- **Signature Brand Logo in Every Theme**: Built around the `tntron-logo.svg` dissolving-pixel mark, automatically tinted per theme (all 12 built-in themes + extension themes) and customizable in **Settings -> Better Themes** (theme accent, monochrome, custom `#hex`, `.svg`/`.png`/`.jxl` file upload, frame badge/glow, and beat-reactive pulse).
- **37+ Sandboxed Lua 5.4 Extensions**: Default-deny capability permissions (`playback:read`, `playback:control`, `audio:dsp`, `ui:*`, `themes:register`, `storage`, `network`) with hot reload, dependency cascades, and declarative UI widgets. See the full [Extension Developer Guide](docs/EXTENSION_GUIDE.md).
- **Visual & Reactive Studio**:
  - **Reactive UI & Particles**: Glow particles, spiral, infinity 8, anime speed lines, bubbles, flare, glamour, aurora, and custom `.svg` / `.png` / `.jxl` beat-reactive sprites.
  - **Button Packs**: Built-in `Openify`, `Windows XP`, `Liquid Glass`, and `Cyberpunk` presets plus selective per-corner rounding & cyberpunk cropping, hue shift, outline, inline/outer glow, bevel emboss, gradient overlays, and reactive hover states.
  - **Icon Packs & Custom Fonts**: 12 bundled icon packs plus runtime `.ttf`/`.otf` font icon importing.
  - **Opacity & Hide-UI Focus Mode**: Window transparency tinting, custom image backgrounds, and a configurable top-right **Hide UI** toggle for distraction-free Focus Mode keeping only reactive visuals, album art, and controls visible.
  - **Discord Rich Presence**: Pure-Rust Discord/Vesktop/Flatpak IPC client showing fully configurable live details (Title, Artist, Album, BPM, Codec/Sample-Rate, Speed/Pitch, Loop/Shuffle, Queue position, Timestamps, Action Button, Custom Templates, and Privacy Mode) with a real-time preview in **Settings -> Discord Rich Presence**.
  - **Bass-Reactive GIF Visualizer**: Dedicated background GIF decoder synced to sub-bass transients.
- **Portable `.tnt` Configuration & Profile Sharing**:
  - Top-right user **Profile & Listening Stats** supporting adjustable `.jxl`, `.png`, `.jpg`, `.webp` avatars and stats export.
  - Built-in `.tnt` export/import bundles (`~/.config/tntron/tnt/`) including settings and extensions, plus copy/paste preset sharing for Reactive UI, Button Packs, and Profile stats (with opt-in approval prompt on profile stat imports).
- **Native Multi-DE/WM & Fullscreen Support**:
  - First-class Wayland (`application_id = "tntron"`) and X11 (`WM_CLASS = tntron`) integration across **XFCE, Hyprland, Sway, KDE Plasma, GNOME, i3, COSMIC, MATE, and LXQt**.
  - Native `F11` fullscreen toggle and terminal popup visualizer (`tntron-ctl`).

---

## Quick Install with Nix

### 1. Run Immediately Without Installing (Flakes)

```bash
nix run github:Arcioth/tntron
```

### 2. Install Permanently into Your User Profile

```bash
nix profile install github:Arcioth/tntron
```

*(Or from a local clone: `nix profile install .` or legacy `nix-env -if .`)*

### 3. NixOS Configuration (Flake Overlay + System Package)

Add `tntron` to your `flake.nix` inputs and install cleanly with icons, desktop entry, `mpv`, `djxl`, `imagemagick`, and Wayland/X11 libraries wrapped:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    tntron.url = "github:Arcioth/tntron";
  };

  outputs = { self, nixpkgs, tntron, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ({ pkgs, ... }: {
          nixpkgs.overlays = [ tntron.overlays.default ];
          environment.systemPackages = [ pkgs.tntron ];
        })
      ];
    };
  };
}
```

Or add directly with `pkgs.tntron`:
```nix
environment.systemPackages = [ inputs.tntron.packages.${pkgs.system}.default ];
```

---

## Nightly Releases (Includes Pre-Bundled GIFs & Extensions)

Every nightly build ships **ready to run** with `tntron`, `tntron-ctl`, all bundled extensions, desktop icons, `.desktop` launcher, and the full curated **bass-reactive GIF pack** (`gifs/`):

1. Download **`tntron-nightly-linux-x86_64.tar.gz`** from the [Nightly Release Page](https://github.com/Arcioth/tntron/releases/tag/nightly) (or via `gh`):
   ```bash
   gh release download nightly -R Arcioth/tntron -p "tntron-nightly-linux-*.tar.gz"
   tar -xzf tntron-nightly-linux-*.tar.gz
   cd tntron-nightly-linux-*
   ./install.sh
   ```
2. Run `tntron` directly from your app launcher or terminal (`~/Music` is scanned by default, or pass a folder path: `tntron ~/Music/yt`).

---

## Build from Source Yourself

### Option A: Using Nix (`nix develop` or `nix-shell`)

All Rust toolchains, headers, `mpv`, `djxl`, `imagemagick`, Wayland, X11, GL, and Vulkan dependencies are pre-configured:

```bash
git clone https://github.com/Arcioth/tntron.git
cd tntron

# Legacy nix-shell:
nix-shell --run "cargo build --release --workspace"

# Or Flake devShell:
nix develop -c cargo build --release --workspace

# Run directly:
nix-shell --run "cargo run --release -p tntron -- ~/Music"
```

### Option B: Standard Linux Distributions (Debian/Ubuntu, Fedora, Arch)

Install the Rust toolchain (`rustup.rs`) and system packages:

- **Arch Linux**:
  ```bash
  sudo pacman -S --needed base-devel pkgconf mpv wayland libxkbcommon vulkan-icd-loader mesa fontconfig freetype2 libx11 libxcursor libxi libxrandr imagemagick libjxl yt-dlp
  ```
- **Ubuntu / Debian**:
  ```bash
  sudo apt-get update && sudo apt-get install -y \
    build-essential pkg-config libmpv-dev mpv libwayland-dev libxkbcommon-dev \
    libvulkan-dev libgl1-mesa-dev libfontconfig1-dev libfreetype-dev \
    libx11-dev libxcursor-dev libxi-dev libxrandr-dev imagemagick libjxl-tools
  ```
- **Fedora**:
  ```bash
  sudo dnf install -y \
    gcc pkg-config mpv mpv-devel wayland-devel libxkbcommon-devel \
    vulkan-loader-devel mesa-libGL-devel fontconfig-devel freetype-devel \
    libX11-devel libXcursor-devel libXi-devel libXrandr-devel ImageMagick jxl-pixbuf-loader
  ```

Then compile and optionally install desktop assets locally:

```bash
git clone https://github.com/Arcioth/tntron.git
cd tntron
cargo build --release --workspace

# Optional local user install (~/.local/bin + icons + .desktop):
install -Dm755 target/release/tntron ~/.local/bin/tntron
install -Dm755 target/release/tntron-ctl ~/.local/bin/tntron-ctl
install -Dm644 assets/tntron.desktop ~/.local/share/applications/tntron.desktop
cp -r assets/icons/hicolor ~/.local/share/icons/
```

To build a full nightly archive locally (automatically bundling GIFs from `TNTRON_GIFS_DIR`, `./assets/gifs`, or `~/Pictures/gifs`):

```bash
./scripts/build-nightly.sh
```

### Running Tests & Lint

```bash
nix-shell --run "cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings"
```

---

## CLI Options, Display Backends & Shortcuts

```bash
tntron [MUSIC_DIR] [OPTIONS]
```

| Flag / Env | Description |
|---|---|
| `--fullscreen`, `-f` | Launch directly in native fullscreen |
| `--wayland` | Force native Wayland backend (`WINIT_UNIX_BACKEND=wayland`) |
| `--x11` | Force X11/XWayland backend (`WINIT_UNIX_BACKEND=x11`, recommended for XFCE/i3) |
| `--opaque`, `TNTRON_OPAQUE=1` | Disable transparent window surface (helpful on X11 without a compositor) |
| `TNTRON_GIFS_DIR=/path/to/gifs` | Override the directory scanned by the GIF visualizer |
| `tntron-ctl <cmd>` | Send headless IPC commands (`status`, `play-pause`, `next`, `prev`, `volume`, `waybar`, `popup`) |

### Keybindings

| Key | Action |
|---|---|
| `F11` | Toggle native Fullscreen |
| `Space` | Play / Pause |
| `ArrowUp` / `ArrowDown` | Navigate track selection |
| `Enter` | Play selected track / open selected folder |
| `ArrowLeft` / `ArrowRight` | Seek `-5s` / `+5s` |
| `/` | Focus search bar |
| `Esc` | Close active modal / context menu |

---

## Architecture & Extension Guide

- [`crates/tntron-core`](crates/tntron-core): Jailed music scanner, metadata loader (`lofty`), `mpv` JSON-IPC supervisor, monotonic playback clock, Unix socket IPC, `.tnt` bundle engine, and sandboxed **Lua 5.4 Extension Host**.
- [`crates/tntron-gui`](crates/tntron-gui): Hardware-accelerated `iced 0.14` workspace, theme/logo engine, button style composer, reactive canvas particles, fonticon loader, and GIF decoder.
- [`crates/tntron-cli`](crates/tntron-cli): Headless CLI & Waybar IPC controller (`tntron-ctl`).
- [`docs/EXTENSION_GUIDE.md`](docs/EXTENSION_GUIDE.md): Complete guide to creating extensions, permissions, Lua APIs, UI composition, `.tnt` presets, and architecture flowcharts.

---

## License

Licensed under the **MIT License**. See [`LICENSE`](LICENSE) for full details.
