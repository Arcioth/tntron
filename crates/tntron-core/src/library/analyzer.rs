use std::f32::consts::TAU;
use std::path::{Path, PathBuf};
use std::process::Command;

const SAMPLE_RATE: f32 = 8000.0;
const HOP_SAMPLES: usize = 320; // 25 frames per second (40ms per frame)
const BANDS: usize = 16;

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

/// Decodes `path` via ffmpeg into 8kHz mono PCM f32le and computes 25fps 16-band
/// Goertzel spectrum frames, beat onset flux, RMS energy, peak dBFS, and BPM.
pub fn analyze_track_blocking(path: &Path) -> Option<TrackAnalysis> {
    let output = Command::new("ffmpeg")
        .args([
            "-v", "error", "-nostdin", "-i",
        ])
        .arg(path)
        .args(["-vn", "-ac", "1", "-ar", "8000", "-f", "f32le", "-"])
        .output()
        .ok()?;

    if !output.status.success() || output.stdout.len() < HOP_SAMPLES * 4 {
        return None;
    }

    let bytes = output.stdout;
    let total_samples = bytes.len() / 4;
    let mut samples = Vec::with_capacity(total_samples);
    let mut peak = 0.0f32;
    let mut sq_sum = 0.0f64;

    for chunk in bytes.chunks_exact(4) {
        let s = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let abs = s.abs();
        if abs > peak {
            peak = abs;
        }
        sq_sum += f64::from(s) * f64::from(s);
        samples.push(s);
    }

    let rms = (sq_sum / samples.len().max(1) as f64).sqrt();
    let energy = (rms * 3.2).clamp(0.0, 1.0);
    let peak_db = if peak > 1e-6 {
        20.0 * f64::from(peak).log10()
    } else {
        -60.0
    };

    let coeffs: [f32; BANDS] = std::array::from_fn(|b| {
        let w = TAU * BAND_FREQS_HZ[b] / SAMPLE_RATE;
        2.0 * w.cos()
    });
    let hann: Vec<f32> = (0..HOP_SAMPLES)
        .map(|n| 0.5 * (1.0 - (TAU * n as f32 / (HOP_SAMPLES - 1) as f32).cos()))
        .collect();

    let frame_count = samples.len() / HOP_SAMPLES;
    let mut raw_frames = Vec::with_capacity(frame_count);
    let mut flux = Vec::with_capacity(frame_count);
    let mut prev_low = 0.0f32;
    let mut max_band = 0.01f32;

    for f in 0..frame_count {
        let slice = &samples[f * HOP_SAMPLES..(f + 1) * HOP_SAMPLES];
        let mut bands = [0.0f32; BANDS];
        let mut low_sum = 0.0f32;

        for b in 0..BANDS {
            let coeff = coeffs[b];
            let mut s1 = 0.0f32;
            let mut s2 = 0.0f32;
            for (n, &sample) in slice.iter().enumerate() {
                let s0 = sample * hann[n] + coeff * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            let power = (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt();
            let log_mag = (1.0 + power * 0.28).ln();
            bands[b] = log_mag;
            if log_mag > max_band {
                max_band = log_mag;
            }
            if b < 5 {
                low_sum += log_mag;
            }
        }
        let onset = (low_sum - prev_low).max(0.0);
        prev_low = prev_low * 0.6 + low_sum * 0.4;
        flux.push(onset);
        raw_frames.push(bands);
    }

    let inv_max = 1.0 / max_band.max(0.1);
    let frames: Vec<[u8; BANDS]> = raw_frames
        .into_iter()
        .map(|arr| {
            let mut u = [0u8; BANDS];
            for i in 0..BANDS {
                let norm = (arr[i] * inv_max).powf(0.85).clamp(0.0, 1.0);
                u[i] = (norm * 255.0) as u8;
            }
            u
        })
        .collect();

    let max_flux = flux.iter().copied().fold(0.01f32, f32::max);
    let beats: Vec<u8> = flux
        .iter()
        .map(|&v| ((v / max_flux).clamp(0.0, 1.0).powf(0.75) * 255.0) as u8)
        .collect();

    let bpm = estimate_bpm(&flux, SAMPLE_RATE / HOP_SAMPLES as f32);

    Some(TrackAnalysis {
        path: path.to_path_buf(),
        bpm,
        energy,
        peak_db: (peak_db * 10.0).round() / 10.0,
        fps: SAMPLE_RATE / HOP_SAMPLES as f32,
        frames,
        beats,
    })
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
