# tntron Remote — Android Companion App

Native Android companion app for **tntron** (Android 8.0+ / API 26+).

## Features

- **Player**: Live album cover art, 16-band audio spectrum visualizer, seek bar, full transport controls (`Prev`, `Play/Pause`, `Next`, `Stop`, `Shuffle`, `Loop`, `Favorite`), PC volume & mute, and live synced lyrics.
- **DSP & EQ**: Playback speed (`0.25x`–`3.00x`), pitch shifting & vinyl pitch link, 6 DSP presets (`Flat`, `Bass+`, `Vocal`, `Warm`, `Rock`, `8D Spatial`), 3-band EQ (`Bass`, `Mid`, `Treble`), 8D Spatial orbit Hz, A-B loop controls, and Sleep Timer.
- **Library & Queue**: Live search across your PC music library, folder/playlist browser, play/queue actions, favorites, and active queue management.
- **Audio & Bluetooth (Issue #2)**:
  - 1-tap switch between **PC (`tntron`)** and **Android (Reverse Bluetooth A2DP)** audio sources.
  - Switch PC audio output sinks (speakers, headphones, Bluetooth earbuds) via PipeWire.
  - Connect/disconnect PC Bluetooth devices, trigger **Stream Phone -> PC (A2DP)**, control reverse-BT stream volume, and control Android AVRCP playback.
- **Connect, Themes & Extensions**:
  - **Bluetooth Link (RFCOMM Ch 22)**: Connect directly to your paired PC over Bluetooth — no Wi-Fi, IP address, or firewall port opening needed.
  - **Live Host UI Sync**: Automatically mirrors your PC's active **tntron** theme palette (built-in & extension themes) and `ButtonPackConfig` (per-corner rounding, cyberpunk corner cuts, gradients, outlines, and glow).
  - Optional Wi-Fi LAN / USB (`adb reverse`) TCP fallback, theme switcher, Hide-UI toggle, and Lua extension manager.

## Connecting to tntron on PC

### Option A: Bluetooth Link (Recommended — Zero Config)
1. Pair your Android/HarmonyOS device with your PC via Bluetooth.
2. Open **tntron** on your PC (listens automatically on Bluetooth RFCOMM channel `22`).
3. Open **tntron Remote** on Android, grant **Nearby Devices / Bluetooth** permission when prompted, pick your PC in the **Connect** tab, and tap **Connect Bluetooth** (auto-connects on subsequent launches).

### Option B: Wi-Fi LAN or USB (`adb reverse`)
1. Open **tntron** on your PC and open **Extensions -> Android & Bluetooth Link** to see your PC's LAN IP and port (`26263`), and optionally set a Pairing PIN.
2. In **tntron Remote** -> **Connect**, enter your PC IP, port (`26263`), and PIN (if set), and tap **Connect TCP**.
   - **USB alternative**: connect via USB and run `adb reverse tcp:26263 tcp:26263` on your PC, then tap **USB (127.0.0.1)** in the app.

## Building from Source

```bash
cd android-app
./gradlew assembleRelease
```

Output APK: `android-app/app/build/outputs/apk/release/app-release.apk`
