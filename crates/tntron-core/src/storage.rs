use std::collections::HashMap;
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
}
