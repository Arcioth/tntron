use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::library::track::Track;

#[derive(Debug, Clone)]
pub enum BrowserEntry {
    ParentDir(PathBuf),
    Directory { name: String, path: PathBuf },
    AudioTrack(Arc<Track>),
}

impl BrowserEntry {
    pub fn is_dir(&self) -> bool {
        matches!(self, Self::ParentDir(_) | Self::Directory { .. })
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::ParentDir(p) => p,
            Self::Directory { path, .. } => path,
            Self::AudioTrack(t) => &t.path,
        }
    }
}

/// Canonical jail resolver: returns `Some(canonical_path)` iff `candidate` resides inside `canonical_root`.
pub fn resolve_in_jail(canonical_root: &Path, candidate: &Path) -> Option<PathBuf> {
    let canon_cand = candidate.canonicalize().ok()?;
    if canon_cand == canonical_root || canon_cand.starts_with(canonical_root) {
        Some(canon_cand)
    } else {
        None
    }
}

/// Reads directories & audio files strictly inside `root_boundary` (symlinks are skipped).
pub fn read_directory(dir: &Path, root_boundary: &Path) -> Vec<BrowserEntry> {
    let mut items = Vec::new();

    if dir != root_boundary && dir.starts_with(root_boundary) {
        if let Some(parent) = dir.parent() {
            if let Some(jailed_parent) = resolve_in_jail(root_boundary, parent) {
                items.push(BrowserEntry::ParentDir(jailed_parent));
            }
        }
    }

    if let Ok(entries) = fs::read_dir(dir) {
        let mut dirs = Vec::new();
        let mut tracks = Vec::new();

        for entry in entries.filter_map(|e| e.ok()) {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }
            let path = entry.path();
            if ft.is_dir() {
                if let Some(jailed_dir) = resolve_in_jail(root_boundary, &path) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    dirs.push(BrowserEntry::Directory {
                        name,
                        path: jailed_dir,
                    });
                }
            } else if ft.is_file() && Track::is_audio_file(&path) {
                tracks.push(Track::new(path));
            }
        }

        dirs.sort_by(|a, b| match (a, b) {
            (
                BrowserEntry::Directory { name: an, .. },
                BrowserEntry::Directory { name: bn, .. },
            ) => ascii_case_cmp(an, bn),
            _ => std::cmp::Ordering::Equal,
        });

        tracks.sort_by(|a, b| ascii_case_cmp(&a.filename, &b.filename));

        items.extend(dirs);
        for track in tracks {
            items.push(BrowserEntry::AudioTrack(Arc::new(track)));
        }
    }

    items
}

fn is_ignored_dir_name(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "node_modules" | "__pycache__")
}

fn dir_has_direct_audio(dir: &Path) -> bool {
    let Ok(rd) = fs::read_dir(dir) else {
        return false;
    };
    for entry in rd.filter_map(|e| e.ok()) {
        if let Ok(ft) = entry.file_type() {
            if ft.is_file() && Track::is_audio_file(&entry.path()) {
                return true;
            }
        }
    }
    false
}

/// Collects direct folders and audio-containing subfolders inside `root_boundary` as playlists.
pub fn scan_playlist_folders_in_jail(root_boundary: &Path) -> Vec<(String, PathBuf)> {
    const MAX_DEPTH: usize = 4;

    fn walk(
        dir: &Path,
        root: &Path,
        prefix: &str,
        depth: usize,
        out: &mut Vec<(String, PathBuf)>,
    ) -> bool {
        let Ok(entries) = fs::read_dir(dir) else {
            return false;
        };
        let mut subdirs: Vec<(String, PathBuf)> = Vec::new();
        for entry in entries.filter_map(|e| e.ok()) {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_symlink() || !ft.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if is_ignored_dir_name(&name) {
                continue;
            }
            if let Some(jailed) = resolve_in_jail(root, &entry.path()) {
                subdirs.push((name, jailed));
            }
        }
        subdirs.sort_by(|a, b| ascii_case_cmp(&a.0, &b.0));

        let mut any_audio_below = false;
        for (name, path) in subdirs {
            let label = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix} / {name}")
            };
            let direct_audio = dir_has_direct_audio(&path);
            let mut nested = Vec::new();
            let nested_has_audio = if depth < MAX_DEPTH {
                walk(&path, root, &label, depth + 1, &mut nested)
            } else {
                false
            };

            if depth == 1 || direct_audio || nested_has_audio {
                out.push((label, path));
                out.extend(nested);
                any_audio_below = true;
            }
        }
        any_audio_below
    }

    let mut out = Vec::new();
    walk(root_boundary, root_boundary, "", 1, &mut out);
    out
}

/// Recursively collects all non-symlink audio tracks inside `root_boundary`, sorted by path.
pub fn scan_all_tracks_in_jail(root_boundary: &Path) -> Vec<Track> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<Track>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        let mut subdirs = Vec::new();
        let mut files = Vec::new();
        for entry in entries.filter_map(|e| e.ok()) {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_symlink() {
                continue;
            }
            let p = entry.path();
            if ft.is_dir() {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                if is_ignored_dir_name(&name) {
                    continue;
                }
                if let Some(jailed) = resolve_in_jail(root, &p) {
                    subdirs.push(jailed);
                }
            } else if ft.is_file() && Track::is_audio_file(&p) {
                files.push(p);
            }
        }
        subdirs.sort_by(|a, b| ascii_case_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
        files.sort_by(|a, b| ascii_case_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
        for f in files {
            out.push(Track::new(f));
        }
        for sd in subdirs {
            walk(&sd, root, out);
        }
    }

    let mut out = Vec::new();
    walk(root_boundary, root_boundary, &mut out);
    out
}

/// Validates a single folder/file component name so it cannot traverse directories.
pub fn validate_single_name(name: &str) -> Result<&str, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return Err("Invalid empty or relative name".into());
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains('\0') {
        return Err("Name cannot contain path separators".into());
    }
    Ok(trimmed)
}

/// Creates a subdirectory strictly inside `root_boundary`.
pub fn create_folder_in_jail(
    root_boundary: &Path,
    parent: &Path,
    folder_name: &str,
) -> Result<PathBuf, String> {
    let clean = validate_single_name(folder_name)?;
    let canon_parent = resolve_in_jail(root_boundary, parent)
        .ok_or_else(|| "Parent directory outside music jail".to_string())?;
    let target = canon_parent.join(clean);
    if target.exists() {
        return Err(format!("Folder '{}' already exists", clean));
    }
    fs::create_dir(&target).map_err(|e| format!("Failed to create folder: {e}"))?;
    resolve_in_jail(root_boundary, &target)
        .ok_or_else(|| "Created folder escaped music jail".to_string())
}

/// Renames a file or directory strictly inside `root_boundary`.
pub fn rename_in_jail(
    root_boundary: &Path,
    target: &Path,
    new_name: &str,
) -> Result<PathBuf, String> {
    let clean = validate_single_name(new_name)?;
    let canon_target = resolve_in_jail(root_boundary, target)
        .ok_or_else(|| "Target outside music jail".to_string())?;
    if canon_target == root_boundary {
        return Err("Cannot rename music root".into());
    }
    let parent = canon_target
        .parent()
        .ok_or_else(|| "Missing parent directory".to_string())?;
    let dest = parent.join(clean);
    if dest.exists() {
        return Err(format!("'{}' already exists", clean));
    }
    fs::rename(&canon_target, &dest).map_err(|e| format!("Failed to rename: {e}"))?;
    resolve_in_jail(root_boundary, &dest)
        .ok_or_else(|| "Renamed path escaped music jail".to_string())
}

/// Recursively deletes a playlist directory strictly inside `root_boundary` (never root itself).
pub fn delete_folder_in_jail(root_boundary: &Path, target: &Path) -> Result<PathBuf, String> {
    let canon_root = root_boundary
        .canonicalize()
        .map_err(|e| format!("Invalid music root: {e}"))?;
    let canon_target = resolve_in_jail(&canon_root, target)
        .ok_or_else(|| "Target folder outside music jail".to_string())?;
    if canon_target == canon_root {
        return Err("Cannot delete music root folder".into());
    }
    if !canon_target.is_dir() {
        return Err("Target is not a folder".into());
    }
    let parent = canon_target
        .parent()
        .unwrap_or(&canon_root)
        .to_path_buf();
    fs::remove_dir_all(&canon_target).map_err(|e| format!("Failed to remove folder: {e}"))?;
    Ok(parent)
}

/// Copies an audio file strictly within `root_boundary` into a jailed destination folder.
pub fn copy_track_to_folder_in_jail(
    root_boundary: &Path,
    track: &Path,
    dest_folder: &Path,
) -> Result<PathBuf, String> {
    let canon_track = resolve_in_jail(root_boundary, track)
        .ok_or_else(|| "Source track outside music jail".to_string())?;
    if !canon_track.is_file() {
        return Err("Source track is not a file".into());
    }
    let canon_dir = resolve_in_jail(root_boundary, dest_folder)
        .ok_or_else(|| "Destination playlist outside music jail".to_string())?;
    if !canon_dir.is_dir() {
        return Err("Destination playlist is not a folder".into());
    }
    let fname = canon_track
        .file_name()
        .ok_or_else(|| "Invalid track filename".to_string())?;
    let dest = canon_dir.join(fname);
    if dest.exists() {
        return Err("Track already exists in that playlist folder".into());
    }
    fs::copy(&canon_track, &dest).map_err(|e| format!("Failed to copy track: {e}"))?;
    resolve_in_jail(root_boundary, &dest)
        .ok_or_else(|| "Copied file escaped music jail".to_string())
}

/// Deletes a single file strictly inside `root_boundary` (never deletes directories or root).
pub fn delete_file_in_jail(root_boundary: &Path, target: &Path) -> Result<PathBuf, String> {
    let canon_target = resolve_in_jail(root_boundary, target)
        .ok_or_else(|| "Target file outside music jail".to_string())?;
    if !canon_target.is_file() {
        return Err("Target is not a removable audio file".into());
    }
    fs::remove_file(&canon_target).map_err(|e| format!("Failed to remove file: {e}"))?;
    Ok(canon_target)
}

#[inline]
fn ascii_case_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.bytes()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_in_jail_blocks_escape() {
        let tmp = std::env::temp_dir().join("tntron_test_jail");
        let sub = tmp.join("sub");
        let _ = fs::create_dir_all(&sub);
        let root = tmp.canonicalize().unwrap();

        assert!(resolve_in_jail(&root, &sub).is_some());
        assert!(resolve_in_jail(&root, &tmp).is_some());
        assert!(resolve_in_jail(&root, &tmp.join("../")).is_none());

        assert!(create_folder_in_jail(&root, &root, "../bad").is_err());
        let created = create_folder_in_jail(&root, &root, "CreatedAlbum").unwrap();
        assert!(created.is_dir());
        let renamed = rename_in_jail(&root, &created, "RenamedAlbum").unwrap();
        assert!(renamed.is_dir());

        let song = root.join("demo.mp3");
        fs::write(&song, b"fake").unwrap();
        let copied = copy_track_to_folder_in_jail(&root, &song, &renamed).unwrap();
        assert!(copied.is_file());
        delete_file_in_jail(&root, &copied).unwrap();
        assert!(!copied.exists());
        assert!(delete_folder_in_jail(&root, &root).is_err());
        delete_folder_in_jail(&root, &renamed).unwrap();
        assert!(!renamed.exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_recursive_subfolders_added_as_playlists() {
        let tmp = std::env::temp_dir().join(format!(
            "tntron_test_subfolders_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("yt/Best Phonk")).unwrap();
        fs::create_dir_all(tmp.join("yt/HARD")).unwrap();
        fs::create_dir_all(tmp.join("HARD")).unwrap();
        fs::create_dir_all(tmp.join("EmptyTop")).unwrap();
        fs::create_dir_all(tmp.join("yt/EmptyNested")).unwrap();
        fs::create_dir_all(tmp.join(".hidden")).unwrap();

        fs::write(tmp.join("yt/Best Phonk/a.mp3"), b"mp3").unwrap();
        fs::write(tmp.join("yt/HARD/b.mp3"), b"mp3").unwrap();
        fs::write(tmp.join("HARD/c.mp3"), b"mp3").unwrap();
        fs::write(tmp.join(".hidden/secret.mp3"), b"mp3").unwrap();

        let root = tmp.canonicalize().unwrap();
        let cards = scan_playlist_folders_in_jail(&root);
        let names: Vec<&str> = cards.iter().map(|(n, _)| n.as_str()).collect();

        assert!(names.contains(&"EmptyTop"));
        assert!(names.contains(&"HARD"));
        assert!(names.contains(&"yt"));
        assert!(names.contains(&"yt / Best Phonk"));
        assert!(names.contains(&"yt / HARD"));
        assert!(!names.contains(&"yt / EmptyNested"));
        assert!(!names.contains(&".hidden"));

        let _ = fs::remove_dir_all(&tmp);
    }
}
