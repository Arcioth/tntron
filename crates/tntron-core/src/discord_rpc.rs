use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, Mutex};

use crate::clock::{LoopMode, PlayState, ShuffleMode};
use crate::storage::DiscordRpcConfig;

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct DiscordPlaybackState {
    pub playing: PlayState,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub codec: Option<String>,
    pub bpm: Option<f64>,
    pub speed: f64,
    pub pitch_st: f64,
    pub loop_mode: LoopMode,
    pub shuffle_mode: ShuffleMode,
    pub track_index: Option<usize>,
    pub playlist_len: usize,
    pub queue_len: usize,
}

impl Default for DiscordPlaybackState {
    fn default() -> Self {
        Self {
            playing: PlayState::Stopped,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            position_sec: 0.0,
            duration_sec: 0.0,
            codec: None,
            bpm: None,
            speed: 1.0,
            pitch_st: 0.0,
            loop_mode: LoopMode::Off,
            shuffle_mode: ShuffleMode::Off,
            track_index: None,
            playlist_len: 0,
            queue_len: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiscordPreview {
    pub connected: bool,
    pub socket_label: Option<String>,
    pub error_message: Option<String>,
    pub details: String,
    pub state: String,
    pub footer: String,
}

#[derive(Debug, Clone)]
pub enum DiscordWorkerMsg {
    Sync {
        cfg: Box<DiscordRpcConfig>,
        playback: Box<DiscordPlaybackState>,
    },
    ReconnectNow,
}

#[derive(Debug, Clone)]
pub struct DiscordStatusHandle {
    connected: Arc<AtomicBool>,
    socket_label: Arc<Mutex<Option<String>>>,
    error_message: Arc<Mutex<Option<String>>>,
}

impl DiscordStatusHandle {
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn socket_label_blocking(&self) -> Option<String> {
        self.socket_label.try_lock().ok().and_then(|g| g.clone())
    }

    pub fn error_message_blocking(&self) -> Option<String> {
        self.error_message.try_lock().ok().and_then(|g| g.clone())
    }
}

pub fn epoch_now_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn clamp_discord_text(raw: &str, max_chars: usize) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let count = trimmed.chars().count();
    if count <= max_chars {
        if count == 1 {
            return format!("{trimmed} ");
        }
        return trimmed.to_string();
    }
    let mut out: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn format_vars(
    template: &str,
    cfg: &DiscordRpcConfig,
    pb: &DiscordPlaybackState,
    privacy: bool,
) -> String {
    let title = if !cfg.show_title {
        ""
    } else if privacy {
        "Listening to Music"
    } else if pb.title.trim().is_empty() {
        "Unknown Track"
    } else {
        pb.title.trim()
    };

    let artist = if !cfg.show_artist || privacy {
        ""
    } else {
        pb.artist.trim()
    };

    let album = if !cfg.show_album || privacy {
        ""
    } else {
        pb.album.trim()
    };

    let codec = if cfg.show_codec {
        pb.codec.as_deref().unwrap_or("").trim().to_string()
    } else {
        String::new()
    };

    let bpm = if cfg.show_bpm {
        pb.bpm.map(|b| format!("{b:.0} BPM")).unwrap_or_default()
    } else {
        String::new()
    };

    let speed_pitch = if cfg.show_speed_pitch {
        let spd = if (pb.speed - 1.0).abs() > 0.01 {
            format!("{:.2}x", pb.speed)
        } else {
            String::new()
        };
        let pit = if pb.pitch_st.abs() > 0.05 {
            format!("{:+.1}st", pb.pitch_st)
        } else {
            String::new()
        };
        match (spd.is_empty(), pit.is_empty()) {
            (false, false) => format!("{spd} {pit}"),
            (false, true) => spd,
            (true, false) => pit,
            (true, true) => String::new(),
        }
    } else {
        String::new()
    };

    let queue_txt = if cfg.show_queue_info {
        let pos = match (pb.track_index, pb.playlist_len) {
            (Some(i), len) if len > 0 => format!("{}/{}", i + 1, len),
            _ => String::new(),
        };
        let q = if pb.queue_len > 0 {
            format!("Q:{}", pb.queue_len)
        } else {
            String::new()
        };
        match (pos.is_empty(), q.is_empty()) {
            (false, false) => format!("{pos} · {q}"),
            (false, true) => pos,
            (true, false) => q,
            (true, true) => String::new(),
        }
    } else {
        String::new()
    };

    let loop_txt = if cfg.show_loop_shuffle {
        match pb.loop_mode {
            LoopMode::Off => "",
            LoopMode::Track => "Loop: Track",
            LoopMode::All => "Loop: All",
        }
        .to_string()
    } else {
        String::new()
    };

    let shuffle_txt = if cfg.show_loop_shuffle && pb.shuffle_mode != ShuffleMode::Off {
        "Shuffle".to_string()
    } else {
        String::new()
    };

    let state_txt = match pb.playing {
        PlayState::Playing => "Playing",
        PlayState::Paused => {
            if cfg.show_paused {
                "Paused"
            } else {
                ""
            }
        }
        PlayState::Stopped => "Idle",
    };

    let pitch_txt = if cfg.show_speed_pitch && pb.pitch_st.abs() > 0.05 {
        format!("{:+.1}st", pb.pitch_st)
    } else {
        String::new()
    };

    let mut out = template
        .replace("{title}", title)
        .replace("{artist}", artist)
        .replace("{album}", album)
        .replace("{codec}", &codec)
        .replace("{bpm}", &bpm)
        .replace("{speed}", &speed_pitch)
        .replace("{pitch}", &pitch_txt)
        .replace("{queue}", &queue_txt)
        .replace("{loop}", &loop_txt)
        .replace("{shuffle}", &shuffle_txt)
        .replace("{state}", state_txt);

    // Collapse orphaned dot separators cleanly.
    while out.contains("·  ·") || out.contains("··") {
        out = out.replace("·  ·", "·").replace("··", "·");
    }
    let cleaned_parts: Vec<&str> = out
        .split('·')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    cleaned_parts.join(" · ")
}

fn push_non_empty(parts: &mut Vec<String>, s: String) {
    let t = s.trim();
    if !t.is_empty() {
        parts.push(t.to_string());
    }
}

fn build_details_and_state(
    cfg: &DiscordRpcConfig,
    pb: &DiscordPlaybackState,
) -> (Option<String>, Option<String>) {
    if pb.playing == PlayState::Stopped || pb.title.trim().is_empty() {
        if !cfg.show_idle {
            return (None, None);
        }
        let idle = if cfg.idle_text.trim().is_empty() {
            "Browsing Library · Idle".to_string()
        } else {
            cfg.idle_text.trim().to_string()
        };
        return (Some(clamp_discord_text(&idle, 128)), None);
    }

    if pb.playing == PlayState::Paused && cfg.clear_on_pause {
        return (None, None);
    }

    if cfg.privacy_mode {
        let mut priv_state = Vec::new();
        if pb.playing == PlayState::Paused && cfg.show_paused {
            priv_state.push("Paused".to_string());
        }
        if cfg.show_codec {
            if let Some(c) = &pb.codec {
                push_non_empty(&mut priv_state, c.clone());
            }
        }
        let st = if priv_state.is_empty() {
            None
        } else {
            Some(clamp_discord_text(&priv_state.join(" · "), 128))
        };
        return (Some("Listening to Music in tntron".to_string()), st);
    }

    let details_raw = match cfg.details_mode.as_str() {
        "custom" => format_vars(&cfg.custom_details, cfg, pb, false),
        "title_only" => {
            if cfg.show_title {
                pb.title.trim().to_string()
            } else {
                "tntron".to_string()
            }
        }
        "title_artist" => {
            let mut p = Vec::new();
            if cfg.show_title {
                push_non_empty(&mut p, pb.title.clone());
            }
            if cfg.show_artist && !pb.artist.eq_ignore_ascii_case("Unknown Artist") {
                push_non_empty(&mut p, pb.artist.clone());
            }
            p.join(" — ")
        }
        _ => {
            // "auto" — rich track title + optional live status markers
            let mut p = Vec::new();
            if cfg.show_title {
                push_non_empty(&mut p, pb.title.clone());
            }
            if pb.playing == PlayState::Paused && cfg.show_paused {
                p.push("[Paused]".to_string());
            }
            if cfg.show_bpm {
                if let Some(b) = pb.bpm {
                    p.push(format!("{b:.0} BPM"));
                }
            }
            if cfg.show_speed_pitch && ((pb.speed - 1.0).abs() > 0.01 || pb.pitch_st.abs() > 0.05) {
                p.push(format_vars("{speed}", cfg, pb, false));
            }
            p.join(" · ")
        }
    };

    let state_raw = match cfg.state_mode.as_str() {
        "custom" => format_vars(&cfg.custom_state, cfg, pb, false),
        "artist_album" => {
            let mut p = Vec::new();
            if cfg.show_artist {
                push_non_empty(&mut p, pb.artist.clone());
            }
            if cfg.show_album && !pb.album.is_empty() {
                push_non_empty(&mut p, pb.album.clone());
            }
            p.join(" · ")
        }
        "telemetry" => {
            let mut p = Vec::new();
            if cfg.show_codec {
                if let Some(c) = &pb.codec {
                    push_non_empty(&mut p, c.clone());
                }
            }
            if cfg.show_queue_info {
                push_non_empty(&mut p, format_vars("{queue}", cfg, pb, false));
            }
            if cfg.show_loop_shuffle {
                push_non_empty(&mut p, format_vars("{loop}", cfg, pb, false));
                push_non_empty(&mut p, format_vars("{shuffle}", cfg, pb, false));
            }
            p.join(" · ")
        }
        _ => {
            // "auto" — artist, album, codec, queue & loop/shuffle badges combined cleanly
            let mut p = Vec::new();
            if cfg.show_artist {
                push_non_empty(&mut p, pb.artist.clone());
            }
            if cfg.show_album && !pb.album.eq_ignore_ascii_case("Unknown Album") {
                push_non_empty(&mut p, pb.album.clone());
            }
            if cfg.show_codec {
                if let Some(c) = &pb.codec {
                    push_non_empty(&mut p, c.clone());
                }
            }
            if cfg.show_queue_info {
                push_non_empty(&mut p, format_vars("{queue}", cfg, pb, false));
            }
            if cfg.show_loop_shuffle {
                push_non_empty(&mut p, format_vars("{loop}", cfg, pb, false));
                push_non_empty(&mut p, format_vars("{shuffle}", cfg, pb, false));
            }
            p.join(" · ")
        }
    };

    let details = {
        let d = clamp_discord_text(&details_raw, 128);
        if d.is_empty() { None } else { Some(d) }
    };
    let state = {
        let s = clamp_discord_text(&state_raw, 128);
        if s.is_empty() { None } else { Some(s) }
    };
    (details, state)
}

/// Builds the Discord `SET_ACTIVITY` `activity` JSON value (or `None` to clear presence).
pub fn build_activity_json(
    cfg: &DiscordRpcConfig,
    pb: &DiscordPlaybackState,
    now_epoch: u64,
) -> Option<Value> {
    if !cfg.enabled {
        return None;
    }

    let (details, state) = build_details_and_state(cfg, pb);
    if details.is_none() && state.is_none() {
        return None;
    }

    let mut activity = serde_json::Map::new();
    activity.insert(
        "type".into(),
        json!(match cfg.activity_type {
            0 | 2 | 3 | 5 => cfg.activity_type,
            _ => 2,
        }),
    );

    if let Some(d) = details {
        activity.insert("details".into(), json!(d));
    }
    if let Some(s) = state {
        activity.insert("state".into(), json!(s));
    }

    // Timestamps (only while actively playing a bounded track)
    if cfg.show_timestamps
        && pb.playing == PlayState::Playing
        && !pb.title.trim().is_empty()
        && cfg.timestamp_mode != "none"
    {
        let pos = pb.position_sec.max(0.0).round() as u64;
        let start = now_epoch.saturating_sub(pos);
        if cfg.timestamp_mode == "elapsed" || pb.duration_sec <= 0.5 {
            activity.insert("timestamps".into(), json!({ "start": start }));
        } else {
            let dur = pb.duration_sec.max(1.0).round() as u64;
            let end = start.saturating_add(dur);
            activity.insert("timestamps".into(), json!({ "start": start, "end": end }));
        }
    }

    // Assets & tooltips
    let mut assets = serde_json::Map::new();
    let large_key = cfg.large_image_key.trim();
    if !large_key.is_empty() {
        assets.insert("large_image".into(), json!(large_key));
        let lt = clamp_discord_text(&format_vars(&cfg.large_text, cfg, pb, cfg.privacy_mode), 128);
        if !lt.is_empty() {
            assets.insert("large_text".into(), json!(lt));
        }
    }
    let small_key = cfg.small_image_key.trim();
    if !small_key.is_empty() && pb.playing != PlayState::Stopped {
        assets.insert("small_image".into(), json!(small_key));
        let st = clamp_discord_text(&format_vars(&cfg.small_text, cfg, pb, cfg.privacy_mode), 128);
        if !st.is_empty() {
            assets.insert("small_text".into(), json!(st));
        }
    }
    if !assets.is_empty() {
        activity.insert("assets".into(), Value::Object(assets));
    }

    // Action button (requires non-empty label & http(s) URL)
    if cfg.show_button {
        let lbl = clamp_discord_text(&cfg.button_label, 32);
        let url = cfg.button_url.trim();
        if !lbl.is_empty() && (url.starts_with("https://") || url.starts_with("http://")) {
            activity.insert(
                "buttons".into(),
                json!([{
                    "label": lbl,
                    "url": url,
                }]),
            );
        }
    }

    Some(Value::Object(activity))
}

/// Computes a human-readable snapshot preview matching exactly what Discord IPC receives.
pub fn build_preview(
    cfg: &DiscordRpcConfig,
    pb: &DiscordPlaybackState,
    connected: bool,
    socket_label: Option<String>,
    error_message: Option<String>,
) -> DiscordPreview {
    if !cfg.enabled {
        return DiscordPreview {
            connected,
            socket_label,
            error_message,
            details: "Rich Presence Disabled".to_string(),
            state: "Enable above to broadcast playback status to Discord".to_string(),
            footer: String::new(),
        };
    }

    let Some(act) = build_activity_json(cfg, pb, epoch_now_sec()) else {
        return DiscordPreview {
            connected,
            socket_label,
            error_message,
            details: "(Activity cleared)".to_string(),
            state: if pb.playing == PlayState::Paused && cfg.clear_on_pause {
                "Cleared automatically while paused".to_string()
            } else {
                "Idle hiding enabled".to_string()
            },
            footer: String::new(),
        };
    };

    let kind_word = match cfg.activity_type {
        0 => "Playing tntron",
        3 => "Watching tntron",
        _ => "Listening to tntron",
    };
    let details = act
        .get("details")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let state = act
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut foot_parts = vec![kind_word.to_string()];
    if let Some(lt) = act
        .get("assets")
        .and_then(|a| a.get("large_text"))
        .and_then(|v| v.as_str())
    {
        foot_parts.push(format!("Tip: {lt}"));
    }
    if act.get("timestamps").is_some() {
        foot_parts.push(format!("Timer: {}", cfg.timestamp_mode));
    }
    if let Some(btn) = act
        .get("buttons")
        .and_then(|b| b.as_array())
        .and_then(|a| a.first())
        .and_then(|b| b.get("label"))
        .and_then(|v| v.as_str())
    {
        foot_parts.push(format!("Button: [{btn}]"));
    }

    DiscordPreview {
        connected,
        socket_label,
        error_message,
        details,
        state,
        footer: foot_parts.join(" · "),
    }
}

fn candidate_socket_paths() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for var in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
        if let Ok(val) = std::env::var(var) {
            let p = PathBuf::from(val);
            if p.is_dir() && !dirs.contains(&p) {
                dirs.push(p);
            }
        }
    }
    let uid = unsafe { libc::geteuid() };
    let run_user = PathBuf::from(format!("/run/user/{uid}"));
    if run_user.is_dir() && !dirs.contains(&run_user) {
        dirs.push(run_user);
    }
    let tmp = PathBuf::from("/tmp");
    if !dirs.contains(&tmp) {
        dirs.push(tmp);
    }

    let subdirs = [
        "",
        "app/com.discordapp.Discord",
        "app/dev.vencord.Vesktop",
        "snap.discord",
        ".flatpak/com.discordapp.Discord/xdg-run",
    ];
    let mut out = Vec::new();
    for dir in dirs {
        for sub in subdirs {
            let base = if sub.is_empty() {
                dir.clone()
            } else {
                dir.join(sub)
            };
            if !base.is_dir() {
                continue;
            }
            for i in 0..10 {
                let candidate = base.join(format!("discord-ipc-{i}"));
                if candidate.exists() {
                    out.push(candidate);
                }
            }
        }
    }
    out
}

async fn write_ipc_frame(stream: &mut UnixStream, op: u32, payload: &Value) -> std::io::Result<()> {
    let json_bytes = serde_json::to_vec(payload)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let len = json_bytes.len() as u32;
    let mut buf = Vec::with_capacity(8 + json_bytes.len());
    buf.extend_from_slice(&op.to_le_bytes());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&json_bytes);
    stream.write_all(&buf).await
}

async fn read_ipc_frame(stream: &mut UnixStream) -> std::io::Result<(u32, Vec<u8>)> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header).await?;
    let op = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    if len > 1024 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Discord IPC frame too large",
        ));
    }
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await?;
    Ok((op, body))
}

async fn connect_discord(client_id: &str) -> Result<(UnixStream, String), String> {
    let cid = client_id.trim();
    if cid.is_empty() || cid == "1306739184569253928" {
        return Err(
            "Set your Discord Application ID (from discord.com/developers/applications) and click Apply"
                .to_string(),
        );
    }

    let candidates = candidate_socket_paths();
    if candidates.is_empty() {
        return Err("No Discord IPC socket found (open Discord / Vesktop)".to_string());
    }

    let mut last_err = String::from("Failed to handshake with Discord IPC");
    for path in candidates {
        match UnixStream::connect(&path).await {
            Ok(mut stream) => {
                let handshake = json!({ "v": 1, "client_id": cid });
                if let Err(e) = write_ipc_frame(&mut stream, OP_HANDSHAKE, &handshake).await {
                    last_err = format!("Discord IPC write error: {e}");
                    continue;
                }
                match tokio::time::timeout(Duration::from_secs(6), read_ipc_frame(&mut stream)).await {
                    Ok(Ok((op, body))) => {
                        let parsed = serde_json::from_slice::<Value>(&body).ok();
                        let evt = parsed
                            .as_ref()
                            .and_then(|v| v.get("evt"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let msg = parsed
                            .as_ref()
                            .and_then(|v| v.get("message").or_else(|| v.get("data").and_then(|d| d.get("message"))))
                            .and_then(|v| v.as_str())
                            .unwrap_or("Handshake rejected");
                        if op == OP_FRAME && evt != "ERROR" {
                            let label = Path::new(&path)
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("discord-ipc-0")
                                .to_string();
                            return Ok((stream, label));
                        }
                        last_err = format!("Discord rejected Client ID ({msg})");
                    }
                    Ok(Err(e)) => last_err = format!("Discord IPC closed: {e}"),
                    Err(_) => last_err = "Discord handshake timed out".to_string(),
                }
            }
            Err(e) => last_err = format!("Cannot connect to {}: {e}", path.display()),
        }
    }
    Err(last_err)
}

async fn send_activity(
    stream: &mut UnixStream,
    cfg: &DiscordRpcConfig,
    pb: &DiscordPlaybackState,
    nonce: u64,
) -> Result<(), String> {
    let activity = build_activity_json(cfg, pb, epoch_now_sec());
    let payload = json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": std::process::id(),
            "activity": activity,
        },
        "nonce": format!("tntron-{nonce}"),
    });
    write_ipc_frame(stream, OP_FRAME, &payload)
        .await
        .map_err(|e| format!("IPC write failed: {e}"))?;
    if let Ok(Ok((op, body))) =
        tokio::time::timeout(Duration::from_millis(450), read_ipc_frame(stream)).await
    {
        if let Ok(val) = serde_json::from_slice::<Value>(&body) {
            if op == 2 || val.get("evt").and_then(|v| v.as_str()) == Some("ERROR") {
                let msg = val
                    .get("data")
                    .and_then(|d| d.get("message"))
                    .or_else(|| val.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("SET_ACTIVITY error");
                return Err(format!("Discord error: {msg}"));
            }
        }
    }
    Ok(())
}

/// Spawns the background Discord IPC supervisor task and returns `(sender, status_handle)`.
pub fn spawn_discord_worker() -> (mpsc::Sender<DiscordWorkerMsg>, DiscordStatusHandle) {
    let (tx, mut rx) = mpsc::channel::<DiscordWorkerMsg>(32);
    let connected = Arc::new(AtomicBool::new(false));
    let socket_label = Arc::new(Mutex::new(None));
    let error_message = Arc::new(Mutex::new(None));
    let handle = DiscordStatusHandle {
        connected: Arc::clone(&connected),
        socket_label: Arc::clone(&socket_label),
        error_message: Arc::clone(&error_message),
    };

    tokio::spawn(async move {
        let mut conn: Option<(UnixStream, String, String)> = None;
        let mut latest_cfg: Option<DiscordRpcConfig> = None;
        let mut latest_pb: Option<DiscordPlaybackState> = None;
        let mut nonce: u64 = 1;
        let mut retry_tick = tokio::time::interval(Duration::from_secs(5));
        retry_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            let mut need_send = false;
            let mut force_reconnect = false;

            tokio::select! {
                msg = rx.recv() => {
                    let Some(m) = msg else { break };
                    match m {
                        DiscordWorkerMsg::Sync { cfg, playback } => {
                            let cfg = *cfg;
                            let playback = *playback;
                            if let Some((_, _, cid)) = &conn {
                                if cid != cfg.client_id.trim() {
                                    force_reconnect = true;
                                }
                            }
                            latest_cfg = Some(cfg);
                            latest_pb = Some(playback);
                            need_send = true;
                        }
                        DiscordWorkerMsg::ReconnectNow => {
                            force_reconnect = true;
                            need_send = true;
                        }
                    }
                }
                _ = retry_tick.tick() => {
                    if conn.is_none() && latest_cfg.as_ref().is_some_and(|c| c.enabled) {
                        need_send = true;
                    }
                }
            }

            if force_reconnect {
                conn = None;
                connected.store(false, Ordering::Relaxed);
                *socket_label.lock().await = None;
            }

            if !need_send {
                continue;
            }

            let (Some(cfg), Some(pb)) = (&latest_cfg, &latest_pb) else {
                continue;
            };

            if !cfg.enabled {
                if let Some((mut stream, _, _)) = conn.take() {
                    let _ = send_activity(&mut stream, cfg, pb, nonce).await;
                    nonce = nonce.wrapping_add(1);
                }
                connected.store(false, Ordering::Relaxed);
                *socket_label.lock().await = None;
                *error_message.lock().await = None;
                continue;
            }

            if conn.is_none() {
                match connect_discord(&cfg.client_id).await {
                    Ok((stream, label)) => {
                        connected.store(true, Ordering::Relaxed);
                        *socket_label.lock().await = Some(label.clone());
                        *error_message.lock().await = None;
                        conn = Some((stream, label, cfg.client_id.trim().to_string()));
                    }
                    Err(err) => {
                        connected.store(false, Ordering::Relaxed);
                        *socket_label.lock().await = None;
                        *error_message.lock().await = Some(err);
                    }
                }
            }

            if let Some((stream, _, _)) = &mut conn {
                nonce = nonce.wrapping_add(1);
                if let Err(err) = send_activity(stream, cfg, pb, nonce).await {
                    conn = None;
                    connected.store(false, Ordering::Relaxed);
                    *socket_label.lock().await = None;
                    *error_message.lock().await = Some(err);
                }
            }
        }
    });

    (tx, handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discord_activity_payload_configurable_fields() {
        let mut cfg = DiscordRpcConfig::default();
        let pb = DiscordPlaybackState {
            playing: PlayState::Playing,
            title: "Nightcall".to_string(),
            artist: "Kavinsky".to_string(),
            album: "OutRun".to_string(),
            position_sec: 42.0,
            duration_sec: 258.0,
            codec: Some("FLAC 44.1kHz".to_string()),
            bpm: Some(92.0),
            speed: 1.15,
            pitch_st: 2.0,
            loop_mode: LoopMode::All,
            shuffle_mode: ShuffleMode::On,
            track_index: Some(2),
            playlist_len: 12,
            queue_len: 3,
        };

        let act = build_activity_json(&cfg, &pb, 1_700_000_100).expect("activity");
        assert_eq!(act["type"], 2);
        let details = act["details"].as_str().unwrap();
        let state = act["state"].as_str().unwrap();
        assert!(details.contains("Nightcall"));
        assert!(details.contains("92 BPM"));
        assert!(details.contains("1.15x"));
        assert!(state.contains("Kavinsky"));
        assert!(state.contains("OutRun"));
        assert!(state.contains("FLAC 44.1kHz"));
        assert!(state.contains("3/12"));
        assert!(state.contains("Q:3"));
        assert!(state.contains("Loop: All"));
        assert!(act["timestamps"]["start"].is_u64());
        assert!(act["timestamps"]["end"].is_u64());
        assert_eq!(act["buttons"][0]["label"], "tntron on GitHub");

        // Privacy mode masks sensitive track names
        cfg.privacy_mode = true;
        let priv_act = build_activity_json(&cfg, &pb, 1_700_000_100).expect("priv");
        assert_eq!(priv_act["details"], "Listening to Music in tntron");
        assert!(!priv_act["state"].as_str().unwrap_or("").contains("Kavinsky"));

        // Clear on pause clears payload completely
        cfg.privacy_mode = false;
        cfg.clear_on_pause = true;
        let mut paused = pb.clone();
        paused.playing = PlayState::Paused;
        assert!(build_activity_json(&cfg, &paused, 1_700_000_100).is_none());
    }
}
