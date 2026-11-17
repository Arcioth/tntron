use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::clock::PlayState;
use crate::engine::{CoreCommand, CoreState};
use crate::library::format_mmss;
use crate::storage::MinecraftLinkConfig;

pub fn default_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("tntron.sock")
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/tntron-{uid}.sock"))
    }
}

pub fn default_status_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("tntron-status.json")
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/tntron-status-{uid}.json"))
    }
}

pub fn default_cover_cache_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("tntron-cover.img")
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/tntron-cover-{uid}.img"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PopupTrackItem {
    pub title: String,
    pub artist: String,
    pub duration: f64,
    pub playlist_index: Option<usize>,
    pub is_current: bool,
    pub is_queued: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McNotice {
    pub seq: u64,
    pub kind: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct McTelemetry {
    pub client_id: String,
    pub player_name: String,
    pub dimension: String,
    pub biome: String,
    pub health: f32,
    pub max_health: f32,
    pub in_combat: bool,
    pub boss_active: bool,
    pub boss_name: Option<String>,
    pub paused: bool,
    pub in_world: bool,
    pub weather: String,
    pub underwater: bool,
    pub speed_bps: f32,
    pub tnt_nearby: bool,
    pub game_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct McTelemetrySnapshot {
    pub connected: bool,
    pub clients_count: usize,
    pub last_seen_epoch: u64,
    pub commands_received: u64,
    pub telemetry: McTelemetry,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcLibraryFolder {
    pub name: String,
    pub path: String,
    pub track_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcLibraryTrack {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub path: String,
    pub duration: f64,
    pub bpm: Option<f64>,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcLibraryData {
    pub current_dir: String,
    pub folders: Vec<IpcLibraryFolder>,
    pub tracks: Vec<IpcLibraryTrack>,
    pub total_tracks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcThemeItem {
    pub slug: String,
    pub label: String,
    pub accent: String,
    pub is_extension: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcExtensionItem {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub error: Option<String>,
}

pub const BUILTIN_THEME_ITEMS: &[(&str, &str, &str)] = &[
    ("openify", "Openify", "#1db954"),
    ("dark", "Pure Dark", "#ffffff"),
    ("light", "Daylight", "#18181b"),
    ("blue", "Deep Blue", "#3b82f6"),
    ("purple", "Amethyst", "#a855f7"),
    ("red", "Crimson", "#ef4444"),
    ("gold", "Gilded", "#eab308"),
    ("cyberpunk", "Cyberpunk", "#ff007f"),
    ("terminal", "Terminal", "#00ff00"),
    ("pharoh", "Pharoh", "#d4af37"),
    ("turk", "Crimson Moon", "#e30a17"),
    ("clouds", "Clouds", "#38bdf8"),
];

#[derive(Debug, Clone)]
pub struct McBridgeHandle {
    config_tx: watch::Sender<MinecraftLinkConfig>,
    telemetry: Arc<RwLock<McTelemetrySnapshot>>,
    active_clients: Arc<AtomicUsize>,
    commands_received: Arc<AtomicU64>,
    notice_seq: Arc<AtomicU64>,
    latest_notice: Arc<RwLock<Option<McNotice>>>,
}

impl McBridgeHandle {
    pub fn new(init_cfg: MinecraftLinkConfig) -> Self {
        let (config_tx, _) = watch::channel(init_cfg);
        Self {
            config_tx,
            telemetry: Arc::new(RwLock::new(McTelemetrySnapshot::default())),
            active_clients: Arc::new(AtomicUsize::new(0)),
            commands_received: Arc::new(AtomicU64::new(0)),
            notice_seq: Arc::new(AtomicU64::new(0)),
            latest_notice: Arc::new(RwLock::new(None)),
        }
    }

    pub fn update_config(&self, cfg: MinecraftLinkConfig) {
        let _ = self.config_tx.send(cfg);
    }

    pub fn config_snapshot(&self) -> MinecraftLinkConfig {
        self.config_tx.borrow().clone()
    }

    pub fn subscribe_config(&self) -> watch::Receiver<MinecraftLinkConfig> {
        self.config_tx.subscribe()
    }

    pub fn record_telemetry(&self, t: McTelemetry) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if let Ok(mut g) = self.telemetry.write() {
            g.connected = true;
            g.clients_count = self.active_clients.load(Ordering::Relaxed);
            g.last_seen_epoch = now;
            g.commands_received = self.commands_received.load(Ordering::Relaxed);
            g.telemetry = t;
        }
    }

    pub fn record_command(&self) {
        let count = self.commands_received.fetch_add(1, Ordering::Relaxed) + 1;
        if let Ok(mut g) = self.telemetry.write() {
            g.commands_received = count;
        }
    }

    pub fn inc_clients(&self) {
        let n = self.active_clients.fetch_add(1, Ordering::Relaxed) + 1;
        if let Ok(mut g) = self.telemetry.write() {
            g.clients_count = n;
            g.connected = n > 0;
        }
    }

    pub fn dec_clients(&self) {
        let prev = self.active_clients.fetch_sub(1, Ordering::Relaxed);
        let n = prev.saturating_sub(1);
        if let Ok(mut g) = self.telemetry.write() {
            g.clients_count = n;
            if n == 0 {
                g.connected = false;
            }
        }
    }

    pub fn telemetry_snapshot(&self) -> McTelemetrySnapshot {
        let mut snap = self
            .telemetry
            .read()
            .map(|g| g.clone())
            .unwrap_or_default();
        snap.clients_count = self.active_clients.load(Ordering::Relaxed);
        snap.commands_received = self.commands_received.load(Ordering::Relaxed);
        if snap.clients_count == 0 {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            snap.connected = snap.last_seen_epoch > 0 && now.saturating_sub(snap.last_seen_epoch) <= 15;
        } else {
            snap.connected = true;
        }
        snap
    }

    pub fn push_notice(&self, kind: String, title: String, body: String) -> McNotice {
        let seq = self.notice_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let notice = McNotice {
            seq,
            kind,
            title,
            body,
        };
        if let Ok(mut g) = self.latest_notice.write() {
            *g = Some(notice.clone());
        }
        notice
    }

    pub fn latest_notice(&self) -> Option<McNotice> {
        self.latest_notice.read().ok().and_then(|g| g.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TntronStatus {
    pub state: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(default)]
    pub track_path: String,
    #[serde(default)]
    pub cover_path: Option<String>,
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub speed: f64,
    pub bpm: Option<f64>,
    #[serde(default)]
    pub energy: f64,
    #[serde(default)]
    pub bands: [f32; 16],
    #[serde(default)]
    pub beat: f32,
    pub lyric: String,
    #[serde(default)]
    pub lyrics_window: Vec<String>,
    #[serde(default)]
    pub playlist_items: Vec<PopupTrackItem>,
    #[serde(default)]
    pub queue_items: Vec<PopupTrackItem>,
    pub loop_mode: String,
    pub shuffle: bool,
    pub dsp_preset: String,
    pub spatial_on: bool,
    pub sleep_remaining_sec: Option<u64>,
    pub text: String,
    pub alt: String,
    pub tooltip: String,
    pub class: String,
    pub percentage: u8,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub pitch_st: f64,
    #[serde(default)]
    pub link_pitch: bool,
    #[serde(default)]
    pub preserve_formant: bool,
    #[serde(default)]
    pub spatial_hz: f32,
    #[serde(default)]
    pub bass_db: i8,
    #[serde(default)]
    pub mid_db: i8,
    #[serde(default)]
    pub treble_db: i8,
    #[serde(default)]
    pub ab_loop_a: Option<f64>,
    #[serde(default)]
    pub ab_loop_b: Option<f64>,
    #[serde(default)]
    pub is_favorite: bool,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub bitrate_kbps: Option<u32>,
    #[serde(default)]
    pub sample_rate_hz: Option<u32>,
    #[serde(default)]
    pub channels: Option<u8>,
    #[serde(default)]
    pub peak_db: f64,
    #[serde(default)]
    pub active_theme: String,
    #[serde(default)]
    pub hide_ui: bool,
    #[serde(default)]
    pub current_dir: String,
    #[serde(default)]
    pub mc_link_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mc_notice: Option<McNotice>,
}

impl TntronStatus {
    pub fn from_state(state: &CoreState) -> Self {
        let state_str = match state.play_state {
            PlayState::Playing => "playing",
            PlayState::Paused => "paused",
            PlayState::Stopped => "stopped",
        };
        let (title, artist, album, track_path, bitrate_kbps, sample_rate_hz, channels, is_favorite) =
            if let Some(track) = &state.current_track {
                (
                    track.title.clone(),
                    track.artist.clone(),
                    track.album.clone(),
                    track.path.display().to_string(),
                    track.bitrate_kbps,
                    state
                        .mpv_samplerate
                        .map(|r| r as u32)
                        .or(track.sample_rate_hz),
                    state.mpv_channels.map(|c| c as u8).or(track.channels),
                    state.persisted.favorites.contains(&track.path),
                )
            } else {
                (
                    "No Track".into(),
                    String::new(),
                    String::new(),
                    String::new(),
                    None,
                    None,
                    None,
                    false,
                )
            };
        let pos = state.clock.now();
        let dur = state.duration_sec;
        let (bands, beat) = if state.play_state == PlayState::Playing {
            state
                .active_analysis
                .as_ref()
                .map(|a| a.sample_at(pos * state.speed))
                .unwrap_or(([0.0; 16], 0.0))
        } else {
            ([0.0; 16], 0.0)
        };
        let energy = state
            .active_analysis
            .as_ref()
            .map(|a| a.energy)
            .unwrap_or(0.0);
        let peak_db = state
            .active_analysis
            .as_ref()
            .map(|a| a.peak_db)
            .unwrap_or(-60.0);
        let cover_path = state
            .cover_cache_path
            .as_ref()
            .filter(|p| p.is_file())
            .map(|p| p.display().to_string());
        let lyrics_window = state.current_lyrics_window();
        let percentage = if dur > 0.0 {
            ((pos / dur) * 100.0).clamp(0.0, 100.0) as u8
        } else {
            0
        };
        let lyric = state.current_lyric_line().unwrap_or_default();
        let bpm = state.current_track_bpm();
        let base_label = if state.current_track.is_some() {
            if artist.is_empty() || artist == "Unknown Artist" {
                title.clone()
            } else {
                format!("{artist} — {title}")
            }
        } else {
            "tntron stopped".into()
        };
        let text = if lyric.is_empty() || state.current_track.is_none() {
            base_label.clone()
        } else {
            let short_lyric: String = lyric.chars().take(44).collect();
            format!("{base_label} | {short_lyric}")
        };
        let time_label = format!("{} / {}", format_mmss(pos), format_mmss(dur));
        let tooltip = if state.current_track.is_some() {
            let mut lines = vec![base_label];
            if !lyric.is_empty() {
                lines.push(format!("Lyric: {lyric}"));
            }
            lines.push(format!("Album: {album}"));
            let bpm_part = bpm.map(|b| format!(" | {b:.0} BPM")).unwrap_or_default();
            lines.push(format!("[{time_label}] ({:.2}x{bpm_part})", state.speed));
            lines.join("\n")
        } else {
            "No active track".into()
        };

        let queue_items: Vec<PopupTrackItem> = state
            .queue
            .iter()
            .take(24)
            .enumerate()
            .map(|(i, t)| PopupTrackItem {
                title: t.title.clone(),
                artist: t.artist.clone(),
                duration: t.duration_sec,
                playlist_index: Some(i),
                is_current: false,
                is_queued: true,
            })
            .collect();

        let pl_len = state.active_playlist.len();
        let start_idx = state
            .active_playlist_index
            .map(|i| i.saturating_sub(2))
            .unwrap_or(0)
            .min(pl_len.saturating_sub(24));
        let playlist_items: Vec<PopupTrackItem> = state
            .active_playlist
            .iter()
            .enumerate()
            .skip(start_idx)
            .take(24)
            .map(|(i, t)| PopupTrackItem {
                title: t.title.clone(),
                artist: t.artist.clone(),
                duration: t.duration_sec,
                playlist_index: Some(i),
                is_current: state.active_playlist_index == Some(i),
                is_queued: false,
            })
            .collect();

        Self {
            state: state_str.into(),
            title,
            artist,
            album,
            track_path,
            cover_path,
            position: (pos * 10.0).round() / 10.0,
            duration: (dur * 10.0).round() / 10.0,
            volume: state.volume,
            speed: state.speed,
            bpm,
            energy,
            bands,
            beat,
            lyric,
            lyrics_window,
            playlist_items,
            queue_items,
            loop_mode: state.loop_mode.label().into(),
            shuffle: state.shuffle_mode == crate::clock::ShuffleMode::On,
            dsp_preset: state.persisted.dsp.preset.label().into(),
            spatial_on: state.persisted.dsp.spatial_on,
            sleep_remaining_sec: state.sleep_remaining_sec(),
            text,
            alt: state_str.into(),
            tooltip,
            class: state_str.into(),
            percentage,
            muted: state.is_muted,
            pitch_st: state.persisted.pitch_st,
            link_pitch: state.persisted.link_pitch,
            preserve_formant: state.persisted.preserve_formant,
            spatial_hz: state.persisted.dsp.spatial_hz,
            bass_db: state.persisted.dsp.bass_db,
            mid_db: state.persisted.dsp.mid_db,
            treble_db: state.persisted.dsp.treble_db,
            ab_loop_a: state.ab_loop.a_sec,
            ab_loop_b: state.ab_loop.b_sec,
            is_favorite,
            codec: state.mpv_codec.clone(),
            bitrate_kbps,
            sample_rate_hz,
            channels,
            peak_db,
            active_theme: state.persisted.active_theme.clone(),
            hide_ui: state.persisted.layout.hide_ui,
            current_dir: state.current_dir.display().to_string(),
            mc_link_enabled: state.persisted.minecraft_link.enabled,
            mc_notice: state.mc_bridge.latest_notice(),
        }
    }
}

pub fn write_status_file(status: &TntronStatus) {
    let path = default_status_path();
    let tmp = path.with_extension("json.tmp");
    if let Ok(json) = serde_json::to_string(status) {
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

#[derive(Debug, Clone)]
pub enum IpcQueryKind {
    GetLibrary {
        query: Option<String>,
        folder: Option<String>,
        limit: Option<usize>,
    },
    GetThemes,
    GetExtensions,
    GetCover,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcCommand {
    Auth { token: String },
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    Volume { delta: Option<i8>, value: Option<f64> },
    Seek { delta: Option<i64>, value: Option<f64> },
    Speed { value: f64 },
    Pitch {
        value: Option<f64>,
        delta: Option<f64>,
        link_pitch: Option<bool>,
        preserve_formant: Option<bool>,
    },
    Mute { value: Option<bool> },
    DspPreset { preset: String },
    StepBass { delta: i8 },
    StepMid { delta: i8 },
    StepTreble { delta: i8 },
    ToggleSpatial,
    SetSpatial {
        enabled: Option<bool>,
        delta_hz: Option<f32>,
    },
    AbLoop { action: String },
    ToggleFavorite { path: Option<String> },
    Sleep { minutes: Option<u64> },
    SleepStep { delta_minutes: i64 },
    CycleLoop,
    Shuffle,
    SetTheme { slug: String },
    ToggleHideUi,
    Rescan,
    PlayPath { path: String },
    PlayFolder {
        path: String,
        shuffle: Option<bool>,
    },
    PlayFavorites { shuffle: Option<bool> },
    PlayAllLibrary { shuffle: Option<bool> },
    PlayUpcoming { index: usize },
    PlayQueueItem { index: usize },
    AddToQueue { path: String },
    PlayNextInQueue { path: String },
    AddFolderToQueue { path: String },
    RemoveFromQueue { index: usize },
    ClearQueue,
    NavigateTo { path: String },
    GetLibrary {
        query: Option<String>,
        folder: Option<String>,
        limit: Option<usize>,
    },
    GetThemes,
    GetExtensions,
    ToggleExtension { id: String },
    TriggerExtAction {
        ext_id: String,
        action_id: String,
        value: Option<serde_json::Value>,
    },
    GetCover,
    McTelemetry(McTelemetry),
    McEvent {
        event: String,
        detail: Option<String>,
    },
    Status,
    Subscribe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<TntronStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library: Option<IpcLibraryData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub themes: Option<Vec<IpcThemeItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<IpcExtensionItem>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

pub async fn run_ipc_server(
    cmd_tx: mpsc::Sender<CoreCommand>,
    status_rx: watch::Receiver<TntronStatus>,
    broadcast_tx: broadcast::Sender<TntronStatus>,
    mc_bridge: McBridgeHandle,
) -> anyhow::Result<()> {
    let socket_path = default_socket_path();
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)?;

    loop {
        let (stream, _) = listener.accept().await?;
        let cmd_tx = cmd_tx.clone();
        let status_rx = status_rx.clone();
        let sub_rx = broadcast_tx.subscribe();
        let mc_bridge = mc_bridge.clone();
        tokio::spawn(async move {
            let (read_half, write_half) = stream.into_split();
            let _ = handle_client(read_half, write_half, cmd_tx, status_rx, sub_rx, mc_bridge, false).await;
        });
    }
}

pub async fn run_mc_tcp_server(
    cmd_tx: mpsc::Sender<CoreCommand>,
    status_rx: watch::Receiver<TntronStatus>,
    broadcast_tx: broadcast::Sender<TntronStatus>,
    mc_bridge: McBridgeHandle,
) {
    let mut cfg_rx = mc_bridge.subscribe_config();

    loop {
        let cfg = cfg_rx.borrow().clone();
        if !cfg.enabled || !cfg.tcp_enabled {
            if cfg_rx.changed().await.is_err() {
                return;
            }
            continue;
        }

        let host = if cfg.tcp_host.trim().is_empty() {
            "127.0.0.1".to_string()
        } else {
            cfg.tcp_host.trim().to_string()
        };
        let addr = format!("{host}:{}", cfg.tcp_port);

        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("Minecraft Link TCP bind failed on {addr}: {e}");
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                    res = cfg_rx.changed() => {
                        if res.is_err() {
                            return;
                        }
                    }
                }
                continue;
            }
        };
        tracing::info!("Minecraft Link TCP bridge listening on {addr}");

        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, _)) => {
                            let _ = stream.set_nodelay(true);
                            let cmd_tx = cmd_tx.clone();
                            let status_rx = status_rx.clone();
                            let sub_rx = broadcast_tx.subscribe();
                            let bridge = mc_bridge.clone();
                            tokio::spawn(async move {
                                bridge.inc_clients();
                                let (read_half, write_half) = stream.into_split();
                                let _ = handle_client(
                                    read_half,
                                    write_half,
                                    cmd_tx,
                                    status_rx,
                                    sub_rx,
                                    bridge.clone(),
                                    true,
                                ).await;
                                bridge.dec_clients();
                            });
                        }
                        Err(e) => {
                            tracing::warn!("Minecraft Link TCP accept error: {e}");
                            break;
                        }
                    }
                }
                res = cfg_rx.changed() => {
                    if res.is_err() {
                        return;
                    }
                    let next = cfg_rx.borrow().clone();
                    if next != cfg {
                        break;
                    }
                }
            }
        }
    }
}

async fn send_response<W: AsyncWrite + Unpin>(
    write_half: &mut W,
    resp: &IpcResponse,
) -> anyhow::Result<()> {
    let msg = serde_json::to_string(resp)?;
    write_half.write_all(msg.as_bytes()).await?;
    write_half.write_all(b"\n").await?;
    Ok(())
}

async fn handle_client<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    read_half: R,
    mut write_half: W,
    cmd_tx: mpsc::Sender<CoreCommand>,
    status_rx: watch::Receiver<TntronStatus>,
    mut sub_rx: broadcast::Receiver<TntronStatus>,
    mc_bridge: McBridgeHandle,
    is_tcp: bool,
) -> anyhow::Result<()> {
    let mut lines = BufReader::new(read_half).lines();
    let expected_token = if is_tcp {
        mc_bridge.config_snapshot().auth_token
    } else {
        String::new()
    };
    let mut authed = expected_token.trim().is_empty();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<IpcCommand>(&line) else {
            send_response(
                &mut write_half,
                &IpcResponse {
                    ok: false,
                    error: Some("invalid command JSON".into()),
                    status: None,
                    library: None,
                    themes: None,
                    extensions: None,
                    cover_base64: None,
                    message: None,
                },
            )
            .await?;
            continue;
        };

        if let IpcCommand::Auth { token } = req {
            let current_token = mc_bridge.config_snapshot().auth_token;
            if current_token.trim().is_empty() || token == current_token {
                authed = true;
                let st = status_rx.borrow().clone();
                send_response(
                    &mut write_half,
                    &IpcResponse {
                        ok: true,
                        error: None,
                        status: Some(st),
                        library: None,
                        themes: None,
                        extensions: None,
                        cover_base64: None,
                        message: Some("authenticated".into()),
                    },
                )
                .await?;
            } else {
                send_response(
                    &mut write_half,
                    &IpcResponse {
                        ok: false,
                        error: Some("invalid auth token".into()),
                        status: None,
                        library: None,
                        themes: None,
                        extensions: None,
                        cover_base64: None,
                        message: None,
                    },
                )
                .await?;
            }
            continue;
        }

        if !authed {
            send_response(
                &mut write_half,
                &IpcResponse {
                    ok: false,
                    error: Some("authentication required".into()),
                    status: None,
                    library: None,
                    themes: None,
                    extensions: None,
                    cover_base64: None,
                    message: None,
                },
            )
            .await?;
            continue;
        }

        if matches!(req, IpcCommand::Subscribe) {
            let init = { serde_json::to_string(&*status_rx.borrow())? };
            write_half.write_all(init.as_bytes()).await?;
            write_half.write_all(b"\n").await?;
            loop {
                match sub_rx.recv().await {
                    Ok(st) => {
                        let msg = serde_json::to_string(&st)?;
                        if write_half.write_all(msg.as_bytes()).await.is_err()
                            || write_half.write_all(b"\n").await.is_err()
                        {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            return Ok(());
        }

        match req {
            IpcCommand::GetLibrary {
                query,
                folder,
                limit,
            } => {
                let (tx, rx) = oneshot::channel();
                let _ = cmd_tx
                    .send(CoreCommand::IpcQuery {
                        req: IpcQueryKind::GetLibrary {
                            query,
                            folder,
                            limit,
                        },
                        reply: Arc::new(std::sync::Mutex::new(Some(tx))),
                    })
                    .await;
                if let Ok(Ok(resp)) = tokio::time::timeout(Duration::from_secs(2), rx).await {
                    send_response(&mut write_half, &resp).await?;
                }
                continue;
            }
            IpcCommand::GetThemes => {
                let (tx, rx) = oneshot::channel();
                let _ = cmd_tx
                    .send(CoreCommand::IpcQuery {
                        req: IpcQueryKind::GetThemes,
                        reply: Arc::new(std::sync::Mutex::new(Some(tx))),
                    })
                    .await;
                if let Ok(Ok(resp)) = tokio::time::timeout(Duration::from_secs(2), rx).await {
                    send_response(&mut write_half, &resp).await?;
                }
                continue;
            }
            IpcCommand::GetExtensions => {
                let (tx, rx) = oneshot::channel();
                let _ = cmd_tx
                    .send(CoreCommand::IpcQuery {
                        req: IpcQueryKind::GetExtensions,
                        reply: Arc::new(std::sync::Mutex::new(Some(tx))),
                    })
                    .await;
                if let Ok(Ok(resp)) = tokio::time::timeout(Duration::from_secs(2), rx).await {
                    send_response(&mut write_half, &resp).await?;
                }
                continue;
            }
            IpcCommand::GetCover => {
                let (tx, rx) = oneshot::channel();
                let _ = cmd_tx
                    .send(CoreCommand::IpcQuery {
                        req: IpcQueryKind::GetCover,
                        reply: Arc::new(std::sync::Mutex::new(Some(tx))),
                    })
                    .await;
                if let Ok(Ok(resp)) = tokio::time::timeout(Duration::from_secs(2), rx).await {
                    send_response(&mut write_half, &resp).await?;
                }
                continue;
            }
            IpcCommand::McTelemetry(telemetry) => {
                mc_bridge.record_telemetry(telemetry.clone());
                let _ = cmd_tx.send(CoreCommand::McTelemetry(telemetry)).await;
                let st = status_rx.borrow().clone();
                send_response(
                    &mut write_half,
                    &IpcResponse {
                        ok: true,
                        error: None,
                        status: Some(st),
                        library: None,
                        themes: None,
                        extensions: None,
                        cover_base64: None,
                        message: None,
                    },
                )
                .await?;
                continue;
            }
            IpcCommand::McEvent { event, detail } => {
                mc_bridge.record_command();
                let _ = cmd_tx
                    .send(CoreCommand::McEvent { event, detail })
                    .await;
                let st = status_rx.borrow().clone();
                send_response(
                    &mut write_half,
                    &IpcResponse {
                        ok: true,
                        error: None,
                        status: Some(st),
                        library: None,
                        themes: None,
                        extensions: None,
                        cover_base64: None,
                        message: None,
                    },
                )
                .await?;
                continue;
            }
            _ => {}
        }

        if !matches!(req, IpcCommand::Status) {
            mc_bridge.record_command();
        }

        let current_track_path = status_rx.borrow().track_path.clone();
        let core_cmds = translate_ipc_command(req, &current_track_path);
        for c in core_cmds {
            let _ = cmd_tx.send(c).await;
        }

        let st = status_rx.borrow().clone();
        send_response(
            &mut write_half,
            &IpcResponse {
                ok: true,
                error: None,
                status: Some(st),
                library: None,
                themes: None,
                extensions: None,
                cover_base64: None,
                message: None,
            },
        )
        .await?;
    }

    Ok(())
}

fn translate_ipc_command(req: IpcCommand, current_track_path: &str) -> Vec<CoreCommand> {
    match req {
        IpcCommand::Play => vec![CoreCommand::SetPaused(false)],
        IpcCommand::Pause => vec![CoreCommand::SetPaused(true)],
        IpcCommand::Toggle => vec![CoreCommand::TogglePause],
        IpcCommand::Next => vec![CoreCommand::NextTrack],
        IpcCommand::Prev => vec![CoreCommand::PrevTrack],
        IpcCommand::Stop => vec![CoreCommand::Stop],
        IpcCommand::Volume { delta: Some(d), .. } => vec![CoreCommand::VolumeDelta(d)],
        IpcCommand::Volume { value: Some(v), .. } => vec![CoreCommand::SetVolume(v)],
        IpcCommand::Seek { delta: Some(d), .. } => vec![CoreCommand::SeekDelta(d as f64)],
        IpcCommand::Seek { value: Some(v), .. } => vec![CoreCommand::SeekAbsolute(v)],
        IpcCommand::Speed { value } => vec![CoreCommand::SetSpeed(value)],
        IpcCommand::Pitch {
            value,
            delta,
            link_pitch,
            preserve_formant,
        } => {
            let mut cmds = Vec::new();
            if let Some(lp) = link_pitch {
                cmds.push(CoreCommand::SetLinkPitch(lp));
            }
            if let Some(pf) = preserve_formant {
                cmds.push(CoreCommand::SetPreserveFormant(pf));
            }
            if let Some(v) = value {
                cmds.push(CoreCommand::SetPitchSemitones(v));
            } else if let Some(d) = delta {
                cmds.push(CoreCommand::StepPitchSemitones(d));
            }
            cmds
        }
        IpcCommand::Mute { value: Some(m) } => vec![CoreCommand::SetMute(m)],
        IpcCommand::Mute { value: None } => vec![CoreCommand::ToggleMute],
        IpcCommand::DspPreset { preset } => {
            use crate::clock::AudioPreset;
            let p = match preset.to_ascii_lowercase().as_str() {
                "flat" => Some(AudioPreset::Flat),
                "bass" | "bassboost" | "bass_boost" | "bass boost" => Some(AudioPreset::BassBoost),
                "vocal" => Some(AudioPreset::Vocal),
                "warm" => Some(AudioPreset::Warm),
                "rock" => Some(AudioPreset::Rock),
                "8d" | "spatial" | "spatial8d" | "8d spatial" => Some(AudioPreset::Spatial8D),
                _ => None,
            };
            p.into_iter().map(CoreCommand::SetAudioPreset).collect()
        }
        IpcCommand::StepBass { delta } => vec![CoreCommand::StepBass(delta)],
        IpcCommand::StepMid { delta } => vec![CoreCommand::StepMid(delta)],
        IpcCommand::StepTreble { delta } => vec![CoreCommand::StepTreble(delta)],
        IpcCommand::ToggleSpatial => vec![CoreCommand::ToggleSpatial],
        IpcCommand::SetSpatial { enabled, delta_hz } => {
            let mut cmds = Vec::new();
            if let Some(d) = delta_hz {
                cmds.push(CoreCommand::StepSpatialHz(d));
            } else if enabled.is_some() {
                cmds.push(CoreCommand::ToggleSpatial);
            }
            cmds
        }
        IpcCommand::AbLoop { action } => match action.to_ascii_lowercase().as_str() {
            "mark_a" | "a" => vec![CoreCommand::MarkLoopA],
            "mark_b" | "b" => vec![CoreCommand::MarkLoopB],
            "clear" => vec![CoreCommand::ClearAbLoop],
            "save" => vec![CoreCommand::SaveTrackAbLoop],
            "load" => vec![CoreCommand::LoadTrackAbLoop],
            _ => Vec::new(),
        },
        IpcCommand::ToggleFavorite { path } => {
            let target = path
                .filter(|p| !p.trim().is_empty())
                .unwrap_or_else(|| current_track_path.to_string());
            if target.is_empty() {
                Vec::new()
            } else {
                vec![CoreCommand::ToggleFavorite(PathBuf::from(target))]
            }
        }
        IpcCommand::Sleep { minutes } => vec![CoreCommand::SetSleepTimer(minutes)],
        IpcCommand::SleepStep { delta_minutes } => vec![CoreCommand::StepSleepMinutes(delta_minutes)],
        IpcCommand::CycleLoop => vec![CoreCommand::CycleLoopMode],
        IpcCommand::Shuffle => vec![CoreCommand::ToggleShuffle],
        IpcCommand::SetTheme { slug } => vec![CoreCommand::SetThemeSlug(slug)],
        IpcCommand::ToggleHideUi => vec![CoreCommand::ToggleHideUi],
        IpcCommand::Rescan => vec![CoreCommand::Reload],
        IpcCommand::PlayPath { path } => vec![CoreCommand::PlayPath(PathBuf::from(path))],
        IpcCommand::PlayFolder { path, shuffle } => vec![CoreCommand::PlayFolder {
            path: PathBuf::from(path),
            shuffle: shuffle.unwrap_or(false),
        }],
        IpcCommand::PlayFavorites { shuffle } => vec![CoreCommand::PlayFavorites {
            shuffle: shuffle.unwrap_or(false),
        }],
        IpcCommand::PlayAllLibrary { shuffle } => vec![CoreCommand::PlayAllLibrary {
            shuffle: shuffle.unwrap_or(false),
        }],
        IpcCommand::PlayUpcoming { index } => vec![CoreCommand::PlayUpcomingIndex(index)],
        IpcCommand::PlayQueueItem { index } => vec![CoreCommand::PlayQueueItem(index)],
        IpcCommand::AddToQueue { path } => vec![CoreCommand::AddToQueue(PathBuf::from(path))],
        IpcCommand::PlayNextInQueue { path } => {
            vec![CoreCommand::PlayNextInQueue(PathBuf::from(path))]
        }
        IpcCommand::AddFolderToQueue { path } => {
            vec![CoreCommand::AddFolderToQueue(PathBuf::from(path))]
        }
        IpcCommand::RemoveFromQueue { index } => vec![CoreCommand::RemoveFromQueue(index)],
        IpcCommand::ClearQueue => vec![CoreCommand::ClearQueue],
        IpcCommand::NavigateTo { path } => vec![CoreCommand::NavigateTo(PathBuf::from(path))],
        IpcCommand::ToggleExtension { id } => vec![CoreCommand::ToggleExtension(id)],
        IpcCommand::TriggerExtAction {
            ext_id,
            action_id,
            value,
        } => vec![CoreCommand::TriggerExtAction {
            ext_id,
            action_id,
            value: value.unwrap_or(serde_json::Value::Bool(true)),
        }],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_and_ipc_command_roundtrip() {
        assert_eq!(encode_base64(b"tntron"), "dG50cm9u");
        assert_eq!(encode_base64(b"a"), "YQ==");

        let json = r#"{"cmd":"pitch","value":3.0,"link_pitch":true}"#;
        let parsed: IpcCommand = serde_json::from_str(json).expect("parse pitch");
        let cmds = translate_ipc_command(parsed, "/tmp/a.mp3");
        assert!(matches!(cmds.first(), Some(CoreCommand::SetLinkPitch(true))));
        assert!(matches!(
            cmds.get(1),
            Some(CoreCommand::SetPitchSemitones(v)) if (*v - 3.0).abs() < 0.01
        ));
    }

    #[test]
    fn test_mc_bridge_handle_telemetry_and_notice() {
        let bridge = McBridgeHandle::new(MinecraftLinkConfig {
            enabled: true,
            ..Default::default()
        });
        assert!(!bridge.telemetry_snapshot().connected);
        bridge.inc_clients();
        bridge.record_telemetry(McTelemetry {
            player_name: "Arcioth".into(),
            dimension: "minecraft:overworld".into(),
            health: 20.0,
            max_health: 20.0,
            in_world: true,
            ..Default::default()
        });
        let snap = bridge.telemetry_snapshot();
        assert!(snap.connected);
        assert_eq!(snap.clients_count, 1);
        assert_eq!(snap.telemetry.player_name, "Arcioth");

        let n1 = bridge.push_notice("toast".into(), "Title".into(), "Body".into());
        let n2 = bridge.push_notice("chat".into(), "Title 2".into(), "Body 2".into());
        assert_eq!(n1.seq, 1);
        assert_eq!(n2.seq, 2);
        assert_eq!(bridge.latest_notice().unwrap().seq, 2);
        bridge.dec_clients();
    }

    #[tokio::test]
    async fn test_mc_tcp_server_auth_telemetry_and_commands() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::TcpStream;

        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);

        let cfg = MinecraftLinkConfig {
            enabled: true,
            tcp_enabled: true,
            tcp_host: "127.0.0.1".into(),
            tcp_port: port,
            auth_token: "secret26".into(),
        };
        let bridge = McBridgeHandle::new(cfg);
        let (cmd_tx, mut cmd_rx) = mpsc::channel(16);
        let init_state = CoreState::new(std::env::temp_dir());
        let init_status = TntronStatus::from_state(&init_state);
        let (_status_tx, status_rx) = watch::channel(init_status);
        let (bcast_tx, _) = broadcast::channel(16);

        let server_bridge = bridge.clone();
        let server_task = tokio::spawn(async move {
            run_mc_tcp_server(cmd_tx, status_rx, bcast_tx, server_bridge).await;
        });

        let mut stream = None;
        for _ in 0..30 {
            if let Ok(s) = TcpStream::connect(("127.0.0.1", port)).await {
                stream = Some(s);
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        let stream = stream.expect("TCP server should bind");
        let (rh, mut wh) = stream.into_split();
        let mut lines = BufReader::new(rh).lines();

        // Unauthenticated command rejected
        wh.write_all(b"{\"cmd\":\"toggle\"}\n").await.unwrap();
        let unauth: IpcResponse =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert!(!unauth.ok);

        // Authenticate
        wh.write_all(b"{\"cmd\":\"auth\",\"token\":\"secret26\"}\n")
            .await
            .unwrap();
        let auth_ok: IpcResponse =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert!(auth_ok.ok);

        // Send telemetry + command
        wh.write_all(
            b"{\"cmd\":\"mc_telemetry\",\"player_name\":\"Steve\",\"dimension\":\"minecraft:the_end\",\"in_world\":true}\n",
        )
        .await
        .unwrap();
        let tel_resp: IpcResponse =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert!(tel_resp.ok);
        assert_eq!(bridge.telemetry_snapshot().telemetry.player_name, "Steve");

        wh.write_all(b"{\"cmd\":\"next\"}\n").await.unwrap();
        let next_resp: IpcResponse =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert!(next_resp.ok);

        let c1 = cmd_rx.recv().await.unwrap();
        let c2 = cmd_rx.recv().await.unwrap();
        assert!(matches!(c1, CoreCommand::McTelemetry(_)));
        assert!(matches!(c2, CoreCommand::NextTrack));

        server_task.abort();
    }
}
