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
- **Connect, Themes & Extensions**: Connect over Wi-Fi LAN or USB (`adb reverse`), switch tntron themes & Hide-UI, and toggle tntron Lua extensions.

## Connecting to tntron on PC

1. Open **tntron** on your PC and enable **Extensions -> Android & Bluetooth Link**.
2. Open the **Android** sidebar tab in tntron to view your PC's LAN IP address and port (`26263` by default), and optionally set a Pairing PIN.
3. In **tntron Remote** on Android, go to the **Connect** tab, enter your PC IP, port (`26263`), and PIN (if set), and tap **Connect**.
   - **USB alternative**: connect via USB and run `adb reverse tcp:26263 tcp:26263` on your PC, then tap **USB (127.0.0.1)** in the app.

## Building from Source

```bash
cd android-app
./gradlew assembleRelease
```

Output APK: `android-app/app/build/outputs/apk/release/app-release.apk`
