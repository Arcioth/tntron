use std::path::Path;

pub struct BundledExtension {
    pub slug: &'static str,
    pub manifest: &'static str,
    pub main_lua: &'static str,
}

macro_rules! ext {
    ($slug:expr) => {
        BundledExtension {
            slug: $slug,
            manifest: include_str!(concat!("../../../extensions/", $slug, "/manifest.json")),
            main_lua: include_str!(concat!("../../../extensions/", $slug, "/main.lua")),
        }
    };
}

pub const BUNDLED_EXTENSIONS: &[BundledExtension] = &[
    ext!("better-extensions"),
    ext!("better-folders"),
    ext!("better-home"),
    ext!("better-layouts"),
    ext!("better-library"),
    ext!("better-lyrics"),
    ext!("better-notifications"),
    ext!("better-player"),
    ext!("better-speed"),
    ext!("better-themes"),
    ext!("better-topbar"),
    ext!("better-ui"),
    ext!("better-visualizer"),
    ext!("button-packs"),
    ext!("compact-menu"),
    ext!("daetron-daeboard"),
    ext!("discord-rpc"),
    ext!("equalizer"),
    ext!("icon-packs"),
    ext!("listening-stats"),
    ext!("lyrics"),
    ext!("neon-theme-pack"),
    ext!("now-playing-notify"),
    ext!("playlists"),
    ext!("popup-player"),
    ext!("queue-manager"),
    ext!("reactive-ui"),
    ext!("rss-feeds"),
    ext!("section-loop"),
    ext!("sleep-timer"),
    ext!("song-details"),
    ext!("spatial-audio"),
    ext!("theme-festival"),
    ext!("track-announcer"),
    ext!("transparency"),
    ext!("visualizer"),
    ext!("yt-dlp"),
];

/// Seeds missing bundled extension folders into `extensions_dir` without overwriting user edits.
pub fn seed_missing_into(extensions_dir: &Path) {
    if std::fs::create_dir_all(extensions_dir).is_err() {
        return;
    }
    for b in BUNDLED_EXTENSIONS {
        let folder = extensions_dir.join(b.slug);
        if folder.exists() {
            continue;
        }
        if std::fs::create_dir_all(&folder).is_err() {
            continue;
        }
        let _ = std::fs::write(folder.join("manifest.json"), b.manifest);
        let _ = std::fs::write(folder.join("main.lua"), b.main_lua);
    }
}
