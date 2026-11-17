# tntron Link — Minecraft Fabric 26.2 Mod

Full in-game controller, audio-reactive HUD visualizer, synced karaoke lyrics overlay, library/queue browser, DSP/EQ/pitch rack, theme/extension manager, and game-reactive automation bridge for **tntron** on **Minecraft 26.2 (Fabric)**.

## Features

- **In-Game Control Center (`F8` or `/tntron`)**:
  - **Now Playing**: Live album cover art, track metadata (codec, bitrate, sample rate, BPM), interactive click-to-seek bar with A-B loop markers, full transport deck (`Prev`, `Play/Pause`, `Stop`, `Next`, `Shuffle`, `Loop`, `Favorite`), quick seek (`-30s` to `+30s`), live 16-band spectrum analyzer, and synced lyrics window.
  - **DSP & Audio**: Volume slider & mute, speed slider (`0.25x`–`3.00x`) with presets, pitch shifting (`-12` to `+12 st`, Link Pitch, Preserve Formant), DSP presets (`Flat`, `Bass Boost`, `Vocal`, `Warm`, `Rock`, `8D Spatial`), 3-band EQ (`Bass`, `Mid`, `Treble`), 8D Spatial orbit Hz, A-B section loop (`Mark A/B`, `Clear`, `Save`, `Load`), and Sleep Timer.
  - **Library & Queue**: Live search across your entire tntron library, folder/playlist browser (`Open`, `Play`, `Shuffle`, `Queue Folder`), track actions (`Play`, `+Queue`, `Play Next`, `Favorite`), `Play Favorites`, `Play All`, `Rescan`, and queue/upcoming management.
  - **Themes, Extensions & Config**: One-click switcher for all tntron built-in & extension themes, live toggle for tntron Lua extensions, plus full HUD/reactive/connection settings.
- **Customizable HUD Overlay (`F10`)**:
  - Translucent card in any screen corner with cover art, progress bar, BPM/speed/DSP badges, smoothed 16-band spectrum, and a centered synced karaoke lyrics bar above the hotbar.
  - **Beat-Reactive Effects**: Optional beat edge vignette glow, musical note particles, and subtle FOV pulse.
  - **Auto-Mute Vanilla Music**: Automatically mutes Minecraft's background music while tntron is playing and restores it when paused or stopped.
- **`/tntron` Client Commands**:
  - `/tntron`, `/tntron play|pause|toggle|stop|next|prev|mute|loop|shuffle|fav`
  - `/tntron vol <0..100>`, `/tntron seek <sec>`, `/tntron speed <0.25..3.0>`, `/tntron pitch <-12..12>`
  - `/tntron dsp <flat|bass|vocal|warm|rock|8d>`, `/tntron spatial`, `/tntron ab <mark_a|mark_b|clear|save|load>`
  - `/tntron sleep <minutes|off>`, `/tntron theme <slug>`, `/tntron search <query>`, `/tntron status`
  - `/tntron hud`, `/tntron lyrics`, `/tntron reactive`, `/tntron reconnect`
- **Bidirectional `minecraft-link` Extension**:
  - Enable **Minecraft Link (Fabric 26.2)** inside tntron (**Extensions -> Minecraft Link**) to start the localhost TCP bridge (`127.0.0.1:26262`, optional auth token) and unlock game-reactive rules (auto-pause/duck on MC pause, combat/boss/TNT boost, underwater low-pass filter, dimension DSP & themes, and in-game track announcements).

## Requirements

- Minecraft **26.2**
- **Fabric Loader >= 0.20.1**
- **Fabric API** (`0.161.0+26.2` or compatible)
- **Java 25+**

## Building from Source

```bash
cd minecraft-mod
./gradlew build
```

Output JAR: `minecraft-mod/build/libs/tntron-fabric-26.2-<version>.jar`
