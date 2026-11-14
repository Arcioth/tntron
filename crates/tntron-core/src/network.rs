use std::path::Path;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::library::{parse_lrc, LyricLine};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssEpisode {
    pub title: String,
    pub audio_url: String,
    pub duration: String,
    pub published: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssFeed {
    pub url: String,
    pub title: String,
    pub episodes: Vec<RssEpisode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtSearchResult {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub duration: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
struct LrclibResponse {
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

/// Fetches synced or plain lyrics from LRCLIB (`https://lrclib.net/api/get`).
pub async fn fetch_lrclib_lyrics(
    title: &str,
    artist: &str,
    duration_sec: f64,
) -> Result<(Vec<LyricLine>, Option<String>), String> {
    let clean_artist = if artist == "Unknown Artist" {
        ""
    } else {
        artist
    };

    let mut cmd = Command::new("curl");
    cmd.arg("-fsSL")
        .arg("--max-time")
        .arg("10")
        .arg("-A")
        .arg("tntron/0.3 (https://github.com/Arcioth/tntron)")
        .arg("-G")
        .arg("https://lrclib.net/api/get")
        .arg("--data-urlencode")
        .arg(format!("track_name={title}"));

    if !clean_artist.is_empty() {
        cmd.arg("--data-urlencode")
            .arg(format!("artist_name={clean_artist}"));
    }
    if duration_sec > 1.0 {
        cmd.arg("--data-urlencode")
            .arg(format!("duration={}", duration_sec.round() as u64));
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("curl launch failed: {e}"))?;
    if !output.status.success() {
        return Err(format!("LRCLIB lookup returned status {}", output.status));
    }

    let resp: LrclibResponse = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Invalid LRCLIB response: {e}"))?;

    if let Some(synced) = resp.synced_lyrics {
        let lines = parse_lrc(&synced);
        if !lines.is_empty() {
            return Ok((lines, Some(synced)));
        }
    }

    if let Some(plain) = resp.plain_lyrics {
        let lines: Vec<LyricLine> = plain
            .lines()
            .enumerate()
            .filter_map(|(idx, line)| {
                let t = line.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(LyricLine {
                        time_sec: (idx as f64) * 3.0,
                        text: t.to_string(),
                    })
                }
            })
            .collect();
        if !lines.is_empty() {
            return Ok((lines, None));
        }
    }

    Err("No synced or plain lyrics found on LRCLIB".into())
}

/// Fetches and parses an RSS/podcast feed URL into episodes with audio enclosures.
pub async fn fetch_rss_feed(url: &str) -> Result<RssFeed, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("Feed URL must start with http:// or https://".into());
    }
    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", "12", "-L", url])
        .output()
        .await
        .map_err(|e| format!("Failed to launch curl: {e}"))?;

    if !output.status.success() {
        return Err(format!("RSS fetch failed with status {}", output.status));
    }

    let xml = String::from_utf8_lossy(&output.stdout);
    Ok(parse_rss_xml(url, &xml))
}

pub fn parse_rss_xml(url: &str, xml: &str) -> RssFeed {
    let channel_block = xml.split("<item").next().unwrap_or(xml);
    let title = extract_tag_text(channel_block, "title")
        .unwrap_or_else(|| url.to_string());

    let mut episodes = Vec::new();
    for raw_item in xml.split("<item").skip(1) {
        let item = raw_item.split("</item>").next().unwrap_or(raw_item);
        let ep_title = extract_tag_text(item, "title").unwrap_or_else(|| "Untitled Episode".into());
        let audio_url = extract_attr(item, "enclosure", "url")
            .or_else(|| extract_tag_text(item, "link"));
        let Some(audio_url) = audio_url else {
            continue;
        };
        if audio_url.is_empty() {
            continue;
        }
        let duration = extract_tag_text(item, "itunes:duration").unwrap_or_default();
        let published = extract_tag_text(item, "pubDate").unwrap_or_default();
        episodes.push(RssEpisode {
            title: decode_xml_entities(&ep_title),
            audio_url: decode_xml_entities(&audio_url),
            duration,
            published,
        });
        if episodes.len() >= 40 {
            break;
        }
    }

    RssFeed {
        url: url.to_string(),
        title: decode_xml_entities(&title),
        episodes,
    }
}

fn extract_tag_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = block.find(&open)?;
    let after_open = &block[start + open.len()..];
    let gt = after_open.find('>')?;
    let content_start = &after_open[gt + 1..];
    let close = format!("</{tag}>");
    let end = content_start.find(&close)?;
    let inner = content_start[..end].trim();
    let stripped = inner
        .strip_prefix("<![CDATA[")
        .and_then(|s| s.strip_suffix("]]>"))
        .unwrap_or(inner)
        .trim();
    if stripped.is_empty() {
        None
    } else {
        Some(stripped.to_string())
    }
}

fn extract_attr(block: &str, tag: &str, attr: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = block.find(&open)?;
    let slice = &block[start..];
    let tag_end = slice.find('>')?;
    let tag_slice = &slice[..tag_end];
    for quote in ['"', '\''] {
        let pat = format!("{attr}={quote}");
        if let Some(pos) = tag_slice.find(&pat) {
            let val_slice = &tag_slice[pos + pat.len()..];
            if let Some(end) = val_slice.find(quote) {
                return Some(val_slice[..end].to_string());
            }
        }
    }
    None
}

fn decode_xml_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Searches YouTube using `yt-dlp --dump-json --flat-playlist`.
pub async fn search_ytdlp(query: &str) -> Result<Vec<YtSearchResult>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Err("Enter a search term or YouTube URL".into());
    }

    let target = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("ytsearch10:{trimmed}")
    };

    let output = Command::new("yt-dlp")
        .args([
            "--dump-json",
            "--flat-playlist",
            "--no-warnings",
            "--quiet",
            &target,
        ])
        .output()
        .await
        .map_err(|e| format!("yt-dlp launch failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "yt-dlp exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let id = v
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let title = v
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("Untitled")
            .to_string();
        let channel = v
            .get("channel")
            .or_else(|| v.get("uploader"))
            .and_then(|x| x.as_str())
            .unwrap_or("YouTube")
            .to_string();
        let duration_sec = v.get("duration").and_then(|x| x.as_f64()).unwrap_or(0.0);
        let duration = if duration_sec > 0.0 {
            crate::library::format_mmss(duration_sec)
        } else {
            "--:--".to_string()
        };
        let url = v
            .get("url")
            .or_else(|| v.get("webpage_url"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={id}"));
        let final_url = if url.starts_with("http") {
            url
        } else if !id.is_empty() {
            format!("https://www.youtube.com/watch?v={id}")
        } else {
            continue;
        };
        results.push(YtSearchResult {
            id,
            title,
            channel,
            duration,
            url: final_url,
        });
    }

    Ok(results)
}

/// Resolves direct playable audio stream URL from YouTube via `yt-dlp -f bestaudio -g`.
pub async fn resolve_ytdlp_stream_url(page_url: &str) -> Result<String, String> {
    let output = Command::new("yt-dlp")
        .args(["-f", "bestaudio", "-g", "--no-playlist", page_url])
        .output()
        .await
        .map_err(|e| format!("yt-dlp stream resolve failed: {e}"))?;

    if !output.status.success() {
        return Err(format!("yt-dlp stream resolve exited {}", output.status));
    }
    let stream = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if stream.is_empty() {
        Err("yt-dlp returned empty stream URL".into())
    } else {
        Ok(stream)
    }
}

/// Downloads extracted audio directly into a jailed directory (`%(title)s.%(ext)s`).
pub async fn download_ytdlp_to_dir(page_url: &str, target_dir: &Path) -> Result<String, String> {
    let template = target_dir
        .join("%(title)s.%(ext)s")
        .to_string_lossy()
        .to_string();

    let output = Command::new("yt-dlp")
        .args([
            "-x",
            "--audio-format",
            "opus",
            "--no-playlist",
            "-o",
            &template,
            page_url,
        ])
        .output()
        .await
        .map_err(|e| format!("yt-dlp download failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "yt-dlp download failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok("Audio saved into music library".into())
}

/// Sends a desktop notification via `notify-send` asynchronously.
pub fn spawn_desktop_notification(summary: String, body: String) {
    tokio::spawn(async move {
        let _ = Command::new("notify-send")
            .args([
                "--app-name=tntron",
                "--expire-time=3500",
                &summary,
                &body,
            ])
            .output()
            .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rss_xml_extracts_feed_and_enclosures() {
        let xml = r#"
        <rss><channel>
          <title><![CDATA[Rust Audio Cast &amp; Co]]></title>
          <item>
            <title>Episode 1: Ownership</title>
            <enclosure url="https://example.com/ep1.mp3" type="audio/mpeg"/>
            <itunes:duration>12:34</itunes:duration>
          </item>
          <item>
            <title>Text Only Post</title>
          </item>
        </channel></rss>
        "#;

        let feed = parse_rss_xml("https://example.com/feed.xml", xml);
        assert_eq!(feed.title, "Rust Audio Cast & Co");
        assert_eq!(feed.episodes.len(), 1);
        assert_eq!(feed.episodes[0].title, "Episode 1: Ownership");
        assert_eq!(feed.episodes[0].audio_url, "https://example.com/ep1.mp3");
        assert_eq!(feed.episodes[0].duration, "12:34");
    }
}
