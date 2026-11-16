use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::clock::{AbLoop, DspSettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TrackSort {
    #[default]
    Id,
    Title,
    Duration,
}

impl TrackSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Id => "#",
            Self::Title => "Title",
            Self::Duration => "Duration",
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_glow_style() -> String {
    "bokeh".to_string()
}

fn default_bg_mode() -> String {
    "none".to_string()
}

fn default_bg_fit() -> String {
    "cover".to_string()
}

fn default_bg_alpha() -> f32 {
    0.55
}

fn default_art_mode() -> String {
    "cover".to_string()
}

fn default_ascii_preset() -> String {
    "vinyl".to_string()
}

fn default_list_source() -> String {
    "upcoming".to_string()
}

fn default_list_max() -> usize {
    6
}

fn default_eq_bars_style() -> String {
    "blocks".to_string()
}

fn default_eq_rows() -> usize {
    2
}

fn default_progress_style() -> String {
    "line".to_string()
}

fn default_border_style() -> String {
    "rounded".to_string()
}

fn default_custom_particle_size() -> f32 {
    36.0
}

fn default_custom_particle_count() -> usize {
    18
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReactiveUiConfig {
    pub enabled: bool,
    pub bass_glow: bool,
    pub bass_intensity: f32,
    pub bass_radius: f32,
    pub eq_enabled: bool,
    pub eq_style: String,
    pub eq_sides: String,
    pub eq_width: f32,
    pub eq_opacity: f32,
    pub blur: f32,
    pub particles_enabled: bool,
    pub particle_count: usize,
    pub particle_zoom: f32,
    #[serde(default = "default_true")]
    pub glow_particles_enabled: bool,
    pub glow_particle_intensity: f32,
    pub glow_particle_size: f32,
    #[serde(default = "default_glow_style")]
    pub glow_particle_style: String,
    pub aurora_enabled: bool,
    pub custom_particle_enabled: bool,
    pub custom_particle_path: Option<String>,
    #[serde(default = "default_custom_particle_size")]
    pub custom_particle_size: f32,
    #[serde(default = "default_custom_particle_count")]
    pub custom_particle_count: usize,
    pub hue_loop: bool,
    pub hue_speed: f32,
    pub hue_vocal_react: f32,
    pub hue_saturation: f32,
    pub bg_dim: f32,
}

impl Default for ReactiveUiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bass_glow: true,
            bass_intensity: 0.90,
            bass_radius: 0.85,
            eq_enabled: true,
            eq_style: "bars".to_string(),
            eq_sides: "both".to_string(),
            eq_width: 80.0,
            eq_opacity: 0.62,
            blur: 0.48,
            particles_enabled: true,
            particle_count: 52,
            particle_zoom: 1.35,
            glow_particles_enabled: true,
            glow_particle_intensity: 0.95,
            glow_particle_size: 1.15,
            glow_particle_style: "bokeh".to_string(),
            aurora_enabled: false,
            custom_particle_enabled: false,
            custom_particle_path: None,
            custom_particle_size: 36.0,
            custom_particle_count: 18,
            hue_loop: true,
            hue_speed: 0.45,
            hue_vocal_react: 1.40,
            hue_saturation: 0.85,
            bg_dim: 0.72,
        }
    }
}

impl ReactiveUiConfig {
    pub fn clamped(mut self) -> Self {
        self.bass_intensity = self.bass_intensity.clamp(0.0, 2.0);
        self.bass_radius = self.bass_radius.clamp(0.15, 2.0);
        self.eq_width = self.eq_width.clamp(16.0, 180.0);
        self.eq_opacity = self.eq_opacity.clamp(0.05, 1.0);
        self.blur = self.blur.clamp(0.0, 1.0);
        self.particle_count = self.particle_count.clamp(0, 140);
        self.particle_zoom = self.particle_zoom.clamp(0.1, 3.0);
        self.glow_particle_intensity = self.glow_particle_intensity.clamp(0.0, 2.5);
        self.glow_particle_size = self.glow_particle_size.clamp(0.4, 2.5);
        self.custom_particle_size = self.custom_particle_size.clamp(8.0, 160.0);
        self.custom_particle_count = self.custom_particle_count.clamp(1, 64);
        self.hue_speed = self.hue_speed.clamp(0.0, 2.5);
        self.hue_vocal_react = self.hue_vocal_react.clamp(0.0, 3.0);
        self.hue_saturation = self.hue_saturation.clamp(0.0, 1.0);
        self.bg_dim = self.bg_dim.clamp(0.25, 0.96);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    pub sidebar_side: String,
    pub sidebar_width: f32,
    pub show_banner: bool,
    pub show_recent_strip: bool,
    pub show_col_album: bool,
    pub show_col_duration: bool,
    pub show_telemetry_strip: bool,
    pub show_player_viz: bool,
    pub player_viz_mode: String,
    pub toast_zone: String,
    #[serde(default = "default_true")]
    pub show_topbar_hide_ui: bool,
    pub hide_ui: bool,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            sidebar_side: "left".to_string(),
            sidebar_width: 230.0,
            show_banner: true,
            show_recent_strip: true,
            show_col_album: true,
            show_col_duration: true,
            show_telemetry_strip: false,
            show_player_viz: true,
            player_viz_mode: "bars".to_string(),
            toast_zone: "top-right".to_string(),
            show_topbar_hide_ui: true,
            hide_ui: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeedPreset {
    pub name: String,
    pub speed: f64,
    pub pitch_st: f64,
    pub link_pitch: bool,
    pub preserve_formant: bool,
}

fn default_speed_presets() -> Vec<SpeedPreset> {
    vec![
        SpeedPreset {
            name: "Normal (1.0x)".into(),
            speed: 1.0,
            pitch_st: 0.0,
            link_pitch: false,
            preserve_formant: true,
        },
        SpeedPreset {
            name: "Podcast (1.25x)".into(),
            speed: 1.25,
            pitch_st: 0.0,
            link_pitch: false,
            preserve_formant: true,
        },
        SpeedPreset {
            name: "Nightcore (1.25x +3st)".into(),
            speed: 1.25,
            pitch_st: 3.0,
            link_pitch: true,
            preserve_formant: false,
        },
        SpeedPreset {
            name: "Slowed (0.85x -2st)".into(),
            speed: 0.85,
            pitch_st: -2.0,
            link_pitch: true,
            preserve_formant: false,
        },
        SpeedPreset {
            name: "Vaporwave (0.78x -3st)".into(),
            speed: 0.78,
            pitch_st: -3.0,
            link_pitch: true,
            preserve_formant: false,
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IconPackConfig {
    pub pack: String,
    pub scale: f32,
    pub tint_mode: String,
    pub custom_hex: Option<String>,
    pub overrides: HashMap<String, String>,
    pub font_path: Option<String>,
    pub font_family: Option<String>,
}

impl Default for IconPackConfig {
    fn default() -> Self {
        Self {
            pack: "nerd".to_string(),
            scale: 1.0,
            tint_mode: "theme".to_string(),
            custom_hex: None,
            overrides: HashMap::new(),
            font_path: None,
            font_family: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ButtonPackConfig {
    pub enabled: bool,
    pub preset: String,
    pub apply_pills: bool,
    pub apply_primary: bool,
    pub apply_glyphs: bool,
    pub apply_nav: bool,
    pub apply_rows: bool,
    pub apply_tiles: bool,
    pub round_tl: f32,
    pub round_tr: f32,
    pub round_br: f32,
    pub round_bl: f32,
    pub crop_tl: bool,
    pub crop_tr: bool,
    pub crop_br: bool,
    pub crop_bl: bool,
    pub crop_size: f32,
    pub hue_shift_deg: f32,
    pub hue_strength: f32,
    pub custom_tint_hex: Option<String>,
    pub fill_opacity: f32,
    pub outline_enabled: bool,
    pub outline_width: f32,
    pub outline_hex: Option<String>,
    pub outline_alpha: f32,
    pub inline_glow: f32,
    pub outer_glow: f32,
    pub bevel_emboss: f32,
    pub gradient_enabled: bool,
    pub gradient_strength: f32,
    pub gradient_angle_deg: f32,
    pub gradient_top_hex: Option<String>,
    pub gradient_bottom_hex: Option<String>,
    pub reactive_hover: bool,
    pub hover_hue_shift_deg: f32,
    pub hover_glow_boost: f32,
    pub hover_outline_boost: f32,
    pub hover_brightness: f32,
}

impl Default for ButtonPackConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            preset: "openify".to_string(),
            apply_pills: true,
            apply_primary: true,
            apply_glyphs: true,
            apply_nav: true,
            apply_rows: true,
            apply_tiles: true,
            round_tl: 18.0,
            round_tr: 18.0,
            round_br: 18.0,
            round_bl: 18.0,
            crop_tl: false,
            crop_tr: false,
            crop_br: false,
            crop_bl: false,
            crop_size: 8.0,
            hue_shift_deg: 0.0,
            hue_strength: 0.0,
            custom_tint_hex: None,
            fill_opacity: 1.0,
            outline_enabled: true,
            outline_width: 1.0,
            outline_hex: None,
            outline_alpha: 0.45,
            inline_glow: 0.0,
            outer_glow: 0.0,
            bevel_emboss: 0.0,
            gradient_enabled: false,
            gradient_strength: 0.45,
            gradient_angle_deg: 180.0,
            gradient_top_hex: None,
            gradient_bottom_hex: None,
            reactive_hover: true,
            hover_hue_shift_deg: 0.0,
            hover_glow_boost: 0.55,
            hover_outline_boost: 0.35,
            hover_brightness: 0.14,
        }
    }
}

impl ButtonPackConfig {
    pub fn clamped(mut self) -> Self {
        self.round_tl = self.round_tl.clamp(0.0, 32.0);
        self.round_tr = self.round_tr.clamp(0.0, 32.0);
        self.round_br = self.round_br.clamp(0.0, 32.0);
        self.round_bl = self.round_bl.clamp(0.0, 32.0);
        self.crop_size = self.crop_size.clamp(3.0, 22.0);
        self.hue_shift_deg = self.hue_shift_deg.clamp(-180.0, 180.0);
        self.hue_strength = self.hue_strength.clamp(0.0, 1.0);
        self.fill_opacity = self.fill_opacity.clamp(0.05, 1.0);
        self.outline_width = self.outline_width.clamp(0.5, 4.0);
        self.outline_alpha = self.outline_alpha.clamp(0.0, 1.0);
        self.inline_glow = self.inline_glow.clamp(0.0, 1.5);
        self.outer_glow = self.outer_glow.clamp(0.0, 1.5);
        self.bevel_emboss = self.bevel_emboss.clamp(0.0, 1.0);
        self.gradient_strength = self.gradient_strength.clamp(0.0, 1.0);
        self.gradient_angle_deg = self.gradient_angle_deg.rem_euclid(360.0);
        self.hover_hue_shift_deg = self.hover_hue_shift_deg.clamp(-180.0, 180.0);
        self.hover_glow_boost = self.hover_glow_boost.clamp(0.0, 2.0);
        self.hover_outline_boost = self.hover_outline_boost.clamp(0.0, 1.0);
        self.hover_brightness = self.hover_brightness.clamp(0.0, 0.7);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TransparencyConfig {
    pub enabled: bool,
    pub base_alpha: f32,
    pub surface_alpha: f32,
    pub card_alpha: f32,
    pub player_bar_alpha: f32,
    pub accent_alpha: f32,
    pub text_alpha: f32,
    pub tint_hex: Option<String>,
    #[serde(default = "default_bg_mode")]
    pub bg_mode: String,
    pub bg_image_path: Option<String>,
    #[serde(default = "default_bg_alpha")]
    pub bg_image_alpha: f32,
    #[serde(default = "default_bg_fit")]
    pub bg_fit: String,
    pub tint2_hex: Option<String>,
    pub hide_ui: bool,
    #[serde(default = "default_true")]
    pub keep_player: bool,
    #[serde(default = "default_true")]
    pub keep_effects: bool,
    #[serde(default = "default_true")]
    pub keep_visualizer: bool,
    pub keep_lyrics: bool,
    pub keep_artwork: bool,
    pub keep_topbar: bool,
    pub keep_sidebar: bool,
    pub keep_queue: bool,
}

impl Default for TransparencyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_alpha: 0.78,
            surface_alpha: 0.72,
            card_alpha: 0.68,
            player_bar_alpha: 0.85,
            accent_alpha: 1.0,
            text_alpha: 1.0,
            tint_hex: None,
            bg_mode: "none".to_string(),
            bg_image_path: None,
            bg_image_alpha: 0.55,
            bg_fit: "cover".to_string(),
            tint2_hex: None,
            hide_ui: false,
            keep_player: true,
            keep_effects: true,
            keep_visualizer: true,
            keep_lyrics: false,
            keep_artwork: false,
            keep_topbar: false,
            keep_sidebar: false,
            keep_queue: false,
        }
    }
}

impl TransparencyConfig {
    pub fn clamped(mut self) -> Self {
        self.base_alpha = self.base_alpha.clamp(0.0, 1.0);
        self.surface_alpha = self.surface_alpha.clamp(0.0, 1.0);
        self.card_alpha = self.card_alpha.clamp(0.0, 1.0);
        self.player_bar_alpha = self.player_bar_alpha.clamp(0.0, 1.0);
        self.accent_alpha = self.accent_alpha.clamp(0.1, 1.0);
        self.text_alpha = self.text_alpha.clamp(0.15, 1.0);
        self.bg_image_alpha = self.bg_image_alpha.clamp(0.05, 1.0);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PopupPlayerConfig {
    pub enabled: bool,
    pub terminal: String,
    pub layout: String,
    pub show_picture: bool,
    pub show_lyrics: bool,
    pub show_eq: bool,
    pub reactive_mode: String,
    pub custom_hex: Option<String>,
    pub transparent_bg: bool,
    pub auto_open_on_track: bool,
    pub cols: u16,
    pub rows: u16,
    pub wm_class: String,
    #[serde(default = "default_art_mode")]
    pub art_mode: String,
    #[serde(default = "default_ascii_preset")]
    pub ascii_preset: String,
    #[serde(default = "default_true")]
    pub show_list: bool,
    #[serde(default = "default_list_source")]
    pub list_source: String,
    #[serde(default = "default_list_max")]
    pub list_max_items: usize,
    #[serde(default = "default_eq_bars_style")]
    pub eq_bars_style: String,
    #[serde(default = "default_eq_rows")]
    pub eq_rows: usize,
    #[serde(default = "default_progress_style")]
    pub progress_style: String,
    #[serde(default = "default_border_style")]
    pub border_style: String,
    #[serde(default = "default_true")]
    pub show_album: bool,
    #[serde(default = "default_true")]
    pub show_badges: bool,
    #[serde(default = "default_true")]
    pub show_footer_hints: bool,
    #[serde(default = "default_true")]
    pub auto_resize_layout: bool,
}

impl Default for PopupPlayerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            terminal: "auto".to_string(),
            layout: "full".to_string(),
            show_picture: true,
            show_lyrics: true,
            show_eq: true,
            reactive_mode: "hue_loop".to_string(),
            custom_hex: None,
            transparent_bg: true,
            auto_open_on_track: false,
            cols: 84,
            rows: 24,
            wm_class: "tntron-popup".to_string(),
            art_mode: "cover".to_string(),
            ascii_preset: "vinyl".to_string(),
            show_list: true,
            list_source: "upcoming".to_string(),
            list_max_items: 6,
            eq_bars_style: "blocks".to_string(),
            eq_rows: 2,
            progress_style: "line".to_string(),
            border_style: "rounded".to_string(),
            show_album: true,
            show_badges: true,
            show_footer_hints: true,
            auto_resize_layout: true,
        }
    }
}

impl PopupPlayerConfig {
    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<Self>(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_atomic(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LogoConfig {
    pub enabled: bool,
    /// `"themed"` | `"monochrome"` | `"custom_color"` | `"custom_image"`
    pub mode: String,
    pub custom_hex: Option<String>,
    pub custom_path: Option<String>,
    /// `"clean"` | `"badge"` | `"glow"`
    pub frame_style: String,
    pub size_px: f32,
    pub beat_pulse: bool,
}

impl Default for LogoConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: "themed".to_string(),
            custom_hex: None,
            custom_path: None,
            frame_style: "clean".to_string(),
            size_px: 22.0,
            beat_pulse: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiscordRpcConfig {
    pub enabled: bool,
    pub client_id: String,
    /// `2` = Listening, `0` = Playing, `3` = Watching
    pub activity_type: u8,
    pub show_title: bool,
    pub show_artist: bool,
    pub show_album: bool,
    pub show_timestamps: bool,
    /// `"remaining"` | `"elapsed"` | `"none"`
    pub timestamp_mode: String,
    pub show_paused: bool,
    pub clear_on_pause: bool,
    pub show_codec: bool,
    pub show_bpm: bool,
    pub show_speed_pitch: bool,
    pub show_loop_shuffle: bool,
    pub show_queue_info: bool,
    pub show_idle: bool,
    pub idle_text: String,
    /// `"auto"` | `"title_only"` | `"title_artist"` | `"custom"`
    pub details_mode: String,
    /// `"auto"` | `"artist_album"` | `"telemetry"` | `"custom"`
    pub state_mode: String,
    pub custom_details: String,
    pub custom_state: String,
    pub large_image_key: String,
    pub large_text: String,
    pub small_image_key: String,
    pub small_text: String,
    pub show_button: bool,
    pub button_label: String,
    pub button_url: String,
    pub privacy_mode: bool,
}

impl Default for DiscordRpcConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            client_id: String::new(),
            activity_type: 2,
            show_title: true,
            show_artist: true,
            show_album: true,
            show_timestamps: true,
            timestamp_mode: "remaining".to_string(),
            show_paused: true,
            clear_on_pause: false,
            show_codec: true,
            show_bpm: true,
            show_speed_pitch: true,
            show_loop_shuffle: true,
            show_queue_info: true,
            show_idle: true,
            idle_text: "Browsing library · Idle".to_string(),
            details_mode: "auto".to_string(),
            state_mode: "auto".to_string(),
            custom_details: "{title} · {bpm}".to_string(),
            custom_state: "{artist} · {album} · {codec}".to_string(),
            large_image_key: "https://raw.githubusercontent.com/Arcioth/tntron/main/assets/icons/hicolor/256x256/apps/tntron.png".to_string(),
            large_text: "{album} · {codec}".to_string(),
            small_image_key: String::new(),
            small_text: "{state} · {speed}".to_string(),
            show_button: true,
            button_label: "tntron on GitHub".to_string(),
            button_url: "https://github.com/Arcioth/tntron".to_string(),
            privacy_mode: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProfileTrackStat {
    pub title: String,
    pub artist: String,
    pub count: u64,
}

fn default_profile_name() -> String {
    "Music Lover".to_string()
}

fn default_profile_bio() -> String {
    "tntron listener".to_string()
}

fn default_avatar_size() -> f32 {
    34.0
}

fn default_avatar_shape() -> String {
    "circle".to_string()
}

fn default_avatar_fit() -> String {
    "cover".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileConfig {
    #[serde(default = "default_profile_name")]
    pub name: String,
    #[serde(default = "default_profile_bio")]
    pub bio: String,
    pub avatar_path: Option<String>,
    #[serde(default = "default_avatar_size")]
    pub avatar_size: f32,
    #[serde(default = "default_avatar_shape")]
    pub avatar_shape: String,
    #[serde(default = "default_avatar_fit")]
    pub avatar_fit: String,
    pub stats_total_sec: u64,
    pub stats_total_plays: u64,
    pub stats_track_plays: HashMap<String, ProfileTrackStat>,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            name: default_profile_name(),
            bio: default_profile_bio(),
            avatar_path: None,
            avatar_size: 34.0,
            avatar_shape: "circle".to_string(),
            avatar_fit: "cover".to_string(),
            stats_total_sec: 0,
            stats_total_plays: 0,
            stats_track_plays: HashMap::new(),
        }
    }
}

impl ProfileConfig {
    pub fn clamped(mut self) -> Self {
        self.avatar_size = self.avatar_size.clamp(22.0, 96.0);
        if self.name.trim().is_empty() {
            self.name = default_profile_name();
        }
        self
    }

    pub fn has_stats(&self) -> bool {
        self.stats_total_sec > 0 || self.stats_total_plays > 0 || !self.stats_track_plays.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistentState {
    pub enabled_extensions: Vec<String>,
    pub extension_storage: HashMap<String, HashMap<String, serde_json::Value>>,
    pub active_theme: String,
    pub sort_mode: TrackSort,
    pub view_density: String,
    pub home_layout: String,
    pub layout: LayoutConfig,
    pub reactive_ui: ReactiveUiConfig,
    pub icon_pack: IconPackConfig,
    pub button_pack: ButtonPackConfig,
    pub logo: LogoConfig,
    pub discord_rpc: DiscordRpcConfig,
    pub profile: ProfileConfig,
    pub transparency: TransparencyConfig,
    pub popup_player: PopupPlayerConfig,
    pub dsp: DspSettings,
    pub speed: f64,
    pub pitch_st: f64,
    pub link_pitch: bool,
    pub preserve_formant: bool,
    pub speed_presets: Vec<SpeedPreset>,
    pub favorites: Vec<PathBuf>,
    pub recent_tracks: Vec<PathBuf>,
    pub saved_loops: HashMap<PathBuf, AbLoop>,
}

impl Default for PersistentState {
    fn default() -> Self {
        Self {
            enabled_extensions: Vec::new(),
            extension_storage: HashMap::new(),
            active_theme: "openify".to_string(),
            sort_mode: TrackSort::Id,
            view_density: "Normal".to_string(),
            home_layout: "table".to_string(),
            layout: LayoutConfig::default(),
            reactive_ui: ReactiveUiConfig::default(),
            icon_pack: IconPackConfig::default(),
            button_pack: ButtonPackConfig::default(),
            logo: LogoConfig::default(),
            discord_rpc: DiscordRpcConfig::default(),
            profile: ProfileConfig::default(),
            transparency: TransparencyConfig::default(),
            popup_player: PopupPlayerConfig::default(),
            dsp: DspSettings::default(),
            speed: 1.0,
            pitch_st: 0.0,
            link_pitch: false,
            preserve_formant: true,
            speed_presets: default_speed_presets(),
            favorites: Vec::new(),
            recent_tracks: Vec::new(),
            saved_loops: HashMap::new(),
        }
    }
}

pub fn default_config_dir() -> PathBuf {
    if let Some(cfg) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(cfg).join("tntron")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config").join("tntron")
    } else {
        std::env::temp_dir().join("tntron")
    }
}

pub fn default_extensions_dir() -> PathBuf {
    default_config_dir().join("extensions")
}

pub fn default_tnt_dir() -> PathBuf {
    default_config_dir().join("tnt")
}

pub fn default_popup_config_path() -> PathBuf {
    default_config_dir().join("popup_player.json")
}

/// Resolves a `.tnt` preset name or path safely under `base_dir` (for names) or `$HOME` (for paths).
pub fn resolve_tnt_path(name_or_path: &str, base_dir: &Path) -> Result<PathBuf, String> {
    let trimmed = name_or_path.trim();
    if trimmed.is_empty() {
        return Err("Preset name or path cannot be empty".into());
    }
    let expanded = if let Some(rest) = trimmed.strip_prefix("~/") {
        let home = std::env::var_os("HOME").ok_or_else(|| "HOME not set".to_string())?;
        PathBuf::from(home).join(rest)
    } else if trimmed.starts_with('/') {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let candidate = PathBuf::from(trimmed);
        let in_home = home.as_ref().is_some_and(|h| candidate.starts_with(h));
        let in_tmp = candidate.starts_with(std::env::temp_dir());
        if !in_home && !in_tmp {
            return Err("Export path must stay inside your home directory".into());
        }
        candidate
    } else {
        let clean_name: String = trimmed
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let file = clean_name.trim_start_matches('.');
        if file.is_empty() {
            return Err("Invalid .tnt filename".into());
        }
        base_dir.join(file)
    };

    let with_ext = if expanded
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("tnt"))
    {
        expanded
    } else {
        let mut os = expanded.into_os_string();
        os.push(".tnt");
        PathBuf::from(os)
    };
    Ok(with_ext)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TntFileEnvelope {
    pub tnt_format: String,
    pub name: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundledExtFile {
    pub manifest: String,
    pub main_lua: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullTntBundle {
    pub state: PersistentState,
    #[serde(default)]
    pub extensions: BTreeMap<String, BundledExtFile>,
}

pub fn save_tnt_file_with_format(
    name_or_path: &str,
    tnt_format: &str,
    config: serde_json::Value,
    base_dir: &Path,
) -> Result<PathBuf, String> {
    let target = resolve_tnt_path(name_or_path, base_dir)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed creating folder: {e}"))?;
    }
    let name = target
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("preset")
        .to_string();
    let envelope = TntFileEnvelope {
        tnt_format: if tnt_format.trim().is_empty() {
            "tntron-reactive-v1".to_string()
        } else {
            tnt_format.trim().to_string()
        },
        name,
        config,
    };
    let text = serde_json::to_string_pretty(&envelope)
        .map_err(|e| format!("Failed serializing .tnt: {e}"))?;
    std::fs::write(&target, text).map_err(|e| format!("Failed writing .tnt file: {e}"))?;
    Ok(target)
}

pub fn save_tnt_file(
    name_or_path: &str,
    config: serde_json::Value,
    base_dir: &Path,
) -> Result<PathBuf, String> {
    save_tnt_file_with_format(name_or_path, "tntron-reactive-v1", config, base_dir)
}

pub fn format_tnt_share_json(
    tnt_format: &str,
    name: &str,
    config: serde_json::Value,
) -> Result<String, String> {
    let envelope = TntFileEnvelope {
        tnt_format: tnt_format.to_string(),
        name: name.to_string(),
        config,
    };
    serde_json::to_string_pretty(&envelope).map_err(|e| format!("Failed formatting share JSON: {e}"))
}

pub fn parse_tnt_share_json(raw: &str) -> Result<serde_json::Value, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("Share JSON is empty".into());
    }
    if let Ok(env) = serde_json::from_str::<TntFileEnvelope>(trimmed) {
        return Ok(env.config);
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .map_err(|e| format!("Invalid .tnt / JSON share code: {e}"))
}

pub fn default_full_tnt_export_path() -> PathBuf {
    default_tnt_dir().join("tntron_full_backup.tnt")
}

pub fn collect_installed_extensions(extensions_dir: &Path) -> BTreeMap<String, BundledExtFile> {
    let mut map = BTreeMap::new();
    let Ok(rd) = std::fs::read_dir(extensions_dir) else {
        return map;
    };
    for entry in rd.filter_map(|e| e.ok()) {
        let dir = entry.path();
        let slug = entry.file_name().to_string_lossy().to_string();
        if !dir.is_dir() || slug.starts_with('.') || slug.starts_with('_') {
            continue;
        }
        let manifest_path = dir.join("manifest.json");
        let Ok(manifest) = std::fs::read_to_string(&manifest_path) else {
            continue;
        };
        let main_rel = serde_json::from_str::<serde_json::Value>(&manifest)
            .ok()
            .and_then(|v| v.get("main").and_then(|m| m.as_str()).map(ToString::to_string))
            .unwrap_or_else(|| "main.lua".to_string());
        let main_path = dir.join(&main_rel);
        let Ok(main_lua) = std::fs::read_to_string(&main_path) else {
            continue;
        };
        map.insert(slug, BundledExtFile { manifest, main_lua });
    }
    map
}

pub fn export_full_tnt_bundle(
    name_or_path: &str,
    state: &PersistentState,
    extensions_dir: &Path,
    base_dir: &Path,
) -> Result<PathBuf, String> {
    let bundle = FullTntBundle {
        state: state.clone(),
        extensions: collect_installed_extensions(extensions_dir),
    };
    let val = serde_json::to_value(&bundle)
        .map_err(|e| format!("Failed serializing full bundle: {e}"))?;
    save_tnt_file_with_format(name_or_path, "tntron-bundle-v1", val, base_dir)
}

pub fn import_full_tnt_bundle(
    name_or_path: &str,
    extensions_dir: &Path,
    base_dir: &Path,
) -> Result<(FullTntBundle, usize), String> {
    let val = load_tnt_file(name_or_path, base_dir)?;
    let bundle: FullTntBundle = serde_json::from_value(val)
        .map_err(|e| format!("File is not a full tntron backup .tnt bundle: {e}"))?;
    let mut restored_exts = 0usize;
    if !bundle.extensions.is_empty() {
        std::fs::create_dir_all(extensions_dir)
            .map_err(|e| format!("Failed creating extensions dir: {e}"))?;
        for (slug, files) in &bundle.extensions {
            let safe_slug: String = slug
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                .collect();
            if safe_slug.is_empty() {
                continue;
            }
            let folder = extensions_dir.join(&safe_slug);
            std::fs::create_dir_all(&folder)
                .map_err(|e| format!("Failed creating extension '{safe_slug}': {e}"))?;
            std::fs::write(folder.join("manifest.json"), &files.manifest)
                .map_err(|e| format!("Failed writing manifest for '{safe_slug}': {e}"))?;
            let main_rel = serde_json::from_str::<serde_json::Value>(&files.manifest)
                .ok()
                .and_then(|v| v.get("main").and_then(|m| m.as_str()).map(ToString::to_string))
                .filter(|m| !m.contains("..") && !m.starts_with('/') && !m.starts_with('\\'))
                .unwrap_or_else(|| "main.lua".to_string());
            std::fs::write(folder.join(main_rel), &files.main_lua)
                .map_err(|e| format!("Failed writing script for '{safe_slug}': {e}"))?;
            restored_exts += 1;
        }
    }
    Ok((bundle, restored_exts))
}

pub fn load_tnt_file(name_or_path: &str, base_dir: &Path) -> Result<serde_json::Value, String> {
    let target = resolve_tnt_path(name_or_path, base_dir)?;
    let raw = std::fs::read_to_string(&target)
        .map_err(|e| format!("Could not read {}: {e}", target.display()))?;
    if let Ok(env) = serde_json::from_str::<TntFileEnvelope>(&raw) {
        return Ok(env.config);
    }
    serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|e| format!("Invalid .tnt format in {}: {e}", target.display()))
}

pub fn list_tnt_files(base_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(base_dir) else {
        return out;
    };
    for entry in rd.filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file()
            && p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("tnt"))
        {
            let name = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("preset")
                .to_string();
            out.push((name, p));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

pub fn default_preferences_export_path() -> PathBuf {
    default_config_dir().join("tntron_preferences.json")
}

pub fn default_state_path() -> PathBuf {
    default_config_dir().join("state.json")
}

impl PersistentState {
    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<Self>(&s).ok())
            .unwrap_or_default()
    }

    /// Atomic save using temp file + rename.
    pub fn save_atomic(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let payload = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&tmp, payload)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxDisplayEnv {
    pub session_type: String,
    pub desktop: String,
    pub is_wayland: bool,
    pub is_x11: bool,
    pub is_xfce: bool,
}

impl LinuxDisplayEnv {
    #[allow(clippy::too_many_arguments)]
    pub fn from_env_vars(
        wayland_display: Option<&str>,
        x11_display: Option<&str>,
        xdg_session_type: Option<&str>,
        xdg_current_desktop: Option<&str>,
        desktop_session: Option<&str>,
        hyprland_sig: Option<&str>,
        sway_sock: Option<&str>,
        i3_sock: Option<&str>,
        niri_sock: Option<&str>,
    ) -> Self {
        let wl = wayland_display.is_some_and(|s| !s.trim().is_empty());
        let x11 = x11_display.is_some_and(|s| !s.trim().is_empty());
        let st = xdg_session_type
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_ascii_lowercase)
            .unwrap_or_else(|| {
                if wl {
                    "wayland".to_string()
                } else if x11 {
                    "x11".to_string()
                } else {
                    "unknown".to_string()
                }
            });
        let raw_de = xdg_current_desktop
            .or(desktop_session)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                if hyprland_sig.is_some_and(|s| !s.trim().is_empty()) {
                    "Hyprland".to_string()
                } else if sway_sock.is_some_and(|s| !s.trim().is_empty()) {
                    "Sway".to_string()
                } else if niri_sock.is_some_and(|s| !s.trim().is_empty()) {
                    "Niri".to_string()
                } else if i3_sock.is_some_and(|s| !s.trim().is_empty()) {
                    "i3".to_string()
                } else {
                    "Linux Desktop".to_string()
                }
            });
        let lower_de = raw_de.to_ascii_lowercase();
        let is_xfce = lower_de.contains("xfce");
        let is_wayland = st == "wayland" || wl;
        let is_x11 = st == "x11" || (!is_wayland && x11);
        let session_type = match (is_wayland, x11) {
            (true, true) => "Wayland (XWayland available)".to_string(),
            (true, false) => "Wayland".to_string(),
            (false, true) => "X11".to_string(),
            _ => st,
        };
        Self {
            session_type,
            desktop: raw_de,
            is_wayland,
            is_x11,
            is_xfce,
        }
    }

    pub fn detect() -> Self {
        let get = |k: &str| std::env::var(k).ok();
        Self::from_env_vars(
            get("WAYLAND_DISPLAY").as_deref(),
            get("DISPLAY").as_deref(),
            get("XDG_SESSION_TYPE").as_deref(),
            get("XDG_CURRENT_DESKTOP").as_deref(),
            get("DESKTOP_SESSION").as_deref(),
            get("HYPRLAND_INSTANCE_SIGNATURE").as_deref(),
            get("SWAYSOCK").as_deref(),
            get("I3SOCK").as_deref(),
            get("NIRI_SOCKET").as_deref(),
        )
    }

    pub fn label(&self) -> String {
        format!("{} · {}", self.desktop, self.session_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_state_roundtrip() {
        let dir = std::env::temp_dir().join("tntron_state_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("state.json");

        let mut st = PersistentState {
            active_theme: "blue".into(),
            view_density: "Compact".into(),
            ..Default::default()
        };
        st.enabled_extensions.push("user.sample".into());
        st.save_atomic(&file).unwrap();

        let loaded = PersistentState::load_from(&file);
        assert_eq!(loaded.active_theme, "blue");
        assert_eq!(loaded.view_density, "Compact");
        assert_eq!(loaded.enabled_extensions, vec!["user.sample".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_tnt_save_list_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("tntron_tnt_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = serde_json::json!({
            "enabled": true,
            "eq_style": "mirrored",
            "blur": 0.75,
            "hue_loop": true
        });
        let saved = save_tnt_file("neon_club", cfg.clone(), &dir).expect("save tnt");
        assert!(saved.extension().is_some_and(|e| e == "tnt"));
        let listed = list_tnt_files(&dir);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, "neon_club");
        let loaded = load_tnt_file("neon_club", &dir).expect("load tnt");
        assert_eq!(loaded, cfg);

        let share = format_tnt_share_json("tntron-buttons-v1", "cyber", cfg.clone()).unwrap();
        assert!(share.contains("\"tnt_format\": \"tntron-buttons-v1\""));
        assert_eq!(parse_tnt_share_json(&share).unwrap(), cfg);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_full_tnt_bundle_export_and_import_restores_extensions_and_settings() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("tntron_full_bundle_{unique}"));
        let ext_src = root.join("src_exts");
        let ext_dst = root.join("dst_exts");
        let tnt_dir = root.join("tnt");
        std::fs::create_dir_all(ext_src.join("custom-ext")).unwrap();
        std::fs::write(
            ext_src.join("custom-ext/manifest.json"),
            r#"{"id":"user.custom","name":"Custom","version":"1.0.0","api_version":"1.0","main":"main.lua","permissions":[]}"#,
        )
        .unwrap();
        std::fs::write(ext_src.join("custom-ext/main.lua"), "function on_load() end\n").unwrap();

        let mut st = PersistentState {
            active_theme: "cyber".into(),
            ..Default::default()
        };
        st.button_pack.enabled = true;
        st.button_pack.preset = "cyberpunk".into();
        st.button_pack.crop_tl = true;
        st.enabled_extensions.push("user.custom".into());

        let saved = export_full_tnt_bundle("my_backup", &st, &ext_src, &tnt_dir).unwrap();
        assert!(saved.is_file());

        let (restored, count) = import_full_tnt_bundle("my_backup", &ext_dst, &tnt_dir).unwrap();
        assert_eq!(count, 1);
        assert_eq!(restored.state.active_theme, "cyber");
        assert!(restored.state.button_pack.crop_tl);
        assert!(ext_dst.join("custom-ext/main.lua").is_file());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_linux_de_wm_detection() {
        let xfce = LinuxDisplayEnv::from_env_vars(
            None,
            Some(":0"),
            Some("x11"),
            Some("XFCE"),
            Some("xfce"),
            None,
            None,
            None,
            None,
        );
        assert!(xfce.is_xfce);
        assert!(xfce.is_x11);
        assert!(!xfce.is_wayland);

        let wl_hypr = LinuxDisplayEnv::from_env_vars(
            Some("wayland-1"),
            Some(":0"),
            Some("wayland"),
            Some("Hyprland"),
            None,
            Some("abc"),
            None,
            None,
            None,
        );
        assert!(wl_hypr.is_wayland);
        assert!(wl_hypr.label().contains("Hyprland"));
    }
}
