use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures_util::stream;
use iced::alignment::Vertical;
use iced::event::{self, Event, Status};
use iced::keyboard::{self, key::Named, Key};
use iced::mouse;
use iced::widget::operation;
use iced::widget::{
    column, container, image, mouse_area, row, scrollable, slider, space, stack, text, text_input,
    Column, Id, Row,
};
use iced::window;
use iced::{
    time as iced_time, Alignment, Background, Border, Color, Element, Length, Padding, Point, Size,
    Subscription, Task, Theme,
};
use tntron_core::{
    format_mmss, run_core_runtime, BrowserEntry, CoreCommand, CoreUpdate, ExtWidget, LoopMode,
    PlayState, ShuffleMode, TrackSort, UiSnapshot,
};
use tokio::sync::mpsc;

use crate::icons::{self, resolve_icon};
use crate::theme::{all_themes, resolve_theme, ThemeSpec};
use crate::ui_widgets::{
    card_box, compact_nav_button, context_menu_item, context_menu_panel, folder_sidebar_item,
    glyph_button, openify_panel, pill_button, pill_icon_button, play_circle_button,
    primary_pill_icon_button, render_ext_widget, sidebar_nav_item, slot_badge,
    sort_header_button, theme_swatch_circle, track_monogram_tile, track_row_button,
};
use crate::visualizer::{mini_visualizer_canvas, VizFrameData};

#[derive(Debug, Clone)]
enum ModalState {
    ConfirmDeleteFolder { path: PathBuf, name: String },
    ConfirmDeleteTrack { path: PathBuf, title: String },
    AddToPlaylist { track: PathBuf, title: String },
    RenameEntry { path: PathBuf, original_name: String },
}

#[derive(Debug, Clone, PartialEq)]
enum ContextMenuTarget {
    Track {
        path: PathBuf,
        title: String,
        is_favorite: bool,
    },
    Folder {
        path: PathBuf,
        name: String,
    },
}

#[derive(Debug, Clone)]
struct ContextMenuState {
    position: Point,
    target: ContextMenuTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveView {
    Home,
    Settings,
    Extension { ext_id: String, tab_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewDensity {
    Compact,
    Normal,
    Big,
}

impl ViewDensity {
    pub fn from_str(s: &str) -> Self {
        match s {
            "Compact" => Self::Compact,
            "Big" | "Large" => Self::Big,
            _ => Self::Normal,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Normal => "Normal",
            Self::Big => "Big",
        }
    }

    fn row_padding(self) -> [u16; 2] {
        match self {
            Self::Compact => [5, 10],
            Self::Normal => [8, 12],
            Self::Big => [13, 14],
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Core(CoreUpdate),
    SwitchView(ActiveView),
    ToggleQueueDrawer,
    SearchChanged(String),
    ClearSearch,
    RowClicked(usize),
    PlayFolder(PathBuf, bool),
    PlayAllLibrary(bool),
    PlayPath(PathBuf),
    PlayFavorites(bool),
    LocatePlayingTrack,
    PlayQueueItem(usize),
    PlayUpcomingIndex(usize),
    OpenFolder(PathBuf),
    NavigateUp,
    Reload,
    TogglePause,
    NextTrack,
    PrevTrack,
    SeekPreview(f64),
    SeekCommit,
    VolumeChanged(f64),
    ToggleMute,
    CycleLoop,
    ToggleShuffle,
    ToggleSortColumn(TrackSort),
    SetDensity(ViewDensity),
    SetHomeLayout(String),
    SetTheme(String),
    ExportPreferences,
    ImportPreferences,
    ToggleFavorite(PathBuf),
    AddToQueue(PathBuf),
    PlayNextInQueue(PathBuf),
    AddFolderToQueue(PathBuf),
    RemoveFromQueue(usize),
    ClearQueue,
    CursorMoved(Point),
    WindowResized(Size),
    OpenTrackContext {
        path: PathBuf,
        title: String,
        is_favorite: bool,
    },
    OpenFolderContext {
        path: PathBuf,
        name: String,
    },
    CloseContextMenu,
    PromptDeleteFolder {
        path: PathBuf,
        name: String,
    },
    PromptDeleteTrack {
        path: PathBuf,
        title: String,
    },
    PromptAddToPlaylist {
        track: PathBuf,
        title: String,
    },
    PromptRename {
        path: PathBuf,
        current_name: String,
    },
    RenameDraftChanged(String),
    SubmitRename,
    ConfirmDeleteAction,
    CopyTrackToPlaylist {
        track: PathBuf,
        folder: PathBuf,
    },
    CloseModal,
    ToggleNewFolderPrompt,
    NewFolderInput(String),
    SubmitNewFolder,
    ToggleExtension(String),
    SetAllExtensions(bool),
    ExtSearchChanged(String),
    ToggleExtCompact(bool),
    ReloadExtensions,
    ExtAction {
        ext_id: String,
        action_id: String,
        value: serde_json::Value,
    },
    ExtInputChanged {
        ext_id: String,
        widget_id: String,
        value: String,
    },
    DismissToast(u64),
    VizTick,
    StepSpeed(f64),
    SetSpeedDirect(f64),
    StepPitch(f64),
    SetPitchDirect(f64),
    ToggleLinkPitch(bool),
    TogglePreserveFormant(bool),
    ToggleSpeedPopover,
    ToggleAlertsPopover,
    CloseAlertsPopover,
    PresetNameChanged(String),
    SaveSpeedPreset,
    ApplySpeedPreset(String),
    DeleteSpeedPreset(String),
    ToggleEditLayout,
    SetSidebarSide(String),
    SetSidebarWidth(f32),
    ToggleLayoutFlag(&'static str),
    SetPlayerVizMode(String),
    CyclePlayerVizMode,
    SetToastZone(String),
    ResetLayoutConfig,
    KeyPressed(Key),
    WindowCloseRequested,
}

pub struct TntronApp {
    cmd_tx: mpsc::Sender<CoreCommand>,
    snapshot: Option<UiSnapshot>,
    cover_handle: Option<image::Handle>,
    last_cover_ptr: Option<usize>,
    card_covers: HashMap<PathBuf, (usize, image::Handle)>,
    active_view: ActiveView,
    queue_open: bool,
    showing_favorites: bool,
    search_query: String,
    search_id: Id,
    density: ViewDensity,
    selected_idx: Option<usize>,
    last_click: Option<(usize, Instant)>,
    seek_dragging: Option<f64>,
    show_new_folder: bool,
    new_folder_name: String,
    ext_inputs: HashMap<(String, String), String>,
    cursor_pos: Point,
    window_size: Size,
    context_menu: Option<ContextMenuState>,
    modal: Option<ModalState>,
    rename_draft: String,
    viz_phase: f32,
    viz_bands: [f32; 16],
    viz_beat: f32,
    pos_anchor_sec: f64,
    pos_anchor_time: Instant,
    edit_layout_open: bool,
    speed_popover_open: bool,
    alerts_popover_open: bool,
    preset_name_draft: String,
    ext_search: String,
    ext_compact_cards: bool,
}

impl TntronApp {
    pub fn boot(music_root: PathBuf) -> (Self, Task<Message>) {
        let (cmd_tx, cmd_rx) = mpsc::channel::<CoreCommand>(128);
        let (ui_tx, ui_rx) = mpsc::channel::<CoreUpdate>(64);

        let cmd_tx_ipc = cmd_tx.clone();
        tokio::spawn(async move {
            run_core_runtime(music_root, cmd_rx, cmd_tx_ipc, ui_tx).await;
        });

        let core_stream = stream::unfold(ui_rx, |mut rx| async move {
            rx.recv().await.map(|ev| (Message::Core(ev), rx))
        });

        let app = Self {
            cmd_tx,
            snapshot: None,
            cover_handle: None,
            last_cover_ptr: None,
            card_covers: HashMap::new(),
            active_view: ActiveView::Home,
            queue_open: false,
            showing_favorites: false,
            search_query: String::new(),
            search_id: Id::new("openify_search_input"),
            density: ViewDensity::Normal,
            selected_idx: None,
            last_click: None,
            seek_dragging: None,
            show_new_folder: false,
            new_folder_name: String::new(),
            ext_inputs: HashMap::new(),
            cursor_pos: Point::new(240.0, 200.0),
            window_size: Size::new(1280.0, 820.0),
            context_menu: None,
            modal: None,
            rename_draft: String::new(),
            viz_phase: 0.0,
            viz_bands: [0.0; 16],
            viz_beat: 0.0,
            pos_anchor_sec: 0.0,
            pos_anchor_time: Instant::now(),
            edit_layout_open: false,
            speed_popover_open: false,
            alerts_popover_open: false,
            preset_name_draft: String::new(),
            ext_search: String::new(),
            ext_compact_cards: true,
        };

        (app, Task::stream(core_stream))
    }

    fn viz_frame(&self) -> VizFrameData {
        VizFrameData {
            phase: self.viz_phase,
            bands: self.viz_bands,
            beat: self.viz_beat,
        }
    }

    fn send(&self, cmd: CoreCommand) {
        let _ = self.cmd_tx.try_send(cmd);
    }

    fn current_theme(&self) -> ThemeSpec {
        let Some(snap) = &self.snapshot else {
            return resolve_theme("openify", &[]);
        };
        resolve_theme(&snap.active_theme, &snap.ext_themes)
    }

    pub fn title(&self) -> String {
        if let Some(snap) = &self.snapshot {
            if let Some(track) = &snap.current_track {
                return match snap.play_state {
                    PlayState::Playing => format!("{} — tntron", track.title),
                    PlayState::Paused => format!("{} (Paused) — tntron", track.title),
                    PlayState::Stopped => "tntron".to_string(),
                };
            }
        }
        "tntron".to_string()
    }

    pub fn theme(&self) -> Theme {
        self.current_theme().to_iced_theme()
    }

    fn has_active_visualizer(&self) -> bool {
        let Some(snap) = &self.snapshot else {
            return false;
        };
        let has_viz = |widgets: &[ExtWidget]| -> bool {
            widgets
                .iter()
                .any(|w| matches!(w, ExtWidget::Visualizer { .. }))
        };
        match &self.active_view {
            ActiveView::Extension { ext_id, tab_id } => snap
                .ext_sidebar_tabs
                .iter()
                .find(|t| &t.ext_id == ext_id && &t.id == tab_id)
                .is_some_and(|t| has_viz(&t.widgets)),
            ActiveView::Settings => snap.ext_settings_panels.iter().any(|p| has_viz(&p.widgets)),
            ActiveView::Home => false,
        }
    }

    fn needs_viz_animation(&self) -> bool {
        let Some(snap) = &self.snapshot else {
            return false;
        };
        let playing_mini = snap.play_state == PlayState::Playing && snap.layout.show_player_viz;
        playing_mini || self.has_active_visualizer()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            event::listen_with(|ev, status, _| match (ev, status) {
                (
                    Event::Keyboard(keyboard::Event::KeyPressed { key, .. }),
                    Status::Ignored,
                ) => Some(Message::KeyPressed(key)),
                (Event::Mouse(mouse::Event::CursorMoved { position }), _) => {
                    Some(Message::CursorMoved(position))
                }
                (Event::Window(window::Event::Resized(size)), _) => {
                    Some(Message::WindowResized(size))
                }
                _ => None,
            }),
            window::close_requests().map(|_| Message::WindowCloseRequested),
        ];
        if self.needs_viz_animation() {
            subs.push(iced_time::every(Duration::from_millis(33)).map(|_| Message::VizTick));
        }
        Subscription::batch(subs)
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Core(CoreUpdate::Snapshot(snap)) => {
                let new_ptr = snap
                    .cover_art
                    .as_ref()
                    .map(|arc| std::sync::Arc::as_ptr(arc) as usize);
                if new_ptr != self.last_cover_ptr {
                    self.last_cover_ptr = new_ptr;
                    self.cover_handle = snap
                        .cover_art
                        .as_ref()
                        .map(|bytes| image::Handle::from_bytes(bytes.as_slice().to_vec()));
                }

                for card in &snap.folder_cards {
                    if let Some(art) = &card.cover_art {
                        let ptr = std::sync::Arc::as_ptr(art) as usize;
                        let needs_refresh = self
                            .card_covers
                            .get(&card.path)
                            .is_none_or(|(old_ptr, _)| *old_ptr != ptr);
                        if needs_refresh {
                            let handle = image::Handle::from_bytes(art.as_slice().to_vec());
                            self.card_covers.insert(card.path.clone(), (ptr, handle));
                        }
                    }
                }

                self.density = ViewDensity::from_str(&snap.view_density);

                if let ActiveView::Extension { ext_id, tab_id } = &self.active_view {
                    let still_exists = snap
                        .ext_sidebar_tabs
                        .iter()
                        .any(|t| &t.ext_id == ext_id && &t.id == tab_id);
                    if !still_exists {
                        self.active_view = ActiveView::Home;
                    }
                }

                self.pos_anchor_sec = snap.position_sec;
                self.pos_anchor_time = Instant::now();
                self.snapshot = Some(*snap);
            }
            Message::Core(CoreUpdate::Tick {
                position_sec,
                sleep_remaining_sec,
            }) => {
                self.pos_anchor_sec = position_sec;
                self.pos_anchor_time = Instant::now();
                if let Some(snap) = &mut self.snapshot {
                    snap.position_sec = position_sec;
                    if let Some(s) = sleep_remaining_sec {
                        snap.sleep_remaining_sec = Some(s);
                    }
                }
            }
            Message::Core(CoreUpdate::OpenExtTab { ext_id, tab_id }) => {
                self.active_view = ActiveView::Extension { ext_id, tab_id };
            }
            Message::SwitchView(v) => {
                self.active_view = v;
            }
            Message::ToggleQueueDrawer => {
                self.queue_open = !self.queue_open;
            }
            Message::SearchChanged(q) => {
                self.search_query = q;
            }
            Message::ClearSearch => {
                self.search_query.clear();
            }
            Message::RowClicked(idx) => {
                let now = Instant::now();
                let is_double = self
                    .last_click
                    .is_some_and(|(prev, t)| prev == idx && now.duration_since(t) < Duration::from_millis(420));
                self.selected_idx = Some(idx);
                self.last_click = Some((idx, now));
                if is_double {
                    self.send(CoreCommand::PlayEntryIndex(idx));
                }
            }
            Message::PlayFolder(path, shuffle) => {
                self.context_menu = None;
                self.showing_favorites = false;
                self.active_view = ActiveView::Home;
                self.send(CoreCommand::PlayFolder { path, shuffle });
            }
            Message::PlayAllLibrary(shuffle) => {
                self.context_menu = None;
                self.send(CoreCommand::PlayAllLibrary { shuffle });
            }
            Message::PlayPath(p) => {
                self.context_menu = None;
                self.send(CoreCommand::PlayPath(p));
            }
            Message::PlayFavorites(shuffle) => {
                self.showing_favorites = true;
                self.active_view = ActiveView::Home;
                self.send(CoreCommand::PlayFavorites { shuffle });
            }
            Message::LocatePlayingTrack => {
                self.showing_favorites = false;
                self.active_view = ActiveView::Home;
                self.send(CoreCommand::LocatePlayingTrack);
            }
            Message::PlayQueueItem(i) => {
                self.send(CoreCommand::PlayQueueItem(i));
            }
            Message::PlayUpcomingIndex(i) => {
                self.send(CoreCommand::PlayUpcomingIndex(i));
            }
            Message::OpenFolder(path) => {
                self.context_menu = None;
                self.showing_favorites = false;
                self.active_view = ActiveView::Home;
                self.selected_idx = None;
                self.search_query.clear();
                self.send(CoreCommand::NavigateTo(path));
            }
            Message::NavigateUp => {
                self.showing_favorites = false;
                self.selected_idx = None;
                self.send(CoreCommand::NavigateUp);
            }
            Message::Reload => {
                self.send(CoreCommand::Reload);
            }
            Message::TogglePause => self.send(CoreCommand::TogglePause),
            Message::NextTrack => self.send(CoreCommand::NextTrack),
            Message::PrevTrack => self.send(CoreCommand::PrevTrack),
            Message::SeekPreview(v) => {
                self.seek_dragging = Some(v);
            }
            Message::SeekCommit => {
                if let Some(pos) = self.seek_dragging.take() {
                    self.send(CoreCommand::SeekAbsolute(pos));
                }
            }
            Message::VolumeChanged(v) => self.send(CoreCommand::SetVolume(v)),
            Message::ToggleMute => self.send(CoreCommand::ToggleMute),
            Message::CycleLoop => self.send(CoreCommand::CycleLoopMode),
            Message::ToggleShuffle => self.send(CoreCommand::ToggleShuffle),
            Message::ToggleSortColumn(col) => self.send(CoreCommand::ToggleSortColumn(col)),
            Message::SetDensity(d) => {
                self.density = d;
                self.send(CoreCommand::SetViewDensity(d.label().to_string()));
            }
            Message::SetHomeLayout(layout) => self.send(CoreCommand::SetHomeLayout(layout)),
            Message::SetTheme(slug) => self.send(CoreCommand::SetThemeSlug(slug)),
            Message::ExportPreferences => self.send(CoreCommand::ExportPreferences),
            Message::ImportPreferences => self.send(CoreCommand::ImportPreferences),
            Message::ToggleFavorite(p) => {
                self.context_menu = None;
                self.send(CoreCommand::ToggleFavorite(p));
            }
            Message::AddToQueue(p) => {
                self.context_menu = None;
                self.send(CoreCommand::AddToQueue(p));
            }
            Message::PlayNextInQueue(p) => {
                self.context_menu = None;
                self.send(CoreCommand::PlayNextInQueue(p));
            }
            Message::AddFolderToQueue(p) => {
                self.context_menu = None;
                self.send(CoreCommand::AddFolderToQueue(p));
            }
            Message::RemoveFromQueue(i) => self.send(CoreCommand::RemoveFromQueue(i)),
            Message::ClearQueue => {
                self.context_menu = None;
                self.send(CoreCommand::ClearQueue);
            }
            Message::CursorMoved(pos) => {
                self.cursor_pos = pos;
            }
            Message::WindowResized(size) => {
                self.window_size = size;
            }
            Message::OpenTrackContext {
                path,
                title,
                is_favorite,
            } => {
                self.context_menu = Some(ContextMenuState {
                    position: self.cursor_pos,
                    target: ContextMenuTarget::Track {
                        path,
                        title,
                        is_favorite,
                    },
                });
            }
            Message::OpenFolderContext { path, name } => {
                self.context_menu = Some(ContextMenuState {
                    position: self.cursor_pos,
                    target: ContextMenuTarget::Folder { path, name },
                });
            }
            Message::CloseContextMenu => {
                self.context_menu = None;
            }
            Message::ToggleNewFolderPrompt => {
                self.show_new_folder = !self.show_new_folder;
                if !self.show_new_folder {
                    self.new_folder_name.clear();
                }
            }
            Message::NewFolderInput(s) => {
                self.new_folder_name = s;
            }
            Message::SubmitNewFolder => {
                let clean = self.new_folder_name.trim().to_string();
                if !clean.is_empty() {
                    self.send(CoreCommand::CreateFolder(clean));
                    self.new_folder_name.clear();
                    self.show_new_folder = false;
                }
            }
            Message::ToggleExtension(id) => self.send(CoreCommand::ToggleExtension(id)),
            Message::SetAllExtensions(en) => self.send(CoreCommand::SetAllExtensionsEnabled(en)),
            Message::ExtSearchChanged(q) => {
                self.ext_search = q;
            }
            Message::ToggleExtCompact(c) => {
                self.ext_compact_cards = c;
            }
            Message::ReloadExtensions => self.send(CoreCommand::ReloadExtensions),
            Message::PromptDeleteFolder { path, name } => {
                self.context_menu = None;
                self.modal = Some(ModalState::ConfirmDeleteFolder { path, name });
            }
            Message::PromptDeleteTrack { path, title } => {
                self.context_menu = None;
                self.modal = Some(ModalState::ConfirmDeleteTrack { path, title });
            }
            Message::PromptAddToPlaylist { track, title } => {
                self.context_menu = None;
                self.modal = Some(ModalState::AddToPlaylist { track, title });
            }
            Message::PromptRename { path, current_name } => {
                self.context_menu = None;
                self.rename_draft = current_name.clone();
                self.modal = Some(ModalState::RenameEntry {
                    path,
                    original_name: current_name,
                });
            }
            Message::RenameDraftChanged(s) => {
                self.rename_draft = s;
            }
            Message::SubmitRename => {
                let clean = self.rename_draft.trim().to_string();
                if let Some(ModalState::RenameEntry { path, .. }) = self.modal.take() {
                    if !clean.is_empty() {
                        self.send(CoreCommand::RenameEntry {
                            path,
                            new_name: clean,
                        });
                    }
                    self.rename_draft.clear();
                }
            }
            Message::ConfirmDeleteAction => {
                match self.modal.take() {
                    Some(ModalState::ConfirmDeleteFolder { path, .. }) => {
                        self.send(CoreCommand::DeleteFolder(path));
                    }
                    Some(ModalState::ConfirmDeleteTrack { path, .. }) => {
                        self.send(CoreCommand::DeleteTrackFile(path));
                    }
                    _ => {}
                }
            }
            Message::CopyTrackToPlaylist { track, folder } => {
                self.modal = None;
                self.send(CoreCommand::CopyTrackToFolder { track, folder });
            }
            Message::CloseModal => {
                self.modal = None;
            }
            Message::ExtAction {
                ext_id,
                action_id,
                value,
            } => {
                self.send(CoreCommand::TriggerExtAction {
                    ext_id,
                    action_id,
                    value,
                });
            }
            Message::ExtInputChanged {
                ext_id,
                widget_id,
                value,
            } => {
                self.ext_inputs.insert((ext_id, widget_id), value);
            }
            Message::DismissToast(id) => self.send(CoreCommand::DismissToast(id)),
            Message::VizTick => {
                self.viz_phase = (self.viz_phase + 0.05) % 628.3185;
                if let Some(snap) = &self.snapshot {
                    if snap.play_state == PlayState::Playing {
                        if let Some(an) = &snap.analysis {
                            let elapsed = self.pos_anchor_time.elapsed().as_secs_f64() * snap.speed;
                            let live_pos = self.pos_anchor_sec + elapsed;
                            let (target, beat) = an.sample_at(live_pos);
                            for (dst, &src) in self.viz_bands.iter_mut().zip(target.iter()) {
                                *dst = *dst * 0.35 + src * 0.65;
                            }
                            self.viz_beat = self.viz_beat.max(beat) * 0.86;
                        } else {
                            for dst in &mut self.viz_bands {
                                *dst *= 0.85;
                            }
                            self.viz_beat *= 0.85;
                        }
                    } else {
                        for dst in &mut self.viz_bands {
                            *dst *= 0.78;
                        }
                        self.viz_beat *= 0.78;
                    }
                }
            }
            Message::StepSpeed(d) => self.send(CoreCommand::StepSpeedDelta(d)),
            Message::SetSpeedDirect(s) => self.send(CoreCommand::SetSpeed(s)),
            Message::StepPitch(d) => self.send(CoreCommand::StepPitchSemitones(d)),
            Message::SetPitchDirect(st) => self.send(CoreCommand::SetPitchSemitones(st)),
            Message::ToggleLinkPitch(v) => self.send(CoreCommand::SetLinkPitch(v)),
            Message::TogglePreserveFormant(v) => self.send(CoreCommand::SetPreserveFormant(v)),
            Message::ToggleSpeedPopover => {
                self.speed_popover_open = !self.speed_popover_open;
            }
            Message::ToggleAlertsPopover => {
                self.alerts_popover_open = !self.alerts_popover_open;
            }
            Message::CloseAlertsPopover => {
                self.alerts_popover_open = false;
            }
            Message::PresetNameChanged(s) => {
                self.preset_name_draft = s;
            }
            Message::SaveSpeedPreset => {
                let name = self.preset_name_draft.trim().to_string();
                if !name.is_empty() {
                    self.send(CoreCommand::SaveSpeedPreset(name));
                    self.preset_name_draft.clear();
                }
            }
            Message::ApplySpeedPreset(name) => self.send(CoreCommand::ApplySpeedPreset(name)),
            Message::DeleteSpeedPreset(name) => self.send(CoreCommand::DeleteSpeedPreset(name)),
            Message::ToggleEditLayout => {
                self.edit_layout_open = !self.edit_layout_open;
            }
            Message::SetSidebarSide(side) => {
                if let Some(snap) = &self.snapshot {
                    let mut cfg = snap.layout.clone();
                    cfg.sidebar_side = side;
                    self.send(CoreCommand::UpdateLayoutConfig(cfg));
                }
            }
            Message::SetSidebarWidth(w) => {
                if let Some(snap) = &self.snapshot {
                    let mut cfg = snap.layout.clone();
                    cfg.sidebar_width = w.clamp(190.0, 320.0);
                    self.send(CoreCommand::UpdateLayoutConfig(cfg));
                }
            }
            Message::ToggleLayoutFlag(flag) => {
                if let Some(snap) = &self.snapshot {
                    let mut cfg = snap.layout.clone();
                    match flag {
                        "banner" => cfg.show_banner = !cfg.show_banner,
                        "recent" => cfg.show_recent_strip = !cfg.show_recent_strip,
                        "col_album" => cfg.show_col_album = !cfg.show_col_album,
                        "col_duration" => cfg.show_col_duration = !cfg.show_col_duration,
                        "telemetry" => cfg.show_telemetry_strip = !cfg.show_telemetry_strip,
                        "player_viz" => cfg.show_player_viz = !cfg.show_player_viz,
                        _ => {}
                    }
                    self.send(CoreCommand::UpdateLayoutConfig(cfg));
                }
            }
            Message::SetPlayerVizMode(mode) => {
                if let Some(snap) = &self.snapshot {
                    let mut cfg = snap.layout.clone();
                    cfg.player_viz_mode = mode;
                    self.send(CoreCommand::UpdateLayoutConfig(cfg));
                }
            }
            Message::CyclePlayerVizMode => {
                if let Some(snap) = &self.snapshot {
                    let mut cfg = snap.layout.clone();
                    cfg.player_viz_mode = match cfg.player_viz_mode.as_str() {
                        "bars" => "wave".to_string(),
                        "wave" => "circle".to_string(),
                        "circle" => "flame".to_string(),
                        _ => "bars".to_string(),
                    };
                    self.send(CoreCommand::UpdateLayoutConfig(cfg));
                }
            }
            Message::SetToastZone(zone) => {
                if let Some(snap) = &self.snapshot {
                    let mut cfg = snap.layout.clone();
                    cfg.toast_zone = zone;
                    self.send(CoreCommand::UpdateLayoutConfig(cfg));
                }
            }
            Message::ResetLayoutConfig => self.send(CoreCommand::ResetLayoutConfig),
            Message::KeyPressed(key) => match key {
                Key::Character(ref c) if c.as_str() == "/" => {
                    return operation::focus(self.search_id.clone());
                }
                Key::Named(Named::Space) => self.send(CoreCommand::TogglePause),
                Key::Named(Named::Escape) => {
                    self.context_menu = None;
                    self.modal = None;
                    self.alerts_popover_open = false;
                    self.show_new_folder = false;
                    self.search_query.clear();
                }
                Key::Named(Named::Backspace) => self.send(CoreCommand::NavigateUp),
                Key::Named(Named::Enter) => {
                    if let Some(idx) = self.selected_idx {
                        self.send(CoreCommand::PlayEntryIndex(idx));
                    }
                }
                Key::Named(Named::ArrowRight) => self.send(CoreCommand::SeekDelta(5.0)),
                Key::Named(Named::ArrowLeft) => self.send(CoreCommand::SeekDelta(-5.0)),
                Key::Character(ref c) => match c.as_str() {
                    "m" | "M" => self.send(CoreCommand::ToggleMute),
                    "n" | "N" | "b" | "B" => self.send(CoreCommand::NextTrack),
                    "p" | "P" | "z" => self.send(CoreCommand::PrevTrack),
                    "q" | "Q" => self.queue_open = !self.queue_open,
                    "s" | "S" => self.send(CoreCommand::ToggleShuffle),
                    "r" | "R" | "Z" => self.send(CoreCommand::CycleLoopMode),
                    "v" | "V" => self.send(CoreCommand::SeekDelta(10.0)),
                    "c" | "C" => self.send(CoreCommand::SeekDelta(-10.0)),
                    "." => {
                        self.showing_favorites = false;
                        self.active_view = ActiveView::Home;
                        self.send(CoreCommand::LocatePlayingTrack);
                    }
                    "l" | "L" => self.send(CoreCommand::MarkLoopA),
                    ";" => self.send(CoreCommand::MarkLoopB),
                    "j" => self.step_selection(1),
                    "k" => self.step_selection(-1),
                    "g" => self.selected_idx = Some(0),
                    "G" => {
                        if let Some(snap) = &self.snapshot {
                            if !snap.entries.is_empty() {
                                self.selected_idx = Some(snap.entries.len() - 1);
                            }
                        }
                    }
                    "+" | "=" => self.send(CoreCommand::VolumeDelta(5)),
                    "-" => self.send(CoreCommand::VolumeDelta(-5)),
                    _ => {}
                },
                _ => {}
            },
            Message::WindowCloseRequested => {
                self.send(CoreCommand::Shutdown);
                return window::latest().and_then(window::close);
            }
        }
        Task::none()
    }

    fn step_selection(&mut self, delta: isize) {
        let Some(snap) = &self.snapshot else { return };
        let len = snap.entries.len();
        if len == 0 {
            return;
        }
        let next = match self.selected_idx {
            Some(i) => ((i as isize + delta).clamp(0, (len - 1) as isize)) as usize,
            None => 0,
        };
        self.selected_idx = Some(next);
    }

    pub fn view(&self) -> Element<'_, Message> {
        let theme = self.current_theme();
        let Some(snap) = &self.snapshot else {
            return container(text("Starting tntron...").size(16).color(theme.muted))
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(theme.base)),
                    ..Default::default()
                })
                .into();
        };

        let sidebar = self.view_sidebar(snap, &theme);
        let main_panel = self.view_main_panel(snap, &theme);

        let mut grid_row = Row::new().spacing(8).height(Length::Fill).width(Length::Fill);
        if snap.layout.sidebar_side.eq_ignore_ascii_case("right") {
            grid_row = grid_row.push(main_panel).push(sidebar);
        } else {
            grid_row = grid_row.push(sidebar).push(main_panel);
        }
        if self.queue_open {
            grid_row = grid_row.push(self.view_queue_sidebar(snap, &theme));
        }
        if self.edit_layout_open {
            grid_row = grid_row.push(self.view_layout_editor_drawer(snap, &theme));
        }

        let player_bar = self.view_player_bar(snap, &theme);
        let mut root_col = column![container(grid_row).padding([8, 8]).height(Length::Fill)]
            .spacing(4)
            .width(Length::Fill)
            .height(Length::Fill);
        if self.speed_popover_open {
            root_col = root_col.push(
                container(self.view_speed_presets_strip(snap, &theme)).padding([0, 12]),
            );
        }
        root_col = root_col.push(player_bar);

        let root_shell = container(root_col)
        .width(Length::Fill)
        .height(Length::Fill)
        .style({
            let bg = theme.base;
            move |_| container::Style {
                background: Some(Background::Color(bg)),
                ..Default::default()
            }
        });

        let mut layers: Vec<Element<'_, Message>> = vec![root_shell.into()];
        if let Some(alerts) = self.view_alerts_popover(snap, &theme) {
            layers.push(alerts);
        }
        if let Some(menu) = self.view_context_menu(&theme) {
            layers.push(menu);
        }
        if let Some(modal) = self.view_modal_overlay(snap, &theme) {
            layers.push(modal);
        }
        if !snap.toasts.is_empty() || snap.error_banner.is_some() {
            layers.push(self.view_toast_overlay(snap, &theme));
        }
        stack(layers).into()
    }

    fn view_sidebar<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        // Card 1: Openify Primary Navigation (+ dynamic Extension Tabs)
        let mut nav_col = Column::new().spacing(4).padding(8);
        nav_col = nav_col.push(
            container(
                row![
                    icons::icon(icons::MUSIC, 16.0, theme.accent),
                    text("tntron").size(17).color(theme.text),
                    space::horizontal(),
                    text("OPEN").size(10).color(theme.accent),
                ]
                .spacing(9)
                .align_y(Alignment::Center),
            )
            .padding([6, 10]),
        );

        if snap.compact_menu {
            let mut btns: Vec<Element<'_, Message>> = vec![
                compact_nav_button(
                    icons::HOME,
                    "Home",
                    self.active_view == ActiveView::Home && !self.showing_favorites,
                    theme,
                    Message::OpenFolder(snap.music_root.clone()),
                ),
                compact_nav_button(
                    icons::SETTINGS,
                    "Settings",
                    self.active_view == ActiveView::Settings,
                    theme,
                    Message::SwitchView(ActiveView::Settings),
                ),
            ];
            for tab in &snap.ext_sidebar_tabs {
                let target = ActiveView::Extension {
                    ext_id: tab.ext_id.clone(),
                    tab_id: tab.id.clone(),
                };
                let is_active = self.active_view == target;
                btns.push(compact_nav_button(
                    resolve_icon(&tab.icon),
                    tab.label.as_str(),
                    is_active,
                    theme,
                    Message::SwitchView(target),
                ));
            }
            let mut grid = Column::new().spacing(6);
            while !btns.is_empty() {
                let take = btns.len().min(4);
                let mut chunk: Vec<_> = btns.drain(..take).collect();
                while chunk.len() < 4 {
                    chunk.push(space::horizontal().width(Length::Fill).into());
                }
                grid = grid.push(Row::with_children(chunk).spacing(6).align_y(Alignment::Center));
            }
            nav_col = nav_col.push(container(grid).padding([2, 2]));
        } else {
            nav_col = nav_col.push(sidebar_nav_item(
                icons::HOME,
                "Home",
                self.active_view == ActiveView::Home && !self.showing_favorites,
                theme,
                Message::OpenFolder(snap.music_root.clone()),
            ));
            nav_col = nav_col.push(sidebar_nav_item(
                icons::SETTINGS,
                "Settings",
                self.active_view == ActiveView::Settings,
                theme,
                Message::SwitchView(ActiveView::Settings),
            ));

            if !snap.ext_sidebar_tabs.is_empty() {
                let mut ext_tabs_col = Column::new().spacing(2);
                for tab in &snap.ext_sidebar_tabs {
                    let target = ActiveView::Extension {
                        ext_id: tab.ext_id.clone(),
                        tab_id: tab.id.clone(),
                    };
                    let is_active = self.active_view == target;
                    ext_tabs_col = ext_tabs_col.push(sidebar_nav_item(
                        resolve_icon(&tab.icon),
                        tab.label.as_str(),
                        is_active,
                        theme,
                        Message::SwitchView(target),
                    ));
                }
                let ext_scroll = container(scrollable(ext_tabs_col))
                    .width(Length::Fill)
                    .max_height(240.0);
                nav_col = nav_col.push(ext_scroll);
            }
        }

        let top_card = openify_panel(nav_col, theme).width(Length::Fill);

        // Card 2: Openify Playlists (Folders ARE the playlists)
        let mut lib_header = row![
            icons::icon(icons::LIBRARY, 14.0, theme.text),
            text("Playlists").size(14).color(theme.text),
            space::horizontal(),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        if snap.current_dir != snap.music_root {
            lib_header =
                lib_header.push(glyph_button(icons::ARROW_UP, 13.0, theme, false, Message::NavigateUp));
        }
        lib_header = lib_header
            .push(glyph_button(
                icons::PLUS,
                13.0,
                theme,
                self.show_new_folder,
                Message::ToggleNewFolderPrompt,
            ))
            .push(glyph_button(icons::REFRESH, 13.0, theme, false, Message::Reload));

        let mut folders_col = Column::new().spacing(4);

        if !snap.favorites.is_empty() {
            folders_col = folders_col.push(folder_sidebar_item(
                None,
                icons::STAR_FILLED,
                "Favorites",
                format!("{} starred", snap.favorites.len()),
                self.showing_favorites && self.active_view == ActiveView::Home,
                theme,
                Message::PlayFavorites(false),
            ));
        }

        let at_root = snap.current_dir == snap.music_root && !self.showing_favorites;
        let root_name = snap
            .music_root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("All Music");
        folders_col = folders_col.push(folder_sidebar_item(
            None,
            icons::DISC,
            root_name,
            "Root folder".to_string(),
            at_root && self.active_view == ActiveView::Home,
            theme,
            Message::OpenFolder(snap.music_root.clone()),
        ));

        for card in &snap.folder_cards {
            let active = snap.current_dir == card.path
                && !self.showing_favorites
                && self.active_view == ActiveView::Home;
            let art_handle = self.card_covers.get(&card.path).map(|(_, h)| h);
            let item = folder_sidebar_item(
                art_handle,
                icons::FOLDER,
                &card.name,
                format!("{} tracks", card.track_count),
                active,
                theme,
                Message::OpenFolder(card.path.clone()),
            );
            folders_col = folders_col.push(
                mouse_area(item).on_right_press(Message::OpenFolderContext {
                    path: card.path.clone(),
                    name: card.name.clone(),
                }),
            );
        }

        let mut lib_inner = column![lib_header].spacing(8).padding(10);
        if self.show_new_folder {
            lib_inner = lib_inner.push(
                row![
                    text_input("New folder...", &self.new_folder_name)
                        .on_input(Message::NewFolderInput)
                        .on_submit(Message::SubmitNewFolder)
                        .size(12)
                        .padding([6, 8])
                        .width(Length::Fill),
                    pill_button("Add", theme, true, Message::SubmitNewFolder),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            );
        }
        lib_inner = lib_inner.push(scrollable(folders_col).height(Length::Fill));

        let bottom_card = openify_panel(lib_inner, theme)
            .width(Length::Fill)
            .height(Length::Fill);

        column![top_card, bottom_card]
            .spacing(8)
            .width(Length::Fixed(snap.layout.sidebar_width.clamp(190.0, 320.0)))
            .height(Length::Fill)
            .into()
    }

    fn view_main_panel<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let search_box = container(
            row![
                icons::icon(icons::SEARCH, 13.0, theme.muted),
                text_input("Search songs, artists...", &self.search_query)
                    .id(self.search_id.clone())
                    .on_input(Message::SearchChanged)
                    .size(13)
                    .width(Length::Fill)
                    .padding([4, 6]),
                if self.search_query.is_empty() {
                    Element::from(space::horizontal().width(Length::Fixed(0.0)))
                } else {
                    glyph_button(icons::CLOSE, 12.0, theme, false, Message::ClearSearch)
                },
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Length::Fixed(300.0))
        .padding([4, 12])
        .style({
            let bg = theme.card;
            let border = theme.border;
            move |_| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: 20.0.into(),
                },
                ..Default::default()
            }
        });

        let mut header_right = Row::new().spacing(8).align_y(Alignment::Center);
        for slot in &snap.ext_topbar_slots {
            let press = slot.action_id.as_ref().map(|aid| {
                if aid == "open_alerts" {
                    Message::ToggleAlertsPopover
                } else {
                    Message::ExtAction {
                        ext_id: slot.ext_id.clone(),
                        action_id: aid.clone(),
                        value: serde_json::Value::Bool(true),
                    }
                }
            });
            header_right = header_right.push(slot_badge(
                &slot.text,
                slot.icon.as_deref(),
                slot.accent || (slot.action_id.as_deref() == Some("open_alerts") && self.alerts_popover_open),
                theme,
                press,
            ));
        }

        let q_count = snap.queue.len();
        let q_label = if q_count > 0 {
            format!("Queue ({q_count})")
        } else {
            "Queue".to_string()
        };
        header_right = header_right.push(pill_icon_button(
            q_label,
            Some(icons::QUEUE),
            theme,
            self.queue_open,
            Message::ToggleQueueDrawer,
        ));

        let top_header = container(
            row![search_box, space::horizontal(), header_right]
                .spacing(12)
                .align_y(Alignment::Center),
        )
        .padding([12, 20])
        .width(Length::Fill);

        let body: Element<'a, Message> = match &self.active_view {
            ActiveView::Home => self.view_home(snap, theme),
            ActiveView::Settings => self.view_settings(snap, theme),
            ActiveView::Extension { ext_id, tab_id } => {
                self.view_extension_tab(snap, ext_id, tab_id, theme)
            }
        };

        openify_panel(column![top_header, body].height(Length::Fill), theme)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_home<'a>(&'a self, snap: &'a UiSnapshot, theme: &ThemeSpec) -> Element<'a, Message> {
        let folder_name = if self.showing_favorites {
            "Starred Favorites"
        } else {
            snap.current_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Music")
        };

        let track_total = if self.showing_favorites {
            snap.favorites.len()
        } else {
            snap.library_tracks.len()
        };

        let banner_art_handle = self
            .card_covers
            .get(&snap.current_dir)
            .map(|(_, h)| h)
            .or(self.cover_handle.as_ref());

        let banner_cover: Element<'a, Message> = if let Some(h) = banner_art_handle {
            image(h.clone())
                .width(Length::Fixed(140.0))
                .height(Length::Fixed(140.0))
                .into()
        } else {
            container(icons::icon(icons::MUSIC, 44.0, theme.accent))
                .width(Length::Fixed(140.0))
                .height(Length::Fixed(140.0))
                .center_x(Length::Fixed(140.0))
                .center_y(Length::Fixed(140.0))
                .style({
                    let bg = theme.card;
                    let border = theme.border;
                    move |_| container::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            color: border,
                            width: 1.0,
                            radius: 8.0.into(),
                        },
                        ..Default::default()
                    }
                })
                .into()
        };

        let kicker = if self.showing_favorites {
            "PLAYLIST"
        } else {
            "COLLECTION"
        };
        let play_action = if self.showing_favorites {
            Message::PlayFavorites(false)
        } else {
            Message::PlayFolder(snap.current_dir.clone(), false)
        };
        let shuffle_action = if self.showing_favorites {
            Message::PlayFavorites(true)
        } else {
            Message::PlayFolder(snap.current_dir.clone(), true)
        };

        let mut banner_btns = Row::new()
            .spacing(8)
            .align_y(Alignment::Center)
            .push(primary_pill_icon_button(
                "Play",
                Some(icons::PLAY),
                theme,
                play_action,
            ))
            .push(pill_icon_button(
                "Shuffle",
                Some(icons::SHUFFLE),
                theme,
                false,
                shuffle_action,
            ))
            .push(pill_icon_button(
                "Shuffle All",
                Some(icons::GLOBE),
                theme,
                false,
                Message::PlayAllLibrary(true),
            ));
        if snap.current_track.is_some() {
            banner_btns = banner_btns.push(pill_icon_button(
                "Locate",
                Some(icons::DISC),
                theme,
                false,
                Message::LocatePlayingTrack,
            ));
        }

        let banner = row![
            banner_cover,
            column![
                text(kicker).size(11).color(theme.muted),
                text(folder_name).size(34).color(theme.text),
                text(format!(
                    "{track_total} songs  ({} total in library)",
                    snap.all_tracks.len()
                ))
                .size(13)
                .color(theme.muted),
                banner_btns.wrap(),
            ]
            .spacing(8),
        ]
        .spacing(22)
        .padding(pad4(6.0, 20.0, 14.0, 20.0))
        .align_y(Alignment::Center);

        let cur_layout = snap.home_layout.as_str();
        let controls_row = row![
            pill_button(
                "Compact",
                theme,
                self.density == ViewDensity::Compact,
                Message::SetDensity(ViewDensity::Compact),
            ),
            pill_button(
                "Normal",
                theme,
                self.density == ViewDensity::Normal,
                Message::SetDensity(ViewDensity::Normal),
            ),
            pill_button(
                "Big",
                theme,
                self.density == ViewDensity::Big,
                Message::SetDensity(ViewDensity::Big),
            ),
            space::horizontal(),
            pill_button(
                "Table",
                theme,
                cur_layout == "table",
                Message::SetHomeLayout("table".into()),
            ),
            pill_button(
                "Cards",
                theme,
                cur_layout == "cards",
                Message::SetHomeLayout("cards".into()),
            ),
            pill_button(
                "Gallery",
                theme,
                cur_layout == "gallery",
                Message::SetHomeLayout("gallery".into()),
            ),
            pill_button(
                "Minimal",
                theme,
                cur_layout == "minimal",
                Message::SetHomeLayout("minimal".into()),
            ),
            space::horizontal(),
            text("Sort:").size(12).color(theme.muted),
            pill_button(
                "#",
                theme,
                snap.sort_mode == TrackSort::Id,
                Message::ToggleSortColumn(TrackSort::Id),
            ),
            pill_button(
                "Title",
                theme,
                snap.sort_mode == TrackSort::Title,
                Message::ToggleSortColumn(TrackSort::Title),
            ),
            pill_button(
                "Duration",
                theme,
                snap.sort_mode == TrackSort::Duration,
                Message::ToggleSortColumn(TrackSort::Duration),
            ),
        ]
        .spacing(6)
        .padding([0, 20])
        .align_y(Alignment::Center);

        let mut recent_bar = Row::new()
            .spacing(8)
            .padding([0, 20])
            .align_y(Alignment::Center);
        let has_recents = !snap.recent_tracks.is_empty();
        if has_recents {
            recent_bar = recent_bar.push(text("Recent:").size(11).color(theme.muted));
            for rt in snap.recent_tracks.iter().take(5) {
                recent_bar = recent_bar.push(pill_icon_button(
                    rt.title.as_str(),
                    Some(icons::MUSIC),
                    theme,
                    snap.current_track
                        .as_ref()
                        .is_some_and(|c| c.path == rt.path),
                    Message::PlayPath(rt.path.clone()),
                ));
            }
        }

        let mut header_row = Row::new()
            .spacing(12)
            .padding([6, 12])
            .align_y(Alignment::Center)
            .push(
                container(sort_header_button(
                    "#",
                    snap.sort_mode == TrackSort::Id,
                    snap.sort_desc,
                    theme,
                    Message::ToggleSortColumn(TrackSort::Id),
                ))
                .width(Length::Fixed(34.0)),
            )
            .push(
                container(sort_header_button(
                    "Title",
                    snap.sort_mode == TrackSort::Title,
                    snap.sort_desc,
                    theme,
                    Message::ToggleSortColumn(TrackSort::Title),
                ))
                .width(Length::Fill),
            );
        if snap.layout.show_col_album {
            header_row = header_row.push(
                text("Folder / Album")
                    .size(11)
                    .color(theme.muted)
                    .width(Length::Fixed(180.0)),
            );
        }
        if snap.layout.show_col_duration {
            header_row = header_row.push(
                container(sort_header_button(
                    "Time",
                    snap.sort_mode == TrackSort::Duration,
                    snap.sort_desc,
                    theme,
                    Message::ToggleSortColumn(TrackSort::Duration),
                ))
                .width(Length::Fixed(52.0)),
            );
        }
        header_row = header_row.push(text("").width(Length::Fixed(76.0)));
        let table_header = container(header_row).padding([0, 20]);

        let q = self.search_query.trim().to_ascii_lowercase();
        let mut list_col = Column::new().spacing(2).padding(pad4(4.0, 20.0, 16.0, 20.0));

        if self.showing_favorites {
            let mut count = 0usize;
            for fav_path in &snap.favorites {
                let name = fav_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Track");
                if !q.is_empty() && !name.to_ascii_lowercase().contains(&q) {
                    continue;
                }
                count += 1;
                let playing = snap
                    .current_track
                    .as_ref()
                    .is_some_and(|c| &c.path == fav_path);
                let row_inner = row![
                    text(format!("{count}"))
                        .size(12)
                        .color(if playing { theme.accent } else { theme.muted })
                        .width(Length::Fixed(34.0)),
                    text(name)
                        .size(14)
                        .color(if playing { theme.accent } else { theme.text })
                        .width(Length::Fill),
                    text("Favorites")
                        .size(12)
                        .color(theme.muted)
                        .width(Length::Fixed(180.0)),
                    glyph_button(
                        icons::STAR_FILLED,
                        13.0,
                        theme,
                        true,
                        Message::ToggleFavorite(fav_path.clone()),
                    ),
                    pill_button("+Q", theme, false, Message::AddToQueue(fav_path.clone())),
                ]
                .spacing(12)
                .align_y(Alignment::Center);

                let btn = track_row_button(
                    row_inner,
                    self.density.row_padding(),
                    false,
                    playing,
                    theme,
                    Message::PlayPath(fav_path.clone()),
                );
                list_col = list_col.push(
                    mouse_area(btn).on_right_press(Message::OpenTrackContext {
                        path: fav_path.clone(),
                        title: name.to_string(),
                        is_favorite: true,
                    }),
                );
            }
            if count == 0 {
                list_col = list_col.push(
                    container(text("No starred songs match.").size(13).color(theme.muted))
                        .padding(24),
                );
            }
        } else {
            let mut row_num = 0usize;
            for (idx, entry) in snap.entries.iter().enumerate() {
                match entry {
                    BrowserEntry::ParentDir(_) => {}
                    BrowserEntry::Directory { name, path } => {
                        if !q.is_empty() && !name.to_ascii_lowercase().contains(&q) {
                            continue;
                        }
                        let selected = self.selected_idx == Some(idx);
                        let row_inner = row![
                            container(icons::icon(icons::FOLDER, 14.0, theme.accent))
                                .width(Length::Fixed(34.0)),
                            text(name).size(14).color(theme.text).width(Length::Fill),
                            text("Subfolder")
                                .size(12)
                                .color(theme.muted)
                                .width(Length::Fixed(180.0)),
                            text("").width(Length::Fixed(52.0)),
                            glyph_button(
                                icons::PLAY,
                                12.0,
                                theme,
                                false,
                                Message::PlayFolder(path.clone(), false),
                            ),
                        ]
                        .spacing(12)
                        .align_y(Alignment::Center);

                        let btn = track_row_button(
                            row_inner,
                            self.density.row_padding(),
                            selected,
                            false,
                            theme,
                            Message::OpenFolder(path.clone()),
                        );
                        list_col = list_col.push(
                            mouse_area(btn).on_right_press(Message::OpenFolderContext {
                                path: path.clone(),
                                name: name.clone(),
                            }),
                        );
                    }
                    BrowserEntry::AudioTrack(track) => {
                        if !matches_search(track, &q) {
                            continue;
                        }
                        row_num += 1;
                        let selected = self.selected_idx == Some(idx);
                        let playing = snap
                            .current_track
                            .as_ref()
                            .is_some_and(|c| c.path == track.path);
                        let fav = snap.favorites.iter().any(|f| f == &track.path);

                        let title_col: Element<'a, Message> = match self.density {
                            ViewDensity::Compact => text(&track.title)
                                .size(13)
                                .color(if playing { theme.accent } else { theme.text })
                                .into(),
                            ViewDensity::Normal | ViewDensity::Big => {
                                let thumb_size = if self.density == ViewDensity::Big {
                                    48.0
                                } else {
                                    36.0
                                };
                                let art_box: Element<'a, Message> = if let Some((_, h)) =
                                    self.card_covers.get(&snap.current_dir)
                                {
                                    image(h.clone())
                                        .width(Length::Fixed(thumb_size))
                                        .height(Length::Fixed(thumb_size))
                                        .into()
                                } else {
                                    let card_bg = theme.card;
                                    let muted = theme.muted;
                                    let acc = theme.accent;
                                    container(icons::icon(
                                        if playing { icons::PLAY } else { icons::MUSIC },
                                        13.0,
                                        if playing { acc } else { muted },
                                    ))
                                    .width(Length::Fixed(thumb_size))
                                    .height(Length::Fixed(thumb_size))
                                    .center_x(Length::Fixed(thumb_size))
                                    .center_y(Length::Fixed(thumb_size))
                                    .style(move |_| container::Style {
                                        background: Some(Background::Color(card_bg)),
                                        border: Border {
                                            radius: 5.0.into(),
                                            ..Default::default()
                                        },
                                        ..Default::default()
                                    })
                                    .into()
                                };

                                row![
                                    art_box,
                                    column![
                                        text(&track.title)
                                            .size(if self.density == ViewDensity::Big {
                                                15
                                            } else {
                                                14
                                            })
                                            .color(if playing { theme.accent } else { theme.text }),
                                        text(&track.artist).size(12).color(theme.muted),
                                    ]
                                    .spacing(2),
                                ]
                                .spacing(10)
                                .align_y(Alignment::Center)
                                .into()
                            }
                        };

                        let idx_cell: Element<'a, Message> = if playing {
                            container(icons::icon(icons::PLAY, 12.0, theme.accent))
                                .width(Length::Fixed(34.0))
                                .into()
                        } else {
                            text(row_num.to_string())
                                .size(12)
                                .color(theme.muted)
                                .width(Length::Fixed(34.0))
                                .into()
                        };

                        let mut row_inner = Row::new()
                            .spacing(12)
                            .align_y(Alignment::Center)
                            .push(idx_cell)
                            .push(container(title_col).width(Length::Fill));
                        if snap.layout.show_col_album {
                            row_inner = row_inner.push(
                                text(&track.album)
                                    .size(12)
                                    .color(theme.muted)
                                    .width(Length::Fixed(180.0)),
                            );
                        }
                        if snap.layout.show_col_duration {
                            row_inner = row_inner.push(
                                text(&track.duration_label)
                                    .size(12)
                                    .color(theme.muted)
                                    .width(Length::Fixed(52.0)),
                            );
                        }
                        row_inner = row_inner.push(
                            row![
                                glyph_button(
                                    if fav {
                                        icons::STAR_FILLED
                                    } else {
                                        icons::STAR_OUTLINE
                                    },
                                    13.0,
                                    theme,
                                    fav,
                                    Message::ToggleFavorite(track.path.clone()),
                                ),
                                pill_button("+Q", theme, false, Message::AddToQueue(track.path.clone())),
                            ]
                            .spacing(4)
                            .width(Length::Fixed(76.0))
                            .align_y(Alignment::Center),
                        );

                        let btn = track_row_button(
                            row_inner,
                            self.density.row_padding(),
                            selected,
                            playing,
                            theme,
                            Message::RowClicked(idx),
                        );
                        list_col = list_col.push(
                            mouse_area(btn).on_right_press(Message::OpenTrackContext {
                                path: track.path.clone(),
                                title: track.title.clone(),
                                is_favorite: fav,
                            }),
                        );
                    }
                }
            }
            if row_num == 0 && !q.is_empty() && cur_layout != "cards" && cur_layout != "gallery" {
                list_col = list_col.push(
                    container(text("No tracks in this folder match your search.").size(13).color(theme.muted))
                        .padding(14),
                );
            }
            if !q.is_empty() {
                let mut global_hits = Vec::new();
                for tr in &snap.all_tracks {
                    if tr.path.parent() == Some(snap.current_dir.as_path()) {
                        continue;
                    }
                    if matches_search(tr, &q) {
                        global_hits.push(tr);
                    }
                }
                if !global_hits.is_empty() {
                    list_col = list_col.push(
                        container(
                            text(format!("OTHER FOLDERS ({})", global_hits.len()))
                                .size(11)
                                .color(theme.accent),
                        )
                        .padding(pad4(14.0, 12.0, 6.0, 12.0)),
                    );
                    for (gi, tr) in global_hits.into_iter().take(40).enumerate() {
                        let playing = snap
                            .current_track
                            .as_ref()
                            .is_some_and(|c| c.path == tr.path);
                        let fav = snap.favorites.iter().any(|f| f == &tr.path);
                        let folder_lbl = tr
                            .path
                            .parent()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("Library");
                        let row_inner = row![
                            text(format!("G{}", gi + 1))
                                .size(11)
                                .color(if playing { theme.accent } else { theme.muted })
                                .width(Length::Fixed(34.0)),
                            column![
                                text(&tr.title)
                                    .size(14)
                                    .color(if playing { theme.accent } else { theme.text }),
                                text(&tr.artist).size(12).color(theme.muted),
                            ]
                            .spacing(2)
                            .width(Length::Fill),
                            text(folder_lbl)
                                .size(12)
                                .color(theme.muted)
                                .width(Length::Fixed(180.0)),
                            text(&tr.duration_label)
                                .size(12)
                                .color(theme.muted)
                                .width(Length::Fixed(52.0)),
                            row![
                                glyph_button(
                                    if fav {
                                        icons::STAR_FILLED
                                    } else {
                                        icons::STAR_OUTLINE
                                    },
                                    13.0,
                                    theme,
                                    fav,
                                    Message::ToggleFavorite(tr.path.clone()),
                                ),
                                pill_button("+Q", theme, false, Message::AddToQueue(tr.path.clone())),
                            ]
                            .spacing(4)
                            .width(Length::Fixed(76.0))
                            .align_y(Alignment::Center),
                        ]
                        .spacing(12)
                        .align_y(Alignment::Center);

                        let btn = track_row_button(
                            row_inner,
                            self.density.row_padding(),
                            false,
                            playing,
                            theme,
                            Message::PlayPath(tr.path.clone()),
                        );
                        list_col = list_col.push(mouse_area(btn).on_right_press(
                            Message::OpenTrackContext {
                                path: tr.path.clone(),
                                title: tr.title.clone(),
                                is_favorite: fav,
                            },
                        ));
                    }
                }
            }
        }

        let mut out = Column::new().spacing(10).height(Length::Fill);
        if snap.layout.show_banner {
            out = out.push(banner);
        }
        out = out.push(controls_row);
        if has_recents && snap.layout.show_recent_strip {
            out = out.push(recent_bar);
        }

        if !self.showing_favorites && (cur_layout == "cards" || cur_layout == "gallery") {
            let tile_px = if cur_layout == "gallery" { 96.0 } else { 64.0 };
            let card_w = if cur_layout == "gallery" { 235.0 } else { 205.0 };
            let mut grid = Row::new().spacing(12);
            let mut shown = 0usize;
            for (idx, entry) in snap.entries.iter().enumerate() {
                match entry {
                    BrowserEntry::ParentDir(_) => {}
                    BrowserEntry::Directory { name, path } => {
                        if !q.is_empty() && !name.to_ascii_lowercase().contains(&q) {
                            continue;
                        }
                        shown += 1;
                        let card = card_box(
                            column![
                                row![
                                    icons::icon(icons::FOLDER, 28.0, theme.accent),
                                    space::horizontal(),
                                    glyph_button(
                                        icons::PLAY,
                                        13.0,
                                        theme,
                                        false,
                                        Message::PlayFolder(path.clone(), false),
                                    ),
                                ]
                                .align_y(Alignment::Center),
                                text(name).size(14).color(theme.text),
                                text("Playlist Folder").size(11).color(theme.muted),
                            ]
                            .spacing(8)
                            .padding(12),
                            theme,
                            false,
                        )
                        .width(Length::Fixed(card_w));
                        let area = mouse_area(
                            track_row_button(
                                card,
                                [0, 0],
                                self.selected_idx == Some(idx),
                                false,
                                theme,
                                Message::OpenFolder(path.clone()),
                            ),
                        )
                        .on_right_press(Message::OpenFolderContext {
                            path: path.clone(),
                            name: name.clone(),
                        });
                        grid = grid.push(container(area).width(Length::Fixed(card_w)));
                    }
                    BrowserEntry::AudioTrack(track) => {
                        if !matches_search(track, &q) {
                            continue;
                        }
                        shown += 1;
                        let playing = snap
                            .current_track
                            .as_ref()
                            .is_some_and(|c| c.path == track.path);
                        let fav = snap.favorites.iter().any(|f| f == &track.path);
                        let card = card_box(
                            column![
                                row![
                                    track_monogram_tile(&track.title, tile_px, playing, theme),
                                    space::horizontal(),
                                    column![
                                        glyph_button(
                                            if fav {
                                                icons::STAR_FILLED
                                            } else {
                                                icons::STAR_OUTLINE
                                            },
                                            13.0,
                                            theme,
                                            fav,
                                            Message::ToggleFavorite(track.path.clone()),
                                        ),
                                        pill_button(
                                            "+Q",
                                            theme,
                                            false,
                                            Message::AddToQueue(track.path.clone()),
                                        ),
                                    ]
                                    .spacing(6)
                                    .align_x(Alignment::End),
                                ]
                                .align_y(Alignment::Start),
                                text(&track.title)
                                    .size(if cur_layout == "gallery" { 15 } else { 13 })
                                    .color(if playing { theme.accent } else { theme.text }),
                                row![
                                    text(&track.artist).size(11).color(theme.muted).width(Length::Fill),
                                    text(&track.duration_label).size(11).color(theme.muted),
                                ]
                                .spacing(6),
                            ]
                            .spacing(8)
                            .padding(12),
                            theme,
                            playing,
                        )
                        .width(Length::Fixed(card_w));
                        let area = mouse_area(
                            track_row_button(
                                card,
                                [0, 0],
                                self.selected_idx == Some(idx),
                                playing,
                                theme,
                                Message::PlayPath(track.path.clone()),
                            ),
                        )
                        .on_right_press(Message::OpenTrackContext {
                            path: track.path.clone(),
                            title: track.title.clone(),
                            is_favorite: fav,
                        });
                        grid = grid.push(container(area).width(Length::Fixed(card_w)));
                    }
                }
            }
            if shown == 0 {
                out = out.push(
                    container(text("No items match your view.").size(13).color(theme.muted))
                        .padding(24),
                );
            } else {
                out = out.push(
                    scrollable(container(grid.wrap()).padding(pad4(6.0, 20.0, 20.0, 20.0)))
                        .height(Length::Fill),
                );
            }
            return out.into();
        }

        if cur_layout == "minimal" && !self.showing_favorites {
            let mut min_col = Column::new().spacing(1).padding(pad4(4.0, 20.0, 16.0, 20.0));
            let mut mnum = 0usize;
            for (idx, entry) in snap.entries.iter().enumerate() {
                if let BrowserEntry::AudioTrack(track) = entry {
                    if !matches_search(track, &q) {
                        continue;
                    }
                    mnum += 1;
                    let playing = snap
                        .current_track
                        .as_ref()
                        .is_some_and(|c| c.path == track.path);
                    let fav = snap.favorites.iter().any(|f| f == &track.path);
                    let r_inner = row![
                        text(format!("{mnum}"))
                            .size(11)
                            .color(if playing { theme.accent } else { theme.muted })
                            .width(Length::Fixed(28.0)),
                        text(&track.title)
                            .size(13)
                            .color(if playing { theme.accent } else { theme.text })
                            .width(Length::Fill),
                        text(&track.duration_label)
                            .size(11)
                            .color(theme.muted)
                            .width(Length::Fixed(46.0)),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center);
                    let btn = track_row_button(
                        r_inner,
                        [4, 10],
                        self.selected_idx == Some(idx),
                        playing,
                        theme,
                        Message::RowClicked(idx),
                    );
                    min_col = min_col.push(mouse_area(btn).on_right_press(
                        Message::OpenTrackContext {
                            path: track.path.clone(),
                            title: track.title.clone(),
                            is_favorite: fav,
                        },
                    ));
                }
            }
            out = out.push(scrollable(min_col).height(Length::Fill));
            return out.into();
        }

        out = out
            .push(table_header)
            .push(scrollable(list_col).height(Length::Fill));
        out.into()
    }

    fn view_settings<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let mut col = Column::new().spacing(18).padding(pad4(8.0, 22.0, 24.0, 22.0));

        col = col.push(text("Settings").size(28).color(theme.text));

        // 1. Openify Appearances (Circular Swatches + extension themes)
        let themes = all_themes(&snap.ext_themes);
        let mut swatch_row = Row::new().spacing(10).align_y(Alignment::Center);
        for spec in &themes {
            let selected = snap.active_theme == spec.slug;
            swatch_row = swatch_row.push(theme_swatch_circle(
                spec,
                selected,
                Message::SetTheme(spec.slug.clone()),
            ));
        }

        col = col.push(
            card_box(
                column![
                    text("Appearances").size(16).color(theme.text),
                    text("Built-in themes and themes registered via extensions")
                        .size(12)
                        .color(theme.muted),
                    swatch_row.wrap(),
                ]
                .spacing(10)
                .padding(16),
                theme,
                false,
            )
            .width(Length::Fill),
        );

        let cfg = &snap.layout;
        let cur_hl = snap.home_layout.as_str();
        let flag_btn = |label: &'static str, on: bool, flag: &'static str| {
            pill_button(
                format!("{label}: {}", if on { "ON" } else { "OFF" }),
                theme,
                on,
                Message::ToggleLayoutFlag(flag),
            )
        };
        col = col.push(
            card_box(
                column![
                    row![
                        column![
                            text("Layout & Interface").size(16).color(theme.text),
                            text("Customize sidebar side & width, tracklist view, visible panels, and live layout editor drawer")
                                .size(12)
                                .color(theme.muted),
                        ]
                        .spacing(3),
                        space::horizontal(),
                        pill_icon_button(
                            if self.edit_layout_open { "Close Layout Drawer" } else { "Edit Layout Drawer" },
                            Some(icons::SLIDERS),
                            theme,
                            self.edit_layout_open,
                            Message::ToggleEditLayout,
                        ),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                    row![
                        pill_button("Sidebar Left", theme, !cfg.sidebar_side.eq_ignore_ascii_case("right"), Message::SetSidebarSide("left".into())),
                        pill_button("Sidebar Right", theme, cfg.sidebar_side.eq_ignore_ascii_case("right"), Message::SetSidebarSide("right".into())),
                        pill_button("Table", theme, cur_hl == "table", Message::SetHomeLayout("table".into())),
                        pill_button("Cards", theme, cur_hl == "cards", Message::SetHomeLayout("cards".into())),
                        pill_button("Gallery", theme, cur_hl == "gallery", Message::SetHomeLayout("gallery".into())),
                        pill_button("Minimal", theme, cur_hl == "minimal", Message::SetHomeLayout("minimal".into())),
                        pill_button("Compact Rows", theme, self.density == ViewDensity::Compact, Message::SetDensity(ViewDensity::Compact)),
                        pill_button("Normal Rows", theme, self.density == ViewDensity::Normal, Message::SetDensity(ViewDensity::Normal)),
                        pill_button("Large Rows", theme, self.density == ViewDensity::Big, Message::SetDensity(ViewDensity::Big)),
                    ]
                    .spacing(6)
                    .wrap(),
                    row![
                        flag_btn("Banner", cfg.show_banner, "banner"),
                        flag_btn("Recents", cfg.show_recent_strip, "recent"),
                        flag_btn("Album Col", cfg.show_col_album, "col_album"),
                        flag_btn("Time Col", cfg.show_col_duration, "col_duration"),
                        flag_btn("Mini Viz", cfg.show_player_viz, "player_viz"),
                        pill_button("Toast Top-R", theme, cfg.toast_zone == "top-right", Message::SetToastZone("top-right".into())),
                        pill_button("Toast Top-L", theme, cfg.toast_zone == "top-left", Message::SetToastZone("top-left".into())),
                        pill_button("Toast Bot-R", theme, cfg.toast_zone == "bottom-right", Message::SetToastZone("bottom-right".into())),
                        pill_button("Toast Bot-L", theme, cfg.toast_zone == "bottom-left", Message::SetToastZone("bottom-left".into())),
                        pill_icon_button("Reset Layout", Some(icons::REFRESH), theme, false, Message::ResetLayoutConfig),
                    ]
                    .spacing(6)
                    .wrap(),
                ]
                .spacing(10)
                .padding(16),
                theme,
                false,
            )
            .width(Length::Fill),
        );

        let codec_info = snap.mpv_audio_telemetry.as_deref().unwrap_or("Idle (no active stream)");
        let ipc_label = if snap.mpv_connected { "Connected" } else { "Disconnected" };
        col = col.push(
            card_box(
                column![
                    text("Audio & Engine Info").size(16).color(theme.text),
                    text(format!("Audio Stream / Codec: {codec_info}"))
                        .size(13)
                        .color(theme.text),
                    text(format!("mpv IPC: {ipc_label}  ·  Metadata Cache: {}/10000 tracks  ·  Library: {} tracks in {} playlists", snap.cache_len, snap.all_tracks.len(), snap.folder_cards.len()))
                        .size(12)
                        .color(theme.muted),
                    text("Shortcuts: Space Play/Pause · / Search · q Queue · m Mute · j/k Select")
                        .size(12)
                        .color(theme.muted),
                ]
                .spacing(6)
                .padding(16),
                theme,
                false,
            )
            .width(Length::Fill),
        );

        col = col.push(
            card_box(
                column![
                    text("Data Management").size(16).color(theme.text),
                    text("Export or import all preferences, EQ/DSP presets, theme & extension settings (~/.config/tntron/tntron_preferences.json)")
                        .size(12)
                        .color(theme.muted),
                    row![
                        pill_icon_button(
                            "Export Preferences",
                            Some(icons::DOWNLOAD),
                            theme,
                            false,
                            Message::ExportPreferences,
                        ),
                        pill_icon_button(
                            "Import Preferences",
                            Some(icons::REFRESH),
                            theme,
                            false,
                            Message::ImportPreferences,
                        ),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                ]
                .spacing(10)
                .padding(16),
                theme,
                false,
            )
            .width(Length::Fill),
        );

        // 2. Extensions & Sandboxed Lua 5.4 SDK
        let enabled_cnt = snap.extensions.iter().filter(|e| e.enabled).count();
        let ext_header = column![
            row![
                column![
                    text(format!(
                        "Extensions ({enabled_cnt} / {} enabled · Lua 5.4 SDK v1.0)",
                        snap.extensions.len()
                    ))
                    .size(16)
                    .color(theme.text),
                    text(format!(
                        "Directory: {}  (Starter template & API.md in _template/)",
                        snap.extensions_dir.display()
                    ))
                    .size(12)
                    .color(theme.muted),
                ]
                .spacing(3)
                .width(Length::Fill),
                primary_pill_icon_button(
                    "Reload",
                    Some(icons::REFRESH),
                    theme,
                    Message::ReloadExtensions,
                ),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            row![
                text_input("Filter extensions...", &self.ext_search)
                    .on_input(Message::ExtSearchChanged)
                    .size(12)
                    .padding([6, 10])
                    .width(Length::Fill),
                pill_button(
                    "Compact",
                    theme,
                    self.ext_compact_cards,
                    Message::ToggleExtCompact(true),
                ),
                pill_button(
                    "Detailed",
                    theme,
                    !self.ext_compact_cards,
                    Message::ToggleExtCompact(false),
                ),
                pill_button("All On", theme, false, Message::SetAllExtensions(true)),
                pill_button("All Off", theme, false, Message::SetAllExtensions(false)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(10);

        let eq = self.ext_search.trim().to_ascii_lowercase();
        let mut ext_list = Column::new().spacing(if self.ext_compact_cards { 6 } else { 10 });
        if snap.extensions.is_empty() {
            ext_list = ext_list.push(
                text("No extensions enabled or installed yet. Copy _template/ to a folder inside extensions/, edit manifest.json + main.lua, and click Reload Extensions.")
                    .size(13)
                    .color(theme.muted),
            );
        } else {
            for ext in &snap.extensions {
                if !eq.is_empty()
                    && !ext.name.to_ascii_lowercase().contains(&eq)
                    && !ext.id.to_ascii_lowercase().contains(&eq)
                    && !ext.description.to_ascii_lowercase().contains(&eq)
                {
                    continue;
                }
                let mut card_col = column![
                    row![
                        column![
                            row![
                                text(&ext.name).size(if self.ext_compact_cards { 14 } else { 15 }).color(theme.text),
                                text(format!("v{} · {}", ext.version, ext.author)).size(11).color(theme.muted),
                            ]
                            .spacing(8)
                            .align_y(Alignment::Center),
                            text(&ext.description).size(12).color(theme.muted),
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                        pill_button(
                            if ext.enabled { "ON" } else { "OFF" },
                            theme,
                            ext.enabled,
                            Message::ToggleExtension(ext.id.clone()),
                        ),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                ]
                .spacing(6);

                if !self.ext_compact_cards {
                    let mut meta_tags = Row::new().spacing(6);
                    for perm in &ext.permissions {
                        meta_tags = meta_tags.push(slot_badge(perm, None, false, theme, None));
                    }
                    for req in &ext.requires {
                        meta_tags = meta_tags.push(slot_badge(
                            format!("Requires: {req}"),
                            None,
                            false,
                            theme,
                            None,
                        ));
                    }
                    for ub in &ext.used_by {
                        meta_tags = meta_tags.push(slot_badge(
                            format!("Used by: {ub}"),
                            None,
                            false,
                            theme,
                            None,
                        ));
                    }
                    card_col = card_col.push(meta_tags.wrap());
                }

                if let Some(err) = &ext.error {
                    card_col = card_col.push(
                        row![
                            icons::icon(icons::ALERT, 13.0, theme.danger),
                            text(err).size(12).color(theme.danger),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                    );
                }

                ext_list = ext_list.push(
                    card_box(
                        card_col.padding(if self.ext_compact_cards { 10 } else { 14 }),
                        theme,
                        ext.enabled,
                    )
                    .width(Length::Fill),
                );
            }
        }

        col = col.push(
            card_box(
                column![ext_header, ext_list].spacing(12).padding(16),
                theme,
                false,
            )
            .width(Length::Fill),
        );

        let viz = self.viz_frame();
        // 3. Extension Settings Panels registered dynamically via tntron.ui.register_settings
        for panel in &snap.ext_settings_panels {
            let mut pcol = Column::new().spacing(10).padding(16);
            pcol = pcol.push(text(&panel.title).size(15).color(theme.accent));
            for w in &panel.widgets {
                pcol = pcol.push(render_ext_widget(
                    w,
                    &panel.ext_id,
                    theme,
                    &viz,
                    &self.ext_inputs,
                    |ext_id, action_id, value| Message::ExtAction {
                        ext_id,
                        action_id,
                        value,
                    },
                    |ext_id, widget_id, value| Message::ExtInputChanged {
                        ext_id,
                        widget_id,
                        value,
                    },
                ));
            }
            col = col.push(card_box(pcol, theme, false).width(Length::Fill));
        }

        scrollable(col).height(Length::Fill).into()
    }

    fn view_extension_tab<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        ext_id: &'a str,
        tab_id: &str,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let Some(tab) = snap
            .ext_sidebar_tabs
            .iter()
            .find(|t| t.ext_id == ext_id && t.id == tab_id)
        else {
            return container(text("Extension tab is inactive.").size(14).color(theme.muted))
                .padding(24)
                .into();
        };

        let mut col = Column::new().spacing(14).padding(pad4(12.0, 22.0, 24.0, 22.0));
        col = col.push(
            row![
                icons::icon(resolve_icon(&tab.icon), 22.0, theme.accent),
                text(&tab.label).size(26).color(theme.text),
                slot_badge(ext_id, None, false, theme, None),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );

        let viz = self.viz_frame();
        let mut card_col = Column::new().spacing(12).padding(16);
        if tab.widgets.is_empty() {
            card_col = card_col.push(
                text("This extension tab has no rendered widgets yet.")
                    .size(13)
                    .color(theme.muted),
            );
        } else {
            for w in &tab.widgets {
                card_col = card_col.push(render_ext_widget(
                    w,
                    ext_id,
                    theme,
                    &viz,
                    &self.ext_inputs,
                    |eid, aid, value| Message::ExtAction {
                        ext_id: eid,
                        action_id: aid,
                        value,
                    },
                    |eid, wid, value| Message::ExtInputChanged {
                        ext_id: eid,
                        widget_id: wid,
                        value,
                    },
                ));
            }
        }
        col = col.push(card_box(card_col, theme, true).width(Length::Fill));

        scrollable(col).height(Length::Fill).into()
    }

    fn view_queue_sidebar<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let header = row![
            text("Queue").size(16).color(theme.text),
            space::horizontal(),
            if snap.queue.is_empty() {
                Element::from(space::horizontal().width(Length::Fixed(0.0)))
            } else {
                pill_button("Clear", theme, false, Message::ClearQueue)
            },
            glyph_button(icons::CLOSE, 13.0, theme, false, Message::ToggleQueueDrawer),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let mut qcol = Column::new().spacing(8);

        qcol = qcol.push(text("MANUAL QUEUE").size(11).color(theme.accent));
        if snap.queue.is_empty() {
            qcol = qcol.push(
                text("Empty — click +Q on any song")
                    .size(12)
                    .color(theme.muted),
            );
        } else {
            for (i, t) in snap.queue.iter().enumerate() {
                let left_bar = container(text(""))
                    .width(Length::Fixed(2.0))
                    .height(Length::Fixed(28.0))
                    .style({
                        let accent = theme.accent;
                        move |_| container::Style {
                            background: Some(Background::Color(accent)),
                            ..Default::default()
                        }
                    });
                let item = row![
                    left_bar,
                    column![
                        text(&t.title).size(13).color(theme.text),
                        text(&t.artist).size(11).color(theme.muted),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                    glyph_button(icons::CLOSE, 12.0, theme, false, Message::RemoveFromQueue(i)),
                ]
                .spacing(8)
                .align_y(Alignment::Center);

                qcol = qcol.push(track_row_button(
                    item,
                    [6, 8],
                    false,
                    false,
                    theme,
                    Message::PlayQueueItem(i),
                ));
            }
        }

        qcol = qcol.push(
            container(text("COMING UP NEXT").size(11).color(theme.muted))
                .padding(pad4(10.0, 0.0, 0.0, 0.0)),
        );

        let start = snap
            .active_playlist_index
            .map(|i| i + 1)
            .unwrap_or(0);
        let upcoming: Vec<_> = snap
            .active_playlist
            .iter()
            .enumerate()
            .skip(start)
            .take(30)
            .collect();
        if upcoming.is_empty() {
            qcol = qcol.push(text("End of list").size(12).color(theme.muted));
        } else {
            for (pl_idx, t) in upcoming {
                let item = column![
                    text(&t.title).size(13).color(theme.text),
                    text(format!("{} · {}", t.artist, t.duration_label))
                        .size(11)
                        .color(theme.muted),
                ]
                .spacing(2);
                qcol = qcol.push(track_row_button(
                    item,
                    [6, 8],
                    false,
                    false,
                    theme,
                    Message::PlayUpcomingIndex(pl_idx),
                ));
            }
        }

        openify_panel(
            column![header, scrollable(qcol).height(Length::Fill)]
                .spacing(12)
                .padding(14)
                .height(Length::Fill),
            theme,
        )
        .width(Length::Fixed(280.0))
        .height(Length::Fill)
        .into()
    }

    fn view_player_bar<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let art: Element<'a, Message> = if let Some(handle) = &self.cover_handle {
            image(handle.clone())
                .width(Length::Fixed(56.0))
                .height(Length::Fixed(56.0))
                .into()
        } else {
            let card_bg = theme.card;
            let accent = theme.accent;
            container(icons::icon(icons::MUSIC, 20.0, accent))
                .width(Length::Fixed(56.0))
                .height(Length::Fixed(56.0))
                .center_x(Length::Fixed(56.0))
                .center_y(Length::Fixed(56.0))
                .style(move |_| container::Style {
                    background: Some(Background::Color(card_bg)),
                    border: Border {
                        radius: 6.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        };

        let (title, artist, cur_path) = if let Some(t) = &snap.current_track {
            (t.title.as_str(), t.artist.as_str(), Some(t.path.clone()))
        } else {
            ("No track playing", "Select a track", None)
        };

        let artist_line = match (snap.current_track.as_ref(), snap.track_bpm) {
            (Some(_), Some(bpm)) => format!("{artist} · {:.0} BPM", bpm),
            _ => artist.to_string(),
        };
        let mut left_info = row![
            art,
            column![
                text(title).size(14).color(theme.text),
                text(artist_line).size(12).color(theme.muted),
            ]
            .spacing(3)
            .width(Length::Fixed(185.0)),
        ]
        .spacing(12)
        .align_y(Alignment::Center);

        if let Some(cp) = cur_path {
            let fav = snap.favorites.iter().any(|f| f == &cp);
            left_info = left_info.push(glyph_button(
                if fav {
                    icons::STAR_FILLED
                } else {
                    icons::STAR_OUTLINE
                },
                14.0,
                theme,
                fav,
                Message::ToggleFavorite(cp),
            ));
        }

        for pw in &snap.ext_player_widgets {
            let press = pw.action_id.as_ref().map(|aid| Message::ExtAction {
                ext_id: pw.ext_id.clone(),
                action_id: aid.clone(),
                value: serde_json::Value::Bool(true),
            });
            left_info = left_info.push(slot_badge(
                &pw.text,
                pw.icon.as_deref(),
                pw.accent,
                theme,
                press,
            ));
        }

        let play_glyph = match snap.play_state {
            PlayState::Playing => icons::PAUSE,
            PlayState::Paused | PlayState::Stopped => icons::PLAY,
        };
        let shuffle_on = snap.shuffle_mode == ShuffleMode::On;
        let loop_glyph = match snap.loop_mode {
            LoopMode::Track => icons::REPEAT_ONCE,
            LoopMode::Off | LoopMode::All => icons::REPEAT,
        };

        let controls = row![
            glyph_button(icons::SHUFFLE, 14.0, theme, shuffle_on, Message::ToggleShuffle),
            glyph_button(icons::PREV, 15.0, theme, false, Message::PrevTrack),
            play_circle_button(
                play_glyph,
                theme,
                snap.play_state == PlayState::Playing,
                Message::TogglePause,
            ),
            glyph_button(icons::NEXT, 15.0, theme, false, Message::NextTrack),
            glyph_button(
                loop_glyph,
                16.0,
                theme,
                snap.loop_mode != LoopMode::Off,
                Message::CycleLoop,
            ),
        ]
        .spacing(14)
        .align_y(Alignment::Center);

        let dur = snap.duration_sec.max(1.0);
        let pos = self.seek_dragging.unwrap_or(snap.position_sec).clamp(0.0, dur);
        let seek_bar = row![
            text(format_mmss(pos))
                .size(11)
                .color(theme.muted)
                .width(Length::Fixed(38.0)),
            slider(0.0..=dur, pos, Message::SeekPreview)
                .on_release(Message::SeekCommit)
                .step(0.5)
                .width(Length::Fill),
            text(format_mmss(snap.duration_sec))
                .size(11)
                .color(theme.muted)
                .width(Length::Fixed(38.0)),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let center_col = column![controls, seek_bar]
            .spacing(6)
            .align_x(Alignment::Center)
            .width(Length::FillPortion(5));

        let mute_glyph = if snap.is_muted {
            icons::VOLUME_MUTE
        } else {
            icons::VOLUME_HIGH
        };
        let mut right_row = Row::new().spacing(6).align_y(Alignment::Center);
        if snap.layout.show_player_viz {
            let viz_btn = iced::widget::button(mini_visualizer_canvas(
                &snap.layout.player_viz_mode,
                &self.viz_frame(),
                snap.play_state == PlayState::Playing,
                theme,
            ))
            .padding(0)
            .style(|_, _| iced::widget::button::Style {
                background: None,
                ..Default::default()
            })
            .on_press(Message::CyclePlayerVizMode);
            right_row = right_row.push(viz_btn);
        }
        right_row = right_row
            .push(glyph_button(
                mute_glyph,
                14.0,
                theme,
                snap.is_muted,
                Message::ToggleMute,
            ))
            .push(
                slider(0.0..=100.0, snap.volume, Message::VolumeChanged)
                    .step(1.0)
                    .width(Length::Fixed(72.0)),
            )
            .push(pill_icon_button(
                if snap.pitch_st.abs() >= 0.05 {
                    format!("{:.2}x · {:+.1}st", snap.speed, snap.pitch_st)
                } else {
                    format!("{:.2}x", snap.speed)
                },
                Some(icons::SLIDERS),
                theme,
                self.speed_popover_open
                    || (snap.speed - 1.0).abs() > 0.01
                    || snap.pitch_st.abs() >= 0.05,
                Message::ToggleSpeedPopover,
            ));

        let right_controls = container(right_row)
            .width(Length::FillPortion(5))
            .align_x(Alignment::End);

        container(
            row![
                container(left_info).width(Length::FillPortion(3)),
                container(center_col).width(Length::FillPortion(4)),
                right_controls,
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .padding([10, 14]),
        )
        .width(Length::Fill)
        .height(Length::Fixed(88.0))
        .into()
    }

    fn view_speed_presets_strip<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let mut preset_pills = Row::new()
            .spacing(6)
            .align_y(Alignment::Center)
            .push(text("Presets:").size(11).color(theme.muted));
        for preset in &snap.speed_presets {
            let active = (snap.speed - preset.speed).abs() < 0.015
                && (snap.pitch_st - preset.pitch_st).abs() < 0.1
                && snap.link_pitch == preset.link_pitch;
            preset_pills = preset_pills.push(
                row![
                    pill_button(
                        format!(
                            "{} ({:.2}x/{:+.1}st)",
                            preset.name, preset.speed, preset.pitch_st
                        ),
                        theme,
                        active,
                        Message::ApplySpeedPreset(preset.name.clone()),
                    ),
                    glyph_button(
                        icons::CLOSE,
                        10.0,
                        theme,
                        false,
                        Message::DeleteSpeedPreset(preset.name.clone()),
                    ),
                ]
                .spacing(2)
                .align_y(Alignment::Center),
            );
        }

        let sliders_row = row![
            pill_button("-", theme, false, Message::StepSpeed(-0.05)),
            pill_button(
                format!("{:.2}x", snap.speed),
                theme,
                (snap.speed - 1.0).abs() > 0.01,
                Message::SetSpeedDirect(1.0),
            ),
            pill_button("+", theme, false, Message::StepSpeed(0.05)),
            slider(0.25..=2.5, snap.speed, Message::SetSpeedDirect)
                .step(0.01)
                .width(Length::Fixed(120.0)),
            pill_button("-", theme, false, Message::StepPitch(-0.5)),
            pill_button(
                format!("{:+.1}st", snap.pitch_st),
                theme,
                snap.pitch_st.abs() >= 0.05,
                Message::SetPitchDirect(0.0),
            ),
            pill_button("+", theme, false, Message::StepPitch(0.5)),
            slider(-12.0..=12.0, snap.pitch_st, Message::SetPitchDirect)
                .step(0.5)
                .width(Length::Fixed(120.0)),
            pill_button(
                "Vinyl Link",
                theme,
                snap.link_pitch,
                Message::ToggleLinkPitch(!snap.link_pitch),
            ),
            pill_button(
                "Preserve Formant",
                theme,
                snap.preserve_formant,
                Message::TogglePreserveFormant(!snap.preserve_formant),
            ),
            space::horizontal(),
            text_input("Preset name...", &self.preset_name_draft)
                .on_input(Message::PresetNameChanged)
                .on_submit(Message::SaveSpeedPreset)
                .size(12)
                .padding([5, 8])
                .width(Length::Fixed(140.0)),
            pill_icon_button(
                "Save",
                Some(icons::PLUS),
                theme,
                !self.preset_name_draft.trim().is_empty(),
                Message::SaveSpeedPreset,
            ),
            glyph_button(
                icons::CLOSE,
                13.0,
                theme,
                false,
                Message::ToggleSpeedPopover,
            ),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        card_box(
            column![sliders_row, preset_pills.wrap()]
                .spacing(8)
                .padding([10, 14]),
            theme,
            true,
        )
        .width(Length::Fill)
        .into()
    }

    fn view_layout_editor_drawer<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let cfg = &snap.layout;
        let cur_hl = snap.home_layout.as_str();
        let flag_row = |label: &'a str, on: bool, flag: &'static str| -> Element<'a, Message> {
            pill_button(
                if on {
                    format!("{label}: ON")
                } else {
                    format!("{label}: OFF")
                },
                theme,
                on,
                Message::ToggleLayoutFlag(flag),
            )
        };

        let content = column![
            row![
                icons::icon(icons::SLIDERS, 15.0, theme.accent),
                text("Layout Editor").size(16).color(theme.text),
                space::horizontal(),
                glyph_button(icons::CLOSE, 13.0, theme, false, Message::ToggleEditLayout),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            text("Sidebar Position").size(11).color(theme.muted),
            row![
                pill_button(
                    "Left",
                    theme,
                    !cfg.sidebar_side.eq_ignore_ascii_case("right"),
                    Message::SetSidebarSide("left".into()),
                ),
                pill_button(
                    "Right",
                    theme,
                    cfg.sidebar_side.eq_ignore_ascii_case("right"),
                    Message::SetSidebarSide("right".into()),
                ),
            ]
            .spacing(6),
            text(format!("Sidebar Width: {:.0}px", cfg.sidebar_width))
                .size(11)
                .color(theme.muted),
            slider(190.0..=320.0, cfg.sidebar_width, Message::SetSidebarWidth).step(4.0),
            text("Tracklist View").size(11).color(theme.muted),
            row![
                pill_button("Table", theme, cur_hl == "table", Message::SetHomeLayout("table".into())),
                pill_button("Cards", theme, cur_hl == "cards", Message::SetHomeLayout("cards".into())),
                pill_button("Gallery", theme, cur_hl == "gallery", Message::SetHomeLayout("gallery".into())),
                pill_button("Minimal", theme, cur_hl == "minimal", Message::SetHomeLayout("minimal".into())),
            ]
            .spacing(6)
            .wrap(),
            text("Panels & Columns").size(11).color(theme.muted),
            row![
                flag_row("Banner", cfg.show_banner, "banner"),
                flag_row("Recents", cfg.show_recent_strip, "recent"),
                flag_row("Album Col", cfg.show_col_album, "col_album"),
                flag_row("Time Col", cfg.show_col_duration, "col_duration"),
                flag_row("Mini Viz", cfg.show_player_viz, "player_viz"),
            ]
            .spacing(6)
            .wrap(),
            text("Mini Visualizer Mode").size(11).color(theme.muted),
            row![
                pill_button("Bars", theme, cfg.player_viz_mode == "bars", Message::SetPlayerVizMode("bars".into())),
                pill_button("Wave", theme, cfg.player_viz_mode == "wave", Message::SetPlayerVizMode("wave".into())),
                pill_button("Circle", theme, cfg.player_viz_mode == "circle", Message::SetPlayerVizMode("circle".into())),
                pill_button("Flame", theme, cfg.player_viz_mode == "flame", Message::SetPlayerVizMode("flame".into())),
            ]
            .spacing(6)
            .wrap(),
            text("Toast Notification Corner").size(11).color(theme.muted),
            row![
                pill_button("Top-R", theme, cfg.toast_zone == "top-right", Message::SetToastZone("top-right".into())),
                pill_button("Top-L", theme, cfg.toast_zone == "top-left", Message::SetToastZone("top-left".into())),
                pill_button("Bot-R", theme, cfg.toast_zone == "bottom-right", Message::SetToastZone("bottom-right".into())),
                pill_button("Bot-L", theme, cfg.toast_zone == "bottom-left", Message::SetToastZone("bottom-left".into())),
            ]
            .spacing(6)
            .wrap(),
            space::vertical(),
            pill_icon_button(
                "Reset Layout",
                Some(icons::REFRESH),
                theme,
                false,
                Message::ResetLayoutConfig,
            ),
        ]
        .spacing(8)
        .padding(14);

        openify_panel(scrollable(content).height(Length::Fill), theme)
            .width(Length::Fixed(290.0))
            .height(Length::Fill)
            .into()
    }

    fn view_context_menu<'a>(&'a self, theme: &ThemeSpec) -> Option<Element<'a, Message>> {
        let ctx = self.context_menu.as_ref()?;
        let menu_w = 228.0_f32;
        let menu_h = match &ctx.target {
            ContextMenuTarget::Track { .. } => 296.0_f32,
            ContextMenuTarget::Folder { .. } => 228.0_f32,
        };
        let x = ctx
            .position
            .x
            .min((self.window_size.width - menu_w - 10.0).max(10.0))
            .max(10.0);
        let y = ctx
            .position
            .y
            .min((self.window_size.height - menu_h - 10.0).max(10.0))
            .max(10.0);

        let divider = container(text(""))
            .width(Length::Fill)
            .height(Length::Fixed(1.0))
            .style({
                let border = theme.border;
                move |_| container::Style {
                    background: Some(Background::Color(Color { a: 0.65, ..border })),
                    ..Default::default()
                }
            });

        let mut items = Column::new().spacing(2);
        match &ctx.target {
            ContextMenuTarget::Track {
                path,
                title,
                is_favorite,
            } => {
                let fav_label = if *is_favorite {
                    "Remove Favorite"
                } else {
                    "Add to Favorites"
                };
                let fav_icon = if *is_favorite {
                    icons::STAR_FILLED
                } else {
                    icons::STAR_OUTLINE
                };
                items = items
                    .push(
                        container(text(title).size(11).color(theme.muted))
                            .padding([4, 10]),
                    )
                    .push(context_menu_item(
                        icons::PLAY,
                        "Play Now",
                        false,
                        theme,
                        Message::PlayPath(path.clone()),
                    ))
                    .push(context_menu_item(
                        icons::NEXT,
                        "Play After This",
                        false,
                        theme,
                        Message::PlayNextInQueue(path.clone()),
                    ))
                    .push(context_menu_item(
                        icons::PLUS,
                        "Add to Queue",
                        false,
                        theme,
                        Message::AddToQueue(path.clone()),
                    ))
                    .push(context_menu_item(
                        icons::FOLDER,
                        "Add to Playlist...",
                        false,
                        theme,
                        Message::PromptAddToPlaylist {
                            track: path.clone(),
                            title: title.clone(),
                        },
                    ))
                    .push(context_menu_item(
                        fav_icon,
                        fav_label,
                        false,
                        theme,
                        Message::ToggleFavorite(path.clone()),
                    ))
                    .push(context_menu_item(
                        icons::SLIDERS,
                        "Rename Track...",
                        false,
                        theme,
                        Message::PromptRename {
                            path: path.clone(),
                            current_name: path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(title)
                                .to_string(),
                        },
                    ))
                    .push(divider)
                    .push(context_menu_item(
                        icons::CLOSE,
                        "Clear Queue",
                        false,
                        theme,
                        Message::ClearQueue,
                    ))
                    .push(context_menu_item(
                        icons::CLOSE,
                        "Remove from Folder",
                        true,
                        theme,
                        Message::PromptDeleteTrack {
                            path: path.clone(),
                            title: title.clone(),
                        },
                    ));
            }
            ContextMenuTarget::Folder { path, name } => {
                items = items
                    .push(
                        container(text(name).size(11).color(theme.muted))
                            .padding([4, 10]),
                    )
                    .push(context_menu_item(
                        icons::FOLDER,
                        "Open Playlist",
                        false,
                        theme,
                        Message::OpenFolder(path.clone()),
                    ))
                    .push(context_menu_item(
                        icons::PLAY,
                        "Play Playlist",
                        false,
                        theme,
                        Message::PlayFolder(path.clone(), false),
                    ))
                    .push(context_menu_item(
                        icons::PLUS,
                        "Add Playlist to Queue",
                        false,
                        theme,
                        Message::AddFolderToQueue(path.clone()),
                    ))
                    .push(context_menu_item(
                        icons::SLIDERS,
                        "Rename Playlist...",
                        false,
                        theme,
                        Message::PromptRename {
                            path: path.clone(),
                            current_name: name.clone(),
                        },
                    ))
                    .push(divider)
                    .push(context_menu_item(
                        icons::CLOSE,
                        "Clear Queue",
                        false,
                        theme,
                        Message::ClearQueue,
                    ))
                    .push(context_menu_item(
                        icons::CLOSE,
                        "Remove Playlist",
                        true,
                        theme,
                        Message::PromptDeleteFolder {
                            path: path.clone(),
                            name: name.clone(),
                        },
                    ));
            }
        }

        let panel = context_menu_panel(items, theme);
        let backdrop = mouse_area(container(text("")).width(Length::Fill).height(Length::Fill))
            .on_press(Message::CloseContextMenu)
            .on_right_press(Message::CloseContextMenu);
        let positioned = container(panel)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(pad4(y, 0.0, 0.0, x));

        Some(stack([backdrop.into(), positioned.into()]).into())
    }

    fn view_modal_overlay<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Option<Element<'a, Message>> {
        let modal = self.modal.as_ref()?;
        let body: Element<'a, Message> = match modal {
            ModalState::ConfirmDeleteFolder { name, .. } => column![
                row![
                    icons::icon(icons::ALERT, 16.0, theme.danger),
                    text(format!("Remove playlist '{name}'?")).size(16).color(theme.text),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                text("Folders are playlists: this removes the folder and its files inside the music root.")
                    .size(12)
                    .color(theme.muted),
                row![
                    space::horizontal(),
                    pill_button("Cancel", theme, false, Message::CloseModal),
                    pill_button("Remove Playlist", theme, true, Message::ConfirmDeleteAction),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(14)
            .padding(20)
            .into(),
            ModalState::ConfirmDeleteTrack { title, .. } => column![
                row![
                    icons::icon(icons::ALERT, 16.0, theme.danger),
                    text(format!("Remove '{title}'?")).size(16).color(theme.text),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                text("This deletes the audio file from this playlist folder.")
                    .size(12)
                    .color(theme.muted),
                row![
                    space::horizontal(),
                    pill_button("Cancel", theme, false, Message::CloseModal),
                    pill_button("Remove File", theme, true, Message::ConfirmDeleteAction),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(14)
            .padding(20)
            .into(),
            ModalState::AddToPlaylist { track, title } => {
                let mut list = Column::new().spacing(4);
                if snap.folder_cards.is_empty() {
                    list = list.push(
                        text("No playlist folders yet. Use '+' in Playlists sidebar first.")
                            .size(12)
                            .color(theme.muted),
                    );
                } else {
                    for card in &snap.folder_cards {
                        list = list.push(context_menu_item(
                            icons::FOLDER,
                            &card.name,
                            false,
                            theme,
                            Message::CopyTrackToPlaylist {
                                track: track.clone(),
                                folder: card.path.clone(),
                            },
                        ));
                    }
                }
                column![
                    row![
                        icons::icon(icons::FOLDER, 16.0, theme.accent),
                        text(format!("Add '{title}' to Playlist")).size(15).color(theme.text),
                        space::horizontal(),
                        glyph_button(icons::CLOSE, 13.0, theme, false, Message::CloseModal),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                    container(scrollable(list)).max_height(260.0),
                ]
                .spacing(12)
                .padding(18)
                .into()
            }
            ModalState::RenameEntry { original_name, .. } => column![
                row![
                    icons::icon(icons::SLIDERS, 16.0, theme.accent),
                    text(format!("Rename '{original_name}'")).size(16).color(theme.text),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                text_input("New name...", &self.rename_draft)
                    .on_input(Message::RenameDraftChanged)
                    .on_submit(Message::SubmitRename)
                    .padding([8, 12])
                    .size(13),
                row![
                    space::horizontal(),
                    pill_button("Cancel", theme, false, Message::CloseModal),
                    primary_pill_icon_button("Rename", None, theme, Message::SubmitRename),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(14)
            .padding(20)
            .into(),
        };

        let backdrop = mouse_area(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.55))),
                    ..Default::default()
                }),
        )
        .on_press(Message::CloseModal);

        let card = card_box(body, theme, true).width(Length::Fixed(400.0));
        let centered = container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill);

        Some(stack([backdrop.into(), centered.into()]).into())
    }

    fn view_alerts_popover<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Option<Element<'a, Message>> {
        if !self.alerts_popover_open {
            return None;
        }
        let tab = snap
            .ext_sidebar_tabs
            .iter()
            .find(|t| t.id == "notif-tray");

        let mut list = Column::new().spacing(6).width(Length::Fill);
        let mut entry_count = 0usize;
        if let Some(t) = tab {
            for w in &t.widgets {
                if let tntron_core::ExtWidget::Text { text: txt, muted } = w {
                    if *muted && entry_count == 0 {
                        list = list.push(text(txt).size(12).color(theme.muted));
                    } else {
                        entry_count += 1;
                        list = list.push(card_box(
                            container(text(txt).size(12).color(theme.text))
                                .padding([8, 10])
                                .width(Length::Fill),
                            theme,
                            entry_count == 1,
                        ));
                    }
                }
            }
        }
        if entry_count == 0 {
            list = list.push(
                text("No logged events yet. Play or switch tracks to record notifications.")
                    .size(12)
                    .color(theme.muted),
            );
        }

        let ext_id = tab
            .map(|t| t.ext_id.clone())
            .unwrap_or_else(|| "tunotron.better-notifications".to_string());

        let header = row![
            icons::icon(icons::BELL, 15.0, theme.accent),
            text(format!("Notifications ({entry_count})"))
                .size(14)
                .color(theme.text),
            space::horizontal(),
            pill_button(
                "Mark Read",
                theme,
                false,
                Message::ExtAction {
                    ext_id: ext_id.clone(),
                    action_id: "mark_read".into(),
                    value: serde_json::Value::Bool(true),
                },
            ),
            pill_button(
                "Clear",
                theme,
                false,
                Message::ExtAction {
                    ext_id: ext_id.clone(),
                    action_id: "clear".into(),
                    value: serde_json::Value::Bool(true),
                },
            ),
            glyph_button(
                icons::CLOSE,
                13.0,
                theme,
                false,
                Message::CloseAlertsPopover,
            ),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let body = container(scrollable(list).height(Length::Shrink))
            .width(Length::Fill)
            .max_height(280.0);

        let surface = theme.surface;
        let accent = theme.accent;
        let card = container(column![header, body].spacing(10).padding(14))
            .width(Length::Fixed(370.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(surface)),
                border: Border {
                    color: accent,
                    width: 1.0,
                    radius: 14.0.into(),
                },
                shadow: iced::Shadow {
                    color: Color { a: 0.55, ..Color::BLACK },
                    offset: iced::Vector::new(0.0, 8.0),
                    blur_radius: 22.0,
                },
                ..Default::default()
            });

        Some(
            container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(pad4(58.0, 26.0, 0.0, 0.0))
                .align_x(Alignment::End)
                .align_y(Vertical::Top)
                .into(),
        )
    }

    fn view_toast_overlay<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let mut col = Column::new().spacing(6);
        if let Some(err) = &snap.error_banner {
            col = col.push(card_box(
                container(text(err).size(12).color(theme.danger)).padding([8, 12]),
                theme,
                false,
            ));
        }
        for toast in &snap.toasts {
            let fg = if toast.is_error { theme.danger } else { theme.text };
            col = col.push(
                card_box(
                    row![
                        text(&toast.message).size(12).color(fg),
                        glyph_button(
                            icons::CLOSE,
                            12.0,
                            theme,
                            false,
                            Message::DismissToast(toast.id),
                        ),
                    ]
                    .spacing(8)
                    .padding([6, 12])
                    .align_y(Alignment::Center),
                    theme,
                    !toast.is_error,
                ),
            );
        }

        let (ax, ay) = match snap.layout.toast_zone.as_str() {
            "top-left" => (Alignment::Start, Vertical::Top),
            "bottom-right" => (Alignment::End, Vertical::Bottom),
            "bottom-left" => (Alignment::Start, Vertical::Bottom),
            _ => (Alignment::End, Vertical::Top),
        };
        container(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([14, 18])
            .align_x(ax)
            .align_y(ay)
            .into()
    }
}

const fn pad4(top: f32, right: f32, bottom: f32, left: f32) -> Padding {
    Padding {
        top,
        right,
        bottom,
        left,
    }
}

fn matches_search(track: &tntron_core::Track, q: &str) -> bool {
    q.is_empty()
        || track.title.to_ascii_lowercase().contains(q)
        || track.artist.to_ascii_lowercase().contains(q)
        || track.filename.to_ascii_lowercase().contains(q)
}
