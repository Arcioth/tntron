use std::io::{self, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;
use image::imageops::FilterType;
use image::ImageEncoder;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tntron_core::{
    default_popup_config_path, default_socket_path, default_status_path, format_mmss, IpcCommand,
    IpcResponse, PopupPlayerConfig, PopupTrackItem, TntronStatus,
};

struct RawTermGuard {
    orig: libc::termios,
    orig_flags: i32,
}

impl RawTermGuard {
    fn enter() -> Option<Self> {
        unsafe {
            let mut orig: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut orig) != 0 {
                return None;
            }
            let mut raw = orig;
            raw.c_lflag &= !(libc::ICANON | libc::ECHO);
            libc::tcsetattr(0, libc::TCSANOW, &raw);
            let orig_flags = libc::fcntl(0, libc::F_GETFL);
            if orig_flags >= 0 {
                libc::fcntl(0, libc::F_SETFL, orig_flags | libc::O_NONBLOCK);
            }
            Some(Self { orig, orig_flags })
        }
    }
}

impl Drop for RawTermGuard {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(0, libc::TCSANOW, &self.orig);
            if self.orig_flags >= 0 {
                libc::fcntl(0, libc::F_SETFL, self.orig_flags);
            }
        }
        let mut out = io::stdout();
        let _ = write!(out, "\x1b_Ga=d,d=A,q=2;\x1b\\\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = out.flush();
    }
}

struct CachedArt {
    path: String,
    cols: usize,
    rows: usize,
    half_blocks: Vec<String>,
    ascii_art: Vec<String>,
    kitty_png_b64: Option<String>,
}

struct PopupUiState {
    hue_deg: f32,
    anim_phase: f32,
    selected_list_row: usize,
    art_mode_override: Option<String>,
    ascii_preset_override: Option<String>,
    show_list_override: Option<bool>,
    show_lyrics_override: Option<bool>,
    show_eq_override: Option<bool>,
}

impl PopupUiState {
    fn new() -> Self {
        Self {
            hue_deg: 195.0,
            anim_phase: 0.0,
            selected_list_row: 0,
            art_mode_override: None,
            ascii_preset_override: None,
            show_list_override: None,
            show_lyrics_override: None,
            show_eq_override: None,
        }
    }

    fn effective_art_mode<'a>(&'a self, cfg: &'a PopupPlayerConfig) -> &'a str {
        self.art_mode_override
            .as_deref()
            .unwrap_or(cfg.art_mode.as_str())
    }

    fn effective_ascii_preset<'a>(&'a self, cfg: &'a PopupPlayerConfig) -> &'a str {
        self.ascii_preset_override
            .as_deref()
            .unwrap_or(cfg.ascii_preset.as_str())
    }

    fn effective_show_list(&self, cfg: &PopupPlayerConfig) -> bool {
        self.show_list_override.unwrap_or(cfg.show_list)
    }

    fn effective_show_lyrics(&self, cfg: &PopupPlayerConfig) -> bool {
        self.show_lyrics_override.unwrap_or(cfg.show_lyrics)
    }

    fn effective_show_eq(&self, cfg: &PopupPlayerConfig) -> bool {
        self.show_eq_override.unwrap_or(cfg.show_eq)
    }
}

pub async fn run_popup_tui() -> Result<()> {
    let _guard = RawTermGuard::enter();
    {
        let mut out = io::stdout();
        write!(out, "\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H")?;
        out.flush()?;
    }

    let cfg_path = default_popup_config_path();
    let mut cfg = PopupPlayerConfig::load_from(&cfg_path);
    let mut ui = PopupUiState::new();
    let mut tick_count: u64 = 0;
    let mut last_tick = Instant::now();
    let mut cached_art: Option<CachedArt> = None;
    let mut kitty_sent_for: Option<String> = None;
    let mut prev_term_size = terminal_size();

    let is_kitty = std::env::var("KITTY_WINDOW_ID").is_ok()
        || std::env::var("TERM")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("kitty");

    loop {
        if tick_count.is_multiple_of(10) {
            cfg = PopupPlayerConfig::load_from(&cfg_path);
            if !cfg.enabled {
                break;
            }
        }
        tick_count += 1;

        let now = Instant::now();
        let dt = now
            .saturating_duration_since(last_tick)
            .as_secs_f32()
            .clamp(0.01, 0.25);
        last_tick = now;

        let term_size = terminal_size();
        if term_size != prev_term_size {
            prev_term_size = term_size;
            let mut out = io::stdout();
            let _ = write!(out, "\x1b_Ga=d,d=A,q=2;\x1b\\\x1b[2J\x1b[H");
            let _ = out.flush();
            kitty_sent_for = None;
            cached_art = None;
        }

        let status = fetch_live_status().await;
        let visible_list = status
            .as_ref()
            .map(|st| build_visible_list(st, &cfg))
            .unwrap_or_default();
        if !visible_list.is_empty() && ui.selected_list_row >= visible_list.len() {
            ui.selected_list_row = visible_list.len() - 1;
        }

        let mut key_buf = [0u8; 32];
        if let Ok(n) = io::stdin().read(&mut key_buf) {
            if handle_keys(&key_buf[..n], &mut ui, &cfg, &visible_list).await {
                break;
            }
        }

        if let Some(st) = &status {
            let vocal: f32 = st.bands[4..11].iter().sum::<f32>() / 7.0;
            let speed_scale = if st.state == "playing" { 1.0 } else { 0.25 };
            ui.hue_deg = (ui.hue_deg + dt * (28.0 + vocal * 260.0) * speed_scale) % 360.0;
            ui.anim_phase = (ui.anim_phase + dt * (2.6 + st.beat * 4.0) * speed_scale) % 1000.0;
        } else {
            ui.anim_phase = (ui.anim_phase + dt * 1.4) % 1000.0;
        }

        let layout_plan = compute_layout_plan(term_size.0, term_size.1, &cfg, &ui);
        if let Some(st) = &status {
            let art_mode = ui.effective_art_mode(&cfg);
            let need_cover = cfg.show_picture
                && layout_plan.art_rows > 0
                && matches!(art_mode, "cover" | "both" | "ascii")
                && st
                    .cover_path
                    .as_deref()
                    .filter(|s| !s.is_empty() && Path::new(s).is_file())
                    .is_some();

            if need_cover {
                let cpath = st.cover_path.as_deref().unwrap_or("");
                let stale = cached_art.as_ref().is_none_or(|c| {
                    c.path != cpath
                        || c.cols != layout_plan.art_cols
                        || c.rows != layout_plan.art_rows
                });
                if stale {
                    let allow_kitty = is_kitty && matches!(art_mode, "cover" | "both");
                    cached_art = build_cached_art(
                        cpath,
                        layout_plan.art_cols as u32,
                        layout_plan.art_rows as u32,
                        allow_kitty,
                    );
                    if kitty_sent_for.is_some() {
                        let mut out = io::stdout();
                        let _ = write!(out, "\x1b_Ga=d,d=A,q=2;\x1b\\");
                        let _ = out.flush();
                        kitty_sent_for = None;
                    }
                }
            } else {
                if kitty_sent_for.is_some() {
                    let mut out = io::stdout();
                    let _ = write!(out, "\x1b_Ga=d,d=A,q=2;\x1b\\");
                    let _ = out.flush();
                    kitty_sent_for = None;
                }
                cached_art = None;
            }
        }

        draw_frame(
            status.as_ref(),
            &cfg,
            &ui,
            &layout_plan,
            cached_art.as_ref(),
            &visible_list,
            &mut kitty_sent_for,
            is_kitty,
        )?;

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    Ok(())
}

async fn handle_keys(
    bytes: &[u8],
    ui: &mut PopupUiState,
    cfg: &PopupPlayerConfig,
    visible_list: &[PopupTrackItem],
) -> bool {
    const ASCII_CYCLE: &[&str] = &[
        "vinyl", "cassette", "boombox", "note", "orbit", "wave", "eq_tower",
    ];
    const ART_CYCLE: &[&str] = &["cover", "ascii", "both", "none"];

    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'q' | b'Q' => return true,
            0x1b => {
                if i + 2 < bytes.len() && bytes[i + 1] == b'[' {
                    match bytes[i + 2] {
                        b'C' => send_ipc(&IpcCommand::Next).await,
                        b'D' => send_ipc(&IpcCommand::Prev).await,
                        b'A' => {
                            send_ipc(&IpcCommand::Volume {
                                delta: Some(5),
                                value: None,
                            })
                            .await
                        }
                        b'B' => {
                            send_ipc(&IpcCommand::Volume {
                                delta: Some(-5),
                                value: None,
                            })
                            .await
                        }
                        _ => {}
                    }
                    i += 3;
                    continue;
                }
                return true;
            }
            b' ' => send_ipc(&IpcCommand::Toggle).await,
            b'n' | b'N' => send_ipc(&IpcCommand::Next).await,
            b'p' | b'P' => send_ipc(&IpcCommand::Prev).await,
            b'l' | b'L' => {
                send_ipc(&IpcCommand::Seek {
                    delta: Some(5),
                    value: None,
                })
                .await
            }
            b'h' | b'H' => {
                send_ipc(&IpcCommand::Seek {
                    delta: Some(-5),
                    value: None,
                })
                .await
            }
            b'+' | b'=' => {
                send_ipc(&IpcCommand::Volume {
                    delta: Some(5),
                    value: None,
                })
                .await
            }
            b'-' | b'_' => {
                send_ipc(&IpcCommand::Volume {
                    delta: Some(-5),
                    value: None,
                })
                .await
            }
            b'm' | b'M' => send_ipc(&IpcCommand::Mute { value: None }).await,
            b'j' | b'J' if !visible_list.is_empty() => {
                ui.selected_list_row = (ui.selected_list_row + 1) % visible_list.len();
            }
            b'k' | b'K' if !visible_list.is_empty() => {
                ui.selected_list_row = if ui.selected_list_row == 0 {
                    visible_list.len() - 1
                } else {
                    ui.selected_list_row - 1
                };
            }
            b'\r' | b'\n' => {
                if let Some(item) = visible_list.get(ui.selected_list_row) {
                    if let Some(idx) = item.playlist_index {
                        let cmd = if item.is_queued {
                            IpcCommand::PlayQueueItem { index: idx }
                        } else {
                            IpcCommand::PlayUpcoming { index: idx }
                        };
                        send_ipc(&cmd).await;
                    }
                }
            }
            b't' | b'T' => {
                let cur = ui.effective_show_list(cfg);
                ui.show_list_override = Some(!cur);
            }
            b'y' | b'Y' => {
                let cur = ui.effective_show_lyrics(cfg);
                ui.show_lyrics_override = Some(!cur);
            }
            b'e' | b'E' => {
                let cur = ui.effective_show_eq(cfg);
                ui.show_eq_override = Some(!cur);
            }
            b'a' | b'A' => {
                let cur = ui.effective_ascii_preset(cfg);
                let pos = ASCII_CYCLE.iter().position(|p| *p == cur).unwrap_or(0);
                ui.ascii_preset_override =
                    Some(ASCII_CYCLE[(pos + 1) % ASCII_CYCLE.len()].to_string());
            }
            b'v' | b'V' => {
                let cur = ui.effective_art_mode(cfg);
                let pos = ART_CYCLE.iter().position(|p| *p == cur).unwrap_or(0);
                ui.art_mode_override = Some(ART_CYCLE[(pos + 1) % ART_CYCLE.len()].to_string());
            }
            _ => {}
        }
        i += 1;
    }
    false
}

async fn send_ipc(cmd: &IpcCommand) {
    let sock = default_socket_path();
    if let Ok(stream) = UnixStream::connect(&sock).await {
        let (_r, mut w) = stream.into_split();
        if let Ok(json) = serde_json::to_string(cmd) {
            let _ = w.write_all(json.as_bytes()).await;
            let _ = w.write_all(b"\n").await;
        }
    }
}

async fn fetch_live_status() -> Option<TntronStatus> {
    let sock = default_socket_path();
    if let Ok(stream) = UnixStream::connect(&sock).await {
        let (r, mut w) = stream.into_split();
        if let Ok(payload) = serde_json::to_string(&IpcCommand::Status) {
            if w.write_all(payload.as_bytes()).await.is_ok() && w.write_all(b"\n").await.is_ok() {
                let mut line = String::new();
                let mut reader = BufReader::new(r);
                if reader.read_line(&mut line).await.is_ok() {
                    if let Ok(resp) = serde_json::from_str::<IpcResponse>(&line) {
                        if let Some(st) = resp.status {
                            return Some(st);
                        }
                    }
                }
            }
        }
    }
    std::fs::read_to_string(default_status_path())
        .ok()
        .and_then(|s| serde_json::from_str::<TntronStatus>(&s).ok())
}

struct LayoutPlan {
    term_w: usize,
    term_h: usize,
    box_w: usize,
    show_art: bool,
    art_cols: usize,
    art_rows: usize,
    lyric_lines: usize,
    list_rows: usize,
    eq_rows: usize,
    show_footer: bool,
}

fn compute_layout_plan(
    term_w: usize,
    term_h: usize,
    cfg: &PopupPlayerConfig,
    ui: &PopupUiState,
) -> LayoutPlan {
    let w = term_w.max(30);
    let h = term_h.max(8);
    let box_w = w.saturating_sub(2);
    let inner_h = h.saturating_sub(3); // top border, header, bottom border

    let art_mode = ui.effective_art_mode(cfg);
    let want_art = cfg.show_picture
        && art_mode != "none"
        && cfg.layout != "compact"
        && (!cfg.auto_resize_layout || (w >= 54 && h >= 13));

    let art_cols = if want_art {
        if cfg.auto_resize_layout {
            (w / 4).clamp(16, 30)
        } else {
            20
        }
    } else {
        0
    };
    let art_rows = if want_art {
        if cfg.auto_resize_layout {
            ((h.saturating_sub(8)) / 2).clamp(6, 14)
        } else {
            10
        }
    } else {
        0
    };

    let show_footer = cfg.show_footer_hints && h >= 10;
    let footer_h = usize::from(show_footer);

    let want_eq = ui.effective_show_eq(cfg) && h >= 11;
    let eq_rows = if want_eq {
        if cfg.auto_resize_layout {
            if h >= 30 {
                cfg.eq_rows.clamp(1, 5)
            } else if h >= 20 {
                cfg.eq_rows.clamp(1, 3)
            } else {
                1
            }
        } else {
            cfg.eq_rows.clamp(1, 5)
        }
    } else {
        0
    };

    let mut remaining = inner_h.saturating_sub(footer_h + eq_rows);
    let top_block_min = if want_art { art_rows } else { 5 };
    remaining = remaining.saturating_sub(top_block_min);

    let want_lyrics = ui.effective_show_lyrics(cfg);
    let lyric_lines = if want_lyrics {
        if cfg.layout == "lyrics" {
            6.min(art_rows.max(4) + remaining / 2)
        } else if h >= 22 {
            4
        } else {
            2
        }
    } else {
        0
    };

    let want_list = ui.effective_show_list(cfg) && h >= 14;
    let list_rows = if want_list {
        let cap = cfg.list_max_items.clamp(1, 24);
        let budget = if remaining > 1 {
            remaining.saturating_sub(1)
        } else if !want_art && inner_h > 9 {
            inner_h - 8
        } else {
            0
        };
        budget.min(cap)
    } else {
        0
    };

    LayoutPlan {
        term_w: w,
        term_h: h,
        box_w,
        show_art: want_art,
        art_cols,
        art_rows,
        lyric_lines,
        list_rows,
        eq_rows,
        show_footer,
    }
}

fn build_visible_list(st: &TntronStatus, cfg: &PopupPlayerConfig) -> Vec<PopupTrackItem> {
    match cfg.list_source.trim().to_ascii_lowercase().as_str() {
        "queue" => st.queue_items.clone(),
        "playlist" => st.playlist_items.clone(),
        _ => {
            // "upcoming": queued items first, then current + upcoming playlist items
            let mut out = st.queue_items.clone();
            if let Some(cur_pos) = st.playlist_items.iter().position(|x| x.is_current) {
                out.extend(st.playlist_items[cur_pos..].iter().cloned());
            } else {
                out.extend(st.playlist_items.iter().cloned());
            }
            out
        }
    }
}

struct BorderChars {
    tl: char,
    tr: char,
    bl: char,
    br: char,
    h: &'static str,
    v: &'static str,
}

fn border_chars(style: &str) -> BorderChars {
    match style.trim().to_ascii_lowercase().as_str() {
        "double" => BorderChars {
            tl: '╔',
            tr: '╗',
            bl: '╚',
            br: '╝',
            h: "═",
            v: "║",
        },
        "heavy" => BorderChars {
            tl: '┏',
            tr: '┓',
            bl: '┗',
            br: '┛',
            h: "━",
            v: "┃",
        },
        "minimal" => BorderChars {
            tl: ' ',
            tr: ' ',
            bl: ' ',
            br: ' ',
            h: " ",
            v: " ",
        },
        _ => BorderChars {
            tl: '╭',
            tr: '╮',
            bl: '╰',
            br: '╯',
            h: "─",
            v: "│",
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_frame(
    status: Option<&TntronStatus>,
    cfg: &PopupPlayerConfig,
    ui: &PopupUiState,
    plan: &LayoutPlan,
    art: Option<&CachedArt>,
    visible_list: &[PopupTrackItem],
    kitty_sent_for: &mut Option<String>,
    is_kitty: bool,
) -> Result<()> {
    let (ar, ag, ab) = resolve_reactive_rgb(status, cfg, ui.hue_deg);
    let accent = format!("\x1b[38;2;{ar};{ag};{ab}m");
    let dim_rgb = format!(
        "\x1b[38;2;{};{};{}m",
        (ar / 2).max(50),
        (ag / 2).max(50),
        (ab / 2).max(60)
    );
    let bg_ansi = if cfg.transparent_bg {
        ""
    } else {
        "\x1b[48;2;12;13;18m"
    };
    let bc = border_chars(&cfg.border_style);

    let mut frame = String::with_capacity(8192);
    frame.push_str("\x1b[H");
    frame.push_str(bg_ansi);
    frame.push_str(&format!(
        "{accent}{}{}{}\x1b[0m\x1b[K\n",
        bc.tl,
        bc.h.repeat(plan.box_w),
        bc.tr
    ));

    let Some(st) = status else {
        frame.push_str(&format!(
            "{accent}{}\x1b[0m  {accent}Waiting for tntron playback daemon... ({}x{})\x1b[0m\x1b[K\n",
            bc.v, plan.term_w, plan.term_h
        ));
        frame.push_str(&format!(
            "{accent}{}{}{}\x1b[0m\x1b[J",
            bc.bl,
            bc.h.repeat(plan.box_w),
            bc.br
        ));
        let mut out = io::stdout();
        out.write_all(frame.as_bytes())?;
        out.flush()?;
        return Ok(());
    };

    let state_icon = match st.state.as_str() {
        "playing" => "▶",
        "paused" => "❚❚",
        _ => "■",
    };
    let badges = if cfg.show_badges {
        let bpm_str = st.bpm.map(|b| format!(" · {b:.0} BPM")).unwrap_or_default();
        format!(
            "  [{} | Vol {:.0}% | {:.2}x{bpm_str}]",
            if st.shuffle { "Shuffle" } else { &st.loop_mode },
            st.volume,
            st.speed
        )
    } else {
        String::new()
    };
    let header = truncate_chars(
        &format!(" {state_icon}  {}{badges}", st.state.to_uppercase()),
        plan.box_w.saturating_sub(2),
    );
    frame.push_str(&format!(
        "{accent}{}\x1b[0m {accent}\x1b[1m{header}\x1b[0m\x1b[K\n",
        bc.v
    ));

    let art_mode = ui.effective_art_mode(cfg);
    let has_cover = art.is_some();
    let use_kitty_img =
        is_kitty && plan.show_art && matches!(art_mode, "cover" | "both") && art.is_some_and(|a| a.kitty_png_b64.is_some());
    let art_cols = if plan.show_art { plan.art_cols } else { 0 };
    let info_w = plan.box_w.saturating_sub(art_cols + 4).max(16);

    let mut info_lines = Vec::new();
    info_lines.push(format!("\x1b[1m{}\x1b[0m", truncate_chars(&st.title, info_w)));
    info_lines.push(format!(
        "\x1b[38;2;185;190;205m{}\x1b[0m",
        truncate_chars(
            if st.artist.is_empty() {
                "Unknown Artist"
            } else {
                &st.artist
            },
            info_w
        )
    ));
    if cfg.show_album && !st.album.is_empty() {
        info_lines.push(format!(
            "\x1b[38;2;130;136;150m{}\x1b[0m",
            truncate_chars(&st.album, info_w)
        ));
    }
    info_lines.push(String::new());
    info_lines.push(render_progress_bar(st, cfg, &accent, info_w));

    if plan.lyric_lines > 0 {
        info_lines.push(String::new());
        if st.lyrics_window.is_empty() {
            if !st.lyric.is_empty() {
                info_lines.push(format!(
                    "{accent}♪ {}\x1b[0m",
                    truncate_chars(&st.lyric, info_w)
                ));
            } else {
                info_lines.push("\x1b[38;2;115;120;135m♪ No synced lyrics\x1b[0m".into());
            }
        } else {
            for l in st.lyrics_window.iter().take(plan.lyric_lines) {
                let clean = truncate_chars(l, info_w);
                if l.starts_with('>') {
                    info_lines.push(format!("{accent}\x1b[1m{clean}\x1b[0m"));
                } else {
                    info_lines.push(format!("\x1b[38;2;150;156;172m{clean}\x1b[0m"));
                }
            }
        }
    }

    let bass = (st.bands[0] + st.bands[1] + st.bands[2]) / 3.0;
    let ascii_preset = ui.effective_ascii_preset(cfg);
    let ascii_lines = if plan.show_art && (!use_kitty_img || art_mode == "ascii") {
        if (art_mode == "ascii" || ascii_preset == "cover_ascii") && has_cover {
            art.map(|a| a.ascii_art.clone()).unwrap_or_default()
        } else if !has_cover || art_mode == "ascii" {
            render_ascii_art_lines(
                ascii_preset,
                ui.anim_phase,
                st.beat,
                bass,
                art_cols,
                plan.art_rows,
                &accent,
                &dim_rgb,
            )
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let rows_to_draw = if plan.show_art {
        plan.art_rows.max(info_lines.len().min(plan.art_rows + 2))
    } else {
        info_lines.len().max(4)
    };
    let art_row_start = 3usize;

    for r in 0..rows_to_draw {
        let info_str = info_lines.get(r).map(String::as_str).unwrap_or("");
        if plan.show_art {
            if use_kitty_img && r < plan.art_rows {
                let spacer = " ".repeat(art_cols);
                frame.push_str(&format!("{accent}{}\x1b[0m {spacer}  {info_str}\x1b[K\n", bc.v));
            } else if !ascii_lines.is_empty() && r < plan.art_rows {
                let art_str = ascii_lines.get(r).map(String::as_str).unwrap_or("");
                let pad = if art_str.is_empty() {
                    " ".repeat(art_cols)
                } else {
                    String::new()
                };
                frame.push_str(&format!(
                    "{accent}{}\x1b[0m {art_str}{pad}  {info_str}\x1b[K\n",
                    bc.v
                ));
            } else if has_cover && r < plan.art_rows {
                let art_str = art
                    .and_then(|a| a.half_blocks.get(r))
                    .map(String::as_str)
                    .unwrap_or("");
                frame.push_str(&format!("{accent}{}\x1b[0m {art_str}  {info_str}\x1b[K\n", bc.v));
            } else {
                let spacer = " ".repeat(art_cols);
                frame.push_str(&format!("{accent}{}\x1b[0m {spacer}  {info_str}\x1b[K\n", bc.v));
            }
        } else {
            frame.push_str(&format!("{accent}{}\x1b[0m  {info_str}\x1b[K\n", bc.v));
        }
    }

    // Track / Queue / Playlist list section (configurable & window-sized)
    if plan.list_rows > 0 {
        let list_title = match cfg.list_source.as_str() {
            "queue" => "QUEUE",
            "playlist" => "PLAYLIST",
            _ => "UPCOMING LIST",
        };
        let div_len = plan.box_w.saturating_sub(list_title.len() + 6);
        frame.push_str(&format!(
            "{accent}{}\x1b[0m {dim_rgb}── {list_title} {} \x1b[0m\x1b[K\n",
            bc.v,
            "─".repeat(div_len)
        ));
        if visible_list.is_empty() {
            frame.push_str(&format!(
                "{accent}{}\x1b[0m  \x1b[38;2;115;120;135m(List is empty)\x1b[0m\x1b[K\n",
                bc.v
            ));
        } else {
            let max_show = plan.list_rows.min(visible_list.len());
            let scroll_start = if ui.selected_list_row >= max_show {
                ui.selected_list_row + 1 - max_show
            } else {
                0
            };
            let title_w = plan.box_w.saturating_sub(14).max(12);
            for (offset, item) in visible_list
                .iter()
                .skip(scroll_start)
                .take(max_show)
                .enumerate()
            {
                let abs_i = scroll_start + offset;
                let selected = abs_i == ui.selected_list_row;
                let prefix = if item.is_current {
                    "▶"
                } else if item.is_queued {
                    "+"
                } else if selected {
                    ">"
                } else {
                    " "
                };
                let label = if item.artist.is_empty() {
                    item.title.clone()
                } else {
                    format!("{} — {}", item.title, item.artist)
                };
                let dur = format_mmss(item.duration);
                let line = truncate_chars(&label, title_w);
                let color = if item.is_current || selected {
                    format!("{accent}\x1b[1m")
                } else if item.is_queued {
                    "\x1b[38;2;180;210;255m".to_string()
                } else {
                    "\x1b[38;2;165;172;186m".to_string()
                };
                frame.push_str(&format!(
                    "{accent}{}\x1b[0m  {color}{prefix} {line:<width$}  {dur}\x1b[0m\x1b[K\n",
                    bc.v,
                    width = title_w
                ));
            }
        }
    }

    // Audio-reactive multi-row spectrum EQ
    if plan.eq_rows > 0 {
        render_eq_rows(&mut frame, st, cfg, plan, &accent, &dim_rgb, bc.v);
    }

    if plan.show_footer {
        let hint = if plan.list_rows > 0 {
            "[Space] Play  [p/n] Prev/Next  [j/k+Enter] List  [a/v] Art  [t] List  [q] Close"
        } else {
            "[Space] Play/Pause  [p/n] Prev/Next  [h/l] Seek  [a/v] Art  [-/+] Vol  [q] Close"
        };
        let cut = truncate_chars(hint, plan.box_w.saturating_sub(3));
        frame.push_str(&format!(
            "{accent}{}\x1b[0m  \x1b[38;2;130;136;150m{cut}\x1b[0m\x1b[K\n",
            bc.v
        ));
    }
    frame.push_str(&format!(
        "{accent}{}{}{}\x1b[0m\x1b[J",
        bc.bl,
        bc.h.repeat(plan.box_w),
        bc.br
    ));

    let mut out = io::stdout();
    out.write_all(frame.as_bytes())?;

    if use_kitty_img {
        if let Some(a) = art {
            if kitty_sent_for.as_deref() != Some(&a.path) {
                if let Some(b64) = &a.kitty_png_b64 {
                    emit_kitty_image(&mut out, b64, art_row_start, 3, art_cols, plan.art_rows)?;
                    *kitty_sent_for = Some(a.path.clone());
                }
            }
        }
    }
    out.flush()?;
    Ok(())
}

fn render_progress_bar(
    st: &TntronStatus,
    cfg: &PopupPlayerConfig,
    accent: &str,
    info_w: usize,
) -> String {
    let bar_w = info_w.saturating_sub(15).clamp(8, 48);
    let frac = if st.duration > 0.1 {
        (st.position / st.duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = ((bar_w as f64) * frac).round() as usize;
    let empty = bar_w.saturating_sub(filled);
    let (fill_ch, empty_ch) = match cfg.progress_style.trim().to_ascii_lowercase().as_str() {
        "blocks" => ("█", "░"),
        "dots" => ("●", "·"),
        "braille" => ("⣿", "⣀"),
        _ => ("━", "─"),
    };
    format!(
        "{accent}{}\x1b[38;2;75;80;95m{}\x1b[0m {} / {}",
        fill_ch.repeat(filled),
        empty_ch.repeat(empty),
        format_mmss(st.position),
        format_mmss(st.duration),
    )
}

fn render_eq_rows(
    frame: &mut String,
    st: &TntronStatus,
    cfg: &PopupPlayerConfig,
    plan: &LayoutPlan,
    accent: &str,
    dim_rgb: &str,
    border_v: &str,
) {
    let blocks = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let dots = [' ', '⡀', '⡄', '⡆', '⡇', '⣇', '⣧', '⣿'];
    let eq_w = plan.box_w.saturating_sub(4).clamp(16, 120);
    let style = cfg.eq_bars_style.trim().to_ascii_lowercase();
    let rows = plan.eq_rows.max(1);

    for row_idx in 0..rows {
        let mut eq_line = String::with_capacity(eq_w * 4);
        let row_from_bottom = rows - 1 - row_idx;
        for i in 0..eq_w {
            let u = i as f32 / (eq_w.saturating_sub(1).max(1) as f32);
            let sample_u = if style == "mirrored" {
                (u * 2.0 - 1.0).abs()
            } else {
                u
            };
            let f_idx = sample_u * 15.0;
            let lo = (f_idx.floor() as usize).min(15);
            let hi = (lo + 1).min(15);
            let frac = f_idx - lo as f32;
            let raw = (st.bands[lo] * (1.0 - frac) + st.bands[hi] * frac).clamp(0.0, 1.0);
            let scaled = raw * rows as f32 - row_from_bottom as f32;
            let cell = scaled.clamp(0.0, 1.0);
            let ch = if style == "wave" {
                let target_row = ((raw * (rows as f32 - 0.01)).floor() as usize).min(rows - 1);
                if target_row == row_from_bottom {
                    '~'
                } else if row_from_bottom < target_row {
                    '·'
                } else {
                    ' '
                }
            } else if style == "dots" {
                dots[((cell * 7.0).round() as usize).min(7)]
            } else if style == "bars" {
                if cell > 0.65 {
                    '┃'
                } else if cell > 0.25 {
                    '│'
                } else {
                    ' '
                }
            } else if cell <= 0.04 && row_from_bottom > 0 {
                ' '
            } else {
                blocks[((cell * 7.0).round() as usize).min(7)]
            };
            eq_line.push(ch);
        }
        let row_col = if row_idx == 0 { accent } else { dim_rgb };
        frame.push_str(&format!(
            "{accent}{border_v}\x1b[0m  {row_col}{eq_line}\x1b[0m\x1b[K\n"
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn render_ascii_art_lines(
    preset: &str,
    phase: f32,
    beat: f32,
    bass: f32,
    cols: usize,
    rows: usize,
    accent: &str,
    dim_rgb: &str,
) -> Vec<String> {
    let presets = ["vinyl", "cassette", "boombox", "note", "orbit", "wave", "eq_tower"];
    let effective = if preset == "auto" {
        let slot = ((phase / 12.0) as usize) % presets.len();
        presets[slot]
    } else {
        preset
    };

    let raw_art: Vec<String> = match effective {
        "cassette" => {
            let spin = ['|', '/', '-', '\\'][((phase * 3.0) as usize) % 4];
            vec![
                " ╭────────────────╮ ",
                " │ ● TNTRON  TAPE │ ",
                &format!(" │  ({spin}) ════ ({spin})  │ "),
                " │  ┌──────────┐  │ ",
                " │  │░░░░░░░░░░│  │ ",
                " │  └──────────┘  │ ",
                " │ ○  [══][══]  ○ │ ",
                " ╰──/──────────\\──╯ ",
            ]
            .into_iter()
            .map(ToString::to_string)
            .collect()
        }
        "boombox" => {
            let pulse = if beat > 0.45 || bass > 0.45 { '◉' } else { '◎' };
            let ring = if beat > 0.45 { 'O' } else { 'o' };
            vec![
                "    ╭──────────╮    ",
                " ╭──┴──────────┴──╮ ",
                " │ [≡≡≡≡≡  ≡≡≡≡≡] │ ",
                &format!(" │  ╭{ring}╮  {pulse}  ╭{ring}╮  │ "),
                &format!(" │  │{pulse}│ ─── │{pulse}│  │ "),
                &format!(" │  ╰{ring}╯ ─── ╰{ring}╯  │ "),
                " │  [◁  ▶  ▷ ───] │ ",
                " ╰────────────────╯ ",
            ]
            .into_iter()
            .map(ToString::to_string)
            .collect()
        }
        "note" => {
            let bob = ((phase * 2.5).sin() > 0.0) as usize;
            let s1 = if bob == 0 { "♪" } else { " " };
            let s2 = if bob == 1 { "♫" } else { " " };
            vec![
                &format!("   {s1}    ╭────╮   {s2} "),
                "      ╭─╯    ╰─╮    ",
                "      │  ♫  ♪  │    ",
                "      │ ╭────╮ │    ",
                "   ╭──╯ │    │ │    ",
                "  (████)╯   ╭┴─┴╮   ",
                "   ╰──╯    (████)   ",
                &format!("  {s2}         ╰──╯ {s1} "),
            ]
            .into_iter()
            .map(ToString::to_string)
            .collect()
        }
        "orbit" => {
            let pos = ((phase * 2.2) as usize) % 6;
            let star = |i| if i == pos { '✦' } else { '·' };
            vec![
                &format!("      {} ──── {}      ", star(0), star(1)),
                "    ╭───╭──╮───╮    ",
                &format!("  {} │  ╭████╮  │ {}  ", star(5), star(2)),
                "    │  │████│  │    ",
                "    │  ╰████╯  │    ",
                &format!("  {} ╰──────────╯ {}  ", star(4), star(3)),
                "      ·  ════  ·    ",
                "    *   TNTRON   *  ",
            ]
            .into_iter()
            .map(ToString::to_string)
            .collect()
        }
        "wave" => {
            let chars = ['~', '≈', '∿', '〜', '≈', '~'];
            let off = (phase * 4.0) as usize;
            let mut v = Vec::with_capacity(8);
            for r in 0..8 {
                let mut line = String::from(" ");
                for c in 0..18 {
                    let h = ((c + off + r * 2) as f32 * 0.48 + phase).sin();
                    let lvl = ((h + 1.0) * 1.5 + bass * 2.0) as usize;
                    let ch = if (r as isize - 3).abs() <= lvl as isize {
                        chars[(c + off + r) % chars.len()]
                    } else {
                        ' '
                    };
                    line.push(ch);
                }
                line.push(' ');
                v.push(line);
            }
            v
        }
        "eq_tower" => {
            let mut v = Vec::with_capacity(8);
            let pillars = 9usize;
            let heights: Vec<usize> = (0..pillars)
                .map(|i| {
                    let wave = ((phase * 3.0 + i as f32 * 0.7).sin() * 0.5 + 0.5) * 0.65;
                    let e = (wave + bass * 0.55 + beat * 0.35).clamp(0.1, 1.0);
                    (e * 7.0).round() as usize
                })
                .collect();
            for r in 0..8 {
                let row_lvl = 7 - r;
                let mut s = String::from(" ");
                for h in &heights {
                    s.push_str(if *h >= row_lvl { "█ " } else { "  " });
                }
                v.push(s);
            }
            v
        }
        _ => {
            // Spinning Vinyl Disc
            let spin = ['◐', '◓', '◑', '◒'][((phase * 3.5) as usize) % 4];
            let rim = if beat > 0.45 { '▓' } else { '▒' };
            vec![
                "     ╭────────╮     ",
                &format!("   ╭─{rim}{rim}────────{rim}{rim}─╮   "),
                "  ╱  ╭────────╮  ╲  ",
                &format!(" │ {rim} │   {spin}    │ {rim} │ "),
                &format!(" │ {rim} │  (●)   │ {rim} │ "),
                "  ╲  ╰────────╯  ╱  ",
                &format!("   ╰─{rim}{rim}────────{rim}{rim}─╯   "),
                "     ╰────────╯     ",
            ]
            .into_iter()
            .map(ToString::to_string)
            .collect()
        }
    };

    let mut out = Vec::with_capacity(rows);
    let top_pad = rows.saturating_sub(raw_art.len()) / 2;
    for r in 0..rows {
        if r < top_pad || r - top_pad >= raw_art.len() {
            out.push(" ".repeat(cols));
        } else {
            let line = &raw_art[r - top_pad];
            let trimmed = truncate_chars(line, cols);
            let char_len = trimmed.chars().count();
            let left_pad = cols.saturating_sub(char_len) / 2;
            let right_pad = cols.saturating_sub(char_len + left_pad);
            let col = if r % 2 == 0 { accent } else { dim_rgb };
            out.push(format!(
                "{}{col}{trimmed}\x1b[0m{}",
                " ".repeat(left_pad),
                " ".repeat(right_pad)
            ));
        }
    }
    out
}

fn emit_kitty_image(
    out: &mut io::Stdout,
    b64: &str,
    row: usize,
    col: usize,
    cols: usize,
    rows: usize,
) -> Result<()> {
    write!(out, "\x1b[{row};{col}H")?;
    let bytes = b64.as_bytes();
    let chunks: Vec<&[u8]> = bytes.chunks(4096).collect();
    for (idx, chunk) in chunks.iter().enumerate() {
        let more = if idx + 1 < chunks.len() { 1 } else { 0 };
        let s = std::str::from_utf8(chunk).unwrap_or("");
        if idx == 0 {
            write!(
                out,
                "\x1b_Gf=100,a=T,t=d,i=42,c={cols},r={rows},q=2,m={more};{s}\x1b\\"
            )?;
        } else {
            write!(out, "\x1b_Gm={more};{s}\x1b\\")?;
        }
    }
    Ok(())
}

fn build_cached_art(
    path: &str,
    cols: u32,
    rows: u32,
    with_kitty_png: bool,
) -> Option<CachedArt> {
    const SHADE_RAMP: &[u8] = b" .:-=+*#%@";
    let img = image::open(path).ok()?.to_rgba8();
    let pixel_w = cols.max(4);
    let pixel_h = (rows * 2).max(4);
    let small = image::imageops::resize(&img, pixel_w, pixel_h, FilterType::Triangle);

    let mut half_blocks = Vec::with_capacity(rows as usize);
    for r in 0..rows {
        let mut line = String::with_capacity((cols * 24) as usize);
        for c in 0..cols {
            let top = small.get_pixel(c, r * 2);
            let bot = small.get_pixel(c, r * 2 + 1);
            line.push_str(&format!(
                "\x1b[38;2;{};{};{};48;2;{};{};{}m▀",
                top[0], top[1], top[2], bot[0], bot[1], bot[2]
            ));
        }
        line.push_str("\x1b[0m");
        half_blocks.push(line);
    }

    let ascii_small = image::imageops::resize(&img, cols.max(4), rows.max(2), FilterType::Triangle);
    let mut ascii_art = Vec::with_capacity(rows as usize);
    for r in 0..rows {
        let mut line = String::with_capacity((cols * 16) as usize);
        for c in 0..cols {
            let px = ascii_small.get_pixel(c, r);
            let lum = (0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32) / 255.0;
            let idx = ((lum * (SHADE_RAMP.len() - 1) as f32).round() as usize)
                .min(SHADE_RAMP.len() - 1);
            let ch = SHADE_RAMP[idx] as char;
            line.push_str(&format!("\x1b[38;2;{};{};{}m{ch}", px[0], px[1], px[2]));
        }
        line.push_str("\x1b[0m");
        ascii_art.push(line);
    }

    let kitty_png_b64 = if with_kitty_png {
        let hi = image::imageops::resize(&img, 180, 180, FilterType::Triangle);
        let mut png_bytes = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
        if encoder
            .write_image(&hi, hi.width(), hi.height(), image::ExtendedColorType::Rgba8)
            .is_ok()
        {
            Some(encode_base64(&png_bytes))
        } else {
            None
        }
    } else {
        None
    };

    Some(CachedArt {
        path: path.to_string(),
        cols: cols as usize,
        rows: rows as usize,
        half_blocks,
        ascii_art,
        kitty_png_b64,
    })
}

fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(triple & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn resolve_reactive_rgb(
    status: Option<&TntronStatus>,
    cfg: &PopupPlayerConfig,
    hue_deg: f32,
) -> (u8, u8, u8) {
    if cfg.reactive_mode == "custom" {
        if let Some(hex) = &cfg.custom_hex {
            let clean = hex.trim().trim_start_matches('#');
            if clean.len() == 6 {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&clean[0..2], 16),
                    u8::from_str_radix(&clean[2..4], 16),
                    u8::from_str_radix(&clean[4..6], 16),
                ) {
                    return (r, g, b);
                }
            }
        }
    }
    if cfg.reactive_mode == "accent" {
        return (29, 185, 84);
    }
    let bass = status
        .map(|s| (s.bands[0] + s.bands[1] + s.bands[2]) / 3.0)
        .unwrap_or(0.0);
    let beat = status.map(|s| s.beat).unwrap_or(0.0);
    let deg = match cfg.reactive_mode.as_str() {
        "beat_pulse" => (12.0 + (1.0 - beat.clamp(0.0, 1.0)) * 210.0) % 360.0,
        "vocal_wave" => (270.0 + bass * 90.0 + hue_deg * 0.35) % 360.0,
        _ => hue_deg,
    };
    hsl_to_rgb8(deg, 0.85, (0.52 + beat * 0.16).clamp(0.45, 0.72))
}

fn hsl_to_rgb8(h_deg: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = h_deg.rem_euclid(360.0) / 360.0;
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let conv = |mut t: f32| -> u8 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    (conv(h + 1.0 / 3.0), conv(h), conv(h - 1.0 / 3.0))
}

fn terminal_size() -> (usize, usize) {
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        if libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
            return (ws.ws_col as usize, ws.ws_row as usize);
        }
    }
    (84, 24)
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    let prefix: String = s.chars().take(keep).collect();
    format!("{prefix}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popup_helpers() {
        assert_eq!(encode_base64(b"tntron"), "dG50cm9u");
        assert_eq!(truncate_chars("HelloWorld", 5), "Hell…");
        let cfg = PopupPlayerConfig {
            reactive_mode: "custom".into(),
            custom_hex: Some("#ff00aa".into()),
            ..Default::default()
        };
        assert_eq!(resolve_reactive_rgb(None, &cfg, 0.0), (255, 0, 170));
    }

    #[test]
    fn test_ascii_presets_and_dynamic_layout() {
        for preset in ["vinyl", "cassette", "boombox", "note", "orbit", "wave", "eq_tower"] {
            let lines = render_ascii_art_lines(preset, 1.2, 0.6, 0.5, 22, 10, "", "");
            assert_eq!(lines.len(), 10, "preset {preset} should match requested rows");
        }
        let cfg = PopupPlayerConfig::default();
        let ui = PopupUiState::new();
        let small = compute_layout_plan(44, 11, &cfg, &ui);
        assert!(!small.show_art, "small terminal should hide wide art automatically");
        let large = compute_layout_plan(120, 36, &cfg, &ui);
        assert!(large.show_art);
        assert!(large.list_rows > 0, "tall window should reserve list rows");
        assert!(large.art_cols > 20, "wide window should scale art columns up");
    }
}
