pub mod analyzer;
pub mod browser;
pub mod cache;
pub mod lrc;
pub mod scanner;
pub mod track;

pub use analyzer::{analyze_track_blocking, TrackAnalysis};
pub use browser::{
    copy_track_to_folder_in_jail, create_folder_in_jail, delete_file_in_jail, delete_folder_in_jail,
    read_directory, rename_in_jail, resolve_in_jail, scan_all_tracks_in_jail, BrowserEntry,
};
pub use cache::LruCache;
pub use lrc::{
    active_lyric_index, load_companion_lrc, parse_lrc, plain_text_to_timed_lines, LyricLine,
};
pub use scanner::{MetadataPatch, Scanner};
pub use track::{format_mmss, Track, TrackId};
