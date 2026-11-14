use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct LyricLine {
    pub time_sec: f64,
    pub text: String,
}

/// Parses standard LRC `[mm:ss.xx]` or `[mm:ss.xxx]` synced lyrics into sorted lines.
pub fn parse_lrc(input: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for raw in input.lines() {
        let mut rest = raw.trim();
        if rest.is_empty() {
            continue;
        }
        let mut timestamps = Vec::new();
        while rest.starts_with('[') {
            let Some(close) = rest.find(']') else {
                break;
            };
            let tag = &rest[1..close];
            if let Some(ts) = parse_timestamp(tag) {
                timestamps.push(ts);
                rest = rest[close + 1..].trim_start();
            } else {
                // Metadata tag like [ar:Artist] or [ti:Title]
                rest = rest[close + 1..].trim_start();
                if timestamps.is_empty() {
                    rest = "";
                    break;
                }
            }
        }
        if !timestamps.is_empty() {
            let lyric = rest.trim().to_string();
            for t in timestamps {
                lines.push(LyricLine {
                    time_sec: t,
                    text: lyric.clone(),
                });
            }
        }
    }
    lines.sort_by(|a, b| a.time_sec.total_cmp(&b.time_sec));
    lines
}

fn parse_timestamp(tag: &str) -> Option<f64> {
    let (mins_str, sec_str) = tag.split_once(':')?;
    let mins: f64 = mins_str.trim().parse().ok()?;
    let secs: f64 = sec_str.trim().parse().ok()?;
    if mins < 0.0 || secs < 0.0 {
        return None;
    }
    Some(mins * 60.0 + secs)
}

/// Loads a companion `.lrc` file adjacent to the given audio file, if present.
pub fn load_companion_lrc(audio_path: &Path) -> Vec<LyricLine> {
    let lrc_path = audio_path.with_extension("lrc");
    match std::fs::read_to_string(lrc_path) {
        Ok(contents) => parse_lrc(&contents),
        Err(_) => Vec::new(),
    }
}

/// Returns the index of the currently active lyric line for `pos_sec`.
pub fn active_lyric_index(lines: &[LyricLine], pos_sec: f64) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }
    let idx = lines.partition_point(|l| l.time_sec <= pos_sec + 0.05);
    idx.checked_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lrc_and_active_index() {
        let sample = "[ti:Test]\n[00:01.50]First line\n[00:05.00][00:12.25]Chorus\n";
        let parsed = parse_lrc(sample);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].text, "First line");
        assert_eq!(parsed[1].text, "Chorus");
        assert_eq!(parsed[2].time_sec, 12.25);

        assert_eq!(active_lyric_index(&parsed, 0.5), None);
        assert_eq!(active_lyric_index(&parsed, 2.0), Some(0));
        assert_eq!(active_lyric_index(&parsed, 6.0), Some(1));
        assert_eq!(active_lyric_index(&parsed, 15.0), Some(2));
    }
}
