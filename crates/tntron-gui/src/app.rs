use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures_util::stream;
use iced::alignment::Vertical;
use iced::clipboard;
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
    time as iced_time, Alignment, Background, Border, Color, ContentFit, Element, Length, Padding,
    Point, Size, Subscription, Task, Theme,
};
use tntron_core::{
    default_tnt_dir, format_mmss, list_tnt_files, run_core_runtime, BrowserEntry, ButtonPackConfig,
    CoreCommand, CoreUpdate, DiscordRpcConfig, ExtSidebarTab, ExtWidget, LinuxDisplayEnv, LoopMode,
    PlayState, ShuffleMode, TrackSort, UiSnapshot,
};
use tokio::sync::mpsc;

use crate::button_style;
use crate::gif_viz::GifPlayer;
use crate::icons::{self, resolve_icon};
use crate::logo;
use crate::reactive_bg::{custom_bg_canvas, reactive_bg_canvas, ReactiveBgState};
use crate::theme::{all_themes, parse_hex_color, resolve_theme, ThemeSpec};
use crate::ui_widgets::{
    card_box, compact_nav_button, context_menu_item, context_menu_panel, folder_sidebar_item,
    glyph_button, openify_panel, pill_button, pill_icon_button, play_circle_button,
    primary_pill_icon_button, render_ext_widget, sidebar_nav_item, slot_badge,
    sort_header_button, theme_swatch_circle, track_monogram_tile, track_row_button, ExtVizCtx,
};
use crate::visualizer::{mini_visualizer_canvas, visualizer_canvas, VizFrameData};

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
    TntBundleDraftChanged(String),
    ExportFullTnt(String),
    ImportFullTnt(String),
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
    NextGifViz,
    ToggleHomePlus,
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
    ToggleHideUi,
    ToggleDiscordFlag(&'static str),
    SetDiscordOption(&'static str, String),
    DiscordFieldChanged(&'static str, String),
    ApplyDiscordFields,
    ReconnectDiscordRpc,
    ResetDiscordRpc,
    FontLoaded(Result<(), iced::font::Error>),
    Noop,
    ToggleProfilePopover,
    CloseProfilePopover,
    ProfileNameDraftChanged(String),
    SubmitProfileName,
    ProfileAvatarDraftChanged(String),
    SubmitProfileAvatar,
    ClearProfileAvatar,
    SetProfileAvatarSize(f32),
    SetProfileAvatarShape(String),
    SetProfileAvatarFit(String),
    CopyProfileStatsTnt,
    PasteProfileStatsClipboard,
    ProfileShareDraftChanged(String),
    ImportProfileStatsTnt(String),
    ResetProfileStats,
    ResolvePendingProfileImport(bool),
    ToggleFullscreen,
    SetFullscreen(bool),
    KeyPressed(Key),
    WindowCloseRequested,
}

pub struct TntronApp {
    cmd_tx: mpsc::Sender<CoreCommand>,
    snapshot: Option<UiSnapshot>,
    cover_handle: Option<image::Handle>,
    last_cover_ptr: Option<usize>,
    custom_bg_path: Option<String>,
    custom_bg_handle: Option<image::Handle>,
    custom_particle_path: Option<String>,
    custom_particle_handle: Option<image::Handle>,
    custom_logo_path: Option<String>,
    custom_logo_handle: Option<image::Handle>,
    custom_font_path: Option<String>,
    avatar_path: Option<String>,
    avatar_handle: Option<image::Handle>,
    profile_popover_open: bool,
    profile_name_draft: String,
    profile_avatar_draft: String,
    profile_share_draft: String,
    profile_drafts_synced: bool,
    discord_drafts_synced: bool,
    discord_client_id_draft: String,
    discord_idle_draft: String,
    discord_details_draft: String,
    discord_state_draft: String,
    discord_large_key_draft: String,
    discord_large_text_draft: String,
    discord_small_key_draft: String,
    discord_small_text_draft: String,
    discord_btn_label_draft: String,
    discord_btn_url_draft: String,
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
    gif_viz: GifPlayer,
    reactive_bg: ReactiveBgState,
    home_plus_open: bool,
    pos_anchor_sec: f64,
    pos_anchor_time: Instant,
    edit_layout_open: bool,
    speed_popover_open: bool,
    alerts_popover_open: bool,
    preset_name_draft: String,
    ext_search: String,
    ext_compact_cards: bool,
    tnt_bundle_draft: String,
    is_fullscreen: bool,
    display_env: LinuxDisplayEnv,
}

impl TntronApp {
    pub fn boot(music_root: PathBuf, start_fullscreen: bool) -> (Self, Task<Message>) {
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
            custom_bg_path: None,
            custom_bg_handle: None,
            custom_particle_path: None,
            custom_particle_handle: None,
            custom_logo_path: None,
            custom_logo_handle: None,
            custom_font_path: None,
            avatar_path: None,
            avatar_handle: None,
            profile_popover_open: false,
            profile_name_draft: String::new(),
            profile_avatar_draft: String::new(),
            profile_share_draft: String::new(),
            profile_drafts_synced: false,
            discord_drafts_synced: false,
            discord_client_id_draft: String::new(),
            discord_idle_draft: String::new(),
            discord_details_draft: String::new(),
            discord_state_draft: String::new(),
            discord_large_key_draft: String::new(),
            discord_large_text_draft: String::new(),
            discord_small_key_draft: String::new(),
            discord_small_text_draft: String::new(),
            discord_btn_label_draft: String::new(),
            discord_btn_url_draft: String::new(),
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
            gif_viz: GifPlayer::new(),
            reactive_bg: ReactiveBgState::new(),
            home_plus_open: true,
            pos_anchor_sec: 0.0,
            pos_anchor_time: Instant::now(),
            edit_layout_open: false,
            speed_popover_open: false,
            alerts_popover_open: false,
            preset_name_draft: String::new(),
            ext_search: String::new(),
            ext_compact_cards: true,
            tnt_bundle_draft: "tntron_full_backup".to_string(),
            is_fullscreen: start_fullscreen,
            display_env: LinuxDisplayEnv::detect(),
        };

        let mut tasks = vec![Task::stream(core_stream)];
        if start_fullscreen {
            tasks.push(
                window::latest()
                    .and_then(|id| window::set_mode(id, window::Mode::Fullscreen)),
            );
        }
        (app, Task::batch(tasks))
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

    fn sync_discord_drafts(&mut self, cfg: &DiscordRpcConfig) {
        self.discord_client_id_draft = cfg.client_id.clone();
        self.discord_idle_draft = cfg.idle_text.clone();
        self.discord_details_draft = cfg.custom_details.clone();
        self.discord_state_draft = cfg.custom_state.clone();
        self.discord_large_key_draft = cfg.large_image_key.clone();
        self.discord_large_text_draft = cfg.large_text.clone();
        self.discord_small_key_draft = cfg.small_image_key.clone();
        self.discord_small_text_draft = cfg.small_text.clone();
        self.discord_btn_label_draft = cfg.button_label.clone();
        self.discord_btn_url_draft = cfg.button_url.clone();
    }

    fn view_discord_rpc_card<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let dc = &snap.discord_rpc;
        let prev = &snap.discord_preview;

        let flag_chip = |label: &'static str, on: bool, key: &'static str| {
            pill_button(
                format!("{label}: {}", if on { "ON" } else { "OFF" }),
                theme,
                on,
                Message::ToggleDiscordFlag(key),
            )
        };

        let status_txt = if !dc.enabled {
            "Disabled".to_string()
        } else if prev.connected {
            format!(
                "Connected ({})",
                prev.socket_label.as_deref().unwrap_or("discord-ipc-0")
            )
        } else if let Some(err) = &prev.error_message {
            err.clone()
        } else {
            "Waiting for Discord / Vesktop IPC socket".to_string()
        };

        let preview_box = card_box(
            column![
                text("Live Discord Rich Presence Preview")
                    .size(11)
                    .color(theme.accent),
                text(if prev.details.is_empty() {
                    "(No Details)"
                } else {
                    prev.details.as_str()
                })
                .size(13)
                .color(theme.text),
                text(if prev.state.is_empty() {
                    "(No State)"
                } else {
                    prev.state.as_str()
                })
                .size(12)
                .color(theme.muted),
                text(&prev.footer).size(11).color(theme.muted),
            ]
            .spacing(3)
            .padding(10),
            theme,
            prev.connected,
        )
        .width(Length::Fill);

        let field_input = |label: &'static str, field: &'static str, value: &str, placeholder: &'static str| {
            row![
                container(text(label).size(11).color(theme.muted)).width(Length::Fixed(128.0)),
                text_input(placeholder, value)
                    .on_input(move |s| Message::DiscordFieldChanged(field, s))
                    .on_submit(Message::ApplyDiscordFields)
                    .size(12)
                    .padding([5, 9])
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        };

        card_box(
            column![
                row![
                    column![
                        text("Discord Rich Presence").size(16).color(theme.text),
                        text(format!("Status: {status_txt}"))
                            .size(12)
                            .color(if prev.connected { theme.accent } else { theme.muted }),
                    ]
                    .spacing(3),
                    space::horizontal(),
                    flag_chip("Rich Presence", dc.enabled, "enabled"),
                    pill_icon_button(
                        "Reconnect",
                        Some(icons::REFRESH),
                        theme,
                        false,
                        Message::ReconnectDiscordRpc,
                    ),
                    pill_icon_button(
                        "Reset",
                        Some(icons::CLOSE),
                        theme,
                        false,
                        Message::ResetDiscordRpc,
                    ),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                preview_box,
                text("Visible Details & Telemetry").size(12).color(theme.muted),
                row![
                    flag_chip("Title", dc.show_title, "title"),
                    flag_chip("Artist", dc.show_artist, "artist"),
                    flag_chip("Album", dc.show_album, "album"),
                    flag_chip("Timestamps", dc.show_timestamps, "timestamps"),
                    flag_chip("Paused Badge", dc.show_paused, "paused"),
                    flag_chip("Clear on Pause", dc.clear_on_pause, "clear_on_pause"),
                    flag_chip("Codec / Rate", dc.show_codec, "codec"),
                    flag_chip("BPM", dc.show_bpm, "bpm"),
                    flag_chip("Speed / Pitch", dc.show_speed_pitch, "speed_pitch"),
                    flag_chip("Loop / Shuffle", dc.show_loop_shuffle, "loop_shuffle"),
                    flag_chip("Track # / Queue", dc.show_queue_info, "queue"),
                    flag_chip("Idle Status", dc.show_idle, "idle"),
                    flag_chip("Action Button", dc.show_button, "button"),
                    flag_chip("Privacy Mode", dc.privacy_mode, "privacy"),
                ]
                .spacing(6)
                .wrap(),
                row![
                    text("Activity Type:").size(11).color(theme.muted),
                    pill_button("Listening", theme, dc.activity_type == 2, Message::SetDiscordOption("activity_type", "2".into())),
                    pill_button("Playing", theme, dc.activity_type == 0, Message::SetDiscordOption("activity_type", "0".into())),
                    pill_button("Watching", theme, dc.activity_type == 3, Message::SetDiscordOption("activity_type", "3".into())),
                    text("·  Timer:").size(11).color(theme.muted),
                    pill_button("Remaining", theme, dc.timestamp_mode == "remaining", Message::SetDiscordOption("timestamp_mode", "remaining".into())),
                    pill_button("Elapsed", theme, dc.timestamp_mode == "elapsed", Message::SetDiscordOption("timestamp_mode", "elapsed".into())),
                    pill_button("Off", theme, dc.timestamp_mode == "none", Message::SetDiscordOption("timestamp_mode", "none".into())),
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .wrap(),
                row![
                    text("Details Format:").size(11).color(theme.muted),
                    pill_button("Auto (Full)", theme, dc.details_mode == "auto", Message::SetDiscordOption("details_mode", "auto".into())),
                    pill_button("Title Only", theme, dc.details_mode == "title_only", Message::SetDiscordOption("details_mode", "title_only".into())),
                    pill_button("Title — Artist", theme, dc.details_mode == "title_artist", Message::SetDiscordOption("details_mode", "title_artist".into())),
                    pill_button("Custom", theme, dc.details_mode == "custom", Message::SetDiscordOption("details_mode", "custom".into())),
                    text("·  State Format:").size(11).color(theme.muted),
                    pill_button("Auto (Full)", theme, dc.state_mode == "auto", Message::SetDiscordOption("state_mode", "auto".into())),
                    pill_button("Artist · Album", theme, dc.state_mode == "artist_album", Message::SetDiscordOption("state_mode", "artist_album".into())),
                    pill_button("Telemetry", theme, dc.state_mode == "telemetry", Message::SetDiscordOption("state_mode", "telemetry".into())),
                    pill_button("Custom", theme, dc.state_mode == "custom", Message::SetDiscordOption("state_mode", "custom".into())),
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .wrap(),
                text("Placeholders for custom templates: {title} {artist} {album} {codec} {bpm} {speed} {pitch} {queue} {loop} {shuffle} {state}")
                    .size(11)
                    .color(theme.muted),
                text("Setup: Built-in Client App ID 1572028761224904806 works out of the box (custom ID optional via discord.com/developers/applications). Ensure Discord's 'Activity Privacy -> Display current activity' is ON.")
                    .size(11)
                    .color(theme.accent),
                column![
                    field_input("Client App ID", "client_id", &self.discord_client_id_draft, "1572028761224904806"),
                    field_input("Custom Details", "custom_details", &self.discord_details_draft, "{title} · {bpm}"),
                    field_input("Custom State", "custom_state", &self.discord_state_draft, "{artist} · {album} · {codec}"),
                    field_input("Idle Text", "idle_text", &self.discord_idle_draft, "Browsing library · Idle"),
                    field_input("Large Asset Key", "large_key", &self.discord_large_key_draft, "Asset key or https://...png URL"),
                    field_input("Large Tooltip", "large_text", &self.discord_large_text_draft, "{album} · {codec}"),
                    field_input("Small Asset Key", "small_key", &self.discord_small_key_draft, "Optional asset key or https://...png URL"),
                    field_input("Small Tooltip", "small_text", &self.discord_small_text_draft, "{state} · {speed}"),
                    field_input("Button Label", "button_label", &self.discord_btn_label_draft, "tntron on GitHub"),
                    field_input("Button URL", "button_url", &self.discord_btn_url_draft, "https://github.com/Arcioth/tntron"),
                ]
                .spacing(6),
                row![
                    primary_pill_icon_button(
                        "Apply Discord Fields",
                        Some(icons::GLOBE),
                        theme,
                        Message::ApplyDiscordFields,
                    ),
                ]
                .spacing(8),
            ]
            .spacing(10)
            .padding(16),
            theme,
            false,
        )
        .width(Length::Fill)
        .into()
    }

    fn resolve_logo_tint(&self, snap: &UiSnapshot, theme: &ThemeSpec) -> Color {
        match snap.logo.mode.as_str() {
            "monochrome" => theme.text,
            "custom_color" => snap
                .logo
                .custom_hex
                .as_deref()
                .and_then(parse_hex_color)
                .unwrap_or(theme.logo_color),
            _ => theme.logo_color,
        }
    }

    fn view_brand_logo<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
        override_size: Option<f32>,
    ) -> Element<'a, Message> {
        // Keep layout size strictly constant so UI boxes never shift or twitch on audio beats.
        let sz = override_size.unwrap_or(snap.logo.size_px).clamp(14.0, 96.0);
        let beat_alpha_boost = if snap.logo.beat_pulse && snap.play_state == PlayState::Playing {
            (self.viz_beat * 0.35).clamp(0.0, 0.35)
        } else {
            0.0
        };
        let tint = self.resolve_logo_tint(snap, theme);

        let is_custom_img =
            snap.logo.mode == "custom_image" && self.custom_logo_handle.is_some();
        let handle = if is_custom_img {
            self.custom_logo_handle.clone()
        } else {
            logo::cached_tinted_logo_handle(tint)
        };

        let mark: Element<'a, Message> = if let (true, Some(h)) = (snap.logo.enabled, handle) {
            let w = if is_custom_img { sz } else { sz * 1.125 };
            image(h)
                .width(Length::Fixed(w))
                .height(Length::Fixed(sz))
                .content_fit(ContentFit::Contain)
                .into()
        } else {
            icons::icon(icons::MUSIC, (sz * 0.8).max(14.0), tint).into()
        };

        let frame_style = snap.logo.frame_style.as_str();
        if override_size.is_none() && frame_style != "clean" {
            let bg = theme.card;
            let border_col = if frame_style == "glow" {
                tint
            } else {
                theme.border
            };
            let glow_shadow = if frame_style == "glow" {
                iced::Shadow {
                    color: Color {
                        a: (0.45 + beat_alpha_boost).min(0.92),
                        ..tint
                    },
                    offset: iced::Vector::ZERO,
                    blur_radius: 12.0,
                }
            } else {
                iced::Shadow::default()
            };
            container(mark)
                .padding([4, 6])
                .center_x(Length::Shrink)
                .center_y(Length::Shrink)
                .style(move |_| container::Style {
                    background: Some(Background::Color(bg)),
                    border: Border {
                        color: border_col,
                        width: 1.0,
                        radius: 8.0.into(),
                    },
                    shadow: glow_shadow,
                    ..Default::default()
                })
                .into()
        } else {
            mark
        }
    }

    fn current_theme(&self) -> ThemeSpec {
        let Some(snap) = &self.snapshot else {
            return resolve_theme("openify", &[]);
        };
        let mut spec = resolve_theme(&snap.active_theme, &snap.ext_themes);
        if snap.reactive_ui.enabled {
            let dim = snap.reactive_ui.bg_dim.clamp(0.25, 0.96);
            spec.surface.a = (0.48 + dim * 0.42).clamp(0.42, 0.92);
            spec.card.a = (0.56 + dim * 0.36).clamp(0.48, 0.95);
            spec.card_hover.a = (0.68 + dim * 0.28).clamp(0.60, 0.96);
        }
        if snap.transparency.enabled {
            let tr = &snap.transparency;
            if let Some(hex) = &tr.tint_hex {
                if let Some(c) = crate::theme::parse_hex_color(hex) {
                    spec.base = Color {
                        r: c.r,
                        g: c.g,
                        b: c.b,
                        a: spec.base.a,
                    };
                }
            }
            spec.base.a = tr.base_alpha.clamp(0.0, 1.0);
            spec.surface.a = (spec.surface.a * tr.surface_alpha).clamp(0.0, 1.0);
            spec.card.a = (spec.card.a * tr.card_alpha).clamp(0.0, 1.0);
            spec.card_hover.a = (spec.card_hover.a * (tr.card_alpha + 0.08).min(1.0)).clamp(0.0, 1.0);
            spec.sidebar.a = tr.player_bar_alpha.clamp(0.0, 1.0);
            spec.accent.a = tr.accent_alpha.clamp(0.1, 1.0);
            spec.text.a = tr.text_alpha.clamp(0.15, 1.0);
            spec.muted.a = (tr.text_alpha * 0.85).clamp(0.12, 1.0);
        }
        spec
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
        fn contains_viz(widgets: &[ExtWidget]) -> bool {
            widgets.iter().any(|w| match w {
                ExtWidget::Visualizer { .. } => true,
                ExtWidget::Row { children } | ExtWidget::Grid { children, .. } => {
                    contains_viz(children)
                }
                _ => false,
            })
        }
        match &self.active_view {
            ActiveView::Extension { ext_id, tab_id } => {
                let base_has = snap
                    .ext_sidebar_tabs
                    .iter()
                    .find(|t| &t.ext_id == ext_id && &t.id == tab_id)
                    .is_some_and(|t| contains_viz(&t.widgets));
                let overlay_has = overlays_for_ext_tab(&snap.ext_sidebar_tabs, ext_id)
                    .iter()
                    .any(|t| contains_viz(&t.widgets));
                base_has || overlay_has
            }
            ActiveView::Settings => snap
                .ext_settings_panels
                .iter()
                .any(|p| contains_viz(&p.widgets)),
            ActiveView::Home => overlays_for_home(&snap.ext_sidebar_tabs)
                .iter()
                .any(|t| contains_viz(&t.widgets)),
        }
    }

    fn is_gif_viz_visible(&self) -> bool {
        let Some(snap) = &self.snapshot else {
            return false;
        };
        if snap.layout.show_player_viz && snap.layout.player_viz_mode.eq_ignore_ascii_case("gif") {
            return true;
        }
        fn contains_gif_viz(widgets: &[ExtWidget]) -> bool {
            widgets.iter().any(|w| match w {
                ExtWidget::Visualizer { mode, .. } => mode.trim().eq_ignore_ascii_case("gif"),
                ExtWidget::Row { children } | ExtWidget::Grid { children, .. } => {
                    contains_gif_viz(children)
                }
                _ => false,
            })
        }
        match &self.active_view {
            ActiveView::Extension { ext_id, tab_id } => {
                let base_has = snap
                    .ext_sidebar_tabs
                    .iter()
                    .find(|t| &t.ext_id == ext_id && &t.id == tab_id)
                    .is_some_and(|t| contains_gif_viz(&t.widgets));
                let overlay_has = overlays_for_ext_tab(&snap.ext_sidebar_tabs, ext_id)
                    .iter()
                    .any(|t| contains_gif_viz(&t.widgets));
                base_has || overlay_has
            }
            ActiveView::Settings => snap
                .ext_settings_panels
                .iter()
                .any(|p| contains_gif_viz(&p.widgets)),
            ActiveView::Home => overlays_for_home(&snap.ext_sidebar_tabs)
                .iter()
                .any(|t| contains_gif_viz(&t.widgets)),
        }
    }

    fn needs_viz_animation(&self) -> bool {
        let Some(snap) = &self.snapshot else {
            return false;
        };
        let playing_mini = snap.play_state == PlayState::Playing && snap.layout.show_player_viz;
        let hide_stage_anim = snap.play_state == PlayState::Playing
            && is_ui_hidden(snap)
            && (snap.transparency.keep_effects || snap.transparency.keep_visualizer);
        let reactive_active = snap.reactive_ui.enabled
            && (snap.play_state == PlayState::Playing
                || matches!(&self.active_view, ActiveView::Extension { ext_id, .. } if ext_id == "tntron.reactive-ui"));
        playing_mini || hide_stage_anim || reactive_active || self.has_active_visualizer()
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

                let wanted_bg = if snap.transparency.enabled && snap.transparency.bg_mode == "image" {
                    snap.transparency
                        .bg_image_path
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(ToString::to_string)
                } else {
                    None
                };
                if wanted_bg != self.custom_bg_path {
                    self.custom_bg_handle = wanted_bg.as_deref().and_then(decode_any_image_handle);
                    self.custom_bg_path = wanted_bg;
                }

                let wanted_sprite = if snap.reactive_ui.enabled
                    && (snap.reactive_ui.custom_particle_enabled
                        || snap.reactive_ui.glow_particle_style.eq_ignore_ascii_case("custom_sprite"))
                {
                    snap.reactive_ui
                        .custom_particle_path
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(ToString::to_string)
                } else {
                    None
                };
                if wanted_sprite != self.custom_particle_path {
                    self.custom_particle_handle =
                        wanted_sprite.as_deref().and_then(decode_any_image_handle);
                    self.custom_particle_path = wanted_sprite;
                }

                let mut font_task: Option<Task<Message>> = None;
                let wanted_font = snap
                    .icon_pack
                    .font_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string);
                if wanted_font != self.custom_font_path {
                    self.custom_font_path = wanted_font.clone();
                    if let Some(fp) = wanted_font {
                        if let Some((bytes, detected_family)) = icons::load_custom_font_file(&fp) {
                            let fam = snap
                                .icon_pack
                                .font_family
                                .as_deref()
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .unwrap_or(&detected_family);
                            icons::set_custom_font_family(Some(fam));
                            font_task = Some(iced::font::load(bytes).map(Message::FontLoaded));
                        } else {
                            icons::set_custom_font_family(None);
                        }
                    } else {
                        icons::set_custom_font_family(None);
                    }
                } else if let Some(override_fam) = snap
                    .icon_pack
                    .font_family
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    if self.custom_font_path.is_some() {
                        icons::set_custom_font_family(Some(override_fam));
                    }
                }

                let wanted_avatar = snap
                    .profile
                    .avatar_path
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string);
                if wanted_avatar != self.avatar_path {
                    self.avatar_handle = wanted_avatar.as_deref().and_then(decode_any_image_handle);
                    self.avatar_path = wanted_avatar;
                }

                let wanted_logo = if snap.logo.mode == "custom_image" {
                    snap.logo
                        .custom_path
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(ToString::to_string)
                } else {
                    None
                };
                if wanted_logo != self.custom_logo_path {
                    self.custom_logo_handle =
                        wanted_logo.as_deref().and_then(decode_any_image_handle);
                    self.custom_logo_path = wanted_logo;
                }
                if !self.profile_drafts_synced {
                    self.profile_name_draft = snap.profile.name.clone();
                    self.profile_avatar_draft = snap.profile.avatar_path.clone().unwrap_or_default();
                    self.profile_drafts_synced = true;
                }
                if !self.discord_drafts_synced {
                    self.sync_discord_drafts(&snap.discord_rpc);
                    self.discord_drafts_synced = true;
                }

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
                let stage_gif_on = is_ui_hidden(&snap)
                    && snap.transparency.keep_visualizer
                    && snap.layout.player_viz_mode == "gif";
                self.snapshot = Some(*snap);
                if self.is_gif_viz_visible() || stage_gif_on {
                    self.gif_viz.activate();
                }
                if let Some(ft) = font_task {
                    return ft;
                }
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
                if let Some(snap) = &self.snapshot {
                    match overlay_host_for(&ext_id, &snap.ext_sidebar_tabs) {
                        Some(OverlayHost::Home) => {
                            self.active_view = ActiveView::Home;
                            self.home_plus_open = true;
                        }
                        Some(OverlayHost::Extension {
                            ext_id: host_ext,
                            tab_id: host_tab,
                        }) => {
                            self.active_view = ActiveView::Extension {
                                ext_id: host_ext,
                                tab_id: host_tab,
                            };
                        }
                        None => {
                            self.active_view = ActiveView::Extension { ext_id, tab_id };
                        }
                    }
                } else {
                    self.active_view = ActiveView::Extension { ext_id, tab_id };
                }
            }
            Message::Core(CoreUpdate::CopyToClipboard(txt)) => {
                return clipboard::write(txt);
            }
            Message::Core(CoreUpdate::RequestClipboardPaste { ext_id }) => {
                return clipboard::read().map(move |opt| Message::ExtAction {
                    ext_id: ext_id.clone(),
                    action_id: "clipboard_pasted".to_string(),
                    value: serde_json::Value::String(opt.unwrap_or_default()),
                });
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
            Message::TntBundleDraftChanged(s) => self.tnt_bundle_draft = s,
            Message::ExportFullTnt(target) => self.send(CoreCommand::ExportFullTnt(target)),
            Message::ImportFullTnt(target) => self.send(CoreCommand::ImportFullTnt(target)),
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
                let gif_visible = self.is_gif_viz_visible();
                let mut raw_bands = [0.0f32; 16];
                let mut raw_beat = 0.0f32;
                let mut is_playing = false;
                let mut cur_speed = 1.0f64;
                if let Some(snap) = &self.snapshot {
                    is_playing = snap.play_state == PlayState::Playing;
                    cur_speed = snap.speed;
                    if is_playing {
                        if let Some(an) = &snap.analysis {
                            let elapsed = self.pos_anchor_time.elapsed().as_secs_f64() * snap.speed;
                            let live_pos = self.pos_anchor_sec + elapsed;
                            let (target, beat) = an.sample_at(live_pos);
                            raw_bands = target;
                            raw_beat = beat;
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
                self.gif_viz
                    .on_frame(gif_visible, is_playing, cur_speed, &raw_bands, raw_beat);
                if let Some(snap) = &self.snapshot {
                    self.reactive_bg
                        .on_frame(is_playing, &raw_bands, raw_beat, &snap.reactive_ui);
                }
            }
            Message::NextGifViz => {
                self.gif_viz.next_random_now();
            }
            Message::ToggleHomePlus => {
                self.home_plus_open = !self.home_plus_open;
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
                if self.alerts_popover_open {
                    self.profile_popover_open = false;
                }
            }
            Message::CloseAlertsPopover => {
                self.alerts_popover_open = false;
            }
            Message::ToggleProfilePopover => {
                self.profile_popover_open = !self.profile_popover_open;
                if self.profile_popover_open {
                    self.alerts_popover_open = false;
                    if let Some(snap) = &self.snapshot {
                        self.profile_name_draft = snap.profile.name.clone();
                        self.profile_avatar_draft = snap.profile.avatar_path.clone().unwrap_or_default();
                    }
                }
            }
            Message::CloseProfilePopover => {
                self.profile_popover_open = false;
            }
            Message::ProfileNameDraftChanged(s) => self.profile_name_draft = s,
            Message::SubmitProfileName => {
                self.send(CoreCommand::UpdateProfile {
                    name: Some(self.profile_name_draft.clone()),
                    bio: None,
                    avatar_path: None,
                    avatar_size: None,
                    avatar_shape: None,
                    avatar_fit: None,
                });
            }
            Message::ProfileAvatarDraftChanged(s) => self.profile_avatar_draft = s,
            Message::SubmitProfileAvatar => {
                let p = self.profile_avatar_draft.trim().to_string();
                self.send(CoreCommand::UpdateProfile {
                    name: None,
                    bio: None,
                    avatar_path: Some(if p.is_empty() { None } else { Some(p) }),
                    avatar_size: None,
                    avatar_shape: None,
                    avatar_fit: None,
                });
            }
            Message::ClearProfileAvatar => {
                self.profile_avatar_draft.clear();
                self.send(CoreCommand::UpdateProfile {
                    name: None,
                    bio: None,
                    avatar_path: Some(None),
                    avatar_size: None,
                    avatar_shape: None,
                    avatar_fit: None,
                });
            }
            Message::SetProfileAvatarSize(sz) => {
                self.send(CoreCommand::UpdateProfile {
                    name: None,
                    bio: None,
                    avatar_path: None,
                    avatar_size: Some(sz),
                    avatar_shape: None,
                    avatar_fit: None,
                });
            }
            Message::SetProfileAvatarShape(shape) => {
                self.send(CoreCommand::UpdateProfile {
                    name: None,
                    bio: None,
                    avatar_path: None,
                    avatar_size: None,
                    avatar_shape: Some(shape),
                    avatar_fit: None,
                });
            }
            Message::SetProfileAvatarFit(fit) => {
                self.send(CoreCommand::UpdateProfile {
                    name: None,
                    bio: None,
                    avatar_path: None,
                    avatar_size: None,
                    avatar_shape: None,
                    avatar_fit: Some(fit),
                });
            }
            Message::CopyProfileStatsTnt => {
                if let Some(snap) = &self.snapshot {
                    if let Ok(val) = serde_json::to_value(&snap.profile) {
                        if let Ok(txt) = tntron_core::format_tnt_share_json(
                            "tntron-profile-v1",
                            &snap.profile.name,
                            val,
                        ) {
                            self.profile_share_draft = txt;
                        }
                    }
                }
                self.send(CoreCommand::CopyProfileStatsTnt);
            }
            Message::PasteProfileStatsClipboard => {
                return clipboard::read().map(|opt| {
                    Message::ImportProfileStatsTnt(opt.unwrap_or_default())
                });
            }
            Message::ProfileShareDraftChanged(s) => self.profile_share_draft = s,
            Message::ImportProfileStatsTnt(raw) => {
                if !raw.trim().is_empty() {
                    self.profile_share_draft = raw.clone();
                }
                self.send(CoreCommand::ImportProfileStatsTnt(raw));
            }
            Message::ResetProfileStats => {
                self.send(CoreCommand::ResetProfileStats);
            }
            Message::ResolvePendingProfileImport(approve) => {
                self.send(CoreCommand::ResolvePendingProfileImport(approve));
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
                        "topbar_hide_ui" => cfg.show_topbar_hide_ui = !cfg.show_topbar_hide_ui,
                        _ => {}
                    }
                    self.send(CoreCommand::UpdateLayoutConfig(cfg));
                }
            }
            Message::SetPlayerVizMode(mode) => {
                if mode.eq_ignore_ascii_case("gif") {
                    self.gif_viz.activate();
                }
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
                        "flame" => "gif".to_string(),
                        _ => "bars".to_string(),
                    };
                    if cfg.player_viz_mode == "gif" {
                        self.gif_viz.activate();
                    }
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
            Message::ToggleHideUi => self.send(CoreCommand::ToggleHideUi),
            Message::ToggleDiscordFlag(flag) => {
                if let Some(snap) = &self.snapshot {
                    let mut cfg = snap.discord_rpc.clone();
                    match flag {
                        "enabled" => cfg.enabled = !cfg.enabled,
                        "title" => cfg.show_title = !cfg.show_title,
                        "artist" => cfg.show_artist = !cfg.show_artist,
                        "album" => cfg.show_album = !cfg.show_album,
                        "timestamps" => cfg.show_timestamps = !cfg.show_timestamps,
                        "paused" => cfg.show_paused = !cfg.show_paused,
                        "clear_on_pause" => cfg.clear_on_pause = !cfg.clear_on_pause,
                        "codec" => cfg.show_codec = !cfg.show_codec,
                        "bpm" => cfg.show_bpm = !cfg.show_bpm,
                        "speed_pitch" => cfg.show_speed_pitch = !cfg.show_speed_pitch,
                        "loop_shuffle" => cfg.show_loop_shuffle = !cfg.show_loop_shuffle,
                        "queue" => cfg.show_queue_info = !cfg.show_queue_info,
                        "idle" => cfg.show_idle = !cfg.show_idle,
                        "button" => cfg.show_button = !cfg.show_button,
                        "privacy" => cfg.privacy_mode = !cfg.privacy_mode,
                        _ => {}
                    }
                    self.send(CoreCommand::UpdateDiscordRpcConfig(cfg));
                }
            }
            Message::SetDiscordOption(opt, val) => {
                if let Some(snap) = &self.snapshot {
                    let mut cfg = snap.discord_rpc.clone();
                    match opt {
                        "activity_type" => {
                            cfg.activity_type = val.parse::<u8>().unwrap_or(2);
                        }
                        "timestamp_mode" => cfg.timestamp_mode = val,
                        "details_mode" => cfg.details_mode = val,
                        "state_mode" => cfg.state_mode = val,
                        _ => {}
                    }
                    self.send(CoreCommand::UpdateDiscordRpcConfig(cfg));
                }
            }
            Message::DiscordFieldChanged(field, val) => match field {
                "client_id" => self.discord_client_id_draft = val,
                "idle_text" => self.discord_idle_draft = val,
                "custom_details" => self.discord_details_draft = val,
                "custom_state" => self.discord_state_draft = val,
                "large_key" => self.discord_large_key_draft = val,
                "large_text" => self.discord_large_text_draft = val,
                "small_key" => self.discord_small_key_draft = val,
                "small_text" => self.discord_small_text_draft = val,
                "button_label" => self.discord_btn_label_draft = val,
                "button_url" => self.discord_btn_url_draft = val,
                _ => {}
            },
            Message::ApplyDiscordFields => {
                if let Some(snap) = &self.snapshot {
                    let mut cfg = snap.discord_rpc.clone();
                    cfg.client_id = self.discord_client_id_draft.trim().to_string();
                    cfg.idle_text = self.discord_idle_draft.trim().to_string();
                    cfg.custom_details = self.discord_details_draft.clone();
                    cfg.custom_state = self.discord_state_draft.clone();
                    cfg.large_image_key = self.discord_large_key_draft.trim().to_string();
                    cfg.large_text = self.discord_large_text_draft.clone();
                    cfg.small_image_key = self.discord_small_key_draft.trim().to_string();
                    cfg.small_text = self.discord_small_text_draft.clone();
                    cfg.button_label = self.discord_btn_label_draft.trim().to_string();
                    cfg.button_url = self.discord_btn_url_draft.trim().to_string();
                    self.send(CoreCommand::UpdateDiscordRpcConfig(cfg));
                }
            }
            Message::ReconnectDiscordRpc => self.send(CoreCommand::ReconnectDiscordRpc),
            Message::ResetDiscordRpc => {
                self.sync_discord_drafts(&DiscordRpcConfig::default());
                self.send(CoreCommand::ResetDiscordRpcConfig);
            }
            Message::FontLoaded(res) => {
                if let Err(err) = res {
                    tracing::warn!("Failed loading imported icon font: {err:?}");
                }
            }
            Message::Noop => {}
            Message::ToggleFullscreen => {
                self.is_fullscreen = !self.is_fullscreen;
                let mode = if self.is_fullscreen {
                    window::Mode::Fullscreen
                } else {
                    window::Mode::Windowed
                };
                return window::latest().and_then(move |id| window::set_mode(id, mode));
            }
            Message::SetFullscreen(on) => {
                self.is_fullscreen = on;
                let mode = if on {
                    window::Mode::Fullscreen
                } else {
                    window::Mode::Windowed
                };
                return window::latest().and_then(move |id| window::set_mode(id, mode));
            }
            Message::KeyPressed(key) => match key {
                Key::Named(Named::F11) => {
                    return self.update(Message::ToggleFullscreen);
                }
                Key::Character(ref c) if c.as_str() == "/" => {
                    return operation::focus(self.search_id.clone());
                }
                Key::Named(Named::Space) => self.send(CoreCommand::TogglePause),
                Key::Named(Named::Escape) => {
                    let had_overlay = self.context_menu.is_some()
                        || self.modal.is_some()
                        || self.alerts_popover_open
                        || self.profile_popover_open
                        || self.show_new_folder
                        || !self.search_query.is_empty();
                    self.context_menu = None;
                    self.modal = None;
                    self.alerts_popover_open = false;
                    self.profile_popover_open = false;
                    self.show_new_folder = false;
                    self.search_query.clear();
                    if !had_overlay {
                        if self.snapshot.as_ref().is_some_and(is_ui_hidden) {
                            self.send(CoreCommand::ToggleHideUi);
                        } else if self.is_fullscreen {
                            return self.update(Message::SetFullscreen(false));
                        }
                    }
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
            icons::set_active_pack(&tntron_core::IconPackConfig::default());
            button_style::set_active_pack(&ButtonPackConfig::default());
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
        icons::set_active_pack(&snap.icon_pack);
        button_style::set_active_pack(&snap.button_pack);

        let tr = &snap.transparency;
        let hide_ui = is_ui_hidden(snap);
        let reactive_on = snap.reactive_ui.enabled && (!hide_ui || tr.keep_effects);
        let custom_bg = if !hide_ui || tr.keep_effects {
            self.view_custom_background(snap, &theme)
        } else {
            None
        };
        let has_custom_bg = custom_bg.is_some();

        let root_content: Element<'_, Message> = if hide_ui {
            self.view_focus_stage(snap, &theme)
        } else {
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
            root_col.into()
        };

        let root_shell = container(root_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style({
                let bg = if reactive_on || has_custom_bg || hide_ui {
                    Color::TRANSPARENT
                } else {
                    theme.base
                };
                move |_| container::Style {
                    background: Some(Background::Color(bg)),
                    ..Default::default()
                }
            });

        let mut layers: Vec<Element<'_, Message>> = Vec::new();
        if let Some(bg_layer) = custom_bg {
            layers.push(bg_layer);
        }
        if reactive_on {
            layers.push(reactive_bg_canvas(
                &self.reactive_bg,
                &snap.reactive_ui,
                theme.accent,
                theme.base,
                has_custom_bg,
                self.custom_particle_handle.clone(),
                snap.play_state == PlayState::Playing,
            ));
        }
        layers.push(root_shell.into());
        if hide_ui {
            layers.push(
                container(pill_icon_button(
                    "Show UI",
                    Some(icons::LAYERS),
                    &theme,
                    true,
                    Message::ToggleHideUi,
                ))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .align_y(Alignment::Start)
                .padding([12, 16])
                .into(),
            );
        }
        if let Some(alerts) = self.view_alerts_popover(snap, &theme) {
            layers.push(alerts);
        }
        if let Some(prof) = self.view_profile_popover(snap, &theme) {
            layers.push(prof);
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

    fn view_custom_background<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Option<Element<'a, Message>> {
        let tr = &snap.transparency;
        if !tr.enabled {
            return None;
        }
        let alpha = tr.bg_image_alpha.clamp(0.05, 1.0);
        let c1 = tr
            .tint_hex
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(theme.base);
        let c2 = tr
            .tint2_hex
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(Color {
                r: theme.accent.r * 0.45,
                g: theme.accent.g * 0.45,
                b: theme.accent.b * 0.45,
                a: 1.0,
            });
        match tr.bg_mode.trim().to_ascii_lowercase().as_str() {
            "color" => Some(custom_bg_canvas(
                false,
                Color { a: alpha, ..c1 },
                Color { a: alpha, ..c1 },
            )),
            "gradient" => Some(custom_bg_canvas(
                true,
                Color { a: alpha, ..c1 },
                Color { a: alpha, ..c2 },
            )),
            "image" | "cover" => {
                let handle = if tr.bg_mode == "cover" {
                    self.cover_handle.clone().or_else(|| self.custom_bg_handle.clone())
                } else {
                    self.custom_bg_handle.clone()
                }?;
                let fit = match tr.bg_fit.trim().to_ascii_lowercase().as_str() {
                    "contain" => ContentFit::Contain,
                    "fill" => ContentFit::Fill,
                    _ => ContentFit::Cover,
                };
                let bg_tint = Color {
                    a: tr.base_alpha.clamp(0.0, 1.0),
                    ..theme.base
                };
                Some(
                    container(
                        image(handle)
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .content_fit(fit)
                            .opacity(alpha),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(bg_tint)),
                        ..Default::default()
                    })
                    .into(),
                )
            }
            _ => None,
        }
    }

    fn view_focus_stage<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let tr = &snap.transparency;
        let mut root_col = Column::new()
            .spacing(8)
            .width(Length::Fill)
            .height(Length::Fill);

        if tr.keep_topbar {
            let mut top_row = Row::new()
                .spacing(8)
                .padding([10, 16])
                .align_y(Alignment::Center)
                .width(Length::Fill);
            top_row = top_row.push(text("tntron").size(15).color(theme.accent));
            top_row = top_row.push(space::horizontal());
            for slot in &snap.ext_topbar_slots {
                top_row = top_row.push(slot_badge(&slot.text, slot.icon.as_deref(), slot.accent, theme, None));
            }
            root_col = root_col.push(top_row);
        }

        let mut stage_center = Column::new()
            .spacing(16)
            .padding([20, 28])
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .height(Length::Fill);
        stage_center = stage_center.push(space::vertical().height(Length::Fill));

        if tr.keep_artwork {
            if let Some(track) = &snap.current_track {
                let cover_el: Element<'a, Message> = if let Some(handle) = &self.cover_handle {
                    image(handle.clone())
                        .width(Length::Fixed(180.0))
                        .height(Length::Fixed(180.0))
                        .content_fit(ContentFit::Cover)
                        .into()
                } else {
                    container(self.view_brand_logo(snap, theme, Some(52.0)))
                        .width(Length::Fixed(180.0))
                        .height(Length::Fixed(180.0))
                        .center_x(Length::Fixed(180.0))
                        .center_y(Length::Fixed(180.0))
                        .style({
                            let card_bg = theme.card;
                            move |_| container::Style {
                                background: Some(Background::Color(card_bg)),
                                border: Border {
                                    radius: 14.0.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        })
                        .into()
                };
                let bpm_txt = snap
                    .track_bpm
                    .map(|b| format!(" · {b:.0} BPM"))
                    .unwrap_or_default();
                stage_center = stage_center.push(
                    card_box(
                        column![
                            cover_el,
                            text(&track.title).size(22).color(theme.text),
                            text(format!("{}{bpm_txt}", track.artist))
                                .size(14)
                                .color(theme.muted),
                        ]
                        .spacing(8)
                        .align_x(Alignment::Center)
                        .padding(16),
                        theme,
                        false,
                    ),
                );
            }
        }

        if tr.keep_visualizer {
            let viz_el: Element<'a, Message> = if snap.layout.player_viz_mode == "gif" {
                let gv = self.gif_viz.view();
                if let Some(handle) = gv.handle {
                    container(
                        image(handle)
                            .content_fit(ContentFit::Contain)
                            .width(Length::Fixed(340.0))
                            .height(Length::Fixed(210.0)),
                    )
                    .into()
                } else {
                    container(visualizer_canvas(
                        "bars",
                        &self.viz_frame(),
                        snap.play_state == PlayState::Playing,
                        snap.volume,
                        snap.speed,
                        theme,
                    ))
                    .width(Length::Fill)
                    .max_width(760.0)
                    .height(Length::Fixed(180.0))
                    .into()
                }
            } else {
                container(visualizer_canvas(
                    &snap.layout.player_viz_mode,
                    &self.viz_frame(),
                    snap.play_state == PlayState::Playing,
                    snap.volume,
                    snap.speed,
                    theme,
                ))
                .width(Length::Fill)
                .max_width(760.0)
                .height(Length::Fixed(180.0))
                .into()
            };
            stage_center = stage_center.push(viz_el);
        }

        if tr.keep_lyrics {
            let line = snap
                .current_lyric
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("♪ ...");
            stage_center = stage_center.push(
                card_box(
                    container(text(line).size(22).color(theme.accent)).padding([10, 22]),
                    theme,
                    true,
                ),
            );
        }

        stage_center = stage_center.push(space::vertical().height(Length::Fill));

        let mut mid_row = Row::new().spacing(8).width(Length::Fill).height(Length::Fill);
        if tr.keep_sidebar {
            mid_row = mid_row.push(self.view_sidebar(snap, theme));
        }
        mid_row = mid_row.push(stage_center);
        if tr.keep_queue {
            mid_row = mid_row.push(self.view_queue_sidebar(snap, theme));
        }

        root_col = root_col.push(container(mid_row).padding([8, 8]).height(Length::Fill));
        if tr.keep_player {
            if self.speed_popover_open {
                root_col = root_col.push(
                    container(self.view_speed_presets_strip(snap, theme)).padding([0, 12]),
                );
            }
            root_col = root_col.push(self.view_player_bar(snap, theme));
        }
        root_col.into()
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
                    self.view_brand_logo(snap, theme, None),
                    text("tntron").size(17).color(theme.text),
                    space::horizontal(),
                    text("OPEN").size(10).color(theme.accent),
                ]
                .spacing(9)
                .align_y(Alignment::Center),
            )
            .padding([6, 10]),
        );

        let home_has_plus = !overlays_for_home(&snap.ext_sidebar_tabs).is_empty();
        let visible_tabs: Vec<&ExtSidebarTab> = snap
            .ext_sidebar_tabs
            .iter()
            .filter(|tab| overlay_host_for(&tab.ext_id, &snap.ext_sidebar_tabs).is_none())
            .collect();

        if snap.compact_menu {
            let mut btns: Vec<Element<'_, Message>> = vec![
                compact_nav_button(
                    icons::HOME,
                    "Home",
                    self.active_view == ActiveView::Home && !self.showing_favorites,
                    home_has_plus,
                    theme,
                    Message::OpenFolder(snap.music_root.clone()),
                ),
                compact_nav_button(
                    icons::SETTINGS,
                    "Settings",
                    self.active_view == ActiveView::Settings,
                    false,
                    theme,
                    Message::SwitchView(ActiveView::Settings),
                ),
            ];
            for tab in visible_tabs {
                let target = ActiveView::Extension {
                    ext_id: tab.ext_id.clone(),
                    tab_id: tab.id.clone(),
                };
                let is_active = self.active_view == target;
                let has_plus = !overlays_for_ext_tab(&snap.ext_sidebar_tabs, &tab.ext_id).is_empty();
                btns.push(compact_nav_button(
                    resolve_icon(&tab.icon),
                    tab.label.as_str(),
                    is_active,
                    has_plus,
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
                home_has_plus,
                theme,
                Message::OpenFolder(snap.music_root.clone()),
            ));
            nav_col = nav_col.push(sidebar_nav_item(
                icons::SETTINGS,
                "Settings",
                self.active_view == ActiveView::Settings,
                false,
                theme,
                Message::SwitchView(ActiveView::Settings),
            ));

            if !visible_tabs.is_empty() {
                let mut ext_tabs_col = Column::new().spacing(2);
                for tab in visible_tabs {
                    let target = ActiveView::Extension {
                        ext_id: tab.ext_id.clone(),
                        tab_id: tab.id.clone(),
                    };
                    let is_active = self.active_view == target;
                    let has_plus =
                        !overlays_for_ext_tab(&snap.ext_sidebar_tabs, &tab.ext_id).is_empty();
                    ext_tabs_col = ext_tabs_col.push(sidebar_nav_item(
                        resolve_icon(&tab.icon),
                        tab.label.as_str(),
                        is_active,
                        has_plus,
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
        if snap.layout.show_topbar_hide_ui {
            let hidden = is_ui_hidden(snap);
            header_right = header_right.push(pill_icon_button(
                if hidden { "Show UI" } else { "Hide UI" },
                Some(icons::LAYERS),
                theme,
                hidden,
                Message::ToggleHideUi,
            ));
        }
        header_right = header_right.push(pill_button(
            if self.is_fullscreen { "Exit Full" } else { "Fullscreen" },
            theme,
            self.is_fullscreen,
            Message::ToggleFullscreen,
        ));
        header_right = header_right.push(self.view_profile_header_button(snap, theme));

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
            container(self.view_brand_logo(snap, theme, Some(46.0)))
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

        let home_overlays = overlays_for_home(&snap.ext_sidebar_tabs);
        let cur_layout = snap.home_layout.as_str();
        let mut controls_row = row![
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
        if !home_overlays.is_empty() && !self.showing_favorites && snap.current_dir == snap.music_root {
            controls_row = controls_row.push(pill_button(
                "Home+",
                theme,
                self.home_plus_open,
                Message::ToggleHomePlus,
            ));
        }

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
                        let folder_art: Element<'a, Message> = if let Some((_, h)) = self.card_covers.get(path) {
                            container(
                                image(h.clone())
                                    .content_fit(ContentFit::Cover)
                                    .width(Length::Fixed(28.0))
                                    .height(Length::Fixed(28.0)),
                            )
                            .width(Length::Fixed(34.0))
                            .into()
                        } else {
                            container(icons::icon(icons::FOLDER, 14.0, theme.accent))
                                .width(Length::Fixed(34.0))
                                .into()
                        };
                        let row_inner = row![
                            folder_art,
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
        if self.home_plus_open
            && !home_overlays.is_empty()
            && !self.showing_favorites
            && snap.current_dir == snap.music_root
            && q.is_empty()
        {
            let viz = self.viz_frame();
            let ext_viz = ExtVizCtx {
                frame: viz,
                gif: self.gif_viz.view(),
                on_next_gif: Message::NextGifViz,
            };
            let mut hp_col = Column::new().spacing(10).padding(14);
            for tab in &home_overlays {
                for w in &tab.widgets {
                    hp_col = hp_col.push(render_ext_widget(
                        w,
                        &tab.ext_id,
                        theme,
                        &ext_viz,
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
            out = out.push(
                container(scrollable(card_box(hp_col, theme, false).width(Length::Fill)))
                    .padding([0, 20])
                    .max_height(260.0),
            );
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
                        let folder_thumb: Element<'a, Message> = if let Some((_, h)) = self.card_covers.get(path) {
                            image(h.clone())
                                .content_fit(ContentFit::Cover)
                                .width(Length::Fixed(tile_px))
                                .height(Length::Fixed(tile_px))
                                .into()
                        } else {
                            icons::icon(icons::FOLDER, 28.0, theme.accent).into()
                        };
                        let card = card_box(
                            column![
                                row![
                                    folder_thumb,
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
                            if self.is_fullscreen { "Exit Fullscreen (F11)" } else { "Fullscreen (F11)" },
                            Some(icons::LAYERS),
                            theme,
                            self.is_fullscreen,
                            Message::ToggleFullscreen,
                        ),
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
                        flag_btn("Top-Right Hide UI", cfg.show_topbar_hide_ui, "topbar_hide_ui"),
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
                    text(format!("mpv IPC: {ipc_label}  ·  Display: {}  ·  Metadata Cache: {}/10000 tracks  ·  Library: {} tracks in {} playlists", self.display_env.label(), snap.cache_len, snap.all_tracks.len(), snap.folder_cards.len()))
                        .size(12)
                        .color(theme.muted),
                    text("Shortcuts: Space Play/Pause · F11 Fullscreen · / Search · q Queue · m Mute · j/k Select")
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

        col = col.push(self.view_discord_rpc_card(snap, theme));

        let tnt_files = list_tnt_files(&default_tnt_dir());
        let mut tnt_chips = Row::new().spacing(6);
        for (name, _) in tnt_files.iter().take(6) {
            tnt_chips = tnt_chips.push(pill_icon_button(
                format!("{name}.tnt"),
                Some(icons::DOWNLOAD),
                theme,
                false,
                Message::ImportFullTnt(name.clone()),
            ));
        }

        col = col.push(
            card_box(
                column![
                    text("Data Management & Full .tnt Export").size(16).color(theme.text),
                    text("Built-in .tnt bundle export saves ALL settings, theme, DSP, Button/Reactive configs, and ALL installed extensions into one portable .tnt file (~/.config/tntron/tnt/).")
                        .size(12)
                        .color(theme.muted),
                    row![
                        text_input("tntron_full_backup or ~/Documents/backup.tnt", &self.tnt_bundle_draft)
                            .on_input(Message::TntBundleDraftChanged)
                            .size(12)
                            .padding([6, 10])
                            .width(Length::Fill),
                        primary_pill_icon_button(
                            "Export Full .tnt",
                            Some(icons::DOWNLOAD),
                            theme,
                            Message::ExportFullTnt(self.tnt_bundle_draft.clone()),
                        ),
                        pill_icon_button(
                            "Import Full .tnt",
                            Some(icons::REFRESH),
                            theme,
                            false,
                            Message::ImportFullTnt(self.tnt_bundle_draft.clone()),
                        ),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                    if tnt_files.is_empty() {
                        Element::from(space::horizontal().width(Length::Fixed(0.0)))
                    } else {
                        tnt_chips.wrap().into()
                    },
                    text("Quick Preferences JSON (~/.config/tntron/tntron_preferences.json)")
                        .size(11)
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
        let ext_viz = ExtVizCtx {
            frame: viz,
            gif: self.gif_viz.view(),
            on_next_gif: Message::NextGifViz,
        };
        // 3. Extension Settings Panels registered dynamically via tntron.ui.register_settings
        for panel in &snap.ext_settings_panels {
            let mut pcol = Column::new().spacing(10).padding(16);
            pcol = pcol.push(text(&panel.title).size(15).color(theme.accent));
            for w in &panel.widgets {
                pcol = pcol.push(render_ext_widget(
                    w,
                    &panel.ext_id,
                    theme,
                    &ext_viz,
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
        let overlays = overlays_for_ext_tab(&snap.ext_sidebar_tabs, ext_id);
        let mut header_row = row![
            icons::icon(resolve_icon(&tab.icon), 22.0, theme.accent),
            text(if overlays.is_empty() {
                tab.label.clone()
            } else {
                format!("{}+", tab.label)
            })
            .size(26)
            .color(theme.text),
            slot_badge(ext_id, None, false, theme, None),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        for otab in &overlays {
            header_row = header_row.push(slot_badge(&otab.label, Some("plus"), true, theme, None));
        }
        col = col.push(header_row);

        let viz = self.viz_frame();
        let ext_viz = ExtVizCtx {
            frame: viz,
            gif: self.gif_viz.view(),
            on_next_gif: Message::NextGifViz,
        };

        let is_merged_viz = ext_id == "openify.visualizer"
            && overlays
                .iter()
                .any(|o| o.ext_id == "openify.better-visualizer");

        let push_widgets = |dest: &mut Column<'a, Message>, widgets: &'a [ExtWidget], owner: &'a str, skip_viz: bool| {
            for w in widgets {
                if skip_viz && matches!(w, ExtWidget::Visualizer { .. }) {
                    continue;
                }
                *dest = std::mem::replace(dest, Column::new()).push(render_ext_widget(
                    w,
                    owner,
                    theme,
                    &ext_viz,
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
        };

        if is_merged_viz {
            // Put Visualizer+'s enhanced 10-scene stage first, then base Visualizer's energy/topbar controls.
            for otab in &overlays {
                let mut ocol = Column::new().spacing(12).padding(16);
                push_widgets(&mut ocol, &otab.widgets, &otab.ext_id, false);
                col = col.push(card_box(ocol, theme, true).width(Length::Fill));
            }
            let mut base_extra = Column::new().spacing(10).padding(16);
            push_widgets(&mut base_extra, &tab.widgets, ext_id, true);
            col = col.push(card_box(base_extra, theme, false).width(Length::Fill));
        } else {
            let mut card_col = Column::new().spacing(12).padding(16);
            if tab.widgets.is_empty() {
                card_col = card_col.push(
                    text("This extension tab has no rendered widgets yet.")
                        .size(13)
                        .color(theme.muted),
                );
            } else {
                push_widgets(&mut card_col, &tab.widgets, ext_id, false);
            }
            col = col.push(card_box(card_col, theme, true).width(Length::Fill));

            for otab in overlays {
                let mut ocol = Column::new().spacing(12).padding(16);
                ocol = ocol.push(
                    row![
                        slot_badge(&otab.label, Some("plus"), true, theme, None),
                        slot_badge(&otab.ext_id, None, false, theme, None),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                );
                push_widgets(&mut ocol, &otab.widgets, &otab.ext_id, false);
                col = col.push(card_box(ocol, theme, false).width(Length::Fill));
            }
        }

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
            container(self.view_brand_logo(snap, theme, Some(22.0)))
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
            container(
                column![
                    text(title)
                        .size(14)
                        .color(theme.text)
                        .wrapping(iced::widget::text::Wrapping::None),
                    text(artist_line)
                        .size(12)
                        .color(theme.muted)
                        .wrapping(iced::widget::text::Wrapping::None),
                ]
                .spacing(3),
            )
            .width(Length::Fill)
            .clip(true),
        ]
        .spacing(10)
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
            .width(Length::FillPortion(4));

        let mute_glyph = if snap.is_muted {
            icons::VOLUME_MUTE
        } else {
            icons::VOLUME_HIGH
        };
        let mut right_row = Row::new().spacing(6).align_y(Alignment::Center);
        if snap.layout.show_player_viz {
            let gif_view = self.gif_viz.view();
            let viz_inner: Element<'a, Message> = if snap.layout.player_viz_mode == "gif" {
                if let Some(handle) = gif_view.handle {
                    let accent = theme.accent;
                    let card_bg = theme.card;
                    let flash = gif_view.flash;
                    container(
                        image(handle.clone())
                            .content_fit(ContentFit::Cover)
                            .width(Length::Fixed(72.0))
                            .height(Length::Fixed(34.0)),
                    )
                    .width(Length::Fixed(72.0))
                    .height(Length::Fixed(34.0))
                    .style(move |_| container::Style {
                        background: Some(Background::Color(card_bg)),
                        border: Border {
                            color: Color {
                                a: (0.28 + flash * 0.72).clamp(0.28, 1.0),
                                ..accent
                            },
                            width: if flash > 0.25 { 1.6 } else { 1.0 },
                            radius: 8.0.into(),
                        },
                        ..Default::default()
                    })
                    .into()
                } else {
                    mini_visualizer_canvas(
                        &snap.layout.player_viz_mode,
                        &self.viz_frame(),
                        snap.play_state == PlayState::Playing,
                        theme,
                    )
                }
            } else {
                mini_visualizer_canvas(
                    &snap.layout.player_viz_mode,
                    &self.viz_frame(),
                    snap.play_state == PlayState::Playing,
                    theme,
                )
            };
            let viz_btn = iced::widget::button(viz_inner)
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
            .width(Length::FillPortion(4))
            .align_x(Alignment::End);

        let bar_bg = theme.sidebar;
        let bar_border = theme.border;
        container(
            row![
                container(left_info).width(Length::FillPortion(4)),
                container(center_col).width(Length::FillPortion(4)),
                right_controls,
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .padding([10, 14]),
        )
        .width(Length::Fill)
        .height(Length::Fixed(88.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(bar_bg)),
            border: Border {
                color: bar_border,
                width: 0.5,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
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
                flag_row("Top-Right Hide UI", cfg.show_topbar_hide_ui, "topbar_hide_ui"),
            ]
            .spacing(6)
            .wrap(),
            text("Mini Visualizer Mode").size(11).color(theme.muted),
            row![
                pill_button("Bars", theme, cfg.player_viz_mode == "bars", Message::SetPlayerVizMode("bars".into())),
                pill_button("Wave", theme, cfg.player_viz_mode == "wave", Message::SetPlayerVizMode("wave".into())),
                pill_button("Circle", theme, cfg.player_viz_mode == "circle", Message::SetPlayerVizMode("circle".into())),
                pill_button("Flame", theme, cfg.player_viz_mode == "flame", Message::SetPlayerVizMode("flame".into())),
                pill_button("GIF", theme, cfg.player_viz_mode == "gif", Message::SetPlayerVizMode("gif".into())),
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
        if let Some(imported) = &snap.pending_profile_import {
            let mut top_songs = Column::new().spacing(4);
            let mut ranked: Vec<_> = imported.stats_track_plays.values().collect();
            ranked.sort_by_key(|item| std::cmp::Reverse(item.count));
            for (idx, item) in ranked.iter().take(4).enumerate() {
                top_songs = top_songs.push(
                    text(format!("{}. {} — {} ({} plays)", idx + 1, item.title, item.artist, item.count))
                        .size(11)
                        .color(theme.muted),
                );
            }
            let body = column![
                row![
                    icons::icon(icons::CHART, 16.0, theme.accent),
                    text("Import Profile & Listening Stats from .tnt?").size(16).color(theme.text),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
                text("By default, .tnt imports keep your local profile stats unchanged. Approve once to replace your profile stats with the imported data below:")
                    .size(12)
                    .color(theme.muted),
                card_box(
                    column![
                        text(format!("Profile: {}", imported.name)).size(13).color(theme.text),
                        text(format!(
                            "Listening Time: {}  ·  Total Plays: {}  ·  {} Tracks",
                            fmt_duration_words(imported.stats_total_sec),
                            imported.stats_total_plays,
                            imported.stats_track_plays.len()
                        ))
                        .size(12)
                        .color(theme.accent),
                        top_songs,
                    ]
                    .spacing(6)
                    .padding(12),
                    theme,
                    false,
                ),
                row![
                    space::horizontal(),
                    pill_button(
                        "Skip (Keep My Stats)",
                        theme,
                        false,
                        Message::ResolvePendingProfileImport(false),
                    ),
                    primary_pill_icon_button(
                        "Approve & Import Stats",
                        Some(icons::DOWNLOAD),
                        theme,
                        Message::ResolvePendingProfileImport(true),
                    ),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(14)
            .padding(20);

            let backdrop = mouse_area(
                container(text(""))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.58))),
                        ..Default::default()
                    }),
            )
            .on_press(Message::ResolvePendingProfileImport(false));

            let card = card_box(body, theme, true).width(Length::Fixed(460.0));
            let centered = container(card)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill);
            return Some(stack([backdrop.into(), centered.into()]).into());
        }

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

        Some(wrap_dismiss_popover(card.into(), Message::CloseAlertsPopover))
    }

    fn render_avatar_widget<'a>(
        &'a self,
        prof: &tntron_core::ProfileConfig,
        size_px: f32,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let r = match prof.avatar_shape.trim().to_ascii_lowercase().as_str() {
            "square" => 3.0,
            "rounded" => (size_px * 0.24).max(6.0),
            _ => size_px * 0.5,
        };
        if let Some(h) = &self.avatar_handle {
            let fit = if prof.avatar_fit.eq_ignore_ascii_case("contain") {
                ContentFit::Contain
            } else {
                ContentFit::Cover
            };
            container(
                image(h.clone())
                    .width(Length::Fixed(size_px))
                    .height(Length::Fixed(size_px))
                    .content_fit(fit)
                    .border_radius(r),
            )
            .width(Length::Fixed(size_px))
            .height(Length::Fixed(size_px))
            .clip(true)
            .style({
                let accent = theme.accent;
                move |_| container::Style {
                    border: Border {
                        color: accent,
                        width: 1.2,
                        radius: r.into(),
                    },
                    ..Default::default()
                }
            })
            .into()
        } else {
            let ch = prof
                .name
                .chars()
                .find(|c| c.is_alphanumeric())
                .unwrap_or('T')
                .to_ascii_uppercase()
                .to_string();
            let accent = theme.accent;
            let accent_fg = theme.accent_fg;
            container(text(ch).size((size_px * 0.44).max(11.0)).color(accent_fg))
                .width(Length::Fixed(size_px))
                .height(Length::Fixed(size_px))
                .center_x(Length::Fixed(size_px))
                .center_y(Length::Fixed(size_px))
                .style(move |_| container::Style {
                    background: Some(Background::Color(accent)),
                    border: Border {
                        radius: r.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into()
        }
    }

    fn view_profile_header_button<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Element<'a, Message> {
        let prof = &snap.profile;
        let thumb_px = prof.avatar_size.clamp(22.0, 34.0);
        let avatar = self.render_avatar_widget(prof, thumb_px, theme);
        let active = self.profile_popover_open;
        let th = theme.clone();
        let bg = if active { th.accent } else { th.card };
        let fg = if active { th.accent_fg } else { th.text };
        let border = if active { th.accent } else { th.border };
        let hover_bg = th.card_hover;

        iced::widget::button(
            button_style::wrap_chrome(
                row![
                    avatar,
                    text(&prof.name)
                        .size(12)
                        .color(fg)
                        .wrapping(iced::widget::text::Wrapping::None),
                ]
                .spacing(7)
                .align_y(Alignment::Center),
                [3, 10],
                button_style::ButtonKind::Pill,
                &th,
                th.surface,
            ),
        )
        .padding(0)
        .style(move |_, status| {
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
            );
            button_style::compute_button_style(
                button_style::ButtonKind::Pill,
                active,
                hovered,
                bg,
                hover_bg,
                fg,
                border,
                20.0,
                &th,
            )
        })
        .on_press(Message::ToggleProfilePopover)
        .into()
    }

    fn view_profile_popover<'a>(
        &'a self,
        snap: &'a UiSnapshot,
        theme: &ThemeSpec,
    ) -> Option<Element<'a, Message>> {
        if !self.profile_popover_open {
            return None;
        }
        let prof = &snap.profile;
        let preview_size = prof.avatar_size.clamp(28.0, 92.0);
        let avatar_preview = self.render_avatar_widget(prof, preview_size, theme);

        let mut ranked: Vec<_> = prof.stats_track_plays.values().collect();
        ranked.sort_by_key(|item| std::cmp::Reverse(item.count));

        let mut top_col = Column::new().spacing(5).width(Length::Fill);
        if ranked.is_empty() {
            top_col = top_col.push(
                text("Play songs to build your personal listening stats.")
                    .size(12)
                    .color(theme.muted),
            );
        } else {
            for (idx, item) in ranked.iter().take(6).enumerate() {
                top_col = top_col.push(card_box(
                    container(
                        row![
                            text(format!("#{}", idx + 1)).size(11).color(theme.accent),
                            column![
                                text(&item.title).size(12).color(theme.text),
                                text(&item.artist).size(10).color(theme.muted),
                            ]
                            .spacing(1)
                            .width(Length::Fill),
                            text(format!("{}x", item.count)).size(12).color(theme.accent),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                    )
                    .padding([6, 10])
                    .width(Length::Fill),
                    theme,
                    idx == 0,
                ));
            }
        }

        let header = row![
            avatar_preview,
            column![
                text(&prof.name).size(15).color(theme.text),
                text(format!(
                    "{} · {} plays · {} tracks",
                    fmt_duration_words(prof.stats_total_sec),
                    prof.stats_total_plays,
                    prof.stats_track_plays.len()
                ))
                .size(11)
                .color(theme.accent),
            ]
            .spacing(2)
            .width(Length::Fill),
            glyph_button(
                icons::CLOSE,
                13.0,
                theme,
                false,
                Message::CloseProfilePopover,
            ),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let content = column![
            header,
            row![
                text_input("Display Name", &self.profile_name_draft)
                    .on_input(Message::ProfileNameDraftChanged)
                    .on_submit(Message::SubmitProfileName)
                    .size(12)
                    .padding([5, 8])
                    .width(Length::Fill),
                pill_button("Save Name", theme, false, Message::SubmitProfileName),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            text("Profile Picture (.jxl, .png, .jpg, .webp)")
                .size(11)
                .color(theme.muted),
            row![
                text_input("~/Pictures/avatar.jxl", &self.profile_avatar_draft)
                    .on_input(Message::ProfileAvatarDraftChanged)
                    .on_submit(Message::SubmitProfileAvatar)
                    .size(12)
                    .padding([5, 8])
                    .width(Length::Fill),
                pill_button("Set", theme, true, Message::SubmitProfileAvatar),
                glyph_button(icons::CLOSE, 12.0, theme, false, Message::ClearProfileAvatar),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            row![
                text(format!("Avatar Size {:.0}px", prof.avatar_size)).size(11).color(theme.muted),
                slider(24.0..=92.0, prof.avatar_size, Message::SetProfileAvatarSize)
                    .step(2.0)
                    .width(Length::Fill),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                pill_button("Circle", theme, prof.avatar_shape == "circle", Message::SetProfileAvatarShape("circle".into())),
                pill_button("Rounded", theme, prof.avatar_shape == "rounded", Message::SetProfileAvatarShape("rounded".into())),
                pill_button("Square", theme, prof.avatar_shape == "square", Message::SetProfileAvatarShape("square".into())),
                space::horizontal(),
                pill_button("Cover", theme, prof.avatar_fit != "contain", Message::SetProfileAvatarFit("cover".into())),
                pill_button("Fit", theme, prof.avatar_fit == "contain", Message::SetProfileAvatarFit("contain".into())),
            ]
            .spacing(5)
            .align_y(Alignment::Center),
            row![
                text("Listening Stats").size(13).color(theme.text),
                space::horizontal(),
                pill_icon_button("Copy Stats (.tnt)", Some(icons::DOWNLOAD), theme, true, Message::CopyProfileStatsTnt),
                pill_icon_button("Paste", Some(icons::REFRESH), theme, false, Message::PasteProfileStatsClipboard),
                pill_button("Reset", theme, false, Message::ResetProfileStats),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            row![
                text_input("Paste profile .tnt JSON...", &self.profile_share_draft)
                    .on_input(Message::ProfileShareDraftChanged)
                    .on_submit(Message::ImportProfileStatsTnt(self.profile_share_draft.clone()))
                    .size(11)
                    .padding([5, 8])
                    .width(Length::Fill),
                pill_button("Import", theme, false, Message::ImportProfileStatsTnt(self.profile_share_draft.clone())),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            container(scrollable(top_col).height(Length::Shrink)).max_height(185.0),
        ]
        .spacing(10)
        .padding(14);

        let surface = theme.surface;
        let accent = theme.accent;
        let card = container(content)
            .width(Length::Fixed(420.0))
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

        Some(wrap_dismiss_popover(card.into(), Message::CloseProfilePopover))
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum OverlayHost {
    Home,
    Extension { ext_id: String, tab_id: String },
}

/// Maps `+` / `better-*` extensions onto their base UI icon instead of creating duplicate sidebar buttons.
fn overlay_host_for(ext_id: &str, all_tabs: &[ExtSidebarTab]) -> Option<OverlayHost> {
    let find_tab = |target_ext: &str| {
        all_tabs
            .iter()
            .find(|t| t.ext_id == target_ext)
            .map(|t| OverlayHost::Extension {
                ext_id: t.ext_id.clone(),
                tab_id: t.id.clone(),
            })
    };
    match ext_id {
        "openify.better-home" => Some(OverlayHost::Home),
        "openify.better-lyrics" => find_tab("openify.lyrics"),
        "openify.better-visualizer" => find_tab("openify.visualizer"),
        "openify.better-library" => find_tab("tunotron.playlists"),
        "openify.better-folders" => {
            find_tab("tunotron.playlists").or_else(|| find_tab("openify.better-library"))
        }
        _ => None,
    }
}

fn overlays_for_home(all_tabs: &[ExtSidebarTab]) -> Vec<&ExtSidebarTab> {
    all_tabs
        .iter()
        .filter(|t| overlay_host_for(&t.ext_id, all_tabs) == Some(OverlayHost::Home))
        .collect()
}

fn overlays_for_ext_tab<'a>(
    all_tabs: &'a [ExtSidebarTab],
    base_ext_id: &str,
) -> Vec<&'a ExtSidebarTab> {
    all_tabs
        .iter()
        .filter(|t| {
            matches!(
                overlay_host_for(&t.ext_id, all_tabs),
                Some(OverlayHost::Extension { ext_id, .. }) if ext_id == base_ext_id
            )
        })
        .collect()
}

/// Loads `.png`/`.jpg`/`.webp`, `.jxl` (via `djxl`/`magick`), and `.svg` (via `magick`/`rsvg-convert`) into `image::Handle`.
fn is_ui_hidden(snap: &UiSnapshot) -> bool {
    snap.layout.hide_ui || (snap.transparency.enabled && snap.transparency.hide_ui)
}

fn wrap_dismiss_popover<'a>(
    card: Element<'a, Message>,
    on_dismiss: Message,
) -> Element<'a, Message> {
    let backdrop = mouse_area(container(text("")).width(Length::Fill).height(Length::Fill))
        .on_press(on_dismiss.clone())
        .on_right_press(on_dismiss);
    let inner = mouse_area(card)
        .on_press(Message::Noop)
        .on_right_press(Message::Noop);
    let positioned = container(inner)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(pad4(58.0, 26.0, 0.0, 0.0))
        .align_x(Alignment::End)
        .align_y(Vertical::Top);
    stack([backdrop.into(), positioned.into()]).into()
}

fn decode_any_image_handle(raw: &str) -> Option<image::Handle> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let resolved = if let Some(rest) = trimmed.strip_prefix("~/") {
        let home = std::env::var_os("HOME")?;
        PathBuf::from(home).join(rest)
    } else if trimmed == "~" {
        PathBuf::from(std::env::var_os("HOME")?)
    } else {
        PathBuf::from(trimmed)
    };
    if !resolved.is_file() {
        return None;
    }
    let ext = resolved
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    if ext == "jxl" {
        let tmp = std::env::temp_dir().join(format!("tntron_jxl_{}.png", std::process::id()));
        if let Ok(status) = std::process::Command::new("djxl")
            .arg(&resolved)
            .arg(&tmp)
            .status()
        {
            if status.success() {
                let bytes = std::fs::read(&tmp).ok();
                let _ = std::fs::remove_file(&tmp);
                if let Some(b) = bytes {
                    return Some(image::Handle::from_bytes(b));
                }
            }
        }
        let _ = std::fs::remove_file(&tmp);
        if let Ok(out) = std::process::Command::new("magick")
            .arg(&resolved)
            .arg("png:-")
            .output()
        {
            if out.status.success() && !out.stdout.is_empty() {
                return Some(image::Handle::from_bytes(out.stdout));
            }
        }
        return None;
    }

    if ext == "svg" {
        if let Ok(out) = std::process::Command::new("magick")
            .args(["-background", "none"])
            .arg(&resolved)
            .args(["-resize", "256x256", "png:-"])
            .output()
        {
            if out.status.success() && !out.stdout.is_empty() {
                return Some(image::Handle::from_bytes(out.stdout));
            }
        }
        if let Ok(out) = std::process::Command::new("rsvg-convert")
            .args(["-w", "256", "-h", "256"])
            .arg(&resolved)
            .output()
        {
            if out.status.success() && !out.stdout.is_empty() {
                return Some(image::Handle::from_bytes(out.stdout));
            }
        }
        return None;
    }

    let bytes = std::fs::read(&resolved).ok()?;
    Some(image::Handle::from_bytes(bytes))
}

fn fmt_duration_words(sec: u64) -> String {
    let m = sec / 60;
    let h = m / 60;
    if h > 0 {
        format!("{h}h {}m {}s", m % 60, sec % 60)
    } else {
        format!("{m}m {:02}s", sec % 60)
    }
}
