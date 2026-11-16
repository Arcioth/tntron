use std::collections::{HashSet, VecDeque};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use iced::widget::image::Handle as ImageHandle;
use image::codecs::gif::GifDecoder;
use image::imageops::FilterType;
use image::AnimationDecoder;

pub const DEFAULT_GIFS_DIR: &str = "/home/arcioth/Pictures/gifs";

const MAX_CACHED_GIFS: usize = 8;
const MAX_CACHE_BYTES: usize = 72 * 1024 * 1024;
const PRELOAD_QUEUE_TARGET: usize = 3;
const MAX_FRAMES_PER_GIF: usize = 180;
const MAX_GIF_DIM_PX: u32 = 420;
const BASS_SWITCH_COOLDOWN: Duration = Duration::from_millis(220);

#[derive(Debug, Clone)]
pub struct DecodedGifFrame {
    pub handle: ImageHandle,
    pub delay: Duration,
}

#[derive(Debug, Clone)]
pub struct DecodedGif {
    pub path: PathBuf,
    pub name: String,
    pub frames: Vec<DecodedGifFrame>,
    pub total_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct GifVizView<'a> {
    pub handle: Option<&'a ImageHandle>,
    pub name: &'a str,
    pub total_gifs: usize,
    pub switches: u64,
    pub flash: f32,
    pub dir: &'a str,
}

/// Manages scanning `/home/arcioth/Pictures/gifs`, background GIF decoding, bounded LRU caching,
/// frame animation timing, and low-frequency bass-hit random GIF switching.
pub struct GifPlayer {
    dir: PathBuf,
    dir_display: String,
    paths: Vec<PathBuf>,
    shuffle_bag: Vec<usize>,
    rng: u64,
    cache: VecDeque<Arc<DecodedGif>>,
    ready: VecDeque<Arc<DecodedGif>>,
    in_flight: HashSet<PathBuf>,
    failed: HashSet<PathBuf>,
    current: Option<Arc<DecodedGif>>,
    frame_idx: usize,
    frame_elapsed: Duration,
    last_tick: Instant,
    switches: u64,
    last_switch_at: Instant,
    bass_avg: f32,
    prev_bass: f32,
    prev_beat: f32,
    flash_alpha: f32,
    initialized: bool,
    req_tx: mpsc::Sender<PathBuf>,
    res_rx: mpsc::Receiver<(PathBuf, Option<DecodedGif>)>,
}

impl GifPlayer {
    pub fn new() -> Self {
        let dir = resolve_gifs_dir();
        let dir_display = dir.display().to_string();
        let (req_tx, req_rx) = mpsc::channel::<PathBuf>();
        let (res_tx, res_rx) = mpsc::channel::<(PathBuf, Option<DecodedGif>)>();

        let _ = std::thread::Builder::new()
            .name("tntron-gif-decoder".to_string())
            .spawn(move || {
                while let Ok(path) = req_rx.recv() {
                    let decoded = decode_gif(&path);
                    if res_tx.send((path, decoded)).is_err() {
                        break;
                    }
                }
            });

        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15)
            ^ 0x517c_c1b7_2722_0a95;

        Self {
            dir,
            dir_display,
            paths: Vec::new(),
            shuffle_bag: Vec::new(),
            rng: seed.max(1),
            cache: VecDeque::new(),
            ready: VecDeque::new(),
            in_flight: HashSet::new(),
            failed: HashSet::new(),
            current: None,
            frame_idx: 0,
            frame_elapsed: Duration::ZERO,
            last_tick: Instant::now(),
            switches: 0,
            last_switch_at: Instant::now() - Duration::from_secs(5),
            bass_avg: 0.25,
            prev_bass: 0.0,
            prev_beat: 0.0,
            flash_alpha: 0.0,
            initialized: false,
            req_tx,
            res_rx,
        }
    }

    pub fn activate(&mut self) {
        if !self.initialized {
            self.initialized = true;
            self.rescan_dir();
            self.schedule_preload();
        }
    }

    pub fn rescan_dir(&mut self) {
        let mut found = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
                {
                    found.push(path);
                }
            }
        }
        found.sort();
        self.paths = found;
        self.failed.clear();
        self.refill_shuffle_bag();
    }

    /// Advances active GIF animation frames, ingests background-decoded GIFs, and triggers
    /// random GIF switching on sharp low-frequency / sub-bass audio hits.
    pub fn on_frame(
        &mut self,
        active_gif_view: bool,
        playing: bool,
        speed: f64,
        raw_bands: &[f32; 16],
        raw_beat: f32,
    ) {
        if active_gif_view {
            self.activate();
        }

        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.last_tick)
            .min(Duration::from_millis(100));
        self.last_tick = now;

        self.drain_decoded();
        if active_gif_view {
            self.schedule_preload();
        }

        self.flash_alpha = (self.flash_alpha * 0.84).max(0.0);

        if let Some(gif) = &self.current {
            if gif.frames.len() > 1 && (playing || active_gif_view) {
                let tempo = if playing {
                    speed.clamp(0.4, 2.5)
                } else {
                    0.85
                };
                self.frame_elapsed += dt.mul_f64(tempo);
                loop {
                    let delay = gif.frames[self.frame_idx % gif.frames.len()].delay;
                    if self.frame_elapsed < delay {
                        break;
                    }
                    self.frame_elapsed -= delay;
                    self.frame_idx = (self.frame_idx + 1) % gif.frames.len();
                }
            }
        }

        if !playing || !active_gif_view {
            self.prev_bass *= 0.85;
            self.prev_beat *= 0.85;
            return;
        }

        let has_signal = raw_bands.iter().any(|&v| v > 0.002);
        if !has_signal {
            return;
        }

        // Low-frequency energy weighted toward sub-bass & kick bands (55 Hz .. 135 Hz).
        let bass_raw =
            (raw_bands[0] * 1.25 + raw_bands[1] * 1.15 + raw_bands[2] * 0.95 + raw_bands[3] * 0.75)
                / 4.10;
        self.bass_avg = self.bass_avg * 0.88 + bass_raw * 0.12;

        let low_spike = bass_raw > (self.bass_avg * 1.24).max(0.22)
            && (bass_raw - self.prev_bass) > 0.045;
        let flux_kick = raw_beat > 0.30 && (raw_beat - self.prev_beat) > 0.075;

        if self.last_switch_at.elapsed() >= BASS_SWITCH_COOLDOWN && (low_spike || flux_kick) {
            self.switch_to_next_random(false);
        }

        self.prev_bass = bass_raw;
        self.prev_beat = raw_beat;
    }

    pub fn next_random_now(&mut self) {
        self.activate();
        self.switch_to_next_random(true);
    }

    pub fn view(&self) -> GifVizView<'_> {
        let (handle, name) = match &self.current {
            Some(gif) if !gif.frames.is_empty() => (
                Some(&gif.frames[self.frame_idx % gif.frames.len()].handle),
                gif.name.as_str(),
            ),
            _ => (None, ""),
        };
        GifVizView {
            handle,
            name,
            total_gifs: self.paths.len(),
            switches: self.switches,
            flash: self.flash_alpha,
            dir: self.dir_display.as_str(),
        }
    }

    fn drain_decoded(&mut self) {
        while let Ok((path, maybe_gif)) = self.res_rx.try_recv() {
            self.in_flight.remove(&path);
            let Some(decoded) = maybe_gif else {
                self.failed.insert(path);
                continue;
            };
            let arc = Arc::new(decoded);
            self.insert_cache(arc.clone());
            if self.current.is_none() {
                self.current = Some(arc);
                self.frame_idx = 0;
                self.frame_elapsed = Duration::ZERO;
            } else if self.ready.len() < PRELOAD_QUEUE_TARGET + 1 {
                self.ready.push_back(arc);
            }
        }
    }

    fn insert_cache(&mut self, gif: Arc<DecodedGif>) {
        if let Some(pos) = self.cache.iter().position(|g| g.path == gif.path) {
            self.cache.remove(pos);
        }
        self.cache.push_back(gif);
        loop {
            let total_bytes: usize = self.cache.iter().map(|g| g.total_bytes).sum();
            if self.cache.len() <= 2 {
                break;
            }
            if self.cache.len() <= MAX_CACHED_GIFS && total_bytes <= MAX_CACHE_BYTES {
                break;
            }
            self.cache.pop_front();
        }
    }

    fn schedule_preload(&mut self) {
        if self.paths.is_empty() {
            return;
        }
        let target_total = if self.current.is_none() {
            PRELOAD_QUEUE_TARGET + 1
        } else {
            PRELOAD_QUEUE_TARGET
        };
        let mut attempts = 0usize;
        while self.ready.len() + self.in_flight.len() < target_total && attempts < self.paths.len() {
            attempts += 1;
            let Some(path) = self.draw_random_path() else {
                break;
            };
            if self.current.as_ref().is_some_and(|c| c.path == path) && self.paths.len() > 1 {
                continue;
            }
            if self.ready.iter().any(|r| r.path == path) || self.in_flight.contains(&path) {
                continue;
            }
            if let Some(cached) = self.cache.iter().find(|c| c.path == path).cloned() {
                self.ready.push_back(cached);
                continue;
            }
            self.in_flight.insert(path.clone());
            let _ = self.req_tx.send(path);
        }
    }

    fn switch_to_next_random(&mut self, manual: bool) {
        let cur_path = self.current.as_ref().map(|c| c.path.clone());
        let mut next = None;

        while let Some(candidate) = self.ready.pop_front() {
            if cur_path.as_ref().is_none_or(|p| *p != candidate.path) || self.paths.len() <= 1 {
                next = Some(candidate);
                break;
            }
        }

        if next.is_none() && !self.cache.is_empty() {
            let candidates: Vec<Arc<DecodedGif>> = self
                .cache
                .iter()
                .filter(|c| cur_path.as_ref().is_none_or(|p| *p != c.path))
                .cloned()
                .collect();
            if !candidates.is_empty() {
                let idx = (self.next_rand() as usize) % candidates.len();
                next = Some(candidates[idx].clone());
            }
        }

        if let Some(gif) = next {
            self.current = Some(gif);
            self.frame_idx = 0;
            self.frame_elapsed = Duration::ZERO;
            self.switches += 1;
            self.last_switch_at = Instant::now();
            self.flash_alpha = if manual { 0.7 } else { 1.0 };
        }
        self.schedule_preload();
    }

    fn draw_random_path(&mut self) -> Option<PathBuf> {
        if self.paths.is_empty() {
            return None;
        }
        if self.shuffle_bag.is_empty() {
            self.refill_shuffle_bag();
        }
        for _ in 0..self.paths.len() {
            let Some(idx) = self.shuffle_bag.pop() else {
                break;
            };
            if let Some(path) = self.paths.get(idx) {
                if !self.failed.contains(path) {
                    return Some(path.clone());
                }
            }
        }
        None
    }

    fn refill_shuffle_bag(&mut self) {
        self.shuffle_bag = (0..self.paths.len())
            .filter(|idx| !self.failed.contains(&self.paths[*idx]))
            .collect();
        let n = self.shuffle_bag.len();
        for i in (1..n).rev() {
            let j = (self.next_rand() as usize) % (i + 1);
            self.shuffle_bag.swap(i, j);
        }
    }

    fn next_rand(&mut self) -> u64 {
        let mut x = self.rng;
        x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        self.rng = x;
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

fn resolve_gifs_dir() -> PathBuf {
    if let Ok(env_dir) = std::env::var("TNTRON_GIFS_DIR") {
        let p = PathBuf::from(env_dir.trim());
        if p.is_dir() {
            return p;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let sibling = exe_dir.join("gifs");
            if sibling.is_dir() {
                return sibling;
            }
            if let Some(prefix) = exe_dir.parent() {
                let share = prefix.join("share").join("tntron").join("gifs");
                if share.is_dir() {
                    return share;
                }
            }
        }
    }

    if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
        let candidate = PathBuf::from(xdg_data).join("tntron").join("gifs");
        if candidate.is_dir() {
            return candidate;
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let home_path = PathBuf::from(home);
        let local_share = home_path.join(".local").join("share").join("tntron").join("gifs");
        if local_share.is_dir() {
            return local_share;
        }
        let pictures = home_path.join("Pictures").join("gifs");
        if pictures.is_dir() {
            return pictures;
        }
    }

    let repo_assets = PathBuf::from("assets").join("gifs");
    if repo_assets.is_dir() {
        return repo_assets;
    }

    PathBuf::from(DEFAULT_GIFS_DIR)
}

fn decode_gif(path: &Path) -> Option<DecodedGif> {
    let file = File::open(path).ok()?;
    let decoder = GifDecoder::new(BufReader::new(file)).ok()?;
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "gif".to_string());

    let mut frames = Vec::new();
    let mut total_bytes = 0usize;

    for raw in decoder.into_frames().take(MAX_FRAMES_PER_GIF) {
        let Ok(frame) = raw else {
            break;
        };
        let (num_ms, den_ms) = frame.delay().numer_denom_ms();
        let delay_ms = (num_ms / den_ms.max(1)).clamp(20, 400);
        let mut rgba = frame.into_buffer();
        let (w, h) = rgba.dimensions();
        if w == 0 || h == 0 {
            continue;
        }
        let max_dim = w.max(h);
        if max_dim > MAX_GIF_DIM_PX {
            let scale = MAX_GIF_DIM_PX as f32 / max_dim as f32;
            let nw = ((w as f32 * scale).round() as u32).max(1);
            let nh = ((h as f32 * scale).round() as u32).max(1);
            rgba = image::imageops::resize(&rgba, nw, nh, FilterType::Triangle);
        }
        let (fw, fh) = rgba.dimensions();
        let raw_bytes = rgba.into_raw();
        total_bytes += raw_bytes.len();
        frames.push(DecodedGifFrame {
            handle: ImageHandle::from_rgba(fw, fh, raw_bytes),
            delay: Duration::from_millis(u64::from(delay_ms)),
        });
        if total_bytes > 24 * 1024 * 1024 {
            break;
        }
    }

    if frames.is_empty() {
        return None;
    }

    Some(DecodedGif {
        path: path.to_path_buf(),
        name,
        frames,
        total_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::gif::GifEncoder;
    use image::{Delay, Frame, Rgba, RgbaImage};

    fn make_dummy_gif(name: &str) -> Arc<DecodedGif> {
        Arc::new(DecodedGif {
            path: PathBuf::from(format!("/tmp/{name}")),
            name: name.to_string(),
            frames: vec![DecodedGifFrame {
                handle: ImageHandle::from_rgba(1, 1, vec![255, 0, 0, 255]),
                delay: Duration::from_millis(50),
            }],
            total_bytes: 4,
        })
    }

    #[test]
    fn test_bass_hit_switches_gif_and_respects_cooldown() {
        let mut player = GifPlayer::new();
        player.initialized = true;
        player.paths = vec![
            PathBuf::from("/tmp/a.gif"),
            PathBuf::from("/tmp/b.gif"),
            PathBuf::from("/tmp/c.gif"),
        ];
        player.current = Some(make_dummy_gif("a.gif"));
        player.ready.push_back(make_dummy_gif("b.gif"));
        player.ready.push_back(make_dummy_gif("c.gif"));
        player.last_switch_at = Instant::now() - Duration::from_secs(1);

        // Low idle floor does not trigger switch.
        let low = [0.05f32; 16];
        player.on_frame(true, true, 1.0, &low, 0.05);
        assert_eq!(player.switches, 0);

        // Sharp low-frequency sub-bass spike triggers a switch immediately.
        let mut kick = [0.08f32; 16];
        kick[0] = 0.92;
        kick[1] = 0.85;
        kick[2] = 0.72;
        kick[3] = 0.55;
        player.on_frame(true, true, 1.0, &kick, 0.75);
        assert_eq!(player.switches, 1);
        assert_eq!(player.current.as_ref().unwrap().name, "b.gif");

        // Another kick within cooldown (<220ms) should NOT rapid-spam another switch.
        player.prev_bass = 0.05;
        player.on_frame(true, true, 1.0, &kick, 0.85);
        assert_eq!(player.switches, 1);
    }

    #[test]
    fn test_shuffle_bag_visits_all_without_immediate_duplicates() {
        let mut player = GifPlayer::new();
        player.paths = vec![
            PathBuf::from("/tmp/1.gif"),
            PathBuf::from("/tmp/2.gif"),
            PathBuf::from("/tmp/3.gif"),
            PathBuf::from("/tmp/4.gif"),
        ];
        player.refill_shuffle_bag();

        let mut seen = HashSet::new();
        for _ in 0..4 {
            let p = player.draw_random_path().expect("path");
            seen.insert(p);
        }
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn test_decode_multi_frame_gif_file() {
        let tmp = std::env::temp_dir().join("tntron_test_viz_sample.gif");
        {
            let file = File::create(&tmp).expect("create temp gif");
            let mut enc = GifEncoder::new(file);
            let f1 = Frame::from_parts(
                RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255])),
                0,
                0,
                Delay::from_numer_denom_ms(40, 1),
            );
            let f2 = Frame::from_parts(
                RgbaImage::from_pixel(4, 4, Rgba([0, 255, 0, 255])),
                0,
                0,
                Delay::from_numer_denom_ms(60, 1),
            );
            enc.encode_frames([f1, f2]).expect("encode gif");
        }

        let decoded = decode_gif(&tmp).expect("decode temp gif");
        let _ = fs::remove_file(&tmp);

        assert_eq!(decoded.frames.len(), 2);
        assert_eq!(decoded.frames[0].delay, Duration::from_millis(40));
        assert_eq!(decoded.frames[1].delay, Duration::from_millis(60));
    }
}
