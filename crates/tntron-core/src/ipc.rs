use serde::{Deserialize, Serialize};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, ReadBuf};
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::android::{AndroidBridgeHandle, AndroidBridgeStatus};
use crate::clock::PlayState;
use crate::engine::{CoreCommand, CoreState};
use crate::library::format_mmss;
use crate::storage::{ButtonPackConfig, MinecraftLinkConfig};

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

pub const ANDROID_BT_RFCOMM_CHANNEL: u8 = 22;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct HostUiStyle {
    pub theme_slug: String,
    pub theme_label: String,
    pub is_light: bool,
    pub base: String,
    pub sidebar: String,
    pub surface: String,
    pub card: String,
    pub card_hover: String,
    pub text: String,
    pub muted: String,
    pub accent: String,
    pub accent_fg: String,
    pub logo_color: String,
    pub button_pack: ButtonPackConfig,
    pub reactive_enabled: bool,
    pub hue_loop: bool,
    pub bass_glow: bool,
    pub eq_style: String,
    pub transparency_enabled: bool,
    pub card_alpha: f32,
}

impl Default for HostUiStyle {
    fn default() -> Self {
        Self {
            theme_slug: "openify".into(),
            theme_label: "Openify".into(),
            is_light: false,
            base: "#000000".into(),
            sidebar: "#121212".into(),
            surface: "#121212".into(),
            card: "#181818".into(),
            card_hover: "#282828".into(),
            text: "#ffffff".into(),
            muted: "#b3b3b3".into(),
            accent: "#1db954".into(),
            accent_fg: "#000000".into(),
            logo_color: "#1db954".into(),
            button_pack: ButtonPackConfig::default(),
            reactive_enabled: false,
            hue_loop: true,
            bass_glow: true,
            eq_style: "bars".into(),
            transparency_enabled: false,
            card_alpha: 1.0,
        }
    }
}

impl HostUiStyle {
    pub fn from_state(state: &CoreState) -> Self {
        let slug = state.persisted.active_theme.as_str();
        let mut style = if let Some(et) = state.ext_host.themes.iter().find(|t| t.slug == slug) {
            Self {
                theme_slug: et.slug.clone(),
                theme_label: et.label.clone(),
                is_light: false,
                base: et.base.clone(),
                sidebar: et.sidebar.clone(),
                surface: et.main.clone(),
                card: et.card.clone(),
                card_hover: et.card_hover.clone(),
                text: et.text.clone(),
                muted: et.text_sub.clone(),
                accent: et.accent.clone(),
                accent_fg: "#000000".into(),
                logo_color: et.logo_color.clone().unwrap_or_else(|| et.accent.clone()),
                ..Default::default()
            }
        } else {
            Self::builtin(slug)
        };

        if let Some(custom_hex) = &state.persisted.logo.custom_hex {
            if state.persisted.logo.mode == "custom_color" && !custom_hex.trim().is_empty() {
                style.logo_color = custom_hex.clone();
            }
        }
        style.button_pack = state.persisted.button_pack.clone();
        style.reactive_enabled = state.persisted.reactive_ui.enabled;
        style.hue_loop = state.persisted.reactive_ui.hue_loop;
        style.bass_glow = state.persisted.reactive_ui.bass_glow;
        style.eq_style = state.persisted.reactive_ui.eq_style.clone();
        style.transparency_enabled = state.persisted.transparency.enabled;
        style.card_alpha = state.persisted.transparency.card_alpha;
        style
    }

    fn builtin(slug: &str) -> Self {
        match slug {
            "dark" => Self {
                theme_slug: "dark".into(), theme_label: "Pure Dark".into(), is_light: false,
                base: "#0a0a0a".into(), sidebar: "#141414".into(), surface: "#141414".into(),
                card: "#1a1a1a".into(), card_hover: "#262626".into(), text: "#ffffff".into(),
                muted: "#888888".into(), accent: "#ffffff".into(), accent_fg: "#000000".into(),
                logo_color: "#ffffff".into(), ..Default::default()
            },
            "light" => Self {
                theme_slug: "light".into(), theme_label: "Daylight".into(), is_light: true,
                base: "#f4f4f6".into(), sidebar: "#ffffff".into(), surface: "#f9f9fb".into(),
                card: "#eaecef".into(), card_hover: "#dfe2e6".into(), text: "#18181b".into(),
                muted: "#52525b".into(), accent: "#18181b".into(), accent_fg: "#ffffff".into(),
                logo_color: "#18181b".into(), ..Default::default()
            },
            "blue" => Self {
                theme_slug: "blue".into(), theme_label: "Deep Blue".into(), is_light: false,
                base: "#050a14".into(), sidebar: "#0b1528".into(), surface: "#0f1d36".into(),
                card: "#172a4f".into(), card_hover: "#1f396b".into(), text: "#f0f7ff".into(),
                muted: "#94a3b8".into(), accent: "#3b82f6".into(), accent_fg: "#ffffff".into(),
                logo_color: "#60a5fa".into(), ..Default::default()
            },
            "purple" => Self {
                theme_slug: "purple".into(), theme_label: "Amethyst".into(), is_light: false,
                base: "#0d0714".into(), sidebar: "#180d26".into(), surface: "#211233".into(),
                card: "#2d1a45".into(), card_hover: "#3b2259".into(), text: "#faf5ff".into(),
                muted: "#a78bfa".into(), accent: "#a855f7".into(), accent_fg: "#ffffff".into(),
                logo_color: "#c084fc".into(), ..Default::default()
            },
            "red" => Self {
                theme_slug: "red".into(), theme_label: "Crimson".into(), is_light: false,
                base: "#140505".into(), sidebar: "#240a0a".into(), surface: "#2e0e0e".into(),
                card: "#421515".into(), card_hover: "#571d1d".into(), text: "#fff5f5".into(),
                muted: "#f87171".into(), accent: "#ef4444".into(), accent_fg: "#ffffff".into(),
                logo_color: "#f87171".into(), ..Default::default()
            },
            "gold" => Self {
                theme_slug: "gold".into(), theme_label: "Gilded".into(), is_light: false,
                base: "#120e02".into(), sidebar: "#1f1804".into(), surface: "#292006".into(),
                card: "#3a2d09".into(), card_hover: "#4d3c0e".into(), text: "#fefce8".into(),
                muted: "#facc15".into(), accent: "#eab308".into(), accent_fg: "#000000".into(),
                logo_color: "#fbbf24".into(), ..Default::default()
            },
            "cyberpunk" => Self {
                theme_slug: "cyberpunk".into(), theme_label: "Cyberpunk".into(), is_light: false,
                base: "#080412".into(), sidebar: "#120926".into(), surface: "#180c33".into(),
                card: "#24124d".into(), card_hover: "#311869".into(), text: "#00f0ff".into(),
                muted: "#c026d3".into(), accent: "#ff007f".into(), accent_fg: "#ffffff".into(),
                logo_color: "#00f0ff".into(), ..Default::default()
            },
            "terminal" => Self {
                theme_slug: "terminal".into(), theme_label: "Terminal".into(), is_light: false,
                base: "#000500".into(), sidebar: "#001000".into(), surface: "#001400".into(),
                card: "#002200".into(), card_hover: "#003300".into(), text: "#00ff00".into(),
                muted: "#00aa00".into(), accent: "#00ff00".into(), accent_fg: "#000000".into(),
                logo_color: "#00ff00".into(), ..Default::default()
            },
            "pharoh" => Self {
                theme_slug: "pharoh".into(), theme_label: "Pharoh".into(), is_light: false,
                base: "#1a1409".into(), sidebar: "#261d0d".into(), surface: "#332712".into(),
                card: "#473619".into(), card_hover: "#5c4621".into(), text: "#fae8b4".into(),
                muted: "#c2a661".into(), accent: "#d4af37".into(), accent_fg: "#000000".into(),
                logo_color: "#d4af37".into(), ..Default::default()
            },
            "turk" => Self {
                theme_slug: "turk".into(), theme_label: "Crimson Moon".into(), is_light: false,
                base: "#140304".into(), sidebar: "#240608".into(), surface: "#30090c".into(),
                card: "#471015".into(), card_hover: "#5e161d".into(), text: "#ffffff".into(),
                muted: "#fca5a5".into(), accent: "#e30a17".into(), accent_fg: "#ffffff".into(),
                logo_color: "#e30a17".into(), ..Default::default()
            },
            "clouds" => Self {
                theme_slug: "clouds".into(), theme_label: "Clouds".into(), is_light: true,
                base: "#e8f1f8".into(), sidebar: "#ffffff".into(), surface: "#f2f7fc".into(),
                card: "#d9e8f5".into(), card_hover: "#c6dcf0".into(), text: "#1e3a5f".into(),
                muted: "#5a7c9e".into(), accent: "#38bdf8".into(), accent_fg: "#000000".into(),
                logo_color: "#0284c7".into(), ..Default::default()
            },
            _ => Self::default(),
        }
    }
}

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
    pub lyric_translated: String,
    #[serde(default)]
    pub lyrics_window: Vec<String>,
    #[serde(default)]
    pub lyrics_window_translated: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lyrics_target_lang: Option<String>,
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
    #[serde(default)]
    pub ui_style: HostUiStyle,
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
        let lyrics_window_translated = state.current_lyrics_window_translated();
        let percentage = if dur > 0.0 {
            ((pos / dur) * 100.0).clamp(0.0, 100.0) as u8
        } else {
            0
        };
        let lyric = state.current_lyric_line().unwrap_or_default();
        let lyric_translated = state.current_lyric_translated().unwrap_or_default();
        let lyrics_target_lang = state.active_lyrics_lang.clone();
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
            lyric_translated,
            lyrics_window,
            lyrics_window_translated,
            lyrics_target_lang,
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
            ui_style: HostUiStyle::from_state(state),
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
    GetAndroidStatus,
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
    TranslateLyrics { lang: String },
    GetAndroidStatus,
    SetAudioSink { sink_name: String },
    SwitchAudioSource { source: String },
    AndroidMedia {
        action: String,
        player_path: Option<String>,
    },
    BtConnect { device_path: String },
    BtDisconnect { device_path: String },
    BtReverseAudio { device_path: String },
    BtStreamVolume { volume_pct: u32 },
    BtStreamMute { muted: bool },
    Status,
    Subscribe,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    pub android: Option<AndroidBridgeStatus>,
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
            let _ = handle_client(
                read_half,
                write_half,
                cmd_tx,
                status_rx,
                sub_rx,
                mc_bridge,
                String::new(),
            )
            .await;
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
                                let token = bridge.config_snapshot().auth_token;
                                let _ = handle_client(
                                    read_half,
                                    write_half,
                                    cmd_tx,
                                    status_rx,
                                    sub_rx,
                                    bridge.clone(),
                                    token,
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

pub async fn run_android_tcp_server(
    cmd_tx: mpsc::Sender<CoreCommand>,
    status_rx: watch::Receiver<TntronStatus>,
    broadcast_tx: broadcast::Sender<TntronStatus>,
    mc_bridge: McBridgeHandle,
    android_bridge: AndroidBridgeHandle,
) {
    let mut cfg_rx = android_bridge.subscribe_config();

    loop {
        let cfg = cfg_rx.borrow().clone();
        if !cfg.enabled || !cfg.lan_enabled {
            if cfg_rx.changed().await.is_err() {
                return;
            }
            continue;
        }

        let host = if cfg.bind_host.trim().is_empty() {
            "0.0.0.0".to_string()
        } else {
            cfg.bind_host.trim().to_string()
        };
        let addr = format!("{host}:{}", cfg.tcp_port);

        let listener = match TcpListener::bind(&addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("Android Link TCP bind failed on {addr}: {e}");
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
        tracing::info!("Android Link TCP bridge listening on {addr}");

        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, _)) => {
                            let _ = stream.set_nodelay(true);
                            let cmd_tx = cmd_tx.clone();
                            let status_rx = status_rx.clone();
                            let sub_rx = broadcast_tx.subscribe();
                            let mcb = mc_bridge.clone();
                            let ab = android_bridge.clone();
                            tokio::spawn(async move {
                                ab.inc_clients();
                                let (read_half, write_half) = stream.into_split();
                                let token = ab.config_snapshot().auth_token;
                                let _ = handle_client(
                                    read_half,
                                    write_half,
                                    cmd_tx,
                                    status_rx,
                                    sub_rx,
                                    mcb,
                                    token,
                                ).await;
                                ab.dec_clients();
                            });
                        }
                        Err(e) => {
                            tracing::warn!("Android Link TCP accept error: {e}");
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

pub async fn run_android_bt_rfcomm_server(
    cmd_tx: mpsc::Sender<CoreCommand>,
    status_rx: watch::Receiver<TntronStatus>,
    broadcast_tx: broadcast::Sender<TntronStatus>,
    mc_bridge: McBridgeHandle,
    android_bridge: AndroidBridgeHandle,
) {
    let mut cfg_rx = android_bridge.subscribe_config();

    loop {
        let cfg = cfg_rx.borrow().clone();
        if !cfg.enabled {
            if cfg_rx.changed().await.is_err() {
                return;
            }
            continue;
        }

        let listener = match bind_rfcomm_listener(ANDROID_BT_RFCOMM_CHANNEL) {
            Ok(fd) => fd,
            Err(e) => {
                tracing::warn!("Bluetooth RFCOMM bind failed on ch {}: {e}", ANDROID_BT_RFCOMM_CHANNEL);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                    res = cfg_rx.changed() => {
                        if res.is_err() {
                            return;
                        }
                    }
                }
                continue;
            }
        };
        tracing::info!("Android Link Bluetooth RFCOMM listening on channel {}", ANDROID_BT_RFCOMM_CHANNEL);

        loop {
            tokio::select! {
                accepted = accept_rfcomm(&listener) => {
                    match accepted {
                        Ok(client_fd) => {
                            let cmd_tx = cmd_tx.clone();
                            let status_rx = status_rx.clone();
                            let sub_rx = broadcast_tx.subscribe();
                            let mcb = mc_bridge.clone();
                            let ab = android_bridge.clone();
                            tokio::spawn(async move {
                                ab.inc_clients();
                                if let Ok(stream) = RfcommStream::new(client_fd) {
                                    let (read_half, write_half) = tokio::io::split(stream);
                                    let _ = handle_multiplexed_client(
                                        read_half,
                                        write_half,
                                        cmd_tx,
                                        status_rx,
                                        sub_rx,
                                        mcb,
                                    ).await;
                                }
                                ab.dec_clients();
                            });
                        }
                        Err(e) => {
                            tracing::warn!("Bluetooth RFCOMM accept error: {e}");
                            break;
                        }
                    }
                }
                res = cfg_rx.changed() => {
                    if res.is_err() {
                        return;
                    }
                    if !cfg_rx.borrow().enabled {
                        break;
                    }
                }
            }
        }
    }
}

const AF_BLUETOOTH: libc::c_int = 31;
const BTPROTO_RFCOMM: libc::c_int = 3;

#[repr(C)]
struct SockAddrRc {
    rc_family: libc::sa_family_t,
    rc_bdaddr: [u8; 6],
    rc_channel: u8,
}

fn bind_rfcomm_listener(channel: u8) -> std::io::Result<AsyncFd<OwnedFd>> {
    unsafe {
        let fd = libc::socket(
            AF_BLUETOOTH,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            BTPROTO_RFCOMM,
        );
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let owned = OwnedFd::from_raw_fd(fd);
        let addr = SockAddrRc {
            rc_family: AF_BLUETOOTH as libc::sa_family_t,
            rc_bdaddr: [0u8; 6],
            rc_channel: channel,
        };
        let rc = libc::bind(
            owned.as_raw_fd(),
            (&raw const addr).cast::<libc::sockaddr>(),
            std::mem::size_of::<SockAddrRc>() as libc::socklen_t,
        );
        if rc < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if libc::listen(owned.as_raw_fd(), 4) < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(AsyncFd::register(owned)?)
    }
}

async fn accept_rfcomm(listener: &AsyncFd<OwnedFd>) -> std::io::Result<OwnedFd> {
    loop {
        let mut guard = listener.readable().await?;
        let res = guard.try_io(|inner| unsafe {
            let mut addr: SockAddrRc = std::mem::zeroed();
            let mut len = std::mem::size_of::<SockAddrRc>() as libc::socklen_t;
            let fd = libc::accept4(
                inner.get_ref().as_raw_fd(),
                (&raw mut addr).cast::<libc::sockaddr>(),
                &raw mut len,
                libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            );
            if fd < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(OwnedFd::from_raw_fd(fd))
            }
        });
        match res {
            Ok(result) => return result,
            Err(_would_block) => continue,
        }
    }
}

struct RfcommStream {
    inner: AsyncFd<OwnedFd>,
}

impl RfcommStream {
    fn new(fd: OwnedFd) -> std::io::Result<Self> {
        Ok(Self {
            // SAFETY: `fd` is an `OwnedFd` created by `accept4` with `SOCK_NONBLOCK | SOCK_CLOEXEC`
            // and exclusively owned by this `AsyncFd` until dropped.
            inner: unsafe { AsyncFd::register(fd)? },
        })
    }
}

impl AsyncRead for RfcommStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        loop {
            let mut guard = match self.inner.poll_read_ready(cx) {
                Poll::Ready(Ok(g)) => g,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };
            let unfilled = buf.initialize_unfilled();
            match guard.try_io(|inner| unsafe {
                let n = libc::read(
                    inner.get_ref().as_raw_fd(),
                    unfilled.as_mut_ptr().cast(),
                    unfilled.len(),
                );
                if n < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(n as usize)
                }
            }) {
                Ok(Ok(n)) => {
                    buf.advance(n);
                    return Poll::Ready(Ok(()));
                }
                Ok(Err(e)) => return Poll::Ready(Err(e)),
                Err(_would_block) => continue,
            }
        }
    }
}

impl AsyncWrite for RfcommStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        loop {
            let mut guard = match self.inner.poll_write_ready(cx) {
                Poll::Ready(Ok(g)) => g,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            };
            match guard.try_io(|inner| unsafe {
                let n = libc::write(
                    inner.get_ref().as_raw_fd(),
                    data.as_ptr().cast(),
                    data.len(),
                );
                if n < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(n as usize)
                }
            }) {
                Ok(res) => return Poll::Ready(res),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        unsafe {
            libc::shutdown(self.inner.get_ref().as_raw_fd(), libc::SHUT_WR);
        }
        Poll::Ready(Ok(()))
    }
}

/// Single-socket multiplexed JSON-lines session (used by Bluetooth RFCOMM):
/// streams live `TntronStatus` updates while accepting commands & queries on the same connection.
async fn handle_multiplexed_client<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    read_half: R,
    mut write_half: W,
    cmd_tx: mpsc::Sender<CoreCommand>,
    status_rx: watch::Receiver<TntronStatus>,
    mut sub_rx: broadcast::Receiver<TntronStatus>,
    mc_bridge: McBridgeHandle,
) -> anyhow::Result<()> {
    let init = { serde_json::to_string(&*status_rx.borrow())? };
    write_half.write_all(init.as_bytes()).await?;
    write_half.write_all(b"\n").await?;

    let mut lines = BufReader::new(read_half).lines();
    loop {
        tokio::select! {
            bcast = sub_rx.recv() => {
                match bcast {
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
            line_res = lines.next_line() => {
                let Some(line) = line_res? else { break; };
                if line.trim().is_empty() {
                    continue;
                }
                let Ok(req) = serde_json::from_str::<IpcCommand>(&line) else {
                    send_response(
                        &mut write_half,
                        &IpcResponse {
                            ok: false,
                            error: Some("invalid command JSON".into()),
                            ..Default::default()
                        },
                    )
                    .await?;
                    continue;
                };

                if matches!(req, IpcCommand::Subscribe | IpcCommand::Auth { .. }) {
                    let st = status_rx.borrow().clone();
                    send_response(
                        &mut write_half,
                        &IpcResponse {
                            ok: true,
                            status: Some(st),
                            ..Default::default()
                        },
                    )
                    .await?;
                    continue;
                }

                if let Some(q) = match &req {
                    IpcCommand::GetLibrary { query, folder, limit } => Some(IpcQueryKind::GetLibrary {
                        query: query.clone(),
                        folder: folder.clone(),
                        limit: *limit,
                    }),
                    IpcCommand::GetThemes => Some(IpcQueryKind::GetThemes),
                    IpcCommand::GetExtensions => Some(IpcQueryKind::GetExtensions),
                    IpcCommand::GetCover => Some(IpcQueryKind::GetCover),
                    IpcCommand::GetAndroidStatus => Some(IpcQueryKind::GetAndroidStatus),
                    _ => None,
                } {
                    let (tx, rx) = oneshot::channel();
                    let _ = cmd_tx
                        .send(CoreCommand::IpcQuery {
                            req: q,
                            reply: Arc::new(std::sync::Mutex::new(Some(tx))),
                        })
                        .await;
                    if let Ok(Ok(resp)) = tokio::time::timeout(Duration::from_secs(2), rx).await {
                        send_response(&mut write_half, &resp).await?;
                    }
                    continue;
                }

                if !matches!(req, IpcCommand::Status) {
                    mc_bridge.record_command();
                }
                let current_track_path = status_rx.borrow().track_path.clone();
                for c in translate_ipc_command(req, &current_track_path) {
                    let _ = cmd_tx.send(c).await;
                }
                let st = status_rx.borrow().clone();
                send_response(
                    &mut write_half,
                    &IpcResponse {
                        ok: true,
                        status: Some(st),
                        ..Default::default()
                    },
                )
                .await?;
            }
        }
    }
    Ok(())
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
    expected_token: String,
) -> anyhow::Result<()> {
    let mut lines = BufReader::new(read_half).lines();
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
                    ..Default::default()
                },
            )
            .await?;
            continue;
        };

        if let IpcCommand::Auth { token } = req {
            if expected_token.trim().is_empty() || token == expected_token {
                authed = true;
                let st = status_rx.borrow().clone();
                send_response(
                    &mut write_half,
                    &IpcResponse {
                        ok: true,
                        status: Some(st),
                        message: Some("authenticated".into()),
                        ..Default::default()
                    },
                )
                .await?;
            } else {
                send_response(
                    &mut write_half,
                    &IpcResponse {
                        ok: false,
                        error: Some("invalid auth token".into()),
                        ..Default::default()
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
                    ..Default::default()
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
            IpcCommand::GetAndroidStatus => {
                let (tx, rx) = oneshot::channel();
                let _ = cmd_tx
                    .send(CoreCommand::IpcQuery {
                        req: IpcQueryKind::GetAndroidStatus,
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
                        status: Some(st),
                        ..Default::default()
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
                        status: Some(st),
                        ..Default::default()
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
                status: Some(st),
                ..Default::default()
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
        IpcCommand::TranslateLyrics { lang } => vec![CoreCommand::TranslateLyrics(lang)],
        IpcCommand::SetAudioSink { sink_name } => vec![CoreCommand::SetAudioSink(sink_name)],
        IpcCommand::SwitchAudioSource { source } => vec![CoreCommand::SwitchAudioSource(source)],
        IpcCommand::AndroidMedia {
            action,
            player_path,
        } => vec![CoreCommand::AndroidMedia {
            action,
            player_path,
        }],
        IpcCommand::BtConnect { device_path } => vec![CoreCommand::BtConnect(device_path)],
        IpcCommand::BtDisconnect { device_path } => vec![CoreCommand::BtDisconnect(device_path)],
        IpcCommand::BtReverseAudio { device_path } => {
            vec![CoreCommand::BtReverseAudio(device_path)]
        }
        IpcCommand::BtStreamVolume { volume_pct } => vec![CoreCommand::BtStreamVolume(volume_pct)],
        IpcCommand::BtStreamMute { muted } => vec![CoreCommand::BtStreamMute(muted)],
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
