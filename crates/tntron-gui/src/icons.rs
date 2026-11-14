use iced::widget::text;
use iced::{Color, Font};

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

pub fn icon<'a>(glyph: &'a str, size: f32, color: Color) -> iced::widget::Text<'a> {
    text(glyph).font(ICON_FONT).size(size).color(color)
}

/// Resolves semantic extension icon names (e.g. "sliders", "clock", "music") to Nerd Font glyphs.
pub fn resolve_icon(name: &str) -> &str {
    match name.trim().to_ascii_lowercase().as_str() {
        "home" => HOME,
        "settings" | "cog" | "gear" => SETTINGS,
        "library" => LIBRARY,
        "music" | "note" | "sparkles" => MUSIC,
        "disc" | "album" => DISC,
        "search" => SEARCH,
        "folder" => FOLDER,
        "star" | "favorite" => STAR_FILLED,
        "play" => PLAY,
        "pause" => PAUSE,
        "prev" => PREV,
        "next" => NEXT,
        "shuffle" => SHUFFLE,
        "repeat" | "loop" => REPEAT,
        "volume" | "audio" => VOLUME_HIGH,
        "mute" => VOLUME_MUTE,
        "queue" | "playlist" => QUEUE,
        "bars" | "list" | "menu" => BARS,
        "plus" | "add" => PLUS,
        "close" => CLOSE,
        "refresh" | "reload" | "sync" => REFRESH,
        "sliders" | "eq" | "equalizer" | "dsp" => SLIDERS,
        "clock" | "timer" | "sleep" | "time" => CLOCK,
        "alert" | "warn" => ALERT,
        "bookmark" | "loop_ab" | "ab" => BOOKMARK,
        "bell" | "notify" | "notifications" => BELL,
        "download" => DOWNLOAD,
        "rss" | "podcast" => RSS,
        "lyrics" | "text" => LYRICS,
        "wave" | "visualizer" | "spectrum" => WAVE,
        "palette" | "theme" | "themes" => PALETTE,
        "globe" | "web" | "youtube" | "network" => GLOBE,
        "headphones" | "spatial" => HEADPHONES,
        "layers" | "layout" | "layouts" => LAYERS,
        "chart" | "stats" => CHART,
        "keyboard" | "hw" | "daeboard" => KEYBOARD,
        "gauge" | "bpm" | "tempo" | "details" => GAUGE,
        "puzzle" | "plugin" | "extension" => PUZZLE,
        _ => PUZZLE,
    }
}
