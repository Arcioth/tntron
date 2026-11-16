use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use iced::widget::text;
use iced::{Color, Font};
use tntron_core::IconPackConfig;

pub const ICON_FONT_BYTES: &[u8] = include_bytes!("../assets/NerdFontPropo.ttf");
pub const ICON_FONT: Font = Font::with_name("JetBrainsMono Nerd Font Propo");

pub const HOME: &str = "\u{f015}";
pub const SETTINGS: &str = "\u{f013}";
pub const LIBRARY: &str = "\u{f0322}";
pub const MUSIC: &str = "\u{f001}";
pub const DISC: &str = "\u{f01dc}";
pub const SEARCH: &str = "\u{f002}";
pub const FOLDER: &str = "\u{f07b}";
pub const STAR_FILLED: &str = "\u{f005}";
pub const STAR_OUTLINE: &str = "\u{f006}";
pub const PLAY: &str = "\u{f04b}";
pub const PAUSE: &str = "\u{f04c}";
pub const PREV: &str = "\u{f048}";
pub const NEXT: &str = "\u{f051}";
pub const SHUFFLE: &str = "\u{f074}";
pub const REPEAT: &str = "\u{f0456}";
pub const REPEAT_ONCE: &str = "\u{f0458}";
pub const VOLUME_HIGH: &str = "\u{f057e}";
pub const VOLUME_MUTE: &str = "\u{f0581}";
pub const QUEUE: &str = "\u{f0cb1}";
pub const BARS: &str = "\u{f0c9}";
pub const PLUS: &str = "\u{f067}";
pub const CLOSE: &str = "\u{f00d}";
pub const REFRESH: &str = "\u{f021}";
pub const ARROW_UP: &str = "\u{f062}";
pub const SLIDERS: &str = "\u{f1de}";
pub const PUZZLE: &str = "\u{f12e}";
pub const ALERT: &str = "\u{f071}";
pub const CLOCK: &str = "\u{f017}";
pub const BOOKMARK: &str = "\u{f00c0}";
pub const BELL: &str = "\u{f0f3}";
pub const DOWNLOAD: &str = "\u{f019}";
pub const RSS: &str = "\u{f09e}";
pub const LYRICS: &str = "\u{f036}";
pub const WAVE: &str = "\u{f0128}";
pub const PALETTE: &str = "\u{f03d8}";
pub const GLOBE: &str = "\u{f0ac}";
pub const HEADPHONES: &str = "\u{f025}";
pub const LAYERS: &str = "\u{f0328}";
pub const CHART: &str = "\u{f080}";
pub const KEYBOARD: &str = "\u{f11c}";
pub const GAUGE: &str = "\u{f0e4}";

thread_local! {
    static ACTIVE_PACK: RefCell<IconPackConfig> = RefCell::new(IconPackConfig::default());
    static ACTIVE_CUSTOM_FONT: RefCell<Option<Font>> = const { RefCell::new(None) };
}

static FONT_INTERN: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();

pub fn set_active_pack(cfg: &IconPackConfig) {
    ACTIVE_PACK.with(|cell| {
        *cell.borrow_mut() = cfg.clone();
    });
}

pub fn set_custom_font_family(family: Option<&str>) {
    let font = family.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        let cache = FONT_INTERN.get_or_init(|| Mutex::new(HashMap::new()));
        let mut guard = cache.lock().ok()?;
        let static_name: &'static str = if let Some(&s) = guard.get(trimmed) {
            s
        } else {
            let leaked: &'static str = Box::leak(trimmed.to_string().into_boxed_str());
            guard.insert(trimmed.to_string(), leaked);
            leaked
        };
        Some(Font::with_name(static_name))
    });
    ACTIVE_CUSTOM_FONT.with(|cell| {
        *cell.borrow_mut() = font;
    });
}

pub fn current_font() -> Font {
    ACTIVE_CUSTOM_FONT.with(|cell| cell.borrow().unwrap_or(ICON_FONT))
}

/// Parses an OpenType/TrueType `name` table to discover the font family name cleanly offline.
pub fn extract_ttf_family_name(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    let mut name_offset = 0usize;
    let mut name_len = 0usize;
    for i in 0..num_tables {
        let off = 12 + i * 16;
        if off + 16 > bytes.len() {
            break;
        }
        if &bytes[off..off + 4] == b"name" {
            name_offset = u32::from_be_bytes([bytes[off + 8], bytes[off + 9], bytes[off + 10], bytes[off + 11]]) as usize;
            name_len = u32::from_be_bytes([bytes[off + 12], bytes[off + 13], bytes[off + 14], bytes[off + 15]]) as usize;
            break;
        }
    }
    if name_offset == 0 || name_offset + 6 > bytes.len() {
        return None;
    }
    let table = &bytes[name_offset..bytes.len().min(name_offset + name_len.max(6))];
    let count = u16::from_be_bytes([table[2], table[3]]) as usize;
    let str_storage = u16::from_be_bytes([table[4], table[5]]) as usize;

    let mut fallback: Option<String> = None;
    for i in 0..count {
        let rec = 6 + i * 12;
        if rec + 12 > table.len() {
            break;
        }
        let platform_id = u16::from_be_bytes([table[rec], table[rec + 1]]);
        let name_id = u16::from_be_bytes([table[rec + 6], table[rec + 7]]);
        if name_id != 1 && name_id != 16 {
            continue;
        }
        let len = u16::from_be_bytes([table[rec + 8], table[rec + 9]]) as usize;
        let off = str_storage + u16::from_be_bytes([table[rec + 10], table[rec + 11]]) as usize;
        if off + len > table.len() || len == 0 {
            continue;
        }
        let raw = &table[off..off + len];
        let decoded = if platform_id == 0 || platform_id == 3 {
            let u16s: Vec<u16> = raw
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&u16s).ok()
        } else {
            Some(raw.iter().map(|&b| b as char).collect())
        };
        if let Some(s) = decoded.map(|x| x.trim().to_string()).filter(|x| !x.is_empty()) {
            if name_id == 16 && (platform_id == 3 || platform_id == 0) {
                return Some(s);
            }
            if fallback.is_none() || platform_id == 3 || platform_id == 0 {
                fallback = Some(s);
            }
        }
    }
    fallback
}

/// Resolves an imported font path (`~/` supported), detects its family via TTF parser or `fc-query`, and returns `(bytes, family)`.
pub fn load_custom_font_file(raw_path: &str) -> Option<(Vec<u8>, String)> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    let resolved = if let Some(rest) = trimmed.strip_prefix("~/") {
        let home = std::env::var_os("HOME")?;
        Path::new(&home).join(rest)
    } else {
        Path::new(trimmed).to_path_buf()
    };
    if !resolved.is_file() {
        return None;
    }
    let bytes = std::fs::read(&resolved).ok()?;
    let family = extract_ttf_family_name(&bytes).or_else(|| {
        std::process::Command::new("fc-query")
            .args(["-f", "%{family[0]}\n"])
            .arg(&resolved)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.lines().next().unwrap_or("").trim().to_string())
            .filter(|s| !s.is_empty())
    })?;
    Some((bytes, family))
}

pub fn adjusted_size(base_size: f32) -> f32 {
    ACTIVE_PACK.with(|cell| {
        let scale = cell.borrow().scale.clamp(0.7, 1.5);
        (base_size * scale).clamp(7.0, 64.0)
    })
}

pub fn tint_color(default: Color) -> Color {
    ACTIVE_PACK.with(|cell| {
        let cfg = cell.borrow();
        if cfg.tint_mode.eq_ignore_ascii_case("custom") {
            if let Some(hex) = &cfg.custom_hex {
                if let Some(col) = crate::theme::parse_hex_color(hex) {
                    return Color { a: default.a, ..col };
                }
            }
        }
        default
    })
}

pub fn icon<'a>(glyph: &str, size: f32, color: Color) -> iced::widget::Text<'a> {
    text(map_glyph(glyph))
        .font(current_font())
        .size(adjusted_size(size))
        .color(tint_color(color))
}

/// Maps semantic icon names OR default Nerd Font glyph constants through the active icon pack & user overrides.
pub fn map_glyph(input: &str) -> String {
    let slot = canonical_slot(input);
    ACTIVE_PACK.with(|cell| {
        let cfg = cell.borrow();
        if let Some(ov) = cfg.overrides.get(slot) {
            if let Some(parsed) = parse_custom_glyph(ov) {
                return parsed;
            }
        }
        pack_glyph(&cfg.pack, slot).to_string()
    })
}

/// Parses literal strings/emojis OR hex codepoint specs (`"f005"`, `"U+f005"`, `"0xf005"`, `"\u{f005}"`).
pub fn parse_custom_glyph(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    let hex_cand = t
        .strip_prefix("\\u{")
        .and_then(|s| s.strip_suffix('}'))
        .or_else(|| t.strip_prefix("U+"))
        .or_else(|| t.strip_prefix("u+"))
        .or_else(|| t.strip_prefix("0x"))
        .or_else(|| t.strip_prefix("0X"));
    if let Some(hex) = hex_cand {
        if let Ok(cp) = u32::from_str_radix(hex.trim(), 16) {
            if let Some(ch) = char::from_u32(cp) {
                return Some(ch.to_string());
            }
        }
    }
    if t.len() >= 4
        && t.len() <= 6
        && t.chars().all(|c| c.is_ascii_hexdigit())
    {
        if let Ok(cp) = u32::from_str_radix(t, 16) {
            if let Some(ch) = char::from_u32(cp) {
                return Some(ch.to_string());
            }
        }
    }
    Some(t.to_string())
}

fn canonical_slot(input: &str) -> &str {
    match input {
        HOME => "home",
        SETTINGS => "settings",
        LIBRARY => "library",
        MUSIC => "music",
        DISC => "disc",
        SEARCH => "search",
        FOLDER => "folder",
        STAR_FILLED => "star",
        STAR_OUTLINE => "star_outline",
        PLAY => "play",
        PAUSE => "pause",
        PREV => "prev",
        NEXT => "next",
        SHUFFLE => "shuffle",
        REPEAT => "repeat",
        REPEAT_ONCE => "repeat_once",
        VOLUME_HIGH => "volume",
        VOLUME_MUTE => "mute",
        QUEUE => "queue",
        BARS => "bars",
        PLUS => "plus",
        CLOSE => "close",
        REFRESH => "refresh",
        ARROW_UP => "arrow_up",
        SLIDERS => "sliders",
        PUZZLE => "puzzle",
        ALERT => "alert",
        CLOCK => "clock",
        BOOKMARK => "bookmark",
        BELL => "bell",
        DOWNLOAD => "download",
        RSS => "rss",
        LYRICS => "lyrics",
        WAVE => "wave",
        PALETTE => "palette",
        GLOBE => "globe",
        HEADPHONES => "headphones",
        LAYERS => "layers",
        CHART => "chart",
        KEYBOARD => "keyboard",
        GAUGE => "gauge",
        other => match other.trim().to_ascii_lowercase().as_str() {
            "home" => "home",
            "settings" | "cog" | "gear" => "settings",
            "library" => "library",
            "music" | "note" | "sparkles" => "music",
            "disc" | "album" => "disc",
            "search" => "search",
            "folder" => "folder",
            "star" | "favorite" => "star",
            "star_outline" => "star_outline",
            "play" => "play",
            "pause" => "pause",
            "prev" => "prev",
            "next" => "next",
            "shuffle" => "shuffle",
            "repeat" | "loop" => "repeat",
            "repeat_once" => "repeat_once",
            "volume" | "audio" => "volume",
            "mute" => "mute",
            "queue" | "playlist" => "queue",
            "bars" | "list" | "menu" => "bars",
            "plus" | "add" => "plus",
            "close" => "close",
            "refresh" | "reload" | "sync" => "refresh",
            "arrow_up" | "up" => "arrow_up",
            "sliders" | "eq" | "equalizer" | "dsp" => "sliders",
            "clock" | "timer" | "sleep" | "time" => "clock",
            "alert" | "warn" => "alert",
            "bookmark" | "loop_ab" | "ab" => "bookmark",
            "bell" | "notify" | "notifications" => "bell",
            "download" => "download",
            "rss" | "podcast" => "rss",
            "lyrics" | "text" => "lyrics",
            "wave" | "visualizer" | "spectrum" => "wave",
            "palette" | "theme" | "themes" => "palette",
            "globe" | "web" | "youtube" | "network" => "globe",
            "headphones" | "spatial" => "headphones",
            "layers" | "layout" | "layouts" => "layers",
            "chart" | "stats" => "chart",
            "keyboard" | "hw" | "daeboard" => "keyboard",
            "gauge" | "bpm" | "tempo" | "details" => "gauge",
            _ => "puzzle",
        },
    }
}

fn pack_glyph(pack: &str, slot: &str) -> &'static str {
    match pack.trim().to_ascii_lowercase().as_str() {
        "material" => match slot {
            "home" => "\u{f02dc}",
            "settings" => "\u{f0493}",
            "library" => "\u{f0331}",
            "music" => "\u{f075a}",
            "disc" => "\u{f0026}",
            "search" => "\u{f0349}",
            "folder" => "\u{f024b}",
            "star" => "\u{f04ce}",
            "star_outline" => "\u{f04d2}",
            "play" => "\u{f040a}",
            "pause" => "\u{f03e4}",
            "prev" => "\u{f04ae}",
            "next" => "\u{f04ad}",
            "shuffle" => "\u{f049d}",
            "repeat" => "\u{f0456}",
            "repeat_once" => "\u{f0458}",
            "volume" => "\u{f057e}",
            "mute" => "\u{f0581}",
            "queue" => "\u{f0cb1}",
            "bars" => "\u{f035c}",
            "plus" => "\u{f0415}",
            "close" => "\u{f0156}",
            "refresh" => "\u{f0450}",
            "arrow_up" => "\u{f005d}",
            "sliders" => "\u{f062e}",
            "alert" => "\u{f0028}",
            "clock" => "\u{f0954}",
            "bookmark" => "\u{f00c0}",
            "bell" => "\u{f009a}",
            "download" => "\u{f01da}",
            "rss" => "\u{f046b}",
            "lyrics" => "\u{f09a9}",
            "wave" => "\u{f0128}",
            "palette" => "\u{f03d8}",
            "globe" => "\u{f059f}",
            "headphones" => "\u{f02cb}",
            "layers" => "\u{f0328}",
            "chart" => "\u{f012a}",
            "keyboard" => "\u{f030c}",
            "gauge" => "\u{f029a}",
            _ => "\u{f075f}",
        },
        "octicons" => match slot {
            "home" => "\u{f46d}",
            "settings" => "\u{f423}",
            "library" => "\u{f405}",
            "music" => "\u{f4c2}",
            "disc" => "\u{f41b}",
            "search" => "\u{f422}",
            "folder" => "\u{f413}",
            "star" => "\u{f41e}",
            "star_outline" => "\u{f41f}",
            "play" => "\u{f407}",
            "pause" => "\u{f04c}",
            "prev" => "\u{f47c}",
            "next" => "\u{f47d}",
            "shuffle" => "\u{f074}",
            "repeat" => "\u{f46a}",
            "repeat_once" => "\u{f0458}",
            "volume" => "\u{f485}",
            "mute" => "\u{f466}",
            "queue" => "\u{f468}",
            "bars" => "\u{f44e}",
            "plus" => "\u{f44d}",
            "close" => "\u{f467}",
            "refresh" => "\u{f46a}",
            "arrow_up" => "\u{f431}",
            "sliders" => "\u{f4e3}",
            "alert" => "\u{f421}",
            "clock" => "\u{f43a}",
            "bookmark" => "\u{f461}",
            "bell" => "\u{f49a}",
            "download" => "\u{f409}",
            "rss" => "\u{f427}",
            "lyrics" => "\u{f440}",
            "wave" => "\u{f408}",
            "palette" => "\u{f447}",
            "globe" => "\u{f484}",
            "headphones" => "\u{f025}",
            "layers" => "\u{f4d7}",
            "chart" => "\u{f437}",
            "keyboard" => "\u{f11c}",
            "gauge" => "\u{f4a0}",
            _ => "\u{f497}",
        },
        "codicons" => match slot {
            "home" => "\u{eb06}",
            "settings" => "\u{eb51}",
            "library" => "\u{ea73}",
            "music" => "\u{f001}",
            "disc" => "\u{eb6b}",
            "search" => "\u{ea6d}",
            "folder" => "\u{ea83}",
            "star" => "\u{eb59}",
            "star_outline" => "\u{eb59}",
            "play" => "\u{eb2c}",
            "pause" => "\u{eb1e}",
            "prev" => "\u{ea9b}",
            "next" => "\u{ea9c}",
            "shuffle" => "\u{f074}",
            "repeat" => "\u{eb37}",
            "repeat_once" => "\u{eb37}",
            "volume" => "\u{f057e}",
            "mute" => "\u{eb7c}",
            "queue" => "\u{eb0c}",
            "bars" => "\u{eb03}",
            "plus" => "\u{ea60}",
            "close" => "\u{ea76}",
            "refresh" => "\u{eb37}",
            "arrow_up" => "\u{eaa1}",
            "sliders" => "\u{eb52}",
            "alert" => "\u{ea6c}",
            "clock" => "\u{ea6e}",
            "bookmark" => "\u{ea8b}",
            "bell" => "\u{eaa2}",
            "download" => "\u{eaa8}",
            "rss" => "\u{eb38}",
            "lyrics" => "\u{eb15}",
            "wave" => "\u{eb2e}",
            "palette" => "\u{eb75}",
            "globe" => "\u{eb01}",
            "headphones" => "\u{f025}",
            "layers" => "\u{eb0b}",
            "chart" => "\u{eaf3}",
            "keyboard" => "\u{f11c}",
            "gauge" => "\u{ea7f}",
            _ => "\u{eaf4}",
        },
        "fontawesome" => match slot {
            "home" => "\u{f015}",
            "settings" => "\u{f085}",
            "library" => "\u{f02d}",
            "music" => "\u{f001}",
            "disc" => "\u{f51f}",
            "search" => "\u{f002}",
            "folder" => "\u{f07c}",
            "star" => "\u{f005}",
            "star_outline" => "\u{f006}",
            "play" => "\u{f144}",
            "pause" => "\u{f28b}",
            "prev" => "\u{f049}",
            "next" => "\u{f050}",
            "shuffle" => "\u{f074}",
            "repeat" => "\u{f01e}",
            "repeat_once" => "\u{f2f9}",
            "volume" => "\u{f028}",
            "mute" => "\u{f026}",
            "queue" => "\u{f0cb}",
            "bars" => "\u{f0c9}",
            "plus" => "\u{f055}",
            "close" => "\u{f057}",
            "refresh" => "\u{f021}",
            "arrow_up" => "\u{f0aa}",
            "sliders" => "\u{f1de}",
            "alert" => "\u{f06a}",
            "clock" => "\u{f017}",
            "bookmark" => "\u{f02e}",
            "bell" => "\u{f0f3}",
            "download" => "\u{f0ab}",
            "rss" => "\u{f09e}",
            "lyrics" => "\u{f044}",
            "wave" => "\u{f83e}",
            "palette" => "\u{f53f}",
            "globe" => "\u{f57d}",
            "headphones" => "\u{f58f}",
            "layers" => "\u{f5fd}",
            "chart" => "\u{f201}",
            "keyboard" => "\u{f11c}",
            "gauge" => "\u{f625}",
            _ => "\u{f12e}",
        },
        "weather" => match slot {
            "home" => "\u{e30d}",
            "settings" => "\u{e313}",
            "library" => "\u{e302}",
            "music" => "\u{e359}",
            "disc" => "\u{e32b}",
            "search" => "\u{e374}",
            "folder" => "\u{e312}",
            "star" => "\u{e370}",
            "star_outline" => "\u{e371}",
            "play" => "\u{e30d}",
            "pause" => "\u{e32c}",
            "prev" => "\u{e34a}",
            "next" => "\u{e34b}",
            "shuffle" => "\u{e33c}",
            "repeat" => "\u{e33e}",
            "repeat_once" => "\u{e33f}",
            "volume" => "\u{e37e}",
            "mute" => "\u{e37f}",
            "queue" => "\u{e311}",
            "bars" => "\u{e35e}",
            "plus" => "\u{e31f}",
            "close" => "\u{e36f}",
            "refresh" => "\u{e352}",
            "arrow_up" => "\u{e353}",
            "sliders" => "\u{e354}",
            "alert" => "\u{e31d}",
            "clock" => "\u{e381}",
            "bookmark" => "\u{e36b}",
            "bell" => "\u{e317}",
            "download" => "\u{e319}",
            "rss" => "\u{e350}",
            "lyrics" => "\u{e341}",
            "wave" => "\u{e323}",
            "palette" => "\u{e34c}",
            "globe" => "\u{e36e}",
            "headphones" => "\u{e321}",
            "layers" => "\u{e32e}",
            "chart" => "\u{e329}",
            "keyboard" => "\u{e344}",
            "gauge" => "\u{e355}",
            _ => "\u{e365}",
        },
        "emoji" => match slot {
            "home" => "🏠",
            "settings" => "⚙️",
            "library" => "📚",
            "music" => "🎵",
            "disc" => "💿",
            "search" => "🔍",
            "folder" => "📁",
            "star" => "⭐",
            "star_outline" => "☆",
            "play" => "▶",
            "pause" => "⏸",
            "prev" => "⏮",
            "next" => "⏭",
            "shuffle" => "🔀",
            "repeat" => "🔁",
            "repeat_once" => "🔂",
            "volume" => "🔊",
            "mute" => "🔇",
            "queue" => "🧾",
            "bars" => "☰",
            "plus" => "➕",
            "close" => "✖",
            "refresh" => "🔄",
            "arrow_up" => "⬆",
            "sliders" => "🎛",
            "alert" => "⚠",
            "clock" => "🕒",
            "bookmark" => "🔖",
            "bell" => "🔔",
            "download" => "⬇",
            "rss" => "📡",
            "lyrics" => "📝",
            "wave" => "🌊",
            "palette" => "🎨",
            "globe" => "🌐",
            "headphones" => "🎧",
            "layers" => "🪟",
            "chart" => "📊",
            "keyboard" => "⌨",
            "gauge" => "⏱",
            _ => "🧩",
        },
        "arcade" => match slot {
            "home" => "⌂",
            "settings" => "❖",
            "library" => "▣",
            "music" => "♫",
            "disc" => "◉",
            "search" => "⌖",
            "folder" => "▤",
            "star" => "★",
            "star_outline" => "☆",
            "play" => "▶",
            "pause" => "❚❚",
            "prev" => "◀◀",
            "next" => "▶▶",
            "shuffle" => "⇄",
            "repeat" => "⟳",
            "repeat_once" => "①",
            "volume" => "▮",
            "mute" => "▯",
            "queue" => "≡",
            "bars" => "⊞",
            "plus" => "✚",
            "close" => "✖",
            "refresh" => "↺",
            "arrow_up" => "▲",
            "sliders" => "⫯",
            "alert" => "⚡",
            "clock" => "◷",
            "bookmark" => "⚑",
            "bell" => "◈",
            "download" => "▼",
            "rss" => "◎",
            "lyrics" => "¶",
            "wave" => "∿",
            "palette" => "◈",
            "globe" => "⊕",
            "headphones" => "∩",
            "layers" => "❐",
            "chart" => "▇",
            "keyboard" => "⌨",
            "gauge" => "◔",
            _ => "◈",
        },
        "kawaii" => match slot {
            "home" => "♡",
            "settings" => "✿",
            "library" => "❀",
            "music" => "♪",
            "disc" => "✧",
            "search" => "✦",
            "folder" => "❁",
            "star" => "★",
            "star_outline" => "☆",
            "play" => "▶",
            "pause" => "‖",
            "prev" => "≪",
            "next" => "≫",
            "shuffle" => "♾",
            "repeat" => "↻",
            "repeat_once" => "①",
            "volume" => "♫",
            "mute" => "☁",
            "queue" => "☰",
            "bars" => "❀",
            "plus" => "⊹",
            "close" => "✕",
            "refresh" => "✿",
            "arrow_up" => "↑",
            "sliders" => "✧",
            "alert" => "⚡",
            "clock" => "◔",
            "bookmark" => "❦",
            "bell" => "🔔",
            "download" => "↓",
            "rss" => "✨",
            "lyrics" => "✎",
            "wave" => "≈",
            "palette" => "🌸",
            "globe" => "☀",
            "headphones" => "🎧",
            "layers" => "❁",
            "chart" => "☄",
            "keyboard" => "⌨",
            "gauge" => "✾",
            _ => "❄",
        },
        "cyber" => match slot {
            "home" => "⟨H⟩",
            "settings" => "⛭",
            "library" => "▤",
            "music" => "♫",
            "disc" => "◎",
            "search" => "⌖",
            "folder" => "◰",
            "star" => "◆",
            "star_outline" => "◇",
            "play" => "▷",
            "pause" => "‖",
            "prev" => "◁◁",
            "next" => "▷▷",
            "shuffle" => "⤨",
            "repeat" => "⟲",
            "repeat_once" => "①",
            "volume" => "▁█",
            "mute" => "▁×",
            "queue" => "≡",
            "bars" => "⊞",
            "plus" => "+",
            "close" => "╳",
            "refresh" => "⟳",
            "arrow_up" => "△",
            "sliders" => "⫯",
            "alert" => "⚠",
            "clock" => "◷",
            "bookmark" => "⚑",
            "bell" => "◈",
            "download" => "▽",
            "rss" => "◉",
            "lyrics" => "¶",
            "wave" => "∿",
            "palette" => "◈",
            "globe" => "⊕",
            "headphones" => "∩",
            "layers" => "❐",
            "chart" => "▊",
            "keyboard" => "⌘",
            "gauge" => "◔",
            _ => "⬡",
        },
        "minimal" => match slot {
            "home" => "⌂",
            "settings" => "⚙",
            "library" => "▤",
            "music" => "♫",
            "disc" => "◎",
            "search" => "⌕",
            "folder" => "▢",
            "star" => "★",
            "star_outline" => "☆",
            "play" => "▶",
            "pause" => "❚❚",
            "prev" => "◁",
            "next" => "▷",
            "shuffle" => "⇄",
            "repeat" => "↻",
            "repeat_once" => "①",
            "volume" => "♪",
            "mute" => "✕",
            "queue" => "≡",
            "bars" => "☰",
            "plus" => "+",
            "close" => "✕",
            "refresh" => "⟳",
            "arrow_up" => "↑",
            "sliders" => "⊹",
            "alert" => "▲",
            "clock" => "◷",
            "bookmark" => "⚑",
            "bell" => "◈",
            "download" => "↓",
            "rss" => "◉",
            "lyrics" => "¶",
            "wave" => "∿",
            "palette" => "◐",
            "globe" => "⊕",
            "headphones" => "∩",
            "layers" => "❏",
            "chart" => "▊",
            "keyboard" => "⌨",
            "gauge" => "◔",
            _ => "◇",
        },
        "ascii" => match slot {
            "home" => "[H]",
            "settings" => "[*]",
            "library" => "[L]",
            "music" => "[M]",
            "disc" => "(o)",
            "search" => "[?]",
            "folder" => "[D]",
            "star" => "[+]",
            "star_outline" => "[ ]",
            "play" => "|>",
            "pause" => "||",
            "prev" => "<<",
            "next" => ">>",
            "shuffle" => "<~>",
            "repeat" => "[R]",
            "repeat_once" => "[1]",
            "volume" => "V+",
            "mute" => "Vx",
            "queue" => "[Q]",
            "bars" => "[=]",
            "plus" => "+",
            "close" => "x",
            "refresh" => "(R)",
            "arrow_up" => "^",
            "sliders" => "[S]",
            "alert" => "!",
            "clock" => "[T]",
            "bookmark" => "[B]",
            "bell" => "[!]",
            "download" => "[v]",
            "rss" => "[F]",
            "lyrics" => "[T]",
            "wave" => "~~~",
            "palette" => "[C]",
            "globe" => "[@]",
            "headphones" => "[A]",
            "layers" => "[#]",
            "chart" => "[%]",
            "keyboard" => "[K]",
            "gauge" => "[G]",
            _ => "[P]",
        },
        _ => match slot {
            "home" => HOME,
            "settings" => SETTINGS,
            "library" => LIBRARY,
            "music" => MUSIC,
            "disc" => DISC,
            "search" => SEARCH,
            "folder" => FOLDER,
            "star" => STAR_FILLED,
            "star_outline" => STAR_OUTLINE,
            "play" => PLAY,
            "pause" => PAUSE,
            "prev" => PREV,
            "next" => NEXT,
            "shuffle" => SHUFFLE,
            "repeat" => REPEAT,
            "repeat_once" => REPEAT_ONCE,
            "volume" => VOLUME_HIGH,
            "mute" => VOLUME_MUTE,
            "queue" => QUEUE,
            "bars" => BARS,
            "plus" => PLUS,
            "close" => CLOSE,
            "refresh" => REFRESH,
            "arrow_up" => ARROW_UP,
            "sliders" => SLIDERS,
            "alert" => ALERT,
            "clock" => CLOCK,
            "bookmark" => BOOKMARK,
            "bell" => BELL,
            "download" => DOWNLOAD,
            "rss" => RSS,
            "lyrics" => LYRICS,
            "wave" => WAVE,
            "palette" => PALETTE,
            "globe" => GLOBE,
            "headphones" => HEADPHONES,
            "layers" => LAYERS,
            "chart" => CHART,
            "keyboard" => KEYBOARD,
            "gauge" => GAUGE,
            _ => PUZZLE,
        },
    }
}

/// Resolves semantic extension icon names (e.g. "sliders", "clock", "music") to Nerd Font glyphs.
pub fn resolve_icon(name: &str) -> &'static str {
    pack_glyph("nerd", canonical_slot(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_pack_switching_and_custom_override() {
        let mut cfg = IconPackConfig {
            pack: "ascii".into(),
            ..Default::default()
        };
        cfg.overrides.insert("play".into(), "U+25B6".into());
        set_active_pack(&cfg);

        assert_eq!(map_glyph(HOME), "[H]");
        assert_eq!(map_glyph(PLAY), "▶");

        set_active_pack(&IconPackConfig::default());
        assert_eq!(map_glyph(HOME), HOME);
        assert!(extract_ttf_family_name(ICON_FONT_BYTES).is_some());
    }
}
