use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlayState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LoopMode {
    Off,
    Track,
    All,
}

impl LoopMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Track,
            Self::Track => Self::All,
            Self::All => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Track => "Track",
            Self::All => "All",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShuffleMode {
    Off,
    On,
}

impl ShuffleMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Off => Self::On,
            Self::On => Self::Off,
        }
    }
}

/// Built-in mpv audio filter presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioPreset {
    Flat,
    BassBoost,
    Vocal,
    Warm,
    Rock,
    Spatial8D,
    Custom,
}

impl AudioPreset {
    pub const PRESETS: [Self; 6] = [
        Self::Flat,
        Self::BassBoost,
        Self::Vocal,
        Self::Warm,
        Self::Rock,
        Self::Spatial8D,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Flat => "Flat",
            Self::BassBoost => "Bass Boost",
            Self::Vocal => "Vocal",
            Self::Warm => "Warm",
            Self::Rock => "Rock",
            Self::Spatial8D => "8D Spatial",
            Self::Custom => "Custom",
        }
    }
}

/// Live tactile DSP parameters (3-band EQ + 8D binaural orbit) mapped directly to mpv `af`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DspSettings {
    pub preset: AudioPreset,
    pub bass_db: i8,
    pub mid_db: i8,
    pub treble_db: i8,
    pub spatial_on: bool,
    pub spatial_hz: f32,
}

impl Default for DspSettings {
    fn default() -> Self {
        Self {
            preset: AudioPreset::Flat,
            bass_db: 0,
            mid_db: 0,
            treble_db: 0,
            spatial_on: false,
            spatial_hz: 0.08,
        }
    }
}

impl DspSettings {
    pub fn apply_preset(&mut self, preset: AudioPreset) {
        self.preset = preset;
        match preset {
            AudioPreset::Flat => {
                self.bass_db = 0;
                self.mid_db = 0;
                self.treble_db = 0;
                self.spatial_on = false;
            }
            AudioPreset::BassBoost => {
                self.bass_db = 6;
                self.mid_db = 0;
                self.treble_db = 1;
                self.spatial_on = false;
            }
            AudioPreset::Vocal => {
                self.bass_db = -1;
                self.mid_db = 4;
                self.treble_db = 2;
                self.spatial_on = false;
            }
            AudioPreset::Warm => {
                self.bass_db = 3;
                self.mid_db = 1;
                self.treble_db = -2;
                self.spatial_on = false;
            }
            AudioPreset::Rock => {
                self.bass_db = 5;
                self.mid_db = -1;
                self.treble_db = 4;
                self.spatial_on = false;
            }
            AudioPreset::Spatial8D => {
                self.bass_db = 2;
                self.mid_db = 0;
                self.treble_db = 1;
                self.spatial_on = true;
                self.spatial_hz = 0.08;
            }
            AudioPreset::Custom => {}
        }
    }

    pub fn step_bass(&mut self, delta: i8) {
        self.bass_db = (self.bass_db + delta).clamp(-12, 12);
        self.preset = AudioPreset::Custom;
    }

    pub fn step_mid(&mut self, delta: i8) {
        self.mid_db = (self.mid_db + delta).clamp(-12, 12);
        self.preset = AudioPreset::Custom;
    }

    pub fn step_treble(&mut self, delta: i8) {
        self.treble_db = (self.treble_db + delta).clamp(-12, 12);
        self.preset = AudioPreset::Custom;
    }

    pub fn toggle_spatial(&mut self) {
        self.spatial_on = !self.spatial_on;
        self.preset = AudioPreset::Custom;
    }

    pub fn step_spatial_hz(&mut self, delta: f32) {
        self.spatial_hz = ((self.spatial_hz + delta) * 100.0).round() / 100.0;
        self.spatial_hz = self.spatial_hz.clamp(0.02, 0.40);
        self.spatial_on = true;
        self.preset = AudioPreset::Custom;
    }

    pub fn build_mpv_filter(&self) -> String {
        let mut nodes = Vec::new();
        if self.bass_db != 0 {
            nodes.push(format!(
                "equalizer=f=90:width_type=o:width=2:g={}",
                self.bass_db
            ));
        }
        if self.mid_db != 0 {
            nodes.push(format!(
                "equalizer=f=1200:width_type=o:width=2:g={}",
                self.mid_db
            ));
        }
        if self.treble_db != 0 {
            nodes.push(format!(
                "equalizer=f=7500:width_type=o:width=2:g={}",
                self.treble_db
            ));
        }
        if self.spatial_on {
            nodes.push(format!(
                "apulsator=mode=sine:hz={:.2}:amount=0.85,stereowiden=delay=20:feedback=0.3,aecho=0.8:0.88:40:0.3",
                self.spatial_hz
            ));
        }
        if nodes.is_empty() {
            String::new()
        } else {
            format!("lavfi=[{}]", nodes.join(","))
        }
    }
}

/// Millisecond-precision A-B section loop state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct AbLoop {
    pub a_sec: Option<f64>,
    pub b_sec: Option<f64>,
}

impl AbLoop {
    pub fn is_active(&self) -> bool {
        matches!((self.a_sec, self.b_sec), (Some(a), Some(b)) if b > a + 0.2)
    }

    pub fn clear(&mut self) {
        self.a_sec = None;
        self.b_sec = None;
    }
}

/// Monotonic playback clock that interpolates track time locally to minimize mpv IPC traffic.
#[derive(Debug, Clone)]
pub struct PlaybackClock {
    anchor_pos: f64,
    anchor_at: Instant,
    playing: bool,
    speed: f64,
}

impl PlaybackClock {
    pub fn new() -> Self {
        Self {
            anchor_pos: 0.0,
            anchor_at: Instant::now(),
            playing: false,
            speed: 1.0,
        }
    }

    pub fn now(&self) -> f64 {
        if self.playing {
            self.anchor_pos + self.anchor_at.elapsed().as_secs_f64() * self.speed
        } else {
            self.anchor_pos
        }
    }

    pub fn sync(&mut self, pos: f64) {
        self.anchor_pos = pos.max(0.0);
        self.anchor_at = Instant::now();
    }

    pub fn set_playing(&mut self, playing: bool) {
        self.anchor_pos = self.now();
        self.anchor_at = Instant::now();
        self.playing = playing;
    }

    pub fn set_speed(&mut self, speed: f64) {
        self.anchor_pos = self.now();
        self.anchor_at = Instant::now();
        self.speed = speed.clamp(0.25, 3.0);
    }
}

impl Default for PlaybackClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Fast deterministic PRNG for Fisher-Yates shuffle decks.
#[derive(Debug, Clone)]
pub struct XorShift64(pub u64);

impl XorShift64 {
    pub fn from_entropy() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self(nanos | 1)
    }

    pub fn next_bound(&mut self, max: usize) -> usize {
        if max <= 1 {
            return 0;
        }
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x as usize) % max
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_mode_cycle() {
        assert_eq!(LoopMode::Off.next(), LoopMode::Track);
        assert_eq!(LoopMode::Track.next(), LoopMode::All);
        assert_eq!(LoopMode::All.next(), LoopMode::Off);
    }

    #[test]
    fn test_dsp_filter_builder() {
        let mut dsp = DspSettings::default();
        assert_eq!(dsp.build_mpv_filter(), "");
        dsp.apply_preset(AudioPreset::BassBoost);
        let f = dsp.build_mpv_filter();
        assert!(f.starts_with("lavfi=["));
        assert!(f.contains("equalizer=f=90"));
        dsp.toggle_spatial();
        assert!(dsp.build_mpv_filter().contains("apulsator"));
    }

    #[test]
    fn test_xorshift_bounds() {
        let mut rng = XorShift64(42);
        for _ in 0..100 {
            assert!(rng.next_bound(7) < 7);
        }
        assert_eq!(rng.next_bound(1), 0);
        assert_eq!(rng.next_bound(0), 0);
    }
}
