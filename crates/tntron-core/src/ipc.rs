use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, mpsc};

use crate::clock::PlayState;
use crate::engine::{CoreCommand, CoreState};
use crate::library::format_mmss;

pub fn default_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("tntron.sock")
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/tntron-{uid}.sock"))
    }
}

pub fn default_status_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("tntron-status.json")
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/tntron-status-{uid}.json"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TntronStatus {
    pub state: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub speed: f64,
    pub bpm: Option<f64>,
    pub lyric: String,
    pub loop_mode: String,
    pub shuffle: bool,
    pub dsp_preset: String,
    pub spatial_on: bool,
    pub sleep_remaining_sec: Option<u64>,
    pub text: String,
    pub alt: String,
    pub tooltip: String,
    pub class: String,
    pub percentage: u8,
}

impl TntronStatus {
    pub fn from_state(state: &CoreState) -> Self {
        let state_str = match state.play_state {
            PlayState::Playing => "playing",
            PlayState::Paused => "paused",
            PlayState::Stopped => "stopped",
        };
        let (title, artist, album) = if let Some(track) = &state.current_track {
            (track.title.clone(), track.artist.clone(), track.album.clone())
        } else {
            ("No Track".into(), String::new(), String::new())
        };
        let pos = state.clock.now();
        let dur = state.duration_sec;
        let percentage = if dur > 0.0 {
            ((pos / dur) * 100.0).clamp(0.0, 100.0) as u8
        } else {
            0
        };
        let lyric = state.current_lyric_line().unwrap_or_default();
        let bpm = state.current_track_bpm();
        let base_label = if state.current_track.is_some() {
            if artist.is_empty() || artist == "Unknown Artist" {
                title.clone()
            } else {
                format!("{artist} — {title}")
            }
        } else {
            "tntron stopped".into()
        };
        let text = if lyric.is_empty() || state.current_track.is_none() {
            base_label.clone()
        } else {
            let short_lyric: String = lyric.chars().take(44).collect();
            format!("{base_label} | {short_lyric}")
        };
        let time_label = format!("{} / {}", format_mmss(pos), format_mmss(dur));
        let tooltip = if state.current_track.is_some() {
            let mut lines = vec![base_label];
            if !lyric.is_empty() {
                lines.push(format!("Lyric: {lyric}"));
            }
            lines.push(format!("Album: {album}"));
            let bpm_part = bpm.map(|b| format!(" | {b:.0} BPM")).unwrap_or_default();
            lines.push(format!("[{time_label}] ({:.2}x{bpm_part})", state.speed));
            lines.join("\n")
        } else {
            "No active track".into()
        };

        Self {
            state: state_str.into(),
            title,
            artist,
            album,
            position: (pos * 10.0).round() / 10.0,
            duration: (dur * 10.0).round() / 10.0,
            volume: state.volume,
            speed: state.speed,
            bpm,
            lyric,
            loop_mode: state.loop_mode.label().into(),
            shuffle: state.shuffle_mode == crate::clock::ShuffleMode::On,
            dsp_preset: state.persisted.dsp.preset.label().into(),
            spatial_on: state.persisted.dsp.spatial_on,
            sleep_remaining_sec: state.sleep_remaining_sec(),
            text,
            alt: state_str.into(),
            tooltip,
            class: state_str.into(),
            percentage,
        }
    }
}

pub fn write_status_file(status: &TntronStatus) {
    let path = default_status_path();
    let tmp = path.with_extension("json.tmp");
    if let Ok(json) = serde_json::to_string(status) {
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcCommand {
    Play,
    Pause,
    Toggle,
    Next,
    Prev,
    Stop,
    Volume { delta: Option<i8>, value: Option<f64> },
    Seek { delta: Option<i64>, value: Option<f64> },
    Speed { value: f64 },
    Mute { value: Option<bool> },
    DspPreset { preset: String },
    ToggleSpatial,
    Sleep { minutes: Option<u64> },
    CycleLoop,
    Shuffle,
    Status,
    Subscribe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<TntronStatus>,
}

pub async fn run_ipc_server(
    cmd_tx: mpsc::Sender<CoreCommand>,
    status_rx: tokio::sync::watch::Receiver<TntronStatus>,
    broadcast_tx: broadcast::Sender<TntronStatus>,
) -> anyhow::Result<()> {
    let socket_path = default_socket_path();
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)?;

    loop {
        let (stream, _) = listener.accept().await?;
        let cmd_tx = cmd_tx.clone();
        let status_rx = status_rx.clone();
        let sub_rx = broadcast_tx.subscribe();
        tokio::spawn(async move {
            let _ = handle_client(stream, cmd_tx, status_rx, sub_rx).await;
        });
    }
}

async fn handle_client(
    stream: UnixStream,
    cmd_tx: mpsc::Sender<CoreCommand>,
    status_rx: tokio::sync::watch::Receiver<TntronStatus>,
    mut sub_rx: broadcast::Receiver<TntronStatus>,
) -> anyhow::Result<()> {
    let (read_half, mut write_half) = stream.into_split();
    let mut lines = BufReader::new(read_half).lines();

    while let Some(line) = lines.next_line().await? {
        let Ok(req) = serde_json::from_str::<IpcCommand>(&line) else {
            let err = serde_json::to_string(&IpcResponse {
                ok: false,
                error: Some("invalid command JSON".into()),
                status: None,
            })?;
            write_half.write_all(err.as_bytes()).await?;
            write_half.write_all(b"\n").await?;
            continue;
        };

        if matches!(req, IpcCommand::Subscribe) {
            let init = serde_json::to_string(&*status_rx.borrow())?;
            write_half.write_all(init.as_bytes()).await?;
            write_half.write_all(b"\n").await?;
            while let Ok(st) = sub_rx.recv().await {
                let msg = serde_json::to_string(&st)?;
                if write_half.write_all(msg.as_bytes()).await.is_err()
                    || write_half.write_all(b"\n").await.is_err()
                {
                    break;
                }
            }
            return Ok(());
        }

        let core_cmd = match req {
            IpcCommand::Play => Some(CoreCommand::SetPaused(false)),
            IpcCommand::Pause => Some(CoreCommand::SetPaused(true)),
            IpcCommand::Toggle => Some(CoreCommand::TogglePause),
            IpcCommand::Next => Some(CoreCommand::NextTrack),
            IpcCommand::Prev => Some(CoreCommand::PrevTrack),
            IpcCommand::Stop => Some(CoreCommand::Stop),
            IpcCommand::Volume { delta: Some(d), .. } => Some(CoreCommand::VolumeDelta(d)),
            IpcCommand::Volume { value: Some(v), .. } => Some(CoreCommand::SetVolume(v)),
            IpcCommand::Seek { delta: Some(d), .. } => Some(CoreCommand::SeekDelta(d as f64)),
            IpcCommand::Seek { value: Some(v), .. } => Some(CoreCommand::SeekAbsolute(v)),
            IpcCommand::Speed { value } => Some(CoreCommand::SetSpeed(value)),
            IpcCommand::Mute { value: Some(m) } => Some(CoreCommand::SetMute(m)),
            IpcCommand::Mute { value: None } => Some(CoreCommand::ToggleMute),
            IpcCommand::DspPreset { preset } => {
                use crate::clock::AudioPreset;
                let p = match preset.to_ascii_lowercase().as_str() {
                    "flat" => Some(AudioPreset::Flat),
                    "bass" | "bassboost" | "bass_boost" => Some(AudioPreset::BassBoost),
                    "vocal" => Some(AudioPreset::Vocal),
                    "warm" => Some(AudioPreset::Warm),
                    "rock" => Some(AudioPreset::Rock),
                    "8d" | "spatial" | "spatial8d" => Some(AudioPreset::Spatial8D),
                    _ => None,
                };
                p.map(CoreCommand::SetAudioPreset)
            }
            IpcCommand::ToggleSpatial => Some(CoreCommand::ToggleSpatial),
            IpcCommand::Sleep { minutes } => Some(CoreCommand::SetSleepTimer(minutes)),
            IpcCommand::CycleLoop => Some(CoreCommand::CycleLoopMode),
            IpcCommand::Shuffle => Some(CoreCommand::ToggleShuffle),
            IpcCommand::Status | IpcCommand::Subscribe => None,
            _ => None,
        };

        if let Some(c) = core_cmd {
            let _ = cmd_tx.send(c).await;
        }

        let resp = serde_json::to_string(&IpcResponse {
            ok: true,
            error: None,
            status: Some(status_rx.borrow().clone()),
        })?;
        write_half.write_all(resp.as_bytes()).await?;
        write_half.write_all(b"\n").await?;
    }

    Ok(())
}
