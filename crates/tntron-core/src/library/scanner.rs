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

    /// Extracts embedded front-cover (or first available) image bytes for the GUI player bar.
    pub fn extract_cover_art_blocking(path: &Path) -> Option<Vec<u8>> {
        let tagged_file = Probe::open(path).ok()?.read().ok()?;
        let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag())?;
        let pic = tag.pictures().first()?;
        Some(pic.data().to_vec())
    }

    /// Counts audio tracks inside `folder` and returns cover art from the first track that has one.
    pub fn scan_folder_summary_blocking(folder: &Path) -> (usize, Option<Vec<u8>>) {
        let mut count = 0usize;
        let mut cover = None;
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
