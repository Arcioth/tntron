use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

use crate::storage::AndroidLinkConfig;

pub const A2DP_SOURCE_UUID: &str = "0000110a-0000-1000-8000-00805f9b34fb";
pub const A2DP_SINK_UUID: &str = "0000110b-0000-1000-8000-00805f9b34fb";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BluetoothDeviceInfo {
    pub object_path: String,
    pub address: String,
    pub name: String,
    pub alias: String,
    pub connected: bool,
    pub paired: bool,
    pub trusted: bool,
    pub icon: String,
    pub is_audio_source: bool,
    pub is_audio_sink: bool,
    pub battery_pct: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AndroidMediaPlayer {
    pub object_path: String,
    pub device_path: String,
    pub device_name: String,
    pub status: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub shuffle: Option<String>,
    pub repeat: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AudioSinkInfo {
    pub index: u32,
    pub name: String,
    pub description: String,
    pub volume_pct: u32,
    pub muted: bool,
    pub is_default: bool,
    pub is_bluetooth: bool,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BtAudioStreamInfo {
    pub sink_input_index: u32,
    pub name: String,
    pub device_mac: String,
    pub sink_index: u32,
    pub volume_pct: u32,
    pub muted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AndroidBridgeStatus {
    pub enabled: bool,
    pub bluez_available: bool,
    pub adapter_path: String,
    pub adapter_name: String,
    pub adapter_powered: bool,
    pub adapter_discoverable: bool,
    pub devices: Vec<BluetoothDeviceInfo>,
    pub media_players: Vec<AndroidMediaPlayer>,
    pub sinks: Vec<AudioSinkInfo>,
    pub default_sink: String,
    pub bt_streams: Vec<BtAudioStreamInfo>,
    pub active_source: String,
    pub reverse_bt_active: bool,
    pub lan_enabled: bool,
    pub lan_bind: String,
    pub lan_port: u16,
    pub lan_ips: Vec<String>,
    pub android_clients: usize,
    pub last_error: Option<String>,
}

impl Default for AndroidBridgeStatus {
    fn default() -> Self {
        Self {
            enabled: false,
            bluez_available: false,
            adapter_path: "/org/bluez/hci0".to_string(),
            adapter_name: String::new(),
            adapter_powered: false,
            adapter_discoverable: false,
            devices: Vec::new(),
            media_players: Vec::new(),
            sinks: Vec::new(),
            default_sink: String::new(),
            bt_streams: Vec::new(),
            active_source: "pc".to_string(),
            reverse_bt_active: false,
            lan_enabled: true,
            lan_bind: "0.0.0.0".to_string(),
            lan_port: 26263,
            lan_ips: Vec::new(),
            android_clients: 0,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AndroidWorkerCmd {
    RefreshNow,
    BtConnect { device_path: String },
    BtDisconnect { device_path: String },
    BtConnectReverseAudio { device_path: String },
    BtSetDiscoverable { discoverable: bool },
    AvrcpControl {
        player_path: Option<String>,
        action: String,
    },
    SetAudioSink { sink_name: String },
    SetBtStreamVolume { volume_pct: u32 },
    SetBtStreamMute { muted: bool },
    SetActiveSource { source: String },
}

#[derive(Debug, Clone)]
pub struct AndroidBridgeHandle {
    config_tx: watch::Sender<AndroidLinkConfig>,
    status: Arc<RwLock<AndroidBridgeStatus>>,
    android_clients: Arc<AtomicUsize>,
    cmd_tx: Arc<RwLock<Option<mpsc::Sender<AndroidWorkerCmd>>>>,
}

impl AndroidBridgeHandle {
    pub fn new(init_cfg: AndroidLinkConfig) -> Self {
        let mut init_status = AndroidBridgeStatus {
            enabled: init_cfg.enabled,
            lan_enabled: init_cfg.lan_enabled,
            lan_bind: init_cfg.bind_host.clone(),
            lan_port: init_cfg.tcp_port,
            ..Default::default()
        };
        init_status.lan_ips = detect_lan_ips();
        let (config_tx, _) = watch::channel(init_cfg);
        Self {
            config_tx,
            status: Arc::new(RwLock::new(init_status)),
            android_clients: Arc::new(AtomicUsize::new(0)),
            cmd_tx: Arc::new(RwLock::new(None)),
        }
    }

    pub fn attach_worker_tx(&self, tx: mpsc::Sender<AndroidWorkerCmd>) {
        if let Ok(mut g) = self.cmd_tx.write() {
            *g = Some(tx);
        }
    }

    pub fn send_cmd(&self, cmd: AndroidWorkerCmd) {
        if let Ok(g) = self.cmd_tx.read() {
            if let Some(tx) = g.as_ref() {
                let _ = tx.try_send(cmd);
            }
        }
    }

    pub fn update_config(&self, cfg: AndroidLinkConfig) {
        if let Ok(mut st) = self.status.write() {
            st.enabled = cfg.enabled;
            st.lan_enabled = cfg.lan_enabled;
            st.lan_bind = cfg.bind_host.clone();
            st.lan_port = cfg.tcp_port;
        }
        let _ = self.config_tx.send(cfg);
        self.send_cmd(AndroidWorkerCmd::RefreshNow);
    }

    pub fn config_snapshot(&self) -> AndroidLinkConfig {
        self.config_tx.borrow().clone()
    }

    pub fn subscribe_config(&self) -> watch::Receiver<AndroidLinkConfig> {
        self.config_tx.subscribe()
    }

    pub fn inc_clients(&self) {
        let n = self.android_clients.fetch_add(1, Ordering::Relaxed) + 1;
        if let Ok(mut st) = self.status.write() {
            st.android_clients = n;
        }
    }

    pub fn dec_clients(&self) {
        let prev = self.android_clients.fetch_sub(1, Ordering::Relaxed);
        let n = prev.saturating_sub(1);
        if let Ok(mut st) = self.status.write() {
            st.android_clients = n;
        }
    }

    pub fn status_snapshot(&self) -> AndroidBridgeStatus {
        let mut snap = self
            .status
            .read()
            .map(|g| g.clone())
            .unwrap_or_default();
        snap.android_clients = self.android_clients.load(Ordering::Relaxed);
        snap
    }

    fn set_status(&self, mut next: AndroidBridgeStatus) {
        let cfg = self.config_snapshot();
        next.enabled = cfg.enabled;
        next.lan_enabled = cfg.lan_enabled;
        next.lan_bind = cfg.bind_host;
        next.lan_port = cfg.tcp_port;
        next.android_clients = self.android_clients.load(Ordering::Relaxed);
        if let Ok(mut g) = self.status.write() {
            if next.active_source.is_empty() {
                next.active_source = g.active_source.clone();
            }
            *g = next;
        }
    }

    pub fn set_active_source_label(&self, source: &str) {
        if let Ok(mut g) = self.status.write() {
            g.active_source = source.to_string();
        }
    }
}

pub fn spawn_android_worker(handle: AndroidBridgeHandle) -> mpsc::Sender<AndroidWorkerCmd> {
    let (tx, mut rx) = mpsc::channel::<AndroidWorkerCmd>(32);
    handle.attach_worker_tx(tx.clone());

    tokio::spawn(async move {
        let mut poll = tokio::time::interval(Duration::from_millis(2500));
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut cfg_rx = handle.subscribe_config();

        loop {
            tokio::select! {
                Some(cmd) = rx.recv() => {
                    let h = handle.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        execute_worker_cmd(&h, cmd);
                    }).await;
                }
                _ = poll.tick() => {
                    if handle.config_snapshot().enabled {
                        let h = handle.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            refresh_snapshot_blocking(&h);
                        }).await;
                    }
                }
                res = cfg_rx.changed() => {
                    if res.is_err() {
                        break;
                    }
                    if handle.config_snapshot().enabled {
                        let h = handle.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            refresh_snapshot_blocking(&h);
                        }).await;
                    }
                }
            }
        }
    });

    tx
}

fn execute_worker_cmd(handle: &AndroidBridgeHandle, cmd: AndroidWorkerCmd) {
    let mut err_msg: Option<String> = None;
    match cmd {
        AndroidWorkerCmd::RefreshNow => {}
        AndroidWorkerCmd::BtConnect { device_path } => {
            if let Err(e) = call_bluez_device_method(&device_path, "Connect", &[]) {
                err_msg = Some(e);
            }
        }
        AndroidWorkerCmd::BtDisconnect { device_path } => {
            if let Err(e) = call_bluez_device_method(&device_path, "Disconnect", &[]) {
                err_msg = Some(e);
            }
        }
        AndroidWorkerCmd::BtConnectReverseAudio { device_path } => {
            if let Err(e) = call_bluez_device_method(
                &device_path,
                "ConnectProfile",
                &["s", A2DP_SOURCE_UUID],
            ) {
                // Fallback to general Connect if ConnectProfile isn't accepted while disconnected
                if let Err(e2) = call_bluez_device_method(&device_path, "Connect", &[]) {
                    err_msg = Some(format!("{e}; {e2}"));
                }
            }
        }
        AndroidWorkerCmd::BtSetDiscoverable { discoverable } => {
            let adapter = handle.status_snapshot().adapter_path;
            let val = if discoverable { "true" } else { "false" };
            let out = Command::new("busctl")
                .args([
                    "set-property",
                    "org.bluez",
                    &adapter,
                    "org.bluez.Adapter1",
                    "Discoverable",
                    "b",
                    val,
                ])
                .output();
            if let Err(e) = out {
                err_msg = Some(format!("busctl failed: {e}"));
            } else if let Ok(o) = out {
                if !o.status.success() {
                    err_msg = Some(String::from_utf8_lossy(&o.stderr).trim().to_string());
                }
            }
        }
        AndroidWorkerCmd::AvrcpControl {
            player_path,
            action,
        } => {
            let snap = handle.status_snapshot();
            let target = player_path.or_else(|| {
                snap.media_players
                    .first()
                    .map(|p| p.object_path.clone())
            });
            if let Some(path) = target {
                let method = match action.to_ascii_lowercase().as_str() {
                    "play" => Some("Play"),
                    "pause" => Some("Pause"),
                    "stop" => Some("Stop"),
                    "next" => Some("Next"),
                    "prev" | "previous" => Some("Previous"),
                    "toggle" => {
                        let is_playing = snap
                            .media_players
                            .iter()
                            .find(|p| p.object_path == path)
                            .is_some_and(|p| p.status.eq_ignore_ascii_case("playing"));
                        Some(if is_playing { "Pause" } else { "Play" })
                    }
                    _ => None,
                };
                if let Some(m) = method {
                    let out = Command::new("busctl")
                        .args(["call", "org.bluez", &path, "org.bluez.MediaPlayer1", m])
                        .output();
                    if let Ok(o) = out {
                        if !o.status.success() {
                            err_msg = Some(String::from_utf8_lossy(&o.stderr).trim().to_string());
                        }
                    }
                }
            } else {
                err_msg = Some("No active Bluetooth AVRCP media player found".to_string());
            }
        }
        AndroidWorkerCmd::SetAudioSink { sink_name } => {
            if let Err(e) = set_default_sink_and_move_streams(&sink_name) {
                err_msg = Some(e);
            }
        }
        AndroidWorkerCmd::SetBtStreamVolume { volume_pct } => {
            let pct = volume_pct.min(150);
            let snap = handle.status_snapshot();
            for s in &snap.bt_streams {
                let _ = Command::new("pactl")
                    .args([
                        "set-sink-input-volume",
                        &s.sink_input_index.to_string(),
                        &format!("{pct}%"),
                    ])
                    .output();
            }
        }
        AndroidWorkerCmd::SetBtStreamMute { muted } => {
            let snap = handle.status_snapshot();
            let val = if muted { "1" } else { "0" };
            for s in &snap.bt_streams {
                let _ = Command::new("pactl")
                    .args(["set-sink-input-mute", &s.sink_input_index.to_string(), val])
                    .output();
            }
        }
        AndroidWorkerCmd::SetActiveSource { source } => {
            let norm = if source.eq_ignore_ascii_case("android") {
                "android"
            } else {
                "pc"
            };
            handle.set_active_source_label(norm);
            let snap = handle.status_snapshot();
            if norm == "android" {
                for s in &snap.bt_streams {
                    let _ = Command::new("pactl")
                        .args(["set-sink-input-mute", &s.sink_input_index.to_string(), "0"])
                        .output();
                }
                if let Some(p) = snap.media_players.first() {
                    if !p.status.eq_ignore_ascii_case("playing") {
                        let _ = Command::new("busctl")
                            .args([
                                "call",
                                "org.bluez",
                                &p.object_path,
                                "org.bluez.MediaPlayer1",
                                "Play",
                            ])
                            .output();
                    }
                }
            } else if let Some(p) = snap.media_players.first() {
                if p.status.eq_ignore_ascii_case("playing") {
                    let _ = Command::new("busctl")
                        .args([
                            "call",
                            "org.bluez",
                            &p.object_path,
                            "org.bluez.MediaPlayer1",
                            "Pause",
                        ])
                        .output();
                }
            }
        }
    }

    refresh_snapshot_blocking(handle);
    if let Some(err) = err_msg {
        if let Ok(mut st) = handle.status.write() {
            st.last_error = Some(err);
        }
    }
}

fn call_bluez_device_method(
    device_path: &str,
    method: &str,
    extra_args: &[&str],
) -> Result<(), String> {
    if !device_path.starts_with("/org/bluez/") {
        return Err(format!("Invalid BlueZ object path: {device_path}"));
    }
    let mut cmd = Command::new("busctl");
    cmd.args(["call", "org.bluez", device_path, "org.bluez.Device1", method]);
    for a in extra_args {
        cmd.arg(a);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("busctl failed: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(if err.is_empty() {
            format!("BlueZ {method} failed")
        } else {
            err
        });
    }
    Ok(())
}

fn set_default_sink_and_move_streams(sink_name: &str) -> Result<(), String> {
    let out = Command::new("pactl")
        .args(["set-default-sink", sink_name])
        .output()
        .map_err(|e| format!("pactl failed: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }

    // Move existing sink-inputs to the new default sink so audio switches immediately.
    if let Ok(list) = Command::new("pactl")
        .args(["-f", "json", "list", "sink-inputs"])
        .output()
    {
        if list.status.success() {
            let raw = String::from_utf8_lossy(&list.stdout);
            if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(&raw) {
                for item in arr {
                    if let Some(idx) = item.get("index").and_then(|v| v.as_u64()) {
                        let _ = Command::new("pactl")
                            .args(["move-sink-input", &idx.to_string(), sink_name])
                            .output();
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn refresh_snapshot_blocking(handle: &AndroidBridgeHandle) {
    let prev = handle.status_snapshot();
    let mut next = AndroidBridgeStatus {
        active_source: prev.active_source,
        lan_ips: detect_lan_ips(),
        ..Default::default()
    };

    if let Ok(out) = Command::new("busctl")
        .args([
            "--json=short",
            "call",
            "org.bluez",
            "/",
            "org.freedesktop.DBus.ObjectManager",
            "GetManagedObjects",
        ])
        .output()
    {
        if out.status.success() {
            let raw = String::from_utf8_lossy(&out.stdout);
            if let Ok(parsed) = parse_bluez_managed_objects(&raw) {
                next.bluez_available = true;
                next.adapter_path = parsed.adapter_path;
                next.adapter_name = parsed.adapter_name;
                next.adapter_powered = parsed.adapter_powered;
                next.adapter_discoverable = parsed.adapter_discoverable;
                next.devices = parsed.devices;
                next.media_players = parsed.media_players;
            }
        }
    }

    let default_sink = Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    next.default_sink = default_sink.clone();

    if let Ok(out) = Command::new("pactl")
        .args(["-f", "json", "list", "sinks"])
        .output()
    {
        if out.status.success() {
            let raw = String::from_utf8_lossy(&out.stdout);
            next.sinks = parse_pactl_sinks(&raw, &default_sink);
        }
    }

    if let Ok(out) = Command::new("pactl")
        .args(["-f", "json", "list", "sink-inputs"])
        .output()
    {
        if out.status.success() {
            let raw = String::from_utf8_lossy(&out.stdout);
            next.bt_streams = parse_pactl_bt_streams(&raw);
        }
    }

    next.reverse_bt_active = !next.bt_streams.is_empty()
        || next
            .media_players
            .iter()
            .any(|p| p.status.eq_ignore_ascii_case("playing"));

    handle.set_status(next);
}

#[derive(Debug, Default)]
pub struct ParsedBluez {
    pub adapter_path: String,
    pub adapter_name: String,
    pub adapter_powered: bool,
    pub adapter_discoverable: bool,
    pub devices: Vec<BluetoothDeviceInfo>,
    pub media_players: Vec<AndroidMediaPlayer>,
}

fn dbus_val<'a>(props: &'a serde_json::Map<String, Value>, key: &str) -> Option<&'a Value> {
    props.get(key).and_then(|v| v.get("data"))
}

fn dbus_str(props: &serde_json::Map<String, Value>, key: &str) -> String {
    dbus_val(props, key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn dbus_bool(props: &serde_json::Map<String, Value>, key: &str) -> bool {
    dbus_val(props, key)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn dbus_u64(props: &serde_json::Map<String, Value>, key: &str) -> u64 {
    dbus_val(props, key).and_then(|v| v.as_u64()).unwrap_or(0)
}

pub fn parse_bluez_managed_objects(raw_json: &str) -> Result<ParsedBluez, String> {
    let root: Value =
        serde_json::from_str(raw_json).map_err(|e| format!("Invalid BlueZ JSON: {e}"))?;
    let objects = root
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .and_then(|o| o.as_object())
        .ok_or_else(|| "Unexpected BlueZ ObjectManager payload".to_string())?;

    let mut out = ParsedBluez {
        adapter_path: "/org/bluez/hci0".to_string(),
        ..Default::default()
    };

    let mut device_names: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for (path, ifaces_val) in objects {
        let Some(ifaces) = ifaces_val.as_object() else {
            continue;
        };

        if let Some(adapter) = ifaces
            .get("org.bluez.Adapter1")
            .and_then(|v| v.as_object())
        {
            out.adapter_path = path.clone();
            let alias = dbus_str(adapter, "Alias");
            let name = dbus_str(adapter, "Name");
            out.adapter_name = if !alias.is_empty() { alias } else { name };
            out.adapter_powered = dbus_bool(adapter, "Powered");
            out.adapter_discoverable = dbus_bool(adapter, "Discoverable");
        }

        if let Some(dev) = ifaces.get("org.bluez.Device1").and_then(|v| v.as_object()) {
            let address = dbus_str(dev, "Address");
            let name = dbus_str(dev, "Name");
            let alias = dbus_str(dev, "Alias");
            let display = if !alias.is_empty() {
                alias.clone()
            } else if !name.is_empty() {
                name.clone()
            } else {
                address.clone()
            };
            device_names.insert(path.clone(), display);

            let uuids: Vec<String> = dbus_val(dev, "UUIDs")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|u| u.as_str().map(str::to_ascii_lowercase))
                        .collect()
                })
                .unwrap_or_default();

            let is_audio_source = uuids.iter().any(|u| u == A2DP_SOURCE_UUID);
            let is_audio_sink = uuids.iter().any(|u| u == A2DP_SINK_UUID);

            let battery_pct = ifaces
                .get("org.bluez.Battery1")
                .and_then(|v| v.as_object())
                .and_then(|b| dbus_val(b, "Percentage"))
                .and_then(|v| v.as_u64())
                .map(|p| p.min(100) as u8);

            out.devices.push(BluetoothDeviceInfo {
                object_path: path.clone(),
                address,
                name,
                alias,
                connected: dbus_bool(dev, "Connected"),
                paired: dbus_bool(dev, "Paired"),
                trusted: dbus_bool(dev, "Trusted"),
                icon: dbus_str(dev, "Icon"),
                is_audio_source,
                is_audio_sink,
                battery_pct,
            });
        }
    }

    for (path, ifaces_val) in objects {
        let Some(ifaces) = ifaces_val.as_object() else {
            continue;
        };
        if let Some(mp) = ifaces
            .get("org.bluez.MediaPlayer1")
            .and_then(|v| v.as_object())
        {
            let device_path = dbus_str(mp, "Device");
            let device_name = device_names
                .get(&device_path)
                .cloned()
                .unwrap_or_else(|| "Bluetooth Device".to_string());
            let status = dbus_str(mp, "Status");
            let position_ms = dbus_u64(mp, "Position");

            let (mut title, mut artist, mut album, mut duration_ms) =
                (String::new(), String::new(), String::new(), 0u64);
            if let Some(track_map) = dbus_val(mp, "Track").and_then(|v| v.as_object()) {
                title = dbus_str(track_map, "Title");
                artist = dbus_str(track_map, "Artist");
                album = dbus_str(track_map, "Album");
                duration_ms = dbus_u64(track_map, "Duration");
            }
            let shuffle = dbus_val(mp, "Shuffle")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);
            let repeat = dbus_val(mp, "Repeat")
                .and_then(|v| v.as_str())
                .map(ToString::to_string);

            out.media_players.push(AndroidMediaPlayer {
                object_path: path.clone(),
                device_path,
                device_name,
                status: if status.is_empty() {
                    "stopped".to_string()
                } else {
                    status
                },
                title,
                artist,
                album,
                position_ms,
                duration_ms,
                shuffle,
                repeat,
            });
        }
    }

    // Sort: connected first, then phones/audio sources, then alphabetical
    out.devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| b.is_audio_source.cmp(&a.is_audio_source))
            .then_with(|| a.alias.cmp(&b.alias))
    });

    Ok(out)
}

fn parse_pactl_volume_pct(obj: &Value) -> u32 {
    if let Some(vol_map) = obj.get("volume").and_then(|v| v.as_object()) {
        for ch in vol_map.values() {
            if let Some(pct_str) = ch.get("value_percent").and_then(|v| v.as_str()) {
                let digits = pct_str.trim().trim_end_matches('%');
                if let Ok(n) = digits.parse::<u32>() {
                    return n;
                }
            }
        }
    }
    100
}

pub fn parse_pactl_sinks(raw_json: &str, default_sink: &str) -> Vec<AudioSinkInfo> {
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(raw_json) else {
        return Vec::new();
    };
    arr.into_iter()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?.to_string();
            let index = item.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let description = item
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or(&name)
                .to_string();
            let muted = item.get("mute").and_then(|v| v.as_bool()).unwrap_or(false);
            let state = item
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("IDLE")
                .to_string();
            let is_bluetooth = name.starts_with("bluez_")
                || description.to_ascii_lowercase().contains("bluetooth");
            Some(AudioSinkInfo {
                index,
                is_default: name == default_sink,
                name,
                description,
                volume_pct: parse_pactl_volume_pct(&item),
                muted,
                is_bluetooth,
                state,
            })
        })
        .collect()
}

pub fn parse_pactl_bt_streams(raw_json: &str) -> Vec<BtAudioStreamInfo> {
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(raw_json) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in arr {
        let props = item.get("properties").and_then(|v| v.as_object());
        let node_name = props
            .and_then(|p| p.get("node.name"))
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let media_name = props
            .and_then(|p| p.get("media.name"))
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let app_name = props
            .and_then(|p| p.get("application.name"))
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        let is_bt = node_name.starts_with("bluez_input")
            || media_name.to_ascii_lowercase().contains("bluez")
            || app_name.to_ascii_lowercase().contains("bluez")
            || props
                .and_then(|p| p.get("device.api"))
                .and_then(|v| v.as_str())
                .is_some_and(|a| a == "bluez5");

        if !is_bt {
            continue;
        }
        let sink_input_index = item.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let sink_index = item.get("sink").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let muted = item.get("mute").and_then(|v| v.as_bool()).unwrap_or(false);
        let device_mac = props
            .and_then(|p| p.get("api.bluez5.address"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let label = if !media_name.is_empty() {
            media_name.to_string()
        } else if !node_name.is_empty() {
            node_name.to_string()
        } else {
            "Bluetooth Audio".to_string()
        };

        out.push(BtAudioStreamInfo {
            sink_input_index,
            name: label,
            device_mac,
            sink_index,
            volume_pct: parse_pactl_volume_pct(&item),
            muted,
        });
    }
    out
}

pub fn detect_lan_ips() -> Vec<String> {
    let mut ips = Vec::new();
    if let Ok(out) = Command::new("ip")
        .args(["-o", "-4", "addr", "show", "up", "scope", "global"])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pos) = parts.iter().position(|p| *p == "inet") {
                    if let Some(cidr) = parts.get(pos + 1) {
                        if let Some(ip) = cidr.split('/').next() {
                            if !ip.starts_with("127.") && !ips.iter().any(|x| x == ip) {
                                ips.push(ip.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    ips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bluez_managed_objects_extracts_phone_and_avrcp() {
        let sample = r#"{
          "type": "a{oa{sa{sv}}}",
          "data": [{
            "/org/bluez/hci0": {
              "org.bluez.Adapter1": {
                "Alias": {"type": "s", "data": "nixos"},
                "Powered": {"type": "b", "data": true},
                "Discoverable": {"type": "b", "data": false}
              }
            },
            "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF": {
              "org.bluez.Device1": {
                "Address": {"type": "s", "data": "AA:BB:CC:DD:EE:FF"},
                "Name": {"type": "s", "data": "Pixel 9"},
                "Alias": {"type": "s", "data": "Pixel 9"},
                "Connected": {"type": "b", "data": true},
                "Paired": {"type": "b", "data": true},
                "Trusted": {"type": "b", "data": true},
                "Icon": {"type": "s", "data": "phone"},
                "UUIDs": {"type": "as", "data": ["0000110a-0000-1000-8000-00805f9b34fb"]}
              },
              "org.bluez.Battery1": {
                "Percentage": {"type": "y", "data": 84}
              }
            },
            "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF/player0": {
              "org.bluez.MediaPlayer1": {
                "Device": {"type": "o", "data": "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF"},
                "Status": {"type": "s", "data": "playing"},
                "Position": {"type": "u", "data": 42000},
                "Track": {
                  "type": "a{sv}",
                  "data": {
                    "Title": {"type": "s", "data": "Instant Crush"},
                    "Artist": {"type": "s", "data": "Daft Punk"},
                    "Album": {"type": "s", "data": "RAM"},
                    "Duration": {"type": "u", "data": 337000}
                  }
                }
              }
            }
          }]
        }"#;

        let parsed = parse_bluez_managed_objects(sample).expect("parse bluez");
        assert!(parsed.adapter_powered);
        assert_eq!(parsed.adapter_name, "nixos");
        assert_eq!(parsed.devices.len(), 1);
        assert!(parsed.devices[0].is_audio_source);
        assert_eq!(parsed.devices[0].battery_pct, Some(84));
        assert_eq!(parsed.media_players.len(), 1);
        assert_eq!(parsed.media_players[0].title, "Instant Crush");
        assert_eq!(parsed.media_players[0].artist, "Daft Punk");
        assert_eq!(parsed.media_players[0].device_name, "Pixel 9");
    }

    #[test]
    fn test_parse_pactl_sinks_and_bt_streams() {
        let sinks_json = r#"[
          {"index":65,"state":"RUNNING","name":"alsa_output.pci.analog","description":"Speakers","mute":false,"volume":{"front-left":{"value_percent":"45%"}}},
          {"index":88,"state":"IDLE","name":"bluez_output.B0_54_76.1","description":"Galaxy Buds FE","mute":false,"volume":{"front-left":{"value_percent":"80%"}}}
        ]"#;
        let sinks = parse_pactl_sinks(sinks_json, "alsa_output.pci.analog");
        assert_eq!(sinks.len(), 2);
        assert!(sinks[0].is_default);
        assert_eq!(sinks[0].volume_pct, 45);
        assert!(sinks[1].is_bluetooth);

        let inputs_json = r#"[
          {"index":101,"sink":65,"mute":false,"volume":{"front-left":{"value_percent":"90%"}},"properties":{"node.name":"bluez_input.AA_BB.a2dp","media.name":"Pixel 9 A2DP","api.bluez5.address":"AA:BB:CC:DD:EE:FF"}},
          {"index":102,"sink":65,"mute":false,"volume":{"front-left":{"value_percent":"100%"}},"properties":{"node.name":"mpv","application.name":"mpv"}}
        ]"#;
        let streams = parse_pactl_bt_streams(inputs_json);
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].sink_input_index, 101);
        assert_eq!(streams[0].volume_pct, 90);
        assert_eq!(streams[0].device_mac, "AA:BB:CC:DD:EE:FF");
    }
}
