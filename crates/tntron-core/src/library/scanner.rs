use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

use crate::library::track::Track;

#[derive(Debug, Clone)]
pub struct MetadataPatch {
    pub path: PathBuf,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_sec: f64,
    pub track_number: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub channels: Option<u8>,
    pub bpm: Option<f64>,
}

pub struct Scanner;

impl Scanner {
    pub fn scan_paths_in_background(paths: Vec<PathBuf>, tx: mpsc::Sender<Vec<MetadataPatch>>) {
        if paths.is_empty() {
            return;
        }
        tokio::task::spawn_blocking(move || {
            let mut batch = Vec::with_capacity(20);
            for path in paths {
                if let Some(patch) = Self::read_tags_blocking(&path) {
                    batch.push(patch);
                    if batch.len() >= 20 {
                        let _ = tx.blocking_send(std::mem::take(&mut batch));
                    }
                }
            }
            if !batch.is_empty() {
                let _ = tx.blocking_send(batch);
            }
        });
    }

    pub fn read_tags_blocking(path: &Path) -> Option<MetadataPatch> {
        let tagged_file = Probe::open(path).ok()?.read().ok()?;
        let props = tagged_file.properties();
        let duration_sec = props.duration().as_secs_f64();
        let sample_rate_hz = props.sample_rate();
        let bitrate_kbps = props.audio_bitrate();
        let channels = props.channels();

        let mut title = None;
        let mut artist = None;
        let mut album = None;
        let mut track_number = None;
        let mut bpm = None;

        if let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) {
            title = tag
                .title()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            artist = tag
                .artist()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            album = tag
                .album()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            track_number = tag.track();
            bpm = tag
                .get_string(&ItemKey::Bpm)
                .and_then(|s| s.trim().parse::<f64>().ok())
                .filter(|&v| (30.0..=260.0).contains(&v));
        }

        Some(MetadataPatch {
            path: path.to_path_buf(),
            title,
            artist,
            album,
            duration_sec,
            track_number,
            sample_rate_hz,
            bitrate_kbps,
            channels,
            bpm,
        })
    }

    /// Converts raw JPEG XL bytes (if JXL magic header is detected) into PNG bytes using `djxl` or `magick`.
    pub fn normalize_image_bytes_blocking(bytes: Vec<u8>) -> Option<Vec<u8>> {
        let is_jxl = bytes.starts_with(&[0xFF, 0x0A])
            || bytes.starts_with(b"\x00\x00\x00\x0CJXL ");
        if !is_jxl {
            return Some(bytes);
        }

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_in = std::env::temp_dir().join(format!(
            "tntron_jxl_in_{}_{unique}.jxl",
            std::process::id()
        ));
        let tmp_out = std::env::temp_dir().join(format!(
            "tntron_jxl_out_{}_{unique}.png",
            std::process::id()
        ));
        if std::fs::write(&tmp_in, &bytes).is_ok() {
            let mut djxl_bins = vec![std::path::PathBuf::from("djxl")];
            if let Ok(home) = std::env::var("HOME") {
                djxl_bins.push(std::path::PathBuf::from(home).join(".local/bin/djxl"));
            }
            for bin in djxl_bins {
                if let Ok(st) = std::process::Command::new(bin)
                    .arg(&tmp_in)
                    .arg(&tmp_out)
                    .output()
                {
                    if st.status.success() {
                        if let Ok(png) = std::fs::read(&tmp_out) {
                            let _ = std::fs::remove_file(&tmp_in);
                            let _ = std::fs::remove_file(&tmp_out);
                            if !png.is_empty() {
                                return Some(png);
                            }
                        }
                    }
                }
            }
            if let Ok(out) = std::process::Command::new("magick")
                .arg(&tmp_in)
                .arg("png:-")
                .output()
            {
                let _ = std::fs::remove_file(&tmp_in);
                let _ = std::fs::remove_file(&tmp_out);
                if out.status.success() && !out.stdout.is_empty() {
                    return Some(out.stdout);
                }
            }
        }
        let _ = std::fs::remove_file(&tmp_in);
        let _ = std::fs::remove_file(&tmp_out);
        None
    }

    /// Extracts embedded front-cover (or first available) image bytes for the GUI player bar.
    pub fn extract_cover_art_blocking(path: &Path) -> Option<Vec<u8>> {
        let tagged_file = Probe::open(path).ok()?.read().ok()?;
        let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag())?;
        let pic = tag.pictures().first()?;
        Self::normalize_image_bytes_blocking(pic.data().to_vec())
    }

    /// Finds an image file inside `folder` (prioritizing cover/folder/thumb names), returning its bytes.
    pub fn find_folder_image_blocking(folder: &Path) -> Option<Vec<u8>> {
        const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif", "jxl"];
        const PREFERRED_STEMS: &[&str] = &[
            "cover", "folder", "front", "album", "art", "artwork", "thumb", "thumbnail", "poster",
        ];
        const MAX_IMAGE_BYTES: u64 = 16 * 1024 * 1024;

        let rd = std::fs::read_dir(folder).ok()?;
        let mut candidates: Vec<(u8, String, std::path::PathBuf)> = Vec::new();
        for entry in rd.filter_map(|e| e.ok()) {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if !ft.is_file() {
                continue;
            }
            let p = entry.path();
            let Some(ext) = p.extension().and_then(|s| s.to_str()) else {
                continue;
            };
            if !IMAGE_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
                continue;
            }
            if entry.metadata().is_ok_and(|m| m.len() > MAX_IMAGE_BYTES) {
                continue;
            }
            let stem_lower = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let name_lower = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let rank = if PREFERRED_STEMS.iter().any(|s| *s == stem_lower) {
                0
            } else {
                1
            };
            candidates.push((rank, name_lower, p));
        }

        candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        for (_, _, img_path) in candidates {
            if let Ok(bytes) = std::fs::read(&img_path) {
                if !bytes.is_empty() {
                    if let Some(normalized) = Self::normalize_image_bytes_blocking(bytes) {
                        return Some(normalized);
                    }
                }
            }
        }
        None
    }

    /// Counts audio tracks inside `folder` and returns a folder image if present,
    /// falling back to embedded cover art from the first track that has one.
    pub fn scan_folder_summary_blocking(folder: &Path) -> (usize, Option<Vec<u8>>) {
        let mut count = 0usize;
        let mut cover = Self::find_folder_image_blocking(folder);
        if let Ok(rd) = std::fs::read_dir(folder) {
            for entry in rd.filter_map(|e| e.ok()) {
                let Ok(ft) = entry.file_type() else {
                    continue;
                };
                if ft.is_file() && Track::is_audio_file(&entry.path()) {
                    count += 1;
                    if cover.is_none() {
                        cover = Self::extract_cover_art_blocking(&entry.path());
                    }
                }
            }
        }
        (count, cover)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_folder_image_prefers_cover_and_picks_any_image() {
        let dir = std::env::temp_dir().join(format!(
            "tntron_folder_img_test_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("zebra.png"), b"PNGDATA").unwrap();
        assert_eq!(
            Scanner::find_folder_image_blocking(&dir).as_deref(),
            Some(b"PNGDATA".as_slice())
        );

        std::fs::write(dir.join("cover.jpg"), b"COVERJPG").unwrap();
        assert_eq!(
            Scanner::find_folder_image_blocking(&dir).as_deref(),
            Some(b"COVERJPG".as_slice())
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
