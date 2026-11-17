use std::f32::consts::TAU;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const SAMPLE_RATE: f32 = 8000.0;
const HOP_SAMPLES: usize = 320; // 25 frames per second (40ms per frame)
const BANDS: usize = 16;
const FRAME_BYTES: usize = HOP_SAMPLES * 4;
const MAX_STREAM_FRAMES: usize = 25 * 60 * 180; // Cap stream analysis at 3 hours (~4.3 MB)
const STREAM_EMIT_INTERVAL: Duration = Duration::from_millis(450);

// Log-spaced center frequencies from sub-bass (55 Hz) to upper presence (3600 Hz).
const BAND_FREQS_HZ: [f32; BANDS] = [
    55.0, 75.0, 100.0, 135.0, 180.0, 240.0, 320.0, 430.0,
    575.0, 770.0, 1030.0, 1380.0, 1850.0, 2400.0, 3000.0, 3600.0,
];

#[derive(Debug, Clone)]
pub struct TrackAnalysis {
    pub path: PathBuf,
    pub bpm: Option<f64>,
    pub energy: f64,
    pub peak_db: f64,
    pub fps: f32,
    pub frames: Vec<[u8; BANDS]>,
    pub beats: Vec<u8>,
}

impl TrackAnalysis {
    pub fn sample_at(&self, sec: f64) -> ([f32; BANDS], f32) {
        let mut out = [0.0f32; BANDS];
        if self.frames.is_empty() || sec < 0.0 {
            return (out, 0.0);
        }
        let pos = (sec as f32) * self.fps;
        let idx0 = (pos.floor() as usize).min(self.frames.len() - 1);
        let idx1 = (idx0 + 1).min(self.frames.len() - 1);
        let frac = (pos - idx0 as f32).clamp(0.0, 1.0);

        let f0 = &self.frames[idx0];
        let f1 = &self.frames[idx1];
        for i in 0..BANDS {
            let a = f32::from(f0[i]) / 255.0;
            let b = f32::from(f1[i]) / 255.0;
            out[i] = a + (b - a) * frac;
        }
        let b0 = f32::from(*self.beats.get(idx0).unwrap_or(&0)) / 255.0;
        let b1 = f32::from(*self.beats.get(idx1).unwrap_or(&0)) / 255.0;
        let beat = b0 + (b1 - b0) * frac;
        (out, beat)
    }
}

struct GoertzelAccumulator {
    path: PathBuf,
    coeffs: [f32; BANDS],
    hann: Vec<f32>,
    raw_frames: Vec<[f32; BANDS]>,
    flux: Vec<f32>,
    prev_low: f32,
    max_band: f32,
    peak: f32,
    sq_sum: f64,
    total_samples: usize,
}

impl GoertzelAccumulator {
    fn new(path: PathBuf) -> Self {
        let coeffs: [f32; BANDS] = std::array::from_fn(|b| {
            let w = TAU * BAND_FREQS_HZ[b] / SAMPLE_RATE;
            2.0 * w.cos()
        });
        let hann: Vec<f32> = (0..HOP_SAMPLES)
            .map(|n| 0.5 * (1.0 - (TAU * n as f32 / (HOP_SAMPLES - 1) as f32).cos()))
            .collect();
        Self {
            path,
            coeffs,
            hann,
            raw_frames: Vec::new(),
            flux: Vec::new(),
            prev_low: 0.0,
            max_band: 0.01,
            peak: 0.0,
            sq_sum: 0.0,
            total_samples: 0,
        }
    }

    fn push_frame_samples(&mut self, samples: &[f32; HOP_SAMPLES]) {
        for &s in samples {
            let abs = s.abs();
            if abs > self.peak {
                self.peak = abs;
            }
            self.sq_sum += f64::from(s) * f64::from(s);
        }
        self.total_samples += HOP_SAMPLES;

        let mut bands = [0.0f32; BANDS];
        let mut low_sum = 0.0f32;
        for (b, slot) in bands.iter_mut().enumerate() {
            let coeff = self.coeffs[b];
            let mut s1 = 0.0f32;
            let mut s2 = 0.0f32;
            for (n, &sample) in samples.iter().enumerate() {
                let s0 = sample * self.hann[n] + coeff * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            let power = (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt();
            let log_mag = (1.0 + power * 0.28).ln();
            *slot = log_mag;
            if log_mag > self.max_band {
                self.max_band = log_mag;
            }
            if b < 5 {
                low_sum += log_mag;
            }
        }
        let onset = (low_sum - self.prev_low).max(0.0);
        self.prev_low = self.prev_low * 0.6 + low_sum * 0.4;
        self.flux.push(onset);
        self.raw_frames.push(bands);
    }

    fn push_frame_bytes(&mut self, bytes: &[u8; FRAME_BYTES]) {
        let samples: [f32; HOP_SAMPLES] = std::array::from_fn(|i| {
            let o = i * 4;
            f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]])
        });
        self.push_frame_samples(&samples);
    }

    fn build_snapshot(&self, include_bpm: bool) -> Option<TrackAnalysis> {
        if self.raw_frames.is_empty() {
            return None;
        }
        let rms = (self.sq_sum / self.total_samples.max(1) as f64).sqrt();
        let energy = (rms * 3.2).clamp(0.0, 1.0);
        let peak_db = if self.peak > 1e-6 {
            20.0 * f64::from(self.peak).log10()
        } else {
            -60.0
        };

        let inv_max = 1.0 / self.max_band.max(0.1);
        let frames: Vec<[u8; BANDS]> = self
            .raw_frames
            .iter()
            .map(|arr| {
                let mut u = [0u8; BANDS];
                for i in 0..BANDS {
                    let norm = (arr[i] * inv_max).powf(0.85).clamp(0.0, 1.0);
                    u[i] = (norm * 255.0) as u8;
                }
                u
            })
            .collect();

        let max_flux = self.flux.iter().copied().fold(0.01f32, f32::max);
        let beats: Vec<u8> = self
            .flux
            .iter()
            .map(|&v| ((v / max_flux).clamp(0.0, 1.0).powf(0.75) * 255.0) as u8)
            .collect();

        let fps = SAMPLE_RATE / HOP_SAMPLES as f32;
        let bpm = if include_bpm {
            estimate_bpm(&self.flux, fps)
        } else {
            None
        };

        Some(TrackAnalysis {
            path: self.path.clone(),
            bpm,
            energy,
            peak_db: (peak_db * 10.0).round() / 10.0,
            fps,
            frames,
            beats,
        })
    }
}

/// Decodes `path` via ffmpeg into 8kHz mono PCM f32le and computes 25fps 16-band
/// Goertzel spectrum frames, beat onset flux, RMS energy, peak dBFS, and BPM.
pub fn analyze_track_blocking(path: &Path) -> Option<TrackAnalysis> {
    let output = Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(path)
        .args(["-vn", "-ac", "1", "-ar", "8000", "-f", "f32le", "-"])
        .output()
        .ok()?;

    if !output.status.success() || output.stdout.len() < FRAME_BYTES {
        return None;
    }

    let mut acc = GoertzelAccumulator::new(path.to_path_buf());
    for chunk in output.stdout.chunks_exact(FRAME_BYTES) {
        let frame_bytes: &[u8; FRAME_BYTES] = chunk.try_into().ok()?;
        acc.push_frame_bytes(frame_bytes);
    }
    acc.build_snapshot(true)
}

/// Progressively decodes an HTTP/HTTPS stream (e.g. YouTube audio) via ffmpeg into 8kHz mono PCM,
/// invoking `on_update` smoothly during playback so reactive visuals respond immediately while
/// streaming, and finishes with BPM estimation at EOF. Stops early if `cancel` is set.
pub fn analyze_stream_progressive<F>(
    stream_url: &str,
    key_path: &Path,
    cancel: &AtomicBool,
    mut on_update: F,
) -> Option<TrackAnalysis>
where
    F: FnMut(TrackAnalysis),
{
    if cancel.load(Ordering::Relaxed) {
        return None;
    }

    let mut target_url = stream_url.to_string();
    let mut attempt = 0;
    loop {
        attempt += 1;
        if let Some(analysis) =
            stream_ffmpeg_frames(&target_url, key_path, cancel, &mut on_update)
        {
            return Some(analysis);
        }
        if cancel.load(Ordering::Relaxed) || attempt >= 2 {
            return None;
        }
        // Fallback: if caller passed a watch page URL instead of direct audio URL, resolve via yt-dlp.
        let resolved = Command::new("yt-dlp")
            .args(["-f", "bestaudio", "-g", "--no-playlist", &target_url])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .map(|l| l.trim().to_string())
            })
            .filter(|s| !s.is_empty());
        match resolved {
            Some(direct) if direct != target_url => target_url = direct,
            _ => return None,
        }
    }
}

fn stream_ffmpeg_frames<F>(
    url: &str,
    key_path: &Path,
    cancel: &AtomicBool,
    on_update: &mut F,
) -> Option<TrackAnalysis>
where
    F: FnMut(TrackAnalysis),
{
    let mut child = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-nostdin",
            "-reconnect",
            "1",
            "-reconnect_streamed",
            "1",
            "-reconnect_delay_max",
            "2",
            "-i",
            url,
            "-vn",
            "-ac",
            "1",
            "-ar",
            "8000",
            "-f",
            "f32le",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let mut stdout = child.stdout.take()?;
    let mut acc = GoertzelAccumulator::new(key_path.to_path_buf());
    let mut frame_buf = [0u8; FRAME_BYTES];
    let mut filled = 0usize;
    let mut last_emit = Instant::now();
    let mut emitted_initial = false;
    let mut read_buf = [0u8; 4096];

    loop {
        if cancel.load(Ordering::Relaxed) || acc.raw_frames.len() >= MAX_STREAM_FRAMES {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        match stdout.read(&mut read_buf) {
            Ok(0) => break,
            Ok(n) => {
                let mut offset = 0;
                while offset < n {
                    let take = (FRAME_BYTES - filled).min(n - offset);
                    frame_buf[filled..filled + take]
                        .copy_from_slice(&read_buf[offset..offset + take]);
                    filled += take;
                    offset += take;
                    if filled == FRAME_BYTES {
                        acc.push_frame_bytes(&frame_buf);
                        filled = 0;

                        let should_emit = (!emitted_initial && acc.raw_frames.len() >= 10)
                            || (emitted_initial && last_emit.elapsed() >= STREAM_EMIT_INTERVAL);
                        if should_emit {
                            if let Some(snap) = acc.build_snapshot(false) {
                                on_update(snap);
                                emitted_initial = true;
                                last_emit = Instant::now();
                            }
                        }
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }

    let _ = child.wait();
    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    let final_snap = acc.build_snapshot(true)?;
    on_update(final_snap.clone());
    Some(final_snap)
}

fn estimate_bpm(flux: &[f32], fps: f32) -> Option<f64> {
    if flux.len() < (fps as usize) * 8 {
        return None;
    }
    let min_bpm = 65usize;
    let max_bpm = 185usize;
    let mut best_bpm = None;
    let mut best_score = 0.0f64;

    for bpm in min_bpm..=max_bpm {
        let lag_f = (60.0 * f64::from(fps)) / (bpm as f64);
        let lag = lag_f.round() as usize;
        if lag == 0 || lag >= flux.len() / 2 {
            continue;
        }
        let mut acc = 0.0f64;
        let len = flux.len() - lag;
        for i in 0..len {
            acc += f64::from(flux[i]) * f64::from(flux[i + lag]);
        }
        let norm = acc / len as f64;
        if norm > best_score {
            best_score = norm;
            best_bpm = Some(bpm as f64);
        }
    }

    best_bpm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goertzel_accumulator_incremental_sine_response() {
        let mut acc = GoertzelAccumulator::new(PathBuf::from("https://example.com/stream"));
        // 100 Hz sine falls cleanly on band index 2 (100.0 Hz)
        let frame: [f32; HOP_SAMPLES] =
            std::array::from_fn(|n| (TAU * 100.0 * (n as f32) / SAMPLE_RATE).sin() * 0.8);
        for _ in 0..12 {
            acc.push_frame_samples(&frame);
        }
        let partial = acc.build_snapshot(false).expect("partial snapshot");
        assert_eq!(partial.frames.len(), 12);
        assert!(partial.energy > 0.5);
        assert!(partial.bpm.is_none());

        let (bands, _beat) = partial.sample_at(0.2);
        assert!(
            bands[2] > bands[15],
            "100Hz band {} should exceed 3600Hz band {}",
            bands[2],
            bands[15]
        );
        assert!(bands[2] > 0.7);
    }
}
