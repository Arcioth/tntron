use serde::{Deserialize, Serialize};

/// Semantic version of the tntron Lua Extension SDK contract.
pub const API_VERSION: &str = "1.0";

/// Exhaustive list of capability permissions an extension manifest may request.
/// Everything is default-deny unless listed in `manifest.permissions`.
pub const VALID_PERMISSIONS: &[&str] = &[
    "playback:read",
    "playback:control",
    "queue:control",
    "library:read",
    "audio:dsp",
    "fs:write",
    "ui:sidebar",
    "ui:settings",
    "ui:topbar",
    "ui:player_widget",
    "themes:register",
    "storage",
    "notify",
    "network",
    "hw:control",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    pub main: String,
    #[serde(default = "default_api_version")]
    pub api_version: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
}

fn default_api_version() -> String {
    API_VERSION.to_string()
}

fn default_step() -> f64 {
    1.0
}

fn default_icon() -> String {
    "puzzle".to_string()
}

/// Declarative UI tree emitted by Lua extensions and rendered natively by iced.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExtWidget {
    Heading {
        text: String,
    },
    Text {
        text: String,
        #[serde(default)]
        muted: bool,
    },
    Badge {
        text: String,
        #[serde(default)]
        icon: Option<String>,
        #[serde(default)]
        accent: bool,
    },
    Row {
        #[serde(default)]
        children: Vec<ExtWidget>,
    },
    Button {
        id: String,
        label: String,
        #[serde(default)]
        icon: Option<String>,
        #[serde(default)]
        accent: bool,
        #[serde(default)]
        active: bool,
    },
    Toggle {
        id: String,
        label: String,
        value: bool,
    },
    Slider {
        id: String,
        label: String,
        min: f64,
        max: f64,
        #[serde(default = "default_step")]
        step: f64,
        value: f64,
        #[serde(default)]
        unit: String,
    },
    Progress {
        label: String,
        value: f64,
        max: f64,
    },
    Input {
        id: String,
        label: String,
        #[serde(default)]
        value: String,
        #[serde(default)]
        placeholder: String,
        #[serde(default)]
        submit_action: Option<String>,
        #[serde(default)]
        submit_label: Option<String>,
    },
    Visualizer {
        mode: String,
        #[serde(default)]
        playing: bool,
        #[serde(default = "default_volume")]
        volume: f64,
        #[serde(default = "default_step")]
        speed: f64,
    },
    Grid {
        #[serde(default = "default_grid_cols")]
        columns: usize,
        #[serde(default)]
        children: Vec<ExtWidget>,
    },
    Card {
        #[serde(default)]
        id: Option<String>,
        title: String,
        #[serde(default)]
        subtitle: Option<String>,
        #[serde(default)]
        eyebrow: Option<String>,
        #[serde(default)]
        icon: Option<String>,
        #[serde(default)]
        badge: Option<String>,
        #[serde(default)]
        variant: Option<String>,
        #[serde(default)]
        accent: bool,
        #[serde(default)]
        active: bool,
    },
}

fn default_grid_cols() -> usize {
    2
}

fn default_volume() -> f64 {
    80.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtSidebarTab {
    pub ext_id: String,
    pub id: String,
    pub label: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default)]
    pub widgets: Vec<ExtWidget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtSettingsPanel {
    pub ext_id: String,
    #[serde(default)]
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub widgets: Vec<ExtWidget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtRegisteredTheme {
    pub ext_id: String,
    pub slug: String,
    pub label: String,
    pub base: String,
    pub sidebar: String,
    pub main: String,
    pub card: String,
    pub card_hover: String,
    pub text: String,
    pub text_sub: String,
    pub accent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtSlotBadge {
    pub ext_id: String,
    pub text: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub accent: bool,
    #[serde(default)]
    pub action_id: Option<String>,
}

/// Host commands produced by Lua extensions via `tntron.*` SDK calls or returned command tables.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ExtHostCommand {
    RegisterSidebar {
        id: String,
        label: String,
        #[serde(default = "default_icon")]
        icon: String,
        #[serde(default)]
        widgets: Vec<ExtWidget>,
    },
    UnregisterSidebar {
        id: String,
    },
    RegisterSettings {
        #[serde(default)]
        id: String,
        title: String,
        #[serde(default)]
        widgets: Vec<ExtWidget>,
    },
    SetTopbarSlot {
        text: String,
        #[serde(default)]
        icon: Option<String>,
        #[serde(default)]
        accent: bool,
        #[serde(default)]
        action_id: Option<String>,
    },
    ClearTopbarSlot,
    SetPlayerWidget {
        text: String,
        #[serde(default)]
        icon: Option<String>,
        #[serde(default)]
        accent: bool,
        #[serde(default)]
        action_id: Option<String>,
    },
    ClearPlayerWidget,
    RegisterTheme {
        slug: String,
        label: String,
        base: String,
        sidebar: String,
        main: String,
        card: String,
        card_hover: String,
        text: String,
        text_sub: String,
        accent: String,
    },
    SetAudioFilter {
        filter: String,
    },
    ClearAudioFilter,
    PlaybackToggle,
    PlaybackSetPaused {
        paused: bool,
    },
    PlaybackPlayPath {
        path: String,
    },
    PlaybackSetMute {
        muted: bool,
    },
    PlaybackNext,
    PlaybackPrev,
    Seek {
        seconds: f64,
    },
    SeekRelative {
        delta: f64,
    },
    SetVolume {
        volume: f64,
    },
    SetSpeed {
        speed: f64,
    },
    QueueAdd {
        path: String,
    },
    QueueClear,
    ThemesSetActive {
        slug: String,
    },
    UiSetDensity {
        density: String,
    },
    UiSetLayout {
        layout: String,
    },
    UiSetCompactMenu {
        compact: bool,
    },
    LibraryRescan,
    FsCreateFolder {
        name: String,
    },
    FsDeleteFolder {
        path: String,
    },
    FsCopyTrackToFolder {
        track_path: String,
        folder_path: String,
    },
    FsDeleteTrack {
        path: String,
    },
    ExtensionsSetAll {
        enabled: bool,
    },
    NotifyDesktop {
        summary: String,
        body: String,
    },
    LyricsLoad {
        path: String,
        title: String,
        artist: String,
        duration_sec: f64,
    },
    NetFetchRss {
        url: String,
    },
    NetYtSearch {
        query: String,
    },
    NetYtPlay {
        url: String,
        #[serde(default)]
        title: Option<String>,
    },
    NetYtDownload {
        url: String,
    },
    StorageSet {
        key: String,
        value: serde_json::Value,
    },
    Toast {
        message: String,
    },
    LibraryOpenFolder {
        path: String,
    },
    LibraryPlayFolder {
        path: String,
        #[serde(default)]
        shuffle: bool,
    },
    AudioSetSpatial {
        enabled: bool,
        orbit_hz: f64,
    },
    AudioSetPitch {
        pitch_st: f64,
        #[serde(default)]
        link_pitch: Option<bool>,
        #[serde(default)]
        preserve_formant: Option<bool>,
    },
    UiSetLayoutConfig {
        patch: serde_json::Value,
    },
    UiOpenSidebar {
        tab_id: String,
    },
    HwDaeboardColor {
        hex: String,
    },
    HwDaeboardBrightness {
        level: u8,
    },
    HwDaeboardStart,
    HwDaeboardStop,
    HwDaetronStart,
    HwDaetronReload,
}

#[derive(Debug, Clone)]
pub struct ExtensionCardInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub permissions: Vec<String>,
    pub requires: Vec<String>,
    pub used_by: Vec<String>,
    pub enabled: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtPlaybackCtx {
    pub playing: bool,
    pub paused: bool,
    pub stopped: bool,
    pub muted: bool,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub volume: f64,
    pub speed: f64,
    pub pitch_st: f64,
    pub link_pitch: bool,
    pub preserve_formant: bool,
    pub spatial_on: bool,
    pub spatial_hz: f32,
    pub shuffle: bool,
    pub loop_mode: String,
    pub codec: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_path: String,
    pub lyric: String,
    pub bpm: Option<f64>,
    pub energy: f64,
    pub peak_db: f64,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
    pub beat_pulse: f32,
    pub bands: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtTrackSummary {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub folder: String,
    pub path: String,
    pub duration_sec: f64,
    pub is_favorite: bool,
    pub bpm: Option<f64>,
    pub bitrate_kbps: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtFolderSummary {
    pub name: String,
    pub path: String,
    pub track_count: usize,
    pub total_duration_sec: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ExtLibraryCtx {
    pub current_dir: String,
    pub tracks: Vec<ExtTrackSummary>,
    pub library_tracks: Vec<ExtTrackSummary>,
    pub folders: Vec<ExtFolderSummary>,
    pub favorites: Vec<String>,
    pub queue: Vec<ExtTrackSummary>,
}
