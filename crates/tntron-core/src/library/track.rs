use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackId(pub u64);

impl TrackId {
    pub fn from_path(path: &Path) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut hasher);
        Self(hasher.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub path: PathBuf,
    pub filename: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_sec: f64,
    pub duration_label: String,
    pub track_number: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub channels: Option<u8>,
    pub bpm: Option<f64>,
}

impl Track {
    pub fn new(path: PathBuf) -> Self {
        let id = TrackId::from_path(&path);
        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| filename.clone());

        Self {
            id,
            path,
            filename,
            title,
            artist: "Unknown Artist".to_string(),
            album: "Unknown Album".to_string(),
            duration_sec: 0.0,
            duration_label: "00:00".to_string(),
            track_number: None,
            sample_rate_hz: None,
            bitrate_kbps: None,
            channels: None,
            bpm: None,
        }
    }

    pub fn set_duration(&mut self, sec: f64) {
        self.duration_sec = sec.max(0.0);
        self.duration_label = format_mmss(self.duration_sec);
    }

    pub fn telemetry_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(sr) = self.sample_rate_hz {
            parts.push(format!("{:.1} kHz", f64::from(sr) / 1000.0));
        }
        if let Some(ch) = self.channels {
            parts.push(format!("{ch} ch"));
        }
        if let Some(br) = self.bitrate_kbps {
            parts.push(format!("{br} kbps"));
        }
        if let Some(ext) = self.path.extension().and_then(|e| e.to_str()) {
            parts.push(ext.to_ascii_uppercase());
        }
        if parts.is_empty() {
            "Audio".to_string()
        } else {
            parts.join(" · ")
        }
    }

    pub fn is_audio_file(path: &Path) -> bool {
        const EXTS: &[&str] = &[
            "mp3", "flac", "ogg", "opus", "m4a", "aac", "wav", "wma", "alac", "aiff",
        ];
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| EXTS.iter().any(|k| ext.eq_ignore_ascii_case(k)))
    }
}

pub fn format_mmss(seconds: f64) -> String {
    let total_sec = seconds.max(0.0).round() as u64;
    format!("{:02}:{:02}", total_sec / 60, total_sec % 60)
}
