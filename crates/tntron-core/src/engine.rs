use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, watch};
use tracing::{error, info};

use crate::audio::{run_mpv_actor, MpvActorEvent, MpvCommand, MpvEvent, MpvSupervisor};
use crate::clock::{
    AbLoop, AudioPreset, DspSettings, LoopMode, PlayState, PlaybackClock, ShuffleMode, XorShift64,
};
use crate::ext_api::{
    ExtFolderSummary, ExtHostCommand, ExtLibraryCtx, ExtPlaybackCtx, ExtRegisteredTheme,
    ExtSettingsPanel, ExtSidebarTab, ExtSlotBadge, ExtTrackSummary, ExtensionCardInfo,
};
use crate::extensions::ExtensionHost;
use crate::hardware::{
    daeboard_start_sync, daeboard_stop_sync, daetron_reload_sync, daetron_start_sync,
    spawn_hardware_worker, HardwareCmd,
};
use crate::ipc::{run_ipc_server, write_status_file, TntronStatus};
use crate::library::{
    analyze_track_blocking, copy_track_to_folder_in_jail, create_folder_in_jail,
    delete_file_in_jail, delete_folder_in_jail, load_companion_lrc, read_directory, rename_in_jail,
    resolve_in_jail, scan_all_tracks_in_jail, BrowserEntry, LruCache, LyricLine, MetadataPatch,
    Scanner, Track, TrackAnalysis,
};
use crate::network;
use crate::storage::{
    default_extensions_dir, default_preferences_export_path, default_state_path, LayoutConfig,
    PersistentState, SpeedPreset, TrackSort,
};

#[derive(Debug, Clone)]
pub enum AsyncExtJob {
    LyricsLoad {
        path: PathBuf,
        title: String,
        artist: String,
        duration_sec: f64,
    },
    FetchRss {
        url: String,
    },
    YtSearch {
        query: String,
    },
    YtPlay {
        url: String,
        title: Option<String>,
    },
    YtDownload {
        url: String,
        target_dir: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub struct ExtAsyncResult {
    pub ext_id: String,
    pub event_name: String,
    pub payload: serde_json::Value,
    pub stream_play: Option<(String, String)>,
    pub rescan_after: bool,
}

#[derive(Debug, Clone)]
pub struct FolderCard {
    pub name: String,
    pub path: PathBuf,
    pub track_count: usize,
    pub cover_art: Option<Arc<Vec<u8>>>,
}

#[derive(Debug, Clone)]
pub struct ToastItem {
    pub id: u64,
    pub message: String,
    pub is_error: bool,
    pub expires_at: Instant,
}

#[derive(Default)]
pub struct CommandEffects {
    pub mpv_cmds: Vec<MpvCommand>,
    pub scan_paths: Vec<PathBuf>,
    pub reload_cards: bool,
    pub track_asset_target: Option<PathBuf>,
    pub ext_jobs: Vec<(String, AsyncExtJob)>,
    pub open_ext_tab: Option<(String, String)>,
}

#[derive(Debug, Clone)]
pub enum CoreCommand {
    NavigateTo(PathBuf),
    NavigateUp,
    Reload,
    PlayEntryIndex(usize),
    PlayPath(PathBuf),
    PlayAllInDir { shuffle: bool },
    PlayFolder { path: PathBuf, shuffle: bool },
    PlayFavorites { shuffle: bool },
    PlayQueueItem(usize),
    PlayUpcomingIndex(usize),
    CreateFolder(String),
    DeleteFolder(PathBuf),
    CopyTrackToFolder {
        track: PathBuf,
        folder: PathBuf,
    },
    DeleteTrackFile(PathBuf),
    RenameEntry { path: PathBuf, new_name: String },
    SetAllExtensionsEnabled(bool),
    ToggleFavorite(PathBuf),
    TogglePause,
    SetPaused(bool),
    Stop,
    NextTrack,
    PrevTrack,
    SeekDelta(f64),
    SeekAbsolute(f64),
    SetVolume(f64),
    VolumeDelta(i8),
    ToggleMute,
    SetMute(bool),
    SetSpeed(f64),
    StepSpeedDelta(f64),
    SetPitchSemitones(f64),
    StepPitchSemitones(f64),
    SetLinkPitch(bool),
    SetPreserveFormant(bool),
    SaveSpeedPreset(String),
    ApplySpeedPreset(String),
    DeleteSpeedPreset(String),
    UpdateLayoutConfig(LayoutConfig),
    ResetLayoutConfig,
    SetTrackBpm { path: PathBuf, bpm: f64 },
    CycleLoopMode,
    ToggleShuffle,
    SetAudioPreset(AudioPreset),
    StepBass(i8),
    StepMid(i8),
    StepTreble(i8),
    ToggleSpatial,
    StepSpatialHz(f32),
    MarkLoopA,
    MarkLoopB,
    ClearAbLoop,
    SaveTrackAbLoop,
    LoadTrackAbLoop,
    SetSleepTimer(Option<u64>),
    StepSleepMinutes(i64),
    SetThemeSlug(String),
    SetSortMode(TrackSort),
    ToggleSortColumn(TrackSort),
    SetViewDensity(String),
    SetHomeLayout(String),
    PlayAllLibrary { shuffle: bool },
    LocatePlayingTrack,
    ExportPreferences,
    ImportPreferences,
    ToggleExtension(String),
    ReloadExtensions,
    TriggerExtAction {
        ext_id: String,
        action_id: String,
        value: serde_json::Value,
    },
    AddToQueue(PathBuf),
    PlayNextInQueue(PathBuf),
    AddFolderToQueue(PathBuf),
    RemoveFromQueue(usize),
    ClearQueue,
    DismissToast(u64),
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct UiSnapshot {
    pub music_root: PathBuf,
    pub current_dir: PathBuf,
    pub folder_cards: Vec<FolderCard>,
    pub entries: Vec<BrowserEntry>,
    pub library_tracks: Vec<Arc<Track>>,
    pub all_tracks: Vec<Arc<Track>>,
    pub current_track: Option<Arc<Track>>,
    pub cover_art: Option<Arc<Vec<u8>>>,
    pub play_state: PlayState,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub volume: f64,
    pub is_muted: bool,
    pub speed: f64,
    pub pitch_st: f64,
    pub link_pitch: bool,
    pub preserve_formant: bool,
    pub speed_presets: Vec<SpeedPreset>,
    pub layout: LayoutConfig,
    pub analysis: Option<Arc<TrackAnalysis>>,
    pub track_bpm: Option<f64>,
    pub track_energy: f64,
    pub track_peak_db: f64,
    pub current_lyric: Option<String>,
    pub loop_mode: LoopMode,
    pub shuffle_mode: ShuffleMode,
    pub dsp: DspSettings,
    pub ab_loop: AbLoop,
    pub saved_loop_for_current: Option<AbLoop>,
    pub sleep_remaining_sec: Option<u64>,
    pub active_theme: String,
    pub sort_mode: TrackSort,
    pub sort_desc: bool,
    pub view_density: String,
    pub home_layout: String,
    pub favorites: Vec<PathBuf>,
    pub recent_tracks: Vec<Arc<Track>>,
    pub active_playlist: Vec<Arc<Track>>,
    pub active_playlist_index: Option<usize>,
    pub queue: Vec<Arc<Track>>,
    pub extensions_dir: PathBuf,
    pub extensions: Vec<ExtensionCardInfo>,
    pub ext_sidebar_tabs: Vec<ExtSidebarTab>,
    pub ext_settings_panels: Vec<ExtSettingsPanel>,
    pub ext_themes: Vec<ExtRegisteredTheme>,
    pub ext_topbar_slots: Vec<ExtSlotBadge>,
    pub ext_player_widgets: Vec<ExtSlotBadge>,
    pub compact_menu: bool,
    pub toasts: Vec<ToastItem>,
    pub mpv_connected: bool,
    pub mpv_audio_telemetry: Option<String>,
    pub cache_len: usize,
    pub error_banner: Option<String>,
}

#[derive(Debug, Clone)]
pub enum CoreUpdate {
    Snapshot(Box<UiSnapshot>),
    Tick {
        position_sec: f64,
        sleep_remaining_sec: Option<u64>,
    },
    OpenExtTab {
        ext_id: String,
        tab_id: String,
    },
}

pub struct CoreState {
    pub music_root: PathBuf,
    pub current_dir: PathBuf,
    pub state_path: Option<PathBuf>,
    pub extensions_dir: PathBuf,
    pub folder_cards: Vec<FolderCard>,
    pub entries: Vec<BrowserEntry>,
    pub all_library_tracks: Vec<Arc<Track>>,
    pub sort_desc: bool,
    pub active_playlist: Vec<Arc<Track>>,
    pub active_playlist_index: Option<usize>,
    pub active_playlist_dir: Option<PathBuf>,
    pub shuffle_deck: Vec<usize>,
    pub shuffle_history: Vec<usize>,
    pub rng: XorShift64,
    pub current_track: Option<Arc<Track>>,
    pub cover_art: Option<Arc<Vec<u8>>>,
    pub play_state: PlayState,
    pub clock: PlaybackClock,
    pub duration_sec: f64,
    pub volume: f64,
    pub is_muted: bool,
    pub speed: f64,
    pub loop_mode: LoopMode,
    pub shuffle_mode: ShuffleMode,
    pub ext_host: ExtensionHost,
    pub ext_audio_filters: HashMap<String, String>,
    pub persisted: PersistentState,
    pub ab_loop: AbLoop,
    pub sleep_deadline: Option<Instant>,
    pub toasts: Vec<ToastItem>,
    pub queue: Vec<Arc<Track>>,
    pub metadata_cache: LruCache<PathBuf, MetadataPatch>,
    pub cache_len: usize,
    pub mpv_connected: bool,
    pub mpv_codec: Option<String>,
    pub mpv_samplerate: Option<u64>,
    pub mpv_channels: Option<u64>,
    pub active_analysis: Option<Arc<TrackAnalysis>>,
    pub active_lyrics: Vec<LyricLine>,
    pub manual_bpms: HashMap<PathBuf, f64>,
    hw_tx: Option<mpsc::Sender<HardwareCmd>>,
    pub error_banner: Option<String>,
    ignore_next_end_file: bool,
    next_toast_id: u64,
    ext_tick_divider: u8,
    dirty_persist: bool,
}

impl CoreState {
    pub const METADATA_CACHE_CAP: usize = 10_000;

    pub fn new(music_root: PathBuf) -> Self {
        Self::with_paths(music_root, None, None)
    }

    pub fn with_persistence(music_root: PathBuf, state_path: PathBuf) -> Self {
        Self::with_paths(music_root, Some(state_path), Some(default_extensions_dir()))
    }

    fn with_paths(
        music_root: PathBuf,
        state_path: Option<PathBuf>,
        extensions_dir: Option<PathBuf>,
    ) -> Self {
        let canonical_root = music_root.canonicalize().unwrap_or(music_root);
        let persisted = state_path
            .as_ref()
            .map(|p| PersistentState::load_from(p))
            .unwrap_or_default();
        let ext_dir = extensions_dir.unwrap_or_else(default_extensions_dir);

        let mut state = Self {
            music_root: canonical_root.clone(),
            current_dir: canonical_root,
            state_path,
            extensions_dir: ext_dir.clone(),
            folder_cards: Vec::new(),
            entries: Vec::new(),
            all_library_tracks: Vec::new(),
            sort_desc: false,
            active_playlist: Vec::new(),
            active_playlist_index: None,
            active_playlist_dir: None,
            shuffle_deck: Vec::new(),
            shuffle_history: Vec::new(),
            rng: XorShift64::from_entropy(),
            current_track: None,
            cover_art: None,
            play_state: PlayState::Stopped,
            clock: PlaybackClock::new(),
            duration_sec: 0.0,
            volume: 100.0,
            is_muted: false,
            speed: persisted.speed.clamp(0.25, 3.0),
            loop_mode: LoopMode::All,
            shuffle_mode: ShuffleMode::Off,
            ext_host: ExtensionHost::new(),
            ext_audio_filters: HashMap::new(),
            persisted,
            ab_loop: AbLoop::default(),
            sleep_deadline: None,
            toasts: Vec::new(),
            queue: Vec::new(),
            metadata_cache: LruCache::new(Self::METADATA_CACHE_CAP),
            cache_len: 0,
            mpv_connected: false,
            mpv_codec: None,
            mpv_samplerate: None,
            mpv_channels: None,
            active_analysis: None,
            active_lyrics: Vec::new(),
            manual_bpms: HashMap::new(),
            hw_tx: None,
            error_banner: None,
            ignore_next_end_file: false,
            next_toast_id: 1,
            ext_tick_divider: 0,
            dirty_persist: false,
        };

        state.refresh_root_folders();
        state.refresh_all_tracks();
        if state.state_path.is_some() {
            state.ext_host.discover(&ext_dir);
            let pb = state.ext_playback_ctx();
            let lib = state.ext_library_ctx();
            let saved = state.persisted.enabled_extensions.clone();
            let cmds = state.ext_host.enable_saved(
                &saved,
                &pb,
                &lib,
                &state.persisted.extension_storage,
            );
            let mut boot_fx = CommandEffects::default();
            state.apply_ext_commands(cmds, &mut boot_fx);
        }
        state
    }

    pub fn push_toast(&mut self, msg: impl Into<String>, is_error: bool) {
        let id = self.next_toast_id;
        self.next_toast_id += 1;
        self.toasts.push(ToastItem {
            id,
            message: msg.into(),
            is_error,
            expires_at: Instant::now() + Duration::from_secs(if is_error { 5 } else { 3 }),
        });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    pub fn mark_dirty_and_save(&mut self) {
        self.persisted.enabled_extensions = self.ext_host.enabled_ids();
        self.dirty_persist = false;
        if let Some(path) = &self.state_path {
            let _ = self.persisted.save_atomic(path);
        }
    }

    pub fn sleep_remaining_sec(&self) -> Option<u64> {
        self.sleep_deadline
            .map(|d| d.saturating_duration_since(Instant::now()).as_secs())
    }

    pub fn current_lyric_line(&self) -> Option<String> {
        if self.active_lyrics.is_empty() {
            return None;
        }
        let pos = self.clock.now() + 0.15;
        self.active_lyrics
            .iter()
            .rfind(|l| l.time_sec <= pos && !l.text.trim().is_empty())
            .map(|l| l.text.trim().to_string())
    }

    pub fn current_track_bpm(&self) -> Option<f64> {
        let cur = self.current_track.as_ref()?;
        self.manual_bpms
            .get(&cur.path)
            .copied()
            .or(cur.bpm)
            .or_else(|| self.active_analysis.as_ref().and_then(|a| a.bpm))
    }

    fn apply_speed_and_pitch(&mut self, fx: &mut CommandEffects) {
        self.speed = self.speed.clamp(0.25, 3.0);
        self.persisted.speed = self.speed;
        if self.persisted.link_pitch {
            self.persisted.pitch_st = (12.0 * self.speed.log2()).clamp(-12.0, 12.0);
        } else {
            self.persisted.pitch_st = self.persisted.pitch_st.clamp(-12.0, 12.0);
        }
        self.clock.set_speed(self.speed);
        fx.mpv_cmds.push(MpvCommand::SetSpeed(self.speed));
        fx.mpv_cmds
            .push(MpvCommand::SetAudioFilter(self.effective_audio_filter()));
        self.dirty_persist = true;
    }

    fn ext_playback_ctx(&self) -> ExtPlaybackCtx {
        let now = self.clock.now();
        let (bands, beat_pulse, energy, peak_db) = if let Some(an) = &self.active_analysis {
            let (b, bt) = an.sample_at(now);
            (b.to_vec(), bt, an.energy, an.peak_db)
        } else {
            (vec![0.0; 16], 0.0, 0.0, -96.0)
        };
        ExtPlaybackCtx {
            playing: self.play_state == PlayState::Playing,
            paused: self.play_state == PlayState::Paused,
            stopped: self.play_state == PlayState::Stopped,
            muted: self.is_muted,
            position_sec: now,
            duration_sec: self.duration_sec,
            volume: self.volume,
            speed: self.speed,
            pitch_st: self.persisted.pitch_st,
            link_pitch: self.persisted.link_pitch,
            preserve_formant: self.persisted.preserve_formant,
            spatial_on: self.persisted.dsp.spatial_on,
            spatial_hz: self.persisted.dsp.spatial_hz,
            shuffle: self.shuffle_mode == ShuffleMode::On,
            loop_mode: self.loop_mode.label().to_string(),
            codec: self.mpv_codec.clone(),
            title: self
                .current_track
                .as_ref()
                .map(|t| t.title.clone())
                .unwrap_or_default(),
            artist: self
                .current_track
                .as_ref()
                .map(|t| t.artist.clone())
                .unwrap_or_default(),
            album: self
                .current_track
                .as_ref()
                .map(|t| t.album.clone())
                .unwrap_or_default(),
            track_path: self
                .current_track
                .as_ref()
                .map(|t| t.path.display().to_string())
                .unwrap_or_default(),
            lyric: self.current_lyric_line().unwrap_or_default(),
            bpm: self.current_track_bpm(),
            energy,
            peak_db,
            bitrate_kbps: self.current_track.as_ref().and_then(|t| t.bitrate_kbps),
            sample_rate_hz: self
                .mpv_samplerate
                .map(|v| v as u32)
                .or_else(|| self.current_track.as_ref().and_then(|t| t.sample_rate_hz)),
            channels: self
                .mpv_channels
                .map(|v| v as u8)
                .or_else(|| self.current_track.as_ref().and_then(|t| t.channels)),
            beat_pulse,
            bands,
        }
    }

    fn track_to_ext_summary(&self, t: &Track) -> ExtTrackSummary {
        let folder = t
            .path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Library".to_string());
        ExtTrackSummary {
            title: t.title.clone(),
            artist: t.artist.clone(),
            album: t.album.clone(),
            folder,
            path: t.path.display().to_string(),
            duration_sec: t.duration_sec,
            is_favorite: self.persisted.favorites.contains(&t.path),
            bpm: self.manual_bpms.get(&t.path).copied().or(t.bpm),
            bitrate_kbps: t.bitrate_kbps,
            sample_rate_hz: t.sample_rate_hz,
            channels: t.channels,
        }
    }

    fn ext_library_ctx(&self) -> ExtLibraryCtx {
        let tracks: Vec<ExtTrackSummary> = self
            .entries
            .iter()
            .filter_map(|e| match e {
                BrowserEntry::AudioTrack(t) => Some(self.track_to_ext_summary(t)),
                _ => None,
            })
            .collect();

        let mut seen = std::collections::HashSet::new();
        let mut library_tracks = Vec::new();
        for t in &tracks {
            if seen.insert(t.path.clone()) {
                library_tracks.push(t.clone());
            }
        }
        for t in &self.all_library_tracks {
            let s = self.track_to_ext_summary(t);
            if seen.insert(s.path.clone()) {
                library_tracks.push(s);
            }
        }
        for t in &self.active_playlist {
            let s = self.track_to_ext_summary(t);
            if seen.insert(s.path.clone()) {
                library_tracks.push(s);
            }
        }
        for p in self
            .persisted
            .favorites
            .iter()
            .chain(self.persisted.recent_tracks.iter())
        {
            if p.exists() {
                let s = p.display().to_string();
                if seen.insert(s) {
                    let tr = self.resolve_track_arc(p);
                    library_tracks.push(self.track_to_ext_summary(&tr));
                }
            }
        }

        let folders = self
            .folder_cards
            .iter()
            .map(|c| {
                let total_duration_sec: f64 = self
                    .all_library_tracks
                    .iter()
                    .filter(|t| t.path.parent() == Some(c.path.as_path()))
                    .map(|t| t.duration_sec)
                    .sum();
                ExtFolderSummary {
                    name: c.name.clone(),
                    path: c.path.display().to_string(),
                    track_count: c.track_count,
                    total_duration_sec,
                }
            })
            .collect();
        let queue = self
            .queue
            .iter()
            .map(|t| self.track_to_ext_summary(t))
            .collect();
        ExtLibraryCtx {
            current_dir: self.current_dir.display().to_string(),
            tracks,
            library_tracks,
            folders,
            favorites: self
                .persisted
                .favorites
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
            queue,
        }
    }

    fn dispatch_ext_event(&mut self, event_name: &str, fx: &mut CommandEffects) {
        if !self.ext_host.has_enabled() {
            return;
        }
        let pb = self.ext_playback_ctx();
        let lib = self.ext_library_ctx();
        let cmds = self.ext_host.dispatch_event(
            event_name,
            &pb,
            &lib,
            &self.persisted.extension_storage,
        );
        self.apply_ext_commands(cmds, fx);
    }

    fn dispatch_ext_tick(&mut self, fx: &mut CommandEffects) {
        if !self.ext_host.has_enabled() {
            return;
        }
        let pb = self.ext_playback_ctx();
        let lib = self.ext_library_ctx();
        let cmds =
            self.ext_host
                .dispatch_tick(&pb, &lib, &self.persisted.extension_storage);
        self.apply_ext_commands(cmds, fx);
    }

    pub fn handle_ext_async_result(&mut self, res: ExtAsyncResult) -> CommandEffects {
        let mut fx = CommandEffects::default();
        if let Some((stream_url, title)) = res.stream_play {
            let mut tr = Track::new(PathBuf::from(stream_url));
            tr.title = title;
            tr.artist = "YouTube".into();
            let arc = Arc::new(tr);
            fx.mpv_cmds.push(self.start_track(arc, None));
            self.dispatch_ext_event("trackChange", &mut fx);
        }
        if res.rescan_after {
            fx.scan_paths = self.load_current_dir();
            self.refresh_root_folders();
            self.refresh_all_tracks();
            fx.reload_cards = true;
        }
        if res.event_name == "lyricsLoaded" {
            let matches_current = self.current_track.as_ref().is_some_and(|cur| {
                res.payload
                    .get("path")
                    .and_then(|v| v.as_str())
                    .is_some_and(|p| Path::new(p) == cur.path)
            });
            if matches_current {
                if let Some(arr) = res.payload.get("lines").and_then(|v| v.as_array()) {
                    let parsed: Vec<LyricLine> = arr
                        .iter()
                        .filter_map(|item| {
                            let time_sec = item.get("time_sec")?.as_f64()?;
                            let text = item.get("text")?.as_str()?.to_string();
                            Some(LyricLine { time_sec, text })
                        })
                        .collect();
                    if !parsed.is_empty() {
                        self.active_lyrics = parsed;
                    }
                }
            }
        }
        let pb = self.ext_playback_ctx();
        let lib = self.ext_library_ctx();
        let cmds = self.ext_host.dispatch_event_to_ext(
            &res.ext_id,
            &res.event_name,
            res.payload,
            &pb,
            &lib,
            &self.persisted.extension_storage,
        );
        self.apply_ext_commands(cmds, &mut fx);
        if self.dirty_persist {
            self.mark_dirty_and_save();
        }
        fx
    }

    fn apply_ext_commands(&mut self, cmds: Vec<(String, ExtHostCommand)>, fx: &mut CommandEffects) {
        for (ext_id, cmd) in cmds {
            match cmd {
                ExtHostCommand::SetAudioFilter { filter } => {
                    let trimmed = filter.trim().to_string();
                    if trimmed.is_empty() {
                        self.ext_audio_filters.remove(&ext_id);
                    } else {
                        self.ext_audio_filters.insert(ext_id, trimmed);
                    }
                    fx.mpv_cmds
                        .push(MpvCommand::SetAudioFilter(self.effective_audio_filter()));
                }
                ExtHostCommand::ClearAudioFilter => {
                    if self.ext_audio_filters.remove(&ext_id).is_some() {
                        fx.mpv_cmds
                            .push(MpvCommand::SetAudioFilter(self.effective_audio_filter()));
                    }
                }
                ExtHostCommand::PlaybackToggle => {
                    if self.current_track.is_some() {
                        let paused = self.play_state == PlayState::Playing;
                        self.play_state = if paused {
                            PlayState::Paused
                        } else {
                            PlayState::Playing
                        };
                        self.clock.set_playing(!paused);
                        fx.mpv_cmds.push(MpvCommand::SetPause(paused));
                    }
                }
                ExtHostCommand::PlaybackSetPaused { paused } => {
                    if self.current_track.is_some() {
                        self.play_state = if paused {
                            PlayState::Paused
                        } else {
                            PlayState::Playing
                        };
                        self.clock.set_playing(!paused);
                        fx.mpv_cmds.push(MpvCommand::SetPause(paused));
                    }
                }
                ExtHostCommand::PlaybackNext => {
                    if let Some(c) = self.advance_next(false) {
                        fx.track_asset_target = self.current_track.as_ref().map(|t| t.path.clone());
                        fx.mpv_cmds.push(c);
                    }
                }
                ExtHostCommand::PlaybackPrev => {
                    if let Some(c) = self.step_prev() {
                        fx.track_asset_target = self.current_track.as_ref().map(|t| t.path.clone());
                        fx.mpv_cmds.push(c);
                    }
                }
                ExtHostCommand::Seek { seconds } => {
                    if self.current_track.is_some() {
                        let target = seconds.clamp(0.0, self.duration_sec.max(0.0));
                        self.clock.sync(target);
                        fx.mpv_cmds.push(MpvCommand::Seek {
                            seconds: target,
                            relative: false,
                        });
                    }
                }
                ExtHostCommand::SeekRelative { delta } => {
                    if self.current_track.is_some() {
                        let target =
                            (self.clock.now() + delta).clamp(0.0, self.duration_sec.max(0.0));
                        self.clock.sync(target);
                        fx.mpv_cmds.push(MpvCommand::Seek {
                            seconds: delta,
                            relative: true,
                        });
                    }
                }
                ExtHostCommand::SetVolume { volume } => {
                    self.volume = volume.clamp(0.0, 100.0);
                    fx.mpv_cmds.push(MpvCommand::SetVolume(self.volume));
                }
                ExtHostCommand::SetSpeed { speed } => {
                    self.speed = speed.clamp(0.25, 3.0);
                    self.apply_speed_and_pitch(fx);
                }
                ExtHostCommand::PlaybackPlayPath { path } => {
                    if path.starts_with("http://") || path.starts_with("https://") {
                        let mut tr = Track::new(PathBuf::from(&path));
                        tr.title = path
                            .rsplit('/')
                            .next()
                            .filter(|s| !s.is_empty())
                            .unwrap_or("Stream Audio")
                            .to_string();
                        tr.artist = "Network Stream".to_string();
                        let arc = Arc::new(tr);
                        fx.mpv_cmds.push(self.start_track(arc, None));
                        self.dispatch_ext_event("trackChange", fx);
                    } else if let Some(jailed) = resolve_in_jail(&self.music_root, Path::new(&path)) {
                        if Track::is_audio_file(&jailed) {
                            let pl_idx = self.active_playlist.iter().position(|t| t.path == jailed);
                            let track = self.resolve_track_arc(&jailed);
                            fx.track_asset_target = Some(track.path.clone());
                            fx.mpv_cmds.push(self.start_track(track, pl_idx));
                            self.dispatch_ext_event("trackChange", fx);
                        }
                    }
                }
                ExtHostCommand::PlaybackSetMute { muted } => {
                    self.is_muted = muted;
                    fx.mpv_cmds.push(MpvCommand::SetMute(muted));
                }
                ExtHostCommand::QueueAdd { path } => {
                    if path.starts_with("http://") || path.starts_with("https://") {
                        let mut tr = Track::new(PathBuf::from(&path));
                        tr.artist = "Stream".to_string();
                        self.queue.push(Arc::new(tr));
                    } else if let Some(jailed) = resolve_in_jail(&self.music_root, Path::new(&path)) {
                        if Track::is_audio_file(&jailed) {
                            let t = self.resolve_track_arc(&jailed);
                            self.queue.push(t);
                        }
                    }
                }
                ExtHostCommand::QueueClear => {
                    self.queue.clear();
                }
                ExtHostCommand::ThemesSetActive { slug } => {
                    self.persisted.active_theme = slug;
                    self.dirty_persist = true;
                    self.mark_dirty_and_save();
                }
                ExtHostCommand::UiSetDensity { density } => {
                    self.persisted.view_density = density;
                    self.dirty_persist = true;
                    self.mark_dirty_and_save();
                }
                ExtHostCommand::UiSetLayout { layout } => {
                    self.persisted.home_layout = normalize_home_layout(&layout);
                    self.dirty_persist = true;
                    self.mark_dirty_and_save();
                }
                ExtHostCommand::LibraryRescan => {
                    fx.scan_paths = self.load_current_dir();
                    self.refresh_root_folders();
                    self.refresh_all_tracks();
                    fx.reload_cards = true;
                }
                ExtHostCommand::FsCreateFolder { name } => {
                    let cur = self.current_dir.clone();
                    match create_folder_in_jail(&self.music_root, &cur, &name) {
                        Ok(created) => {
                            let folder_name = created
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or(name);
                            fx.scan_paths = self.load_current_dir();
                            self.refresh_root_folders();
                            self.refresh_all_tracks();
                            fx.reload_cards = true;
                            self.push_toast(format!("Created playlist folder: {folder_name}"), false);
                        }
                        Err(e) => self.push_toast(e, true),
                    }
                }
                ExtHostCommand::FsDeleteFolder { path } => {
                    self.exec_delete_folder(Path::new(&path), fx);
                }
                ExtHostCommand::FsCopyTrackToFolder {
                    track_path,
                    folder_path,
                } => {
                    self.exec_copy_track_to_folder(
                        Path::new(&track_path),
                        Path::new(&folder_path),
                        fx,
                    );
                }
                ExtHostCommand::FsDeleteTrack { path } => {
                    self.exec_delete_track_file(Path::new(&path), fx);
                }
                ExtHostCommand::ExtensionsSetAll { enabled } => {
                    self.exec_set_all_extensions(enabled, Some(&ext_id), fx);
                }
                ExtHostCommand::UiSetCompactMenu { .. } => {}
                ExtHostCommand::NotifyDesktop { summary, body } => {
                    network::spawn_desktop_notification(summary, body);
                }
                ExtHostCommand::LyricsLoad {
                    path,
                    title,
                    artist,
                    duration_sec,
                } => {
                    fx.ext_jobs.push((
                        ext_id,
                        AsyncExtJob::LyricsLoad {
                            path: PathBuf::from(path),
                            title,
                            artist,
                            duration_sec,
                        },
                    ));
                }
                ExtHostCommand::NetFetchRss { url } => {
                    fx.ext_jobs.push((ext_id, AsyncExtJob::FetchRss { url }));
                }
                ExtHostCommand::NetYtSearch { query } => {
                    fx.ext_jobs.push((ext_id, AsyncExtJob::YtSearch { query }));
                }
                ExtHostCommand::NetYtPlay { url, title } => {
                    fx.ext_jobs.push((ext_id, AsyncExtJob::YtPlay { url, title }));
                }
                ExtHostCommand::NetYtDownload { url } => {
                    fx.ext_jobs.push((
                        ext_id,
                        AsyncExtJob::YtDownload {
                            url,
                            target_dir: self.current_dir.clone(),
                        },
                    ));
                }
                ExtHostCommand::StorageSet { key, value } => {
                    self.persisted
                        .extension_storage
                        .entry(ext_id)
                        .or_default()
                        .insert(key, value);
                    self.dirty_persist = true;
                }
                ExtHostCommand::Toast { message } => {
                    self.push_toast(message, false);
                }
                ExtHostCommand::LibraryOpenFolder { path } => {
                    if let Some(jailed) = resolve_in_jail(&self.music_root, Path::new(&path)) {
                        if jailed.is_dir() {
                            self.current_dir = jailed;
                            fx.scan_paths = self.load_current_dir();
                        }
                    }
                }
                ExtHostCommand::LibraryPlayFolder { path, shuffle } => {
                    if let Some(jailed) = resolve_in_jail(&self.music_root, Path::new(&path)) {
                        if jailed.is_dir() {
                            self.current_dir = jailed;
                            fx.scan_paths = self.load_current_dir();
                            if let Some(mc) = self.play_dir_internal(shuffle) {
                                fx.track_asset_target =
                                    self.current_track.as_ref().map(|t| t.path.clone());
                                fx.mpv_cmds.push(mc);
                                self.dispatch_ext_event("trackChange", fx);
                            }
                        }
                    }
                }
                ExtHostCommand::AudioSetSpatial { enabled, orbit_hz } => {
                    self.persisted.dsp.spatial_on = enabled;
                    self.persisted.dsp.spatial_hz = (orbit_hz as f32).clamp(0.05, 1.5);
                    self.apply_dsp_cmds(fx);
                }
                ExtHostCommand::AudioSetPitch {
                    pitch_st,
                    link_pitch,
                    preserve_formant,
                } => {
                    if let Some(lp) = link_pitch {
                        self.persisted.link_pitch = lp;
                    }
                    if let Some(pf) = preserve_formant {
                        self.persisted.preserve_formant = pf;
                    }
                    self.persisted.pitch_st = pitch_st.clamp(-12.0, 12.0);
                    self.apply_speed_and_pitch(fx);
                }
                ExtHostCommand::UiSetLayoutConfig { patch } => {
                    apply_layout_patch(&mut self.persisted.layout, &patch);
                    if let Some(hl) = patch.get("home_layout").and_then(|v| v.as_str()) {
                        self.persisted.home_layout = normalize_home_layout(hl);
                    }
                    if let Some(d) = patch.get("density").and_then(|v| v.as_str()) {
                        self.persisted.view_density = d.to_string();
                    }
                    self.dirty_persist = true;
                }
                ExtHostCommand::UiOpenSidebar { tab_id } => {
                    fx.open_ext_tab = Some((ext_id, tab_id));
                }
                ExtHostCommand::HwDaeboardColor { hex } => {
                    if let Some(tx) = &self.hw_tx {
                        let _ = tx.try_send(HardwareCmd::DaeboardColor(hex));
                    }
                }
                ExtHostCommand::HwDaeboardBrightness { level } => {
                    if let Some(tx) = &self.hw_tx {
                        let _ = tx.try_send(HardwareCmd::DaeboardBrightness(level));
                    }
                }
                ExtHostCommand::HwDaeboardStart => match daeboard_start_sync() {
                    Ok(msg) => self.push_toast(msg, false),
                    Err(err) => self.push_toast(err, true),
                },
                ExtHostCommand::HwDaeboardStop => match daeboard_stop_sync() {
                    Ok(msg) => self.push_toast(msg, false),
                    Err(err) => self.push_toast(err, true),
                },
                ExtHostCommand::HwDaetronStart => match daetron_start_sync() {
                    Ok(msg) => self.push_toast(msg, false),
                    Err(err) => self.push_toast(err, true),
                },
                ExtHostCommand::HwDaetronReload => {
                    match daetron_reload_sync() {
                        Ok(msg) => self.push_toast(msg, false),
                        Err(err) => self.push_toast(err, true),
                    }
                }
                ExtHostCommand::RegisterSidebar { .. }
                | ExtHostCommand::UnregisterSidebar { .. }
                | ExtHostCommand::RegisterSettings { .. }
                | ExtHostCommand::SetTopbarSlot { .. }
                | ExtHostCommand::ClearTopbarSlot
                | ExtHostCommand::SetPlayerWidget { .. }
                | ExtHostCommand::ClearPlayerWidget
                | ExtHostCommand::RegisterTheme { .. } => {}
            }
        }
    }

    fn effective_audio_filter(&self) -> String {
        let mut stages = Vec::new();
        let base = self.persisted.dsp.build_mpv_filter();
        if !base.is_empty() {
            stages.push(base);
        }
        let mut keys: Vec<&String> = self.ext_audio_filters.keys().collect();
        keys.sort();
        let mut has_ext_rubberband = false;
        for k in keys {
            if let Some(f) = self.ext_audio_filters.get(k) {
                if !f.is_empty() {
                    if f.contains("rubberband=") {
                        has_ext_rubberband = true;
                    }
                    stages.push(f.clone());
                }
            }
        }
        if !has_ext_rubberband && self.persisted.pitch_st.abs() >= 0.05 {
            let pitch_scale = 2.0_f64.powf(self.persisted.pitch_st / 12.0);
            let formant = if self.persisted.preserve_formant {
                "preserved"
            } else {
                "shifted"
            };
            stages.push(format!(
                "rubberband=pitch-scale={pitch_scale:.4}:formant={formant}"
            ));
        }
        stages.join(",")
    }

    fn exec_delete_folder(&mut self, path: &Path, fx: &mut CommandEffects) {
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "playlist".into());
        let canon_target = match resolve_in_jail(&self.music_root, path) {
            Some(p) => p,
            None => {
                self.push_toast("Folder outside music jail", true);
                return;
            }
        };

        let playing_inside = self
            .current_track
            .as_ref()
            .is_some_and(|t| t.path.starts_with(&canon_target));
        if playing_inside {
            self.stop_locally();
            fx.mpv_cmds.push(MpvCommand::Stop);
        }

        match delete_folder_in_jail(&self.music_root, &canon_target) {
            Ok(parent) => {
                if self.current_dir.starts_with(&canon_target) {
                    self.current_dir = parent;
                }
                self.persisted
                    .favorites
                    .retain(|p| !p.starts_with(&canon_target));
                self.persisted
                    .recent_tracks
                    .retain(|p| !p.starts_with(&canon_target));
                self.queue.retain(|t| !t.path.starts_with(&canon_target));
                self.dirty_persist = true;
                self.refresh_root_folders();
                self.refresh_all_tracks();
                fx.scan_paths = self.load_current_dir();
                fx.reload_cards = true;
                self.push_toast(format!("Removed playlist '{label}'"), false);
            }
            Err(e) => self.push_toast(e, true),
        }
    }

    fn exec_copy_track_to_folder(
        &mut self,
        track: &Path,
        folder: &Path,
        fx: &mut CommandEffects,
    ) {
        match copy_track_to_folder_in_jail(&self.music_root, track, folder) {
            Ok(_) => {
                let folder_name = folder
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "playlist".into());
                self.refresh_root_folders();
                self.refresh_all_tracks();
                fx.scan_paths = self.load_current_dir();
                fx.reload_cards = true;
                self.push_toast(format!("Added to playlist '{folder_name}'"), false);
            }
            Err(e) => self.push_toast(e, true),
        }
    }

    fn exec_delete_track_file(&mut self, path: &Path, fx: &mut CommandEffects) {
        let title = path
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "track".into());
        if self
            .current_track
            .as_ref()
            .is_some_and(|t| t.path == path)
        {
            self.stop_locally();
            fx.mpv_cmds.push(MpvCommand::Stop);
        }
        match delete_file_in_jail(&self.music_root, path) {
            Ok(removed) => {
                self.persisted.favorites.retain(|p| p != &removed);
                self.persisted.recent_tracks.retain(|p| p != &removed);
                self.queue.retain(|t| t.path != removed);
                self.dirty_persist = true;
                self.refresh_root_folders();
                self.refresh_all_tracks();
                fx.scan_paths = self.load_current_dir();
                fx.reload_cards = true;
                self.push_toast(format!("Removed '{title}'"), false);
            }
            Err(e) => self.push_toast(e, true),
        }
    }

    fn exec_set_all_extensions(
        &mut self,
        enabled: bool,
        keep_ext_id: Option<&str>,
        fx: &mut CommandEffects,
    ) {
        let pb = self.ext_playback_ctx();
        let lib = self.ext_library_ctx();
        let ids: Vec<String> = self.ext_host.cards().into_iter().map(|c| c.id).collect();
        for id in ids {
            if enabled {
                let cmds = self
                    .ext_host
                    .enable(&id, &pb, &lib, &self.persisted.extension_storage);
                self.apply_ext_commands(cmds, fx);
            } else if keep_ext_id != Some(id.as_str()) {
                let disabled = self
                    .ext_host
                    .disable(&id, &pb, &lib, &self.persisted.extension_storage);
                for d in disabled {
                    self.ext_audio_filters.remove(&d);
                }
            }
        }
        fx.mpv_cmds
            .push(MpvCommand::SetAudioFilter(self.effective_audio_filter()));
        self.dirty_persist = true;
        self.push_toast(
            if enabled {
                "Enabled all extensions"
            } else {
                "Disabled extensions"
            },
            false,
        );
    }

    fn exec_export_preferences(&mut self) {
        let path = default_preferences_export_path();
        self.persisted.enabled_extensions = self.ext_host.enabled_ids();
        match self.persisted.save_atomic(&path) {
            Ok(()) => self.push_toast(
                format!("Exported preferences to {}", path.display()),
                false,
            ),
            Err(e) => self.push_toast(format!("Export failed: {e}"), true),
        }
    }

    fn exec_import_preferences(&mut self, fx: &mut CommandEffects) {
        let path = default_preferences_export_path();
        if !path.is_file() {
            self.push_toast(
                format!("No export found at {}", path.display()),
                true,
            );
            return;
        }
        let Ok(raw) = std::fs::read_to_string(&path) else {
            self.push_toast("Failed to read preferences file", true);
            return;
        };
        let Ok(imported) = serde_json::from_str::<PersistentState>(&raw) else {
            self.push_toast("Invalid preferences JSON format", true);
            return;
        };
        self.persisted = imported;
        self.persisted.home_layout = normalize_home_layout(&self.persisted.home_layout);
        self.dirty_persist = true;
        self.sort_entries();
        self.apply_dsp_cmds(fx);
        let saved = self.persisted.enabled_extensions.clone();
        self.ext_audio_filters.clear();
        let pb = self.ext_playback_ctx();
        let lib = self.ext_library_ctx();
        let cmds = self.ext_host.enable_saved(
            &saved,
            &pb,
            &lib,
            &self.persisted.extension_storage,
        );
        self.apply_ext_commands(cmds, fx);
        fx.mpv_cmds
            .push(MpvCommand::SetAudioFilter(self.effective_audio_filter()));
        self.push_toast("Imported preferences & reloaded extensions", false);
    }

    fn mpv_audio_telemetry_label(&self) -> Option<String> {
        let codec = self.mpv_codec.as_deref()?;
        let sr = self
            .mpv_samplerate
            .map(|h| format!("{:.1}kHz", h as f64 / 1000.0))
            .unwrap_or_default();
        let ch = self.mpv_channels.map(|c| format!("{c}ch")).unwrap_or_default();
        let mut parts = vec![codec.to_uppercase()];
        if !sr.is_empty() {
            parts.push(sr);
        }
        if !ch.is_empty() {
            parts.push(ch);
        }
        Some(parts.join(" · "))
    }

    pub fn refresh_root_folders(&mut self) {
        let old_cards = std::mem::take(&mut self.folder_cards);
        let root_entries = read_directory(&self.music_root, &self.music_root);
        self.folder_cards = root_entries
            .into_iter()
            .filter_map(|e| match e {
                BrowserEntry::Directory { name, path } => {
                    let prev = old_cards.iter().find(|c| c.path == path);
                    Some(FolderCard {
                        name,
                        path,
                        track_count: prev.map(|c| c.track_count).unwrap_or(0),
                        cover_art: prev.and_then(|c| c.cover_art.clone()),
                    })
                }
                _ => None,
            })
            .collect();
    }

    pub fn refresh_all_tracks(&mut self) {
        let raw = scan_all_tracks_in_jail(&self.music_root);
        let mut tracks = Vec::with_capacity(raw.len());
        for mut track in raw {
            if let Some(patch) = self.metadata_cache.get(&track.path) {
                apply_patch(&mut track, patch);
            }
            tracks.push(Arc::new(track));
        }
        self.all_library_tracks = tracks;
    }

    pub fn load_current_dir(&mut self) -> Vec<PathBuf> {
        let mut raw = read_directory(&self.current_dir, &self.music_root);
        let mut uncached = Vec::new();

        for entry in &mut raw {
            if let BrowserEntry::AudioTrack(track_arc) = entry {
                if let Some(patch) = self.metadata_cache.get(&track_arc.path) {
                    let mut t = (**track_arc).clone();
                    apply_patch(&mut t, patch);
                    *track_arc = Arc::new(t);
                } else {
                    uncached.push(track_arc.path.clone());
                }
            }
        }

        self.entries = raw;
        self.sort_entries();
        uncached
    }

    fn sort_entries(&mut self) {
        let sort = self.persisted.sort_mode;
        let mut head_dirs = Vec::new();
        let mut tracks = Vec::new();
        for e in self.entries.drain(..) {
            if e.is_dir() {
                head_dirs.push(e);
            } else if let BrowserEntry::AudioTrack(t) = e {
                tracks.push(t);
            }
        }
        let desc = self.sort_desc;
        tracks.sort_by(|a, b| {
            let ord = match sort {
                TrackSort::Id => a
                    .track_number
                    .unwrap_or(u32::MAX)
                    .cmp(&b.track_number.unwrap_or(u32::MAX))
                    .then_with(|| ascii_cmp(&a.filename, &b.filename)),
                TrackSort::Title => ascii_cmp(&a.title, &b.title),
                TrackSort::Duration => a
                    .duration_sec
                    .partial_cmp(&b.duration_sec)
                    .unwrap_or(std::cmp::Ordering::Equal),
            };
            if desc {
                ord.reverse()
            } else {
                ord
            }
        });
        self.entries = head_dirs;
        for t in tracks {
            self.entries.push(BrowserEntry::AudioTrack(t));
        }
    }

    pub fn apply_metadata_batch(&mut self, batch: Vec<MetadataPatch>) -> bool {
        let mut changed = false;
        for patch in batch {
            if self.metadata_cache.get_immutable(&patch.path).is_none() {
                self.cache_len = (self.cache_len + 1).min(Self::METADATA_CACHE_CAP);
            }
            self.metadata_cache.insert(patch.path.clone(), patch.clone());

            for entry in &mut self.entries {
                if let BrowserEntry::AudioTrack(track_arc) = entry {
                    if track_arc.path == patch.path {
                        let mut t = (**track_arc).clone();
                        apply_patch(&mut t, &patch);
                        *track_arc = Arc::new(t);
                        changed = true;
                    }
                }
            }

            for track_arc in &mut self.active_playlist {
                if track_arc.path == patch.path {
                    let mut t = (**track_arc).clone();
                    apply_patch(&mut t, &patch);
                    *track_arc = Arc::new(t);
                }
            }

            for track_arc in &mut self.all_library_tracks {
                if track_arc.path == patch.path {
                    let mut t = (**track_arc).clone();
                    apply_patch(&mut t, &patch);
                    *track_arc = Arc::new(t);
                }
            }

            if let Some(cur) = &mut self.current_track {
                if cur.path == patch.path {
                    let mut t = (**cur).clone();
                    apply_patch(&mut t, &patch);
                    *cur = Arc::new(t);
                    changed = true;
                }
            }
        }
        if changed {
            self.sort_entries();
        }
        changed
    }

    fn resolve_track_arc(&self, path: &Path) -> Arc<Track> {
        if let Some(cur) = &self.current_track {
            if cur.path == path {
                return Arc::clone(cur);
            }
        }
        for entry in &self.entries {
            if let BrowserEntry::AudioTrack(t) = entry {
                if t.path == path {
                    return Arc::clone(t);
                }
            }
        }
        let mut t = Track::new(path.to_path_buf());
        if let Some(patch) = self.metadata_cache.get_immutable(&path.to_path_buf()) {
            apply_patch(&mut t, patch);
        }
        Arc::new(t)
    }

    pub fn snapshot(&self) -> UiSnapshot {
        let library_tracks: Vec<Arc<Track>> = self
            .entries
            .iter()
            .filter_map(|e| match e {
                BrowserEntry::AudioTrack(t) => Some(Arc::clone(t)),
                _ => None,
            })
            .collect();

        let recent_tracks = self
            .persisted
            .recent_tracks
            .iter()
            .filter(|p| p.exists())
            .map(|p| self.resolve_track_arc(p))
            .collect();

        let saved_loop_for_current = self
            .current_track
            .as_ref()
            .and_then(|t| self.persisted.saved_loops.get(&t.path).copied());

        UiSnapshot {
            music_root: self.music_root.clone(),
            current_dir: self.current_dir.clone(),
            folder_cards: self.folder_cards.clone(),
            entries: self.entries.clone(),
            library_tracks,
            all_tracks: self.all_library_tracks.clone(),
            current_track: self.current_track.clone(),
            cover_art: self.cover_art.clone(),
            play_state: self.play_state,
            position_sec: self.clock.now(),
            duration_sec: self.duration_sec,
            volume: self.volume,
            is_muted: self.is_muted,
            speed: self.speed,
            pitch_st: self.persisted.pitch_st,
            link_pitch: self.persisted.link_pitch,
            preserve_formant: self.persisted.preserve_formant,
            speed_presets: self.persisted.speed_presets.clone(),
            layout: self.persisted.layout.clone(),
            analysis: self.active_analysis.clone(),
            track_bpm: self.current_track_bpm(),
            track_energy: self
                .active_analysis
                .as_ref()
                .map(|a| a.energy)
                .unwrap_or(0.0),
            track_peak_db: self
                .active_analysis
                .as_ref()
                .map(|a| a.peak_db)
                .unwrap_or(-96.0),
            current_lyric: self.current_lyric_line(),
            loop_mode: self.loop_mode,
            shuffle_mode: self.shuffle_mode,
            dsp: self.persisted.dsp.clone(),
            ab_loop: self.ab_loop,
            saved_loop_for_current,
            sleep_remaining_sec: self.sleep_remaining_sec(),
            active_theme: self.persisted.active_theme.clone(),
            sort_mode: self.persisted.sort_mode,
            sort_desc: self.sort_desc,
            view_density: self.persisted.view_density.clone(),
            home_layout: normalize_home_layout(&self.persisted.home_layout),
            favorites: self.persisted.favorites.clone(),
            recent_tracks,
            active_playlist: self.active_playlist.clone(),
            active_playlist_index: self.active_playlist_index,
            queue: self.queue.clone(),
            extensions_dir: self.extensions_dir.clone(),
            extensions: self.ext_host.cards(),
            ext_sidebar_tabs: self.ext_host.sidebar_tabs.clone(),
            ext_settings_panels: self.ext_host.settings_panels.clone(),
            ext_themes: self.ext_host.themes.clone(),
            ext_topbar_slots: self.ext_host.topbar_slots.clone(),
            ext_player_widgets: self.ext_host.player_widgets.clone(),
            compact_menu: self.ext_host.compact_menu,
            toasts: self.toasts.clone(),
            mpv_connected: self.mpv_connected,
            mpv_audio_telemetry: self.mpv_audio_telemetry_label(),
            cache_len: self.cache_len,
            error_banner: self.error_banner.clone(),
        }
    }

    fn activate_current_dir_playlist(&mut self) {
        self.active_playlist = self
            .entries
            .iter()
            .filter_map(|e| match e {
                BrowserEntry::AudioTrack(t) => Some(Arc::clone(t)),
                _ => None,
            })
            .collect();
        self.active_playlist_dir = Some(self.current_dir.clone());
    }

    fn rebuild_shuffle_deck(&mut self, exclude_idx: Option<usize>) {
        let len = self.active_playlist.len();
        self.shuffle_deck.clear();
        if len == 0 {
            return;
        }
        for i in 0..len {
            if Some(i) != exclude_idx {
                self.shuffle_deck.push(i);
            }
        }
        for i in (1..self.shuffle_deck.len()).rev() {
            let j = self.rng.next_bound(i + 1);
            self.shuffle_deck.swap(i, j);
        }
    }

    fn record_track_play(&mut self, track: &Track) {
        let p = &track.path;
        if p.exists() {
            self.persisted.recent_tracks.retain(|x| x != p);
            self.persisted.recent_tracks.insert(0, p.clone());
            self.persisted.recent_tracks.truncate(30);
            self.dirty_persist = true;
        }
    }

    fn start_track(&mut self, track: Arc<Track>, playlist_idx: Option<usize>) -> MpvCommand {
        if self.current_track.is_some() && self.play_state != PlayState::Stopped {
            self.ignore_next_end_file = true;
        }
        self.active_playlist_index = playlist_idx;
        self.record_track_play(&track);
        self.duration_sec = track.duration_sec;
        self.current_track = Some(Arc::clone(&track));
        self.cover_art = None;
        self.active_analysis = None;
        self.active_lyrics = load_companion_lrc(&track.path);
        self.ab_loop.clear();
        self.play_state = PlayState::Playing;
        self.clock.sync(0.0);
        self.clock.set_speed(self.speed);
        self.clock.set_playing(true);

        MpvCommand::LoadFile {
            path: track.path.to_string_lossy().into_owned(),
            replace: true,
        }
    }

    pub fn advance_next(&mut self, from_end_file: bool) -> Option<MpvCommand> {
        if !self.queue.is_empty() {
            let next_track = self.queue.remove(0);
            let idx = self.active_playlist.iter().position(|t| t.path == next_track.path);
            return Some(self.start_track(next_track, idx));
        }

        let len = self.active_playlist.len();
        if len == 0 {
            self.stop_locally();
            return None;
        }

        if from_end_file && self.loop_mode == LoopMode::Track {
            if let Some(idx) = self.active_playlist_index {
                let track = Arc::clone(&self.active_playlist[idx]);
                return Some(self.start_track(track, Some(idx)));
            }
        }

        if self.shuffle_mode == ShuffleMode::On {
            if let Some(cur) = self.active_playlist_index {
                self.shuffle_history.push(cur);
                if self.shuffle_history.len() > 256 {
                    self.shuffle_history.remove(0);
                }
            }
            if self.shuffle_deck.is_empty() {
                if self.loop_mode == LoopMode::Off && from_end_file {
                    self.stop_locally();
                    return None;
                }
                self.rebuild_shuffle_deck(self.active_playlist_index);
            }
            if let Some(next_idx) = self.shuffle_deck.pop() {
                let track = Arc::clone(&self.active_playlist[next_idx]);
                return Some(self.start_track(track, Some(next_idx)));
            }
            self.stop_locally();
            return None;
        }

        let next_idx = match self.active_playlist_index {
            Some(i) if i + 1 < len => i + 1,
            Some(_) => {
                if self.loop_mode == LoopMode::Off && from_end_file {
                    self.stop_locally();
                    return None;
                }
                0
            }
            None => 0,
        };
        let track = Arc::clone(&self.active_playlist[next_idx]);
        Some(self.start_track(track, Some(next_idx)))
    }

    fn step_prev(&mut self) -> Option<MpvCommand> {
        let len = self.active_playlist.len();
        if len == 0 {
            return None;
        }
        let prev_idx = if self.shuffle_mode == ShuffleMode::On {
            self.shuffle_history.pop().unwrap_or(0)
        } else {
            match self.active_playlist_index {
                Some(0) | None => len - 1,
                Some(i) => i - 1,
            }
        };
        let track = Arc::clone(&self.active_playlist[prev_idx]);
        Some(self.start_track(track, Some(prev_idx)))
    }

    fn stop_locally(&mut self) {
        self.play_state = PlayState::Stopped;
        self.clock.set_playing(false);
        self.clock.sync(0.0);
        self.current_track = None;
        self.cover_art = None;
        self.ab_loop.clear();
        self.duration_sec = 0.0;
    }

    fn apply_dsp_cmds(&mut self, fx: &mut CommandEffects) {
        fx.mpv_cmds
            .push(MpvCommand::SetAudioFilter(self.effective_audio_filter()));
        self.dirty_persist = true;
    }

    fn play_path_list(
        &mut self,
        paths: &[PathBuf],
        start_path: Option<&Path>,
        shuffle: bool,
    ) -> Option<MpvCommand> {
        let tracks: Vec<Arc<Track>> = paths
            .iter()
            .filter(|p| p.is_file() && Track::is_audio_file(p))
            .map(|p| self.resolve_track_arc(p))
            .collect();
        if tracks.is_empty() {
            return None;
        }
        self.active_playlist = tracks;
        self.active_playlist_dir = None;
        if shuffle {
            self.shuffle_mode = ShuffleMode::On;
            self.rebuild_shuffle_deck(None);
            let idx = self.shuffle_deck.pop().unwrap_or(0);
            let track = Arc::clone(&self.active_playlist[idx]);
            Some(self.start_track(track, Some(idx)))
        } else {
            let idx = start_path
                .and_then(|sp| self.active_playlist.iter().position(|t| t.path == sp))
                .unwrap_or(0);
            let track = Arc::clone(&self.active_playlist[idx]);
            Some(self.start_track(track, Some(idx)))
        }
    }

    fn play_dir_internal(&mut self, shuffle: bool) -> Option<MpvCommand> {
        self.activate_current_dir_playlist();
        if self.active_playlist.is_empty() {
            return None;
        }
        if shuffle {
            self.shuffle_mode = ShuffleMode::On;
            self.rebuild_shuffle_deck(None);
            let first = self.shuffle_deck.pop().unwrap_or(0);
            let track = Arc::clone(&self.active_playlist[first]);
            Some(self.start_track(track, Some(first)))
        } else {
            self.shuffle_mode = ShuffleMode::Off;
            let track = Arc::clone(&self.active_playlist[0]);
            Some(self.start_track(track, Some(0)))
        }
    }

    pub fn handle_command(&mut self, cmd: CoreCommand) -> CommandEffects {
        let mut fx = CommandEffects::default();

        match cmd {
            CoreCommand::NavigateTo(path) => {
                if let Some(jailed) = resolve_in_jail(&self.music_root, &path) {
                    if jailed.is_dir() {
                        self.current_dir = jailed;
                        fx.scan_paths = self.load_current_dir();
                    }
                }
            }
            CoreCommand::NavigateUp => {
                if self.current_dir != self.music_root {
                    if let Some(parent) = self.current_dir.parent() {
                        if let Some(jailed) = resolve_in_jail(&self.music_root, parent) {
                            self.current_dir = jailed;
                            fx.scan_paths = self.load_current_dir();
                        }
                    }
                }
            }
            CoreCommand::Reload => {
                self.refresh_root_folders();
                self.refresh_all_tracks();
                fx.scan_paths = self.load_current_dir();
                fx.reload_cards = true;
            }
            CoreCommand::PlayEntryIndex(idx) => {
                if let Some(entry) = self.entries.get(idx).cloned() {
                    match entry {
                        BrowserEntry::ParentDir(_) | BrowserEntry::Directory { .. } => {
                            if let Some(jailed) = resolve_in_jail(&self.music_root, entry.path()) {
                                if jailed.is_dir() {
                                    self.current_dir = jailed;
                                    fx.scan_paths = self.load_current_dir();
                                }
                            }
                        }
                        BrowserEntry::AudioTrack(track) => {
                            self.activate_current_dir_playlist();
                            let pl_idx = self.active_playlist.iter().position(|t| t.path == track.path);
                            if self.shuffle_mode == ShuffleMode::On {
                                self.rebuild_shuffle_deck(pl_idx);
                            }
                            fx.track_asset_target = Some(track.path.clone());
                            fx.mpv_cmds.push(self.start_track(track, pl_idx));
                            self.dispatch_ext_event("trackChange", &mut fx);
                        }
                    }
                }
            }
            CoreCommand::PlayPath(path) => {
                if let Some(jailed) = resolve_in_jail(&self.music_root, &path) {
                    if let Some(parent) = jailed.parent() {
                        self.current_dir = parent.to_path_buf();
                        fx.scan_paths = self.load_current_dir();
                        self.activate_current_dir_playlist();
                    }
                    let pl_idx = self.active_playlist.iter().position(|t| t.path == jailed);
                    let track = self.resolve_track_arc(&jailed);
                    fx.track_asset_target = Some(track.path.clone());
                    fx.mpv_cmds.push(self.start_track(track, pl_idx));
                    self.dispatch_ext_event("trackChange", &mut fx);
                }
            }
            CoreCommand::PlayFolder { path, shuffle } => {
                if let Some(jailed) = resolve_in_jail(&self.music_root, &path) {
                    if jailed.is_dir() {
                        self.current_dir = jailed;
                        fx.scan_paths = self.load_current_dir();
                        if let Some(mc) = self.play_dir_internal(shuffle) {
                            fx.track_asset_target = self.current_track.as_ref().map(|t| t.path.clone());
                            fx.mpv_cmds.push(mc);
                            self.dispatch_ext_event("trackChange", &mut fx);
                        }
                    }
                }
            }
            CoreCommand::PlayAllInDir { shuffle } => {
                if let Some(mc) = self.play_dir_internal(shuffle) {
                    fx.track_asset_target = self.current_track.as_ref().map(|t| t.path.clone());
                    fx.mpv_cmds.push(mc);
                    self.dispatch_ext_event("trackChange", &mut fx);
                }
            }
            CoreCommand::PlayFavorites { shuffle } => {
                let favs = self.persisted.favorites.clone();
                if let Some(mc) = self.play_path_list(&favs, None, shuffle) {
                    fx.track_asset_target = self.current_track.as_ref().map(|t| t.path.clone());
                    fx.mpv_cmds.push(mc);
                    self.dispatch_ext_event("trackChange", &mut fx);
                } else {
                    self.push_toast("No playable favorite tracks yet", true);
                }
            }
            CoreCommand::PlayQueueItem(idx) => {
                if idx < self.queue.len() {
                    let track = self.queue.remove(idx);
                    let pl_idx = self.active_playlist.iter().position(|t| t.path == track.path);
                    fx.track_asset_target = Some(track.path.clone());
                    fx.mpv_cmds.push(self.start_track(track, pl_idx));
                    self.dispatch_ext_event("trackChange", &mut fx);
                }
            }
            CoreCommand::PlayUpcomingIndex(idx) => {
                if let Some(track) = self.active_playlist.get(idx).cloned() {
                    fx.track_asset_target = Some(track.path.clone());
                    fx.mpv_cmds.push(self.start_track(track, Some(idx)));
                    self.dispatch_ext_event("trackChange", &mut fx);
                }
            }
            CoreCommand::CreateFolder(name) => {
                match create_folder_in_jail(&self.music_root, &self.current_dir, &name) {
                    Ok(_) => {
                        self.refresh_root_folders();
                        self.refresh_all_tracks();
                        fx.scan_paths = self.load_current_dir();
                        fx.reload_cards = true;
                        self.push_toast(format!("Created playlist '{}'", name.trim()), false);
                    }
                    Err(e) => self.push_toast(e, true),
                }
            }
            CoreCommand::DeleteFolder(path) => {
                self.exec_delete_folder(&path, &mut fx);
            }
            CoreCommand::CopyTrackToFolder { track, folder } => {
                self.exec_copy_track_to_folder(&track, &folder, &mut fx);
            }
            CoreCommand::DeleteTrackFile(path) => {
                self.exec_delete_track_file(&path, &mut fx);
            }
            CoreCommand::RenameEntry { path, new_name } => {
                match rename_in_jail(&self.music_root, &path, &new_name) {
                    Ok(_) => {
                        self.refresh_root_folders();
                        self.refresh_all_tracks();
                        fx.scan_paths = self.load_current_dir();
                        fx.reload_cards = true;
                        self.push_toast(format!("Renamed to '{}'", new_name.trim()), false);
                    }
                    Err(e) => self.push_toast(e, true),
                }
            }
            CoreCommand::SetAllExtensionsEnabled(enabled) => {
                self.exec_set_all_extensions(enabled, None, &mut fx);
            }
            CoreCommand::ToggleFavorite(path) => {
                if let Some(pos) = self.persisted.favorites.iter().position(|p| p == &path) {
                    self.persisted.favorites.remove(pos);
                    self.push_toast("Removed from Favorites", false);
                } else {
                    self.persisted.favorites.insert(0, path);
                    self.push_toast("Added to ★ Favorites", false);
                }
                self.dirty_persist = true;
            }
            CoreCommand::TogglePause => {
                if self.current_track.is_some() {
                    let paused = self.play_state == PlayState::Playing;
                    self.play_state = if paused {
                        PlayState::Paused
                    } else {
                        PlayState::Playing
                    };
                    self.clock.set_playing(!paused);
                    fx.mpv_cmds.push(MpvCommand::SetPause(paused));
                    self.dispatch_ext_event("playbackStateChange", &mut fx);
                } else if !self.entries.is_empty() {
                    if let Some(mc) = self.play_dir_internal(self.shuffle_mode == ShuffleMode::On) {
                        fx.track_asset_target = self.current_track.as_ref().map(|t| t.path.clone());
                        fx.mpv_cmds.push(mc);
                        self.dispatch_ext_event("trackChange", &mut fx);
                    }
                }
            }
            CoreCommand::SetPaused(paused) => {
                if self.current_track.is_some() {
                    self.play_state = if paused {
                        PlayState::Paused
                    } else {
                        PlayState::Playing
                    };
                    self.clock.set_playing(!paused);
                    fx.mpv_cmds.push(MpvCommand::SetPause(paused));
                    self.dispatch_ext_event("playbackStateChange", &mut fx);
                }
            }
            CoreCommand::Stop => {
                if self.current_track.is_some() {
                    self.ignore_next_end_file = true;
                    self.stop_locally();
                    fx.mpv_cmds.push(MpvCommand::Stop);
                    self.dispatch_ext_event("playbackStateChange", &mut fx);
                }
            }
            CoreCommand::NextTrack => {
                if let Some(c) = self.advance_next(false) {
                    fx.track_asset_target = self.current_track.as_ref().map(|t| t.path.clone());
                    fx.mpv_cmds.push(c);
                    self.dispatch_ext_event("trackChange", &mut fx);
                }
            }
            CoreCommand::PrevTrack => {
                if let Some(c) = self.step_prev() {
                    fx.track_asset_target = self.current_track.as_ref().map(|t| t.path.clone());
                    fx.mpv_cmds.push(c);
                    self.dispatch_ext_event("trackChange", &mut fx);
                }
            }
            CoreCommand::SeekDelta(delta) => {
                if self.current_track.is_some() {
                    let target = (self.clock.now() + delta).clamp(0.0, self.duration_sec.max(0.0));
                    self.clock.sync(target);
                    fx.mpv_cmds.push(MpvCommand::Seek {
                        seconds: delta,
                        relative: true,
                    });
                }
            }
            CoreCommand::SeekAbsolute(pos) => {
                if self.current_track.is_some() {
                    let target = pos.clamp(0.0, self.duration_sec.max(0.0));
                    self.clock.sync(target);
                    fx.mpv_cmds.push(MpvCommand::Seek {
                        seconds: target,
                        relative: false,
                    });
                }
            }
            CoreCommand::SetVolume(vol) => {
                self.volume = vol.clamp(0.0, 100.0);
                fx.mpv_cmds.push(MpvCommand::SetVolume(self.volume));
            }
            CoreCommand::VolumeDelta(delta) => {
                self.volume = (self.volume + f64::from(delta)).clamp(0.0, 100.0);
                fx.mpv_cmds.push(MpvCommand::SetVolume(self.volume));
            }
            CoreCommand::ToggleMute => {
                self.is_muted = !self.is_muted;
                fx.mpv_cmds.push(MpvCommand::SetMute(self.is_muted));
            }
            CoreCommand::SetMute(m) => {
                self.is_muted = m;
                fx.mpv_cmds.push(MpvCommand::SetMute(m));
            }
            CoreCommand::SetSpeed(s) => {
                self.speed = s.clamp(0.25, 3.0);
                self.apply_speed_and_pitch(&mut fx);
            }
            CoreCommand::StepSpeedDelta(delta) => {
                self.speed = ((self.speed + delta) * 100.0).round() / 100.0;
                self.apply_speed_and_pitch(&mut fx);
            }
            CoreCommand::SetPitchSemitones(st) => {
                self.persisted.link_pitch = false;
                self.persisted.pitch_st = st.clamp(-12.0, 12.0);
                self.apply_speed_and_pitch(&mut fx);
            }
            CoreCommand::StepPitchSemitones(delta) => {
                self.persisted.link_pitch = false;
                self.persisted.pitch_st =
                    ((self.persisted.pitch_st + delta) * 10.0).round() / 10.0;
                self.apply_speed_and_pitch(&mut fx);
            }
            CoreCommand::SetLinkPitch(link) => {
                self.persisted.link_pitch = link;
                self.apply_speed_and_pitch(&mut fx);
            }
            CoreCommand::SetPreserveFormant(preserve) => {
                self.persisted.preserve_formant = preserve;
                self.apply_speed_and_pitch(&mut fx);
            }
            CoreCommand::SaveSpeedPreset(name) => {
                let trimmed = name.trim().to_string();
                if !trimmed.is_empty() {
                    self.persisted
                        .speed_presets
                        .retain(|p| !p.name.eq_ignore_ascii_case(&trimmed));
                    self.persisted.speed_presets.push(SpeedPreset {
                        name: trimmed.clone(),
                        speed: self.speed,
                        pitch_st: self.persisted.pitch_st,
                        link_pitch: self.persisted.link_pitch,
                        preserve_formant: self.persisted.preserve_formant,
                    });
                    self.dirty_persist = true;
                    self.push_toast(format!("Saved speed preset '{trimmed}'"), false);
                }
            }
            CoreCommand::ApplySpeedPreset(name) => {
                if let Some(p) = self
                    .persisted
                    .speed_presets
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(&name))
                    .cloned()
                {
                    self.speed = p.speed.clamp(0.25, 3.0);
                    self.persisted.pitch_st = p.pitch_st.clamp(-12.0, 12.0);
                    self.persisted.link_pitch = p.link_pitch;
                    self.persisted.preserve_formant = p.preserve_formant;
                    self.apply_speed_and_pitch(&mut fx);
                    self.push_toast(
                        format!(
                            "Preset '{}': {:.2}x / {:+.1}st",
                            p.name, self.speed, self.persisted.pitch_st
                        ),
                        false,
                    );
                }
            }
            CoreCommand::DeleteSpeedPreset(name) => {
                let before = self.persisted.speed_presets.len();
                self.persisted
                    .speed_presets
                    .retain(|p| !p.name.eq_ignore_ascii_case(&name));
                if self.persisted.speed_presets.len() != before {
                    self.dirty_persist = true;
                    self.push_toast(format!("Deleted preset '{name}'"), false);
                }
            }
            CoreCommand::UpdateLayoutConfig(cfg) => {
                self.persisted.layout = cfg;
                self.dirty_persist = true;
            }
            CoreCommand::ResetLayoutConfig => {
                self.persisted.layout = LayoutConfig::default();
                self.persisted.view_density = "normal".to_string();
                self.persisted.home_layout = "table".to_string();
                self.dirty_persist = true;
                self.push_toast("Layout reset to defaults", false);
            }
            CoreCommand::SetTrackBpm { path, bpm } => {
                if (30.0..=300.0).contains(&bpm) {
                    self.manual_bpms.insert(path, bpm);
                }
            }
            CoreCommand::CycleLoopMode => {
                self.loop_mode = self.loop_mode.next();
            }
            CoreCommand::ToggleShuffle => {
                self.shuffle_mode = self.shuffle_mode.toggle();
                if self.shuffle_mode == ShuffleMode::On {
                    self.rebuild_shuffle_deck(self.active_playlist_index);
                } else {
                    self.shuffle_deck.clear();
                    self.shuffle_history.clear();
                }
            }
            CoreCommand::SetAudioPreset(preset) => {
                self.persisted.dsp.apply_preset(preset);
                self.apply_dsp_cmds(&mut fx);
                self.push_toast(format!("DSP Preset: {}", preset.label()), false);
            }
            CoreCommand::StepBass(d) => {
                self.persisted.dsp.step_bass(d);
                self.apply_dsp_cmds(&mut fx);
            }
            CoreCommand::StepMid(d) => {
                self.persisted.dsp.step_mid(d);
                self.apply_dsp_cmds(&mut fx);
            }
            CoreCommand::StepTreble(d) => {
                self.persisted.dsp.step_treble(d);
                self.apply_dsp_cmds(&mut fx);
            }
            CoreCommand::ToggleSpatial => {
                self.persisted.dsp.toggle_spatial();
                self.apply_dsp_cmds(&mut fx);
                let st = if self.persisted.dsp.spatial_on { "ON" } else { "OFF" };
                self.push_toast(format!("8D Spatial Orbit: {st}"), false);
            }
            CoreCommand::StepSpatialHz(d) => {
                self.persisted.dsp.step_spatial_hz(d);
                self.apply_dsp_cmds(&mut fx);
            }
            CoreCommand::MarkLoopA => {
                if self.current_track.is_some() {
                    self.ab_loop.a_sec = Some(self.clock.now());
                    if let Some(b) = self.ab_loop.b_sec {
                        if b <= self.ab_loop.a_sec.unwrap_or(0.0) {
                            self.ab_loop.b_sec = None;
                        }
                    }
                }
            }
            CoreCommand::MarkLoopB => {
                if self.current_track.is_some() {
                    let now = self.clock.now();
                    if self.ab_loop.a_sec.is_none() {
                        self.ab_loop.a_sec = Some(0.0);
                    }
                    if now > self.ab_loop.a_sec.unwrap_or(0.0) + 0.2 {
                        self.ab_loop.b_sec = Some(now);
                    }
                }
            }
            CoreCommand::ClearAbLoop => {
                self.ab_loop.clear();
            }
            CoreCommand::SaveTrackAbLoop => {
                if self.ab_loop.is_active() {
                    if let Some(cur) = &self.current_track {
                        self.persisted.saved_loops.insert(cur.path.clone(), self.ab_loop);
                        self.dirty_persist = true;
                        self.push_toast("Saved A-B loop bookmark", false);
                    }
                }
            }
            CoreCommand::LoadTrackAbLoop => {
                if let Some(cur) = &self.current_track {
                    if let Some(&saved) = self.persisted.saved_loops.get(&cur.path) {
                        self.ab_loop = saved;
                        self.push_toast("Restored A-B loop bookmark", false);
                    }
                }
            }
            CoreCommand::SetSleepTimer(mins) => {
                self.sleep_deadline = mins.map(|m| Instant::now() + Duration::from_secs(m * 60));
                match mins {
                    Some(m) => self.push_toast(format!("Sleep Timer: {m} min"), false),
                    None => self.push_toast("Sleep Timer cleared", false),
                }
            }
            CoreCommand::StepSleepMinutes(delta) => {
                let cur_secs = self.sleep_remaining_sec().unwrap_or(0) as i64;
                let next = (cur_secs + delta * 60).max(0) as u64;
                self.sleep_deadline = if next == 0 {
                    None
                } else {
                    Some(Instant::now() + Duration::from_secs(next))
                };
            }
            CoreCommand::SetThemeSlug(slug) => {
                self.persisted.active_theme = slug;
                self.dirty_persist = true;
            }
            CoreCommand::SetSortMode(sort) => {
                self.persisted.sort_mode = sort;
                self.sort_desc = false;
                self.sort_entries();
                self.dirty_persist = true;
            }
            CoreCommand::ToggleSortColumn(col) => {
                if self.persisted.sort_mode == col {
                    self.sort_desc = !self.sort_desc;
                } else {
                    self.persisted.sort_mode = col;
                    self.sort_desc = false;
                }
                self.sort_entries();
                self.dirty_persist = true;
            }
            CoreCommand::SetViewDensity(d) => {
                self.persisted.view_density = d;
                self.dirty_persist = true;
            }
            CoreCommand::SetHomeLayout(layout) => {
                self.persisted.home_layout = normalize_home_layout(&layout);
                self.dirty_persist = true;
            }
            CoreCommand::PlayAllLibrary { shuffle } => {
                let paths: Vec<PathBuf> = self
                    .all_library_tracks
                    .iter()
                    .map(|t| t.path.clone())
                    .collect();
                if let Some(mc) = self.play_path_list(&paths, None, shuffle) {
                    fx.track_asset_target = self.current_track.as_ref().map(|t| t.path.clone());
                    fx.mpv_cmds.push(mc);
                    self.dispatch_ext_event("trackChange", &mut fx);
                } else {
                    self.push_toast("No playable library tracks found", true);
                }
            }
            CoreCommand::LocatePlayingTrack => {
                if let Some(cur) = self.current_track.clone() {
                    if let Some(parent) = cur.path.parent() {
                        if let Some(jailed) = resolve_in_jail(&self.music_root, parent) {
                            if jailed.is_dir() {
                                self.current_dir = jailed;
                                fx.scan_paths = self.load_current_dir();
                                self.push_toast(format!("Located '{}'", cur.title), false);
                            }
                        }
                    }
                } else {
                    self.push_toast("Nothing playing to locate", true);
                }
            }
            CoreCommand::ExportPreferences => {
                self.exec_export_preferences();
            }
            CoreCommand::ImportPreferences => {
                self.exec_import_preferences(&mut fx);
            }
            CoreCommand::ToggleExtension(id) => {
                let pb = self.ext_playback_ctx();
                let lib = self.ext_library_ctx();
                let (now_enabled, disabled_ids, cmds) = self.ext_host.toggle(
                    &id,
                    &pb,
                    &lib,
                    &self.persisted.extension_storage,
                );
                for did in disabled_ids {
                    self.ext_audio_filters.remove(&did);
                }
                self.apply_ext_commands(cmds, &mut fx);
                fx.mpv_cmds
                    .push(MpvCommand::SetAudioFilter(self.effective_audio_filter()));
                let card_info = self
                    .ext_host
                    .cards()
                    .into_iter()
                    .find(|c| c.id == id);
                let label = card_info
                    .as_ref()
                    .map(|c| c.name.clone())
                    .unwrap_or(id);
                let err_msg = card_info.and_then(|c| c.error);
                if let Some(err) = err_msg {
                    self.push_toast(format!("{label}: {err}"), true);
                } else {
                    self.push_toast(
                        if now_enabled {
                            format!("Enabled {label}")
                        } else {
                            format!("Disabled {label}")
                        },
                        false,
                    );
                }
                self.dirty_persist = true;
            }
            CoreCommand::ReloadExtensions => {
                let prev_enabled = self.ext_host.enabled_ids();
                self.ext_host.discover(&self.extensions_dir);
                self.ext_audio_filters.clear();
                let pb = self.ext_playback_ctx();
                let lib = self.ext_library_ctx();
                let cmds = self.ext_host.enable_saved(
                    &prev_enabled,
                    &pb,
                    &lib,
                    &self.persisted.extension_storage,
                );
                self.apply_ext_commands(cmds, &mut fx);
                fx.mpv_cmds
                    .push(MpvCommand::SetAudioFilter(self.effective_audio_filter()));
                self.push_toast(
                    format!("Scanned {} extensions", self.ext_host.cards().len()),
                    false,
                );
            }
            CoreCommand::TriggerExtAction {
                ext_id,
                action_id,
                value,
            } => {
                let pb = self.ext_playback_ctx();
                let lib = self.ext_library_ctx();
                let cmds = self.ext_host.dispatch_action(
                    &ext_id,
                    &action_id,
                    value,
                    &pb,
                    &lib,
                    &self.persisted.extension_storage,
                );
                self.apply_ext_commands(cmds, &mut fx);
            }
            CoreCommand::AddToQueue(path) => {
                if let Some(jailed) = resolve_in_jail(&self.music_root, &path) {
                    if Track::is_audio_file(&jailed) {
                        let t = self.resolve_track_arc(&jailed);
                        let title = t.title.clone();
                        self.queue.push(t);
                        self.push_toast(format!("Queued: {title}"), false);
                    }
                }
            }
            CoreCommand::PlayNextInQueue(path) => {
                if let Some(jailed) = resolve_in_jail(&self.music_root, &path) {
                    if Track::is_audio_file(&jailed) {
                        let t = self.resolve_track_arc(&jailed);
                        let title = t.title.clone();
                        self.queue.insert(0, t);
                        self.push_toast(format!("Playing next: {title}"), false);
                    }
                }
            }
            CoreCommand::AddFolderToQueue(path) => {
                if let Some(jailed) = resolve_in_jail(&self.music_root, &path) {
                    if jailed.is_dir() {
                        let entries = read_directory(&self.music_root, &jailed);
                        let mut added = 0usize;
                        for entry in entries {
                            if let BrowserEntry::AudioTrack(t) = entry {
                                self.queue.push(self.resolve_track_arc(&t.path));
                                added += 1;
                            }
                        }
                        let fname = jailed
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "Folder".into());
                        self.push_toast(format!("Queued {added} tracks from {fname}"), false);
                    }
                }
            }
            CoreCommand::RemoveFromQueue(idx) => {
                if idx < self.queue.len() {
                    self.queue.remove(idx);
                }
            }
            CoreCommand::ClearQueue => {
                self.queue.clear();
            }
            CoreCommand::DismissToast(id) => {
                self.toasts.retain(|t| t.id != id);
            }
            CoreCommand::Shutdown => {
                self.mark_dirty_and_save();
                fx.mpv_cmds.push(MpvCommand::Quit);
            }
        }

        if self.dirty_persist {
            self.mark_dirty_and_save();
        }
        fx
    }

    /// Periodic tick check (200ms): A-B loop wraparound, Sleep timer, 1Hz extension tick, & toasts.
    pub fn check_timers(&mut self) -> (Vec<MpvCommand>, bool) {
        let mut cmds = Vec::new();
        let now = Instant::now();

        let prev_toasts = self.toasts.len();
        self.toasts.retain(|t| t.expires_at > now);
        let mut ui_dirty = self.toasts.len() != prev_toasts;

        if self.ext_host.has_enabled() {
            self.ext_tick_divider = self.ext_tick_divider.saturating_add(1);
            if self.ext_tick_divider >= 5 {
                self.ext_tick_divider = 0;
                let mut tick_fx = CommandEffects::default();
                self.dispatch_ext_tick(&mut tick_fx);
                cmds.extend(tick_fx.mpv_cmds);
                if self.dirty_persist {
                    self.mark_dirty_and_save();
                }
                ui_dirty = true;
            }
        }

        if self.play_state == PlayState::Playing && self.ab_loop.is_active() {
            if let (Some(a), Some(b)) = (self.ab_loop.a_sec, self.ab_loop.b_sec) {
                if self.clock.now() >= b {
                    self.clock.sync(a);
                    cmds.push(MpvCommand::Seek {
                        seconds: a,
                        relative: false,
                    });
                }
            }
        }

        if let Some(deadline) = self.sleep_deadline {
            if now >= deadline {
                self.sleep_deadline = None;
                if self.play_state == PlayState::Playing {
                    self.play_state = PlayState::Paused;
                    self.clock.set_playing(false);
                    cmds.push(MpvCommand::SetPause(true));
                }
                self.push_toast("Sleep Timer paused playback", false);
            }
        }

        (cmds, ui_dirty)
    }

    pub fn handle_mpv_event(&mut self, ev: MpvActorEvent) -> CommandEffects {
        let mut fx = CommandEffects::default();

        match ev {
            MpvActorEvent::TimePos(pos) => {
                self.clock.sync(pos);
            }
            MpvActorEvent::Event(MpvEvent::PropertyChange { name, data, .. }) => match name.as_str() {
                "pause" => {
                    if let Some(paused) = data.and_then(|v| v.as_bool()) {
                        if self.current_track.is_some() {
                            self.play_state = if paused {
                                PlayState::Paused
                            } else {
                                PlayState::Playing
                            };
                            self.clock.set_playing(!paused);
                        }
                    }
                }
                "duration" => {
                    if let Some(dur) = data.and_then(|v| v.as_f64()) {
                        self.duration_sec = dur.max(0.0);
                    }
                }
                "volume" => {
                    if let Some(vol) = data.and_then(|v| v.as_f64()) {
                        self.volume = vol.clamp(0.0, 100.0);
                    }
                }
                "audio-codec-name" => {
                    self.mpv_codec = data.and_then(|v| v.as_str().map(str::to_string));
                }
                "audio-params" => {
                    if let Some(obj) = data.and_then(|v| v.as_object().cloned()) {
                        self.mpv_samplerate = obj.get("samplerate").and_then(|v| v.as_u64());
                        self.mpv_channels = obj.get("channel-count").and_then(|v| v.as_u64());
                    }
                }
                _ => {}
            },
            MpvActorEvent::Event(MpvEvent::EndFile { reason, .. }) => {
                if self.ignore_next_end_file {
                    self.ignore_next_end_file = false;
                } else if reason.as_deref() == Some("eof") {
                    if let Some(c) = self.advance_next(true) {
                        if let Some(cur) = &self.current_track {
                            if cur.path.exists() {
                                fx.track_asset_target = Some(cur.path.clone());
                            }
                        }
                        fx.mpv_cmds.push(c);
                        self.dispatch_ext_event("trackChange", &mut fx);
                    } else {
                        self.dispatch_ext_event("playbackStateChange", &mut fx);
                    }
                }
            }
            MpvActorEvent::Closed => {
                self.mpv_connected = false;
                self.error_banner = Some("mpv backend closed unexpectedly".into());
                self.stop_locally();
            }
            _ => {}
        }

        fx
    }
}

fn ascii_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.bytes()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

fn normalize_home_layout(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "cards" | "card" | "grid" => "cards".to_string(),
        "gallery" | "coverflow" => "gallery".to_string(),
        "minimal" | "compact" | "list" => "minimal".to_string(),
        _ => "table".to_string(),
    }
}

fn apply_patch(track: &mut Track, patch: &MetadataPatch) {
    if let Some(t) = &patch.title {
        track.title = t.clone();
    }
    if let Some(a) = &patch.artist {
        track.artist = a.clone();
    }
    if let Some(al) = &patch.album {
        track.album = al.clone();
    }
    if patch.duration_sec > 0.0 {
        track.set_duration(patch.duration_sec);
    }
    track.track_number = patch.track_number;
    track.sample_rate_hz = patch.sample_rate_hz;
    track.bitrate_kbps = patch.bitrate_kbps;
    track.channels = patch.channels;
    if patch.bpm.is_some() {
        track.bpm = patch.bpm;
    }
}

fn apply_layout_patch(cfg: &mut LayoutConfig, patch: &serde_json::Value) {
    if let Some(s) = patch.get("sidebar_side").and_then(|v| v.as_str()) {
        cfg.sidebar_side = if s.eq_ignore_ascii_case("right") {
            "right".to_string()
        } else {
            "left".to_string()
        };
    }
    if let Some(w) = patch.get("sidebar_width").and_then(|v| v.as_f64()) {
        cfg.sidebar_width = (w as f32).clamp(180.0, 340.0);
    }
    if let Some(b) = patch.get("show_banner").and_then(|v| v.as_bool()) {
        cfg.show_banner = b;
    }
    if let Some(b) = patch.get("show_recent_strip").and_then(|v| v.as_bool()) {
        cfg.show_recent_strip = b;
    }
    if let Some(b) = patch.get("show_col_album").and_then(|v| v.as_bool()) {
        cfg.show_col_album = b;
    }
    if let Some(b) = patch.get("show_col_duration").and_then(|v| v.as_bool()) {
        cfg.show_col_duration = b;
    }
    if let Some(b) = patch.get("show_telemetry_strip").and_then(|v| v.as_bool()) {
        cfg.show_telemetry_strip = b;
    }
    if let Some(b) = patch.get("show_player_viz").and_then(|v| v.as_bool()) {
        cfg.show_player_viz = b;
    }
    if let Some(m) = patch.get("player_viz_mode").and_then(|v| v.as_str()) {
        cfg.player_viz_mode = m.to_string();
    }
    if let Some(z) = patch.get("toast_zone").and_then(|v| v.as_str()) {
        cfg.toast_zone = z.to_string();
    }
}

fn spawn_folder_card_scan(
    cards: &[FolderCard],
    tx: mpsc::Sender<(PathBuf, usize, Option<Vec<u8>>)>,
) {
    let paths: Vec<PathBuf> = cards.iter().map(|c| c.path.clone()).collect();
    if paths.is_empty() {
        return;
    }
    tokio::task::spawn_blocking(move || {
        for dir in paths {
            let (count, cover) = Scanner::scan_folder_summary_blocking(&dir);
            if tx.blocking_send((dir, count, cover)).is_err() {
                break;
            }
        }
    });
}

fn spawn_track_asset_load(path: PathBuf, tx: mpsc::Sender<(PathBuf, Option<Vec<u8>>)>) {
    tokio::task::spawn_blocking(move || {
        let cover = Scanner::extract_cover_art_blocking(&path);
        let _ = tx.blocking_send((path, cover));
    });
}

fn spawn_track_analysis(path: PathBuf, tx: mpsc::Sender<(PathBuf, Option<TrackAnalysis>)>) {
    tokio::task::spawn_blocking(move || {
        let analysis = analyze_track_blocking(&path);
        let _ = tx.blocking_send((path, analysis));
    });
}

/// Spawns the headless mpv supervisor, metadata & folder card scanners, IPC server, and core loop.
pub async fn run_core_runtime(
    music_root: PathBuf,
    mut cmd_rx: mpsc::Receiver<CoreCommand>,
    cmd_tx_for_ipc: mpsc::Sender<CoreCommand>,
    ui_tx: mpsc::Sender<CoreUpdate>,
) {
    let mut state = CoreState::with_persistence(music_root, default_state_path());
    state.hw_tx = Some(spawn_hardware_worker());
    let initial_scan = state.load_current_dir();

    let (scan_tx, mut scan_rx) = mpsc::channel::<Vec<MetadataPatch>>(32);
    Scanner::scan_paths_in_background(initial_scan, scan_tx.clone());

    let (card_tx, mut card_rx) = mpsc::channel::<(PathBuf, usize, Option<Vec<u8>>)>(16);
    spawn_folder_card_scan(&state.folder_cards, card_tx.clone());

    let (asset_tx, mut asset_rx) = mpsc::channel::<(PathBuf, Option<Vec<u8>>)>(8);
    let (analysis_tx, mut analysis_rx) = mpsc::channel::<(PathBuf, Option<TrackAnalysis>)>(8);
    let (ext_async_tx, mut ext_async_rx) = mpsc::channel::<ExtAsyncResult>(16);

    let (mpv_cmd_tx, mpv_cmd_rx) = mpsc::channel::<MpvCommand>(64);
    let (mpv_ev_tx, mut mpv_ev_rx) = mpsc::channel::<MpvActorEvent>(64);
    let _mpv_guard = match MpvSupervisor::spawn().await {
        Ok(sup) => {
            state.mpv_connected = true;
            let guard = sup.guard;
            tokio::spawn(async move {
                if let Err(e) = run_mpv_actor(sup.stream, mpv_cmd_rx, mpv_ev_tx).await {
                    error!("mpv actor error: {e}");
                }
            });
            let init_af = state.effective_audio_filter();
            if !init_af.is_empty() {
                let _ = mpv_cmd_tx.send(MpvCommand::SetAudioFilter(init_af)).await;
            }
            if (state.speed - 1.0).abs() > 0.005 {
                let _ = mpv_cmd_tx.send(MpvCommand::SetSpeed(state.speed)).await;
            }
            Some(guard)
        }
        Err(e) => {
            state.mpv_connected = false;
            state.error_banner = Some(format!(
                "Failed to launch mpv: {e}. Ensure `mpv` is installed on PATH."
            ));
            None
        }
    };

    let init_status = TntronStatus::from_state(&state);
    write_status_file(&init_status);
    let (status_watch_tx, status_watch_rx) = watch::channel(init_status);
    let (broadcast_tx, _) = broadcast::channel::<TntronStatus>(32);
    let mut last_published_lyric = state.current_lyric_line();

    let bcast_clone = broadcast_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = run_ipc_server(cmd_tx_for_ipc, status_watch_rx, bcast_clone).await {
            info!("IPC server stopped: {e}");
        }
    });

    let _ = ui_tx.send(CoreUpdate::Snapshot(Box::new(state.snapshot()))).await;

    let mut ui_tick = tokio::time::interval(Duration::from_millis(200));
    let mut sync_tick = tokio::time::interval(Duration::from_secs(5));
    ui_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    sync_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                let is_shutdown = matches!(cmd, CoreCommand::Shutdown);
                let mut fx = state.handle_command(cmd);
                let open_tab = fx.open_ext_tab.take();
                apply_effects(
                    &state,
                    fx,
                    &mpv_cmd_tx,
                    &scan_tx,
                    &card_tx,
                    &asset_tx,
                    &analysis_tx,
                    &ext_async_tx,
                ).await;
                last_published_lyric = state.current_lyric_line();
                publish_state(&state, &status_watch_tx, &broadcast_tx, &ui_tx).await;
                if let Some((ext_id, tab_id)) = open_tab {
                    let _ = ui_tx.send(CoreUpdate::OpenExtTab { ext_id, tab_id }).await;
                }
                if is_shutdown {
                    break;
                }
            }
            Some(ev) = mpv_ev_rx.recv() => {
                let fx = state.handle_mpv_event(ev);
                apply_effects(
                    &state,
                    fx,
                    &mpv_cmd_tx,
                    &scan_tx,
                    &card_tx,
                    &asset_tx,
                    &analysis_tx,
                    &ext_async_tx,
                ).await;
                last_published_lyric = state.current_lyric_line();
                publish_state(&state, &status_watch_tx, &broadcast_tx, &ui_tx).await;
            }
            Some(res) = ext_async_rx.recv() => {
                let fx = state.handle_ext_async_result(res);
                apply_effects(
                    &state,
                    fx,
                    &mpv_cmd_tx,
                    &scan_tx,
                    &card_tx,
                    &asset_tx,
                    &analysis_tx,
                    &ext_async_tx,
                ).await;
                last_published_lyric = state.current_lyric_line();
                publish_state(&state, &status_watch_tx, &broadcast_tx, &ui_tx).await;
            }
            Some(batch) = scan_rx.recv() => {
                if state.apply_metadata_batch(batch) {
                    publish_state(&state, &status_watch_tx, &broadcast_tx, &ui_tx).await;
                }
            }
            Some((dir, count, cover)) = card_rx.recv() => {
                if let Some(card) = state.folder_cards.iter_mut().find(|c| c.path == dir) {
                    card.track_count = count;
                    card.cover_art = cover.map(Arc::new);
                    let _ = ui_tx.send(CoreUpdate::Snapshot(Box::new(state.snapshot()))).await;
                }
            }
            Some((path, maybe_bytes)) = asset_rx.recv() => {
                if state.current_track.as_ref().is_some_and(|t| t.path == path) {
                    state.cover_art = maybe_bytes.map(Arc::new);
                    let _ = ui_tx.send(CoreUpdate::Snapshot(Box::new(state.snapshot()))).await;
                }
            }
            Some((path, maybe_analysis)) = analysis_rx.recv() => {
                if state.current_track.as_ref().is_some_and(|t| t.path == path) {
                    state.active_analysis = maybe_analysis.map(Arc::new);
                    publish_state(&state, &status_watch_tx, &broadcast_tx, &ui_tx).await;
                }
            }
            _ = ui_tick.tick() => {
                let was_playing = state.play_state == PlayState::Playing;
                let (timer_cmds, toast_changed) = state.check_timers();
                for mc in timer_cmds {
                    let _ = mpv_cmd_tx.send(mc).await;
                }
                let cur_lyric = state.current_lyric_line();
                if cur_lyric != last_published_lyric {
                    last_published_lyric = cur_lyric;
                    let st = TntronStatus::from_state(&state);
                    write_status_file(&st);
                    let _ = status_watch_tx.send(st.clone());
                    let _ = broadcast_tx.send(st);
                }
                if toast_changed || (was_playing && state.play_state != PlayState::Playing) {
                    publish_state(&state, &status_watch_tx, &broadcast_tx, &ui_tx).await;
                } else if state.play_state == PlayState::Playing || state.sleep_deadline.is_some() {
                    let _ = ui_tx.try_send(CoreUpdate::Tick {
                        position_sec: state.clock.now(),
                        sleep_remaining_sec: state.sleep_remaining_sec(),
                    });
                }
            }
            _ = sync_tick.tick() => {
                if state.play_state == PlayState::Playing {
                    let _ = mpv_cmd_tx.send(MpvCommand::GetTimePos).await;
                    let st = TntronStatus::from_state(&state);
                    write_status_file(&st);
                    let _ = status_watch_tx.send(st.clone());
                    let _ = broadcast_tx.send(st);
                }
            }
            else => break,
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn apply_effects(
    state: &CoreState,
    fx: CommandEffects,
    mpv_cmd_tx: &mpsc::Sender<MpvCommand>,
    scan_tx: &mpsc::Sender<Vec<MetadataPatch>>,
    card_tx: &mpsc::Sender<(PathBuf, usize, Option<Vec<u8>>)>,
    asset_tx: &mpsc::Sender<(PathBuf, Option<Vec<u8>>)>,
    analysis_tx: &mpsc::Sender<(PathBuf, Option<TrackAnalysis>)>,
    ext_async_tx: &mpsc::Sender<ExtAsyncResult>,
) {
    for mc in fx.mpv_cmds {
        let _ = mpv_cmd_tx.send(mc).await;
    }
    if !fx.scan_paths.is_empty() {
        Scanner::scan_paths_in_background(fx.scan_paths, scan_tx.clone());
    }
    if fx.reload_cards {
        spawn_folder_card_scan(&state.folder_cards, card_tx.clone());
    }
    if let Some(p) = fx.track_asset_target {
        spawn_track_asset_load(p.clone(), asset_tx.clone());
        if p.exists() {
            spawn_track_analysis(p, analysis_tx.clone());
        }
    }
    for (ext_id, job) in fx.ext_jobs {
        spawn_ext_async_job(ext_id, job, ext_async_tx.clone());
    }
}

fn spawn_ext_async_job(ext_id: String, job: AsyncExtJob, tx: mpsc::Sender<ExtAsyncResult>) {
    tokio::spawn(async move {
        let res = match job {
            AsyncExtJob::LyricsLoad {
                path,
                title,
                artist,
                duration_sec,
            } => {
                let local = load_companion_lrc(&path);
                if !local.is_empty() {
                    let lines: Vec<serde_json::Value> = local
                        .into_iter()
                        .map(|l| serde_json::json!({ "time_sec": l.time_sec, "text": l.text }))
                        .collect();
                    ExtAsyncResult {
                        ext_id,
                        event_name: "lyricsLoaded".into(),
                        payload: serde_json::json!({
                            "path": path.display().to_string(),
                            "source": "Local .lrc",
                            "lines": lines,
                        }),
                        stream_play: None,
                        rescan_after: false,
                    }
                } else {
                    match network::fetch_lrclib_lyrics(&title, &artist, duration_sec).await {
                        Ok((lines, _raw)) => {
                            let arr: Vec<serde_json::Value> = lines
                                .into_iter()
                                .map(|l| serde_json::json!({ "time_sec": l.time_sec, "text": l.text }))
                                .collect();
                            ExtAsyncResult {
                                ext_id,
                                event_name: "lyricsLoaded".into(),
                                payload: serde_json::json!({
                                    "path": path.display().to_string(),
                                    "source": "LRCLIB",
                                    "lines": arr,
                                }),
                                stream_play: None,
                                rescan_after: false,
                            }
                        }
                        Err(err) => ExtAsyncResult {
                            ext_id,
                            event_name: "lyricsLoaded".into(),
                            payload: serde_json::json!({
                                "path": path.display().to_string(),
                                "source": "None",
                                "lines": [],
                                "error": err,
                            }),
                            stream_play: None,
                            rescan_after: false,
                        },
                    }
                }
            }
            AsyncExtJob::FetchRss { url } => match network::fetch_rss_feed(&url).await {
                Ok(feed) => ExtAsyncResult {
                    ext_id,
                    event_name: "rssFetched".into(),
                    payload: serde_json::json!({
                        "ok": true,
                        "url": feed.url,
                        "title": feed.title,
                        "episodes": feed.episodes,
                    }),
                    stream_play: None,
                    rescan_after: false,
                },
                Err(e) => ExtAsyncResult {
                    ext_id,
                    event_name: "rssFetched".into(),
                    payload: serde_json::json!({ "ok": false, "url": url, "error": e }),
                    stream_play: None,
                    rescan_after: false,
                },
            },
            AsyncExtJob::YtSearch { query } => match network::search_ytdlp(&query).await {
                Ok(results) => ExtAsyncResult {
                    ext_id,
                    event_name: "ytSearch".into(),
                    payload: serde_json::json!({ "ok": true, "query": query, "results": results }),
                    stream_play: None,
                    rescan_after: false,
                },
                Err(e) => ExtAsyncResult {
                    ext_id,
                    event_name: "ytSearch".into(),
                    payload: serde_json::json!({ "ok": false, "query": query, "error": e }),
                    stream_play: None,
                    rescan_after: false,
                },
            },
            AsyncExtJob::YtPlay { url, title } => match network::resolve_ytdlp_stream_url(&url).await
            {
                Ok(stream_url) => ExtAsyncResult {
                    ext_id,
                    event_name: "ytPlaying".into(),
                    payload: serde_json::json!({
                        "ok": true,
                        "title": title.clone().unwrap_or_else(|| "YouTube Stream".into()),
                    }),
                    stream_play: Some((
                        stream_url,
                        title.unwrap_or_else(|| "YouTube Stream".into()),
                    )),
                    rescan_after: false,
                },
                Err(e) => ExtAsyncResult {
                    ext_id,
                    event_name: "ytPlaying".into(),
                    payload: serde_json::json!({ "ok": false, "error": e }),
                    stream_play: None,
                    rescan_after: false,
                },
            },
            AsyncExtJob::YtDownload { url, target_dir } => {
                match network::download_ytdlp_to_dir(&url, &target_dir).await {
                    Ok(msg) => ExtAsyncResult {
                        ext_id,
                        event_name: "ytDownloaded".into(),
                        payload: serde_json::json!({ "ok": true, "message": msg }),
                        stream_play: None,
                        rescan_after: true,
                    },
                    Err(e) => ExtAsyncResult {
                        ext_id,
                        event_name: "ytDownloaded".into(),
                        payload: serde_json::json!({ "ok": false, "error": e }),
                        stream_play: None,
                        rescan_after: false,
                    },
                }
            }
        };
        let _ = tx.send(res).await;
    });
}

async fn publish_state(
    state: &CoreState,
    status_watch_tx: &watch::Sender<TntronStatus>,
    broadcast_tx: &broadcast::Sender<TntronStatus>,
    ui_tx: &mpsc::Sender<CoreUpdate>,
) {
    let st = TntronStatus::from_state(state);
    write_status_file(&st);
    let _ = status_watch_tx.send(st.clone());
    let _ = broadcast_tx.send(st);
    let _ = ui_tx.send(CoreUpdate::Snapshot(Box::new(state.snapshot()))).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_test_state() -> CoreState {
        let mut s = CoreState::new(std::env::temp_dir());
        for i in 0..4 {
            let mut t = Track::new(PathBuf::from(format!("/tmp/song_{i}.m4a")));
            t.title = format!("Song {i}");
            t.set_duration(120.0);
            s.active_playlist.push(Arc::new(t));
        }
        s
    }

    #[test]
    fn test_shuffle_deck_visits_all_without_repeats() {
        let mut s = mk_test_state();
        s.shuffle_mode = ShuffleMode::On;
        s.active_playlist_index = Some(0);
        s.current_track = Some(Arc::clone(&s.active_playlist[0]));
        s.play_state = PlayState::Playing;
        s.rebuild_shuffle_deck(Some(0));

        let mut seen = vec![0usize];
        for _ in 0..3 {
            let cmd = s.advance_next(true);
            assert!(cmd.is_some());
            seen.push(s.active_playlist_index.unwrap());
        }
        seen.sort();
        assert_eq!(seen, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_queue_takes_priority_over_playlist() {
        let mut s = mk_test_state();
        s.active_playlist_index = Some(0);
        let q = Track::new(PathBuf::from("/tmp/queued.m4a"));
        s.queue.push(Arc::new(q));

        let cmd = s.advance_next(true);
        assert!(matches!(cmd, Some(MpvCommand::LoadFile { ref path, .. }) if path == "/tmp/queued.m4a"));
        assert!(s.queue.is_empty());
    }

    #[test]
    fn test_loop_off_stops_at_end() {
        let mut s = mk_test_state();
        s.loop_mode = LoopMode::Off;
        s.active_playlist_index = Some(3);
        s.current_track = Some(Arc::clone(&s.active_playlist[3]));
        s.play_state = PlayState::Playing;

        assert!(s.advance_next(true).is_none());
        assert_eq!(s.play_state, PlayState::Stopped);
    }

    #[test]
    fn test_ab_loop_wraps_back_to_a() {
        let mut s = mk_test_state();
        s.current_track = Some(Arc::clone(&s.active_playlist[0]));
        s.play_state = PlayState::Playing;
        s.ab_loop = AbLoop {
            a_sec: Some(10.0),
            b_sec: Some(20.0),
        };
        s.clock.sync(21.0);
        let (cmds, _) = s.check_timers();
        assert!(matches!(
            cmds.as_slice(),
            [MpvCommand::Seek { seconds, relative: false }] if (*seconds - 10.0).abs() < 0.01
        ));
    }
}
