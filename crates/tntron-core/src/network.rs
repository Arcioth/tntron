use std::path::Path;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::library::{parse_lrc, plain_text_to_timed_lines, LyricLine};

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

#[derive(Debug, Clone)]
pub struct FetchedLyrics {
    pub source: String,
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone)]
pub struct TranslatedLyrics {
    pub detected_lang: Option<String>,
    pub target_lang: String,
    pub provider: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LrclibResponse {
    duration: Option<f64>,
    instrumental: Option<bool>,
    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,
}

/// Fetches lyrics using the requested provider (`"auto"`, `"lrclib"`, `"lrclib_search"`,
/// `"netease"`, `"kugou"`, or `"lyrics_ovh"`).
pub async fn fetch_lyrics_with_provider(
    title: &str,
    artist: &str,
    duration_sec: f64,
    provider: &str,
) -> Result<FetchedLyrics, String> {
    let p = provider.trim().to_ascii_lowercase();
    match p.as_str() {
        "lrclib" => fetch_lrclib_lyrics(title, artist, duration_sec)
            .await
            .map(|(lines, _)| FetchedLyrics {
                source: "LRCLIB".into(),
                lines,
            }),
        "lrclib_search" => fetch_lrclib_search_lyrics(title, artist, duration_sec).await,
        "netease" => fetch_netease_lyrics(title, artist, duration_sec).await,
        "kugou" => fetch_kugou_lyrics(title, artist, duration_sec).await,
        "lyrics_ovh" => fetch_lyrics_ovh(title, artist, duration_sec).await,
        _ => {
            let mut errors = Vec::new();
            if let Ok((lines, _)) = fetch_lrclib_lyrics(title, artist, duration_sec).await {
                return Ok(FetchedLyrics {
                    source: "LRCLIB".into(),
                    lines,
                });
            }
            for attempt in [
                fetch_lrclib_search_lyrics(title, artist, duration_sec).await,
                fetch_netease_lyrics(title, artist, duration_sec).await,
                fetch_kugou_lyrics(title, artist, duration_sec).await,
                fetch_lyrics_ovh(title, artist, duration_sec).await,
            ] {
                match attempt {
                    Ok(res) => return Ok(res),
                    Err(e) => errors.push(e),
                }
            }
            Err(errors
                .into_iter()
                .last()
                .unwrap_or_else(|| "No lyrics found across providers".into()))
        }
    }
}

/// Fetches synced or plain lyrics from LRCLIB (`https://lrclib.net/api/get`).
pub async fn fetch_lrclib_lyrics(
    title: &str,
    artist: &str,
    duration_sec: f64,
) -> Result<(Vec<LyricLine>, Option<String>), String> {
    let clean_artist = normalize_artist(artist);

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
        let lines = plain_text_to_timed_lines(&plain, duration_sec);
        if !lines.is_empty() {
            return Ok((lines, None));
        }
    }

    Err("No synced or plain lyrics found on LRCLIB".into())
}

/// Fuzzy search via `https://lrclib.net/api/search`, useful for cleaned filenames or YouTube titles.
pub async fn fetch_lrclib_search_lyrics(
    title: &str,
    artist: &str,
    duration_sec: f64,
) -> Result<FetchedLyrics, String> {
    let (clean_title, clean_artist) = clean_track_query(title, artist);
    let query = if clean_artist.is_empty() {
        clean_title
    } else {
        format!("{clean_artist} {clean_title}")
    };
    if query.trim().is_empty() {
        return Err("Empty search query for LRCLIB Search".into());
    }

    let output = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-A",
            "tntron/0.3 (https://github.com/Arcioth/tntron)",
            "-G",
            "https://lrclib.net/api/search",
            "--data-urlencode",
        ])
        .arg(format!("q={query}"))
        .output()
        .await
        .map_err(|e| format!("curl launch failed: {e}"))?;

    if !output.status.success() {
        return Err(format!("LRCLIB Search status {}", output.status));
    }
    parse_lrclib_search_json(&output.stdout, duration_sec)
}

fn parse_lrclib_search_json(bytes: &[u8], duration_sec: f64) -> Result<FetchedLyrics, String> {
    let items: Vec<LrclibResponse> = serde_json::from_slice(bytes)
        .map_err(|e| format!("Invalid LRCLIB Search response: {e}"))?;

    let mut best_synced: Option<(f64, Vec<LyricLine>)> = None;
    let mut best_plain: Option<(f64, Vec<LyricLine>)> = None;

    for item in items {
        if item.instrumental == Some(true)
            && item.synced_lyrics.as_deref().unwrap_or("").trim().is_empty()
            && item.plain_lyrics.as_deref().unwrap_or("").trim().is_empty()
        {
            continue;
        }
        let dur_diff = match (item.duration, duration_sec > 1.0) {
            (Some(d), true) => (d - duration_sec).abs(),
            _ => 15.0,
        };
        if let Some(synced) = item.synced_lyrics {
            let lines = parse_lrc(&synced);
            if !lines.is_empty()
                && best_synced
                    .as_ref()
                    .is_none_or(|(best_d, _)| dur_diff < *best_d)
            {
                best_synced = Some((dur_diff, lines));
                continue;
            }
        }
        if let Some(plain) = item.plain_lyrics {
            let lines = plain_text_to_timed_lines(&plain, duration_sec);
            if !lines.is_empty()
                && best_plain
                    .as_ref()
                    .is_none_or(|(best_d, _)| dur_diff < *best_d)
            {
                best_plain = Some((dur_diff, lines));
            }
        }
    }

    if let Some((_, lines)) = best_synced.or(best_plain) {
        return Ok(FetchedLyrics {
            source: "LRCLIB Search".into(),
            lines,
        });
    }
    Err("No matching lyrics found via LRCLIB Search".into())
}

/// Fetches synced LRC lyrics from NetEase Cloud Music (`music.163.com`).
pub async fn fetch_netease_lyrics(
    title: &str,
    artist: &str,
    duration_sec: f64,
) -> Result<FetchedLyrics, String> {
    let (clean_title, clean_artist) = clean_track_query(title, artist);
    let query = if clean_artist.is_empty() {
        clean_title
    } else {
        format!("{clean_title} {clean_artist}")
    };
    if query.trim().is_empty() {
        return Err("Empty query for NetEase".into());
    }

    let search_out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-H",
            "Referer: https://music.163.com",
            "-A",
            "Mozilla/5.0 (X11; Linux x86_64) tntron/0.3",
            "-G",
            "https://music.163.com/api/search/get",
            "--data-urlencode",
        ])
        .arg(format!("s={query}"))
        .args(["--data-urlencode", "type=1", "--data-urlencode", "limit=5"])
        .output()
        .await
        .map_err(|e| format!("NetEase search failed: {e}"))?;

    if !search_out.status.success() {
        return Err(format!("NetEase search status {}", search_out.status));
    }

    let song_id = parse_netease_song_id(&search_out.stdout, duration_sec)
        .ok_or_else(|| "No NetEase track match found".to_string())?;

    let lrc_out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-H",
            "Referer: https://music.163.com",
            "-A",
            "Mozilla/5.0 (X11; Linux x86_64) tntron/0.3",
        ])
        .arg(format!(
            "https://music.163.com/api/song/lyric?id={song_id}&lv=1&tv=-1"
        ))
        .output()
        .await
        .map_err(|e| format!("NetEase lyric fetch failed: {e}"))?;

    if !lrc_out.status.success() {
        return Err(format!("NetEase lyric status {}", lrc_out.status));
    }
    parse_netease_lyric_json(&lrc_out.stdout, duration_sec)
}

fn parse_netease_song_id(bytes: &[u8], duration_sec: f64) -> Option<u64> {
    let val: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let songs = val.get("result")?.get("songs")?.as_array()?;
    let mut best: Option<(f64, u64)> = None;
    for song in songs {
        let id = song.get("id")?.as_u64()?;
        let dur_ms = song.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0);
        let diff = if duration_sec > 1.0 && dur_ms > 0.0 {
            ((dur_ms / 1000.0) - duration_sec).abs()
        } else {
            10.0
        };
        if best.is_none_or(|(bd, _)| diff < bd) {
            best = Some((diff, id));
        }
    }
    best.map(|(_, id)| id)
}

fn parse_netease_lyric_json(bytes: &[u8], duration_sec: f64) -> Result<FetchedLyrics, String> {
    let val: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| format!("Invalid NetEase lyric JSON: {e}"))?;
    let raw = val
        .get("lrc")
        .and_then(|l| l.get("lyric"))
        .and_then(|s| s.as_str())
        .unwrap_or("");
    let synced = parse_lrc(raw);
    if !synced.is_empty() {
        return Ok(FetchedLyrics {
            source: "NetEase".into(),
            lines: synced,
        });
    }
    let plain = plain_text_to_timed_lines(raw, duration_sec);
    if !plain.is_empty() {
        return Ok(FetchedLyrics {
            source: "NetEase".into(),
            lines: plain,
        });
    }
    Err("No lyrics returned by NetEase".into())
}

/// Fetches synced LRC lyrics from Kugou (`lyrics.kugou.com`).
pub async fn fetch_kugou_lyrics(
    title: &str,
    artist: &str,
    duration_sec: f64,
) -> Result<FetchedLyrics, String> {
    let (clean_title, clean_artist) = clean_track_query(title, artist);
    let keyword = if clean_artist.is_empty() {
        clean_title
    } else {
        format!("{clean_artist} - {clean_title}")
    };
    if keyword.trim().is_empty() {
        return Err("Empty query for Kugou".into());
    }

    let dur_ms = (duration_sec.max(0.0) * 1000.0).round() as u64;
    let search_out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-G",
            "https://lyrics.kugou.com/search",
            "--data-urlencode",
            "ver=1",
            "--data-urlencode",
            "man=yes",
            "--data-urlencode",
            "client=pc",
            "--data-urlencode",
        ])
        .arg(format!("keyword={keyword}"))
        .arg("--data-urlencode")
        .arg(format!("duration={dur_ms}"))
        .output()
        .await
        .map_err(|e| format!("Kugou search failed: {e}"))?;

    if !search_out.status.success() {
        return Err(format!("Kugou search status {}", search_out.status));
    }

    let (id, accesskey) = parse_kugou_candidate(&search_out.stdout)
        .ok_or_else(|| "No Kugou lyric candidate found".to_string())?;

    let dl_out = Command::new("curl")
        .args([
            "-fsSL",
            "--max-time",
            "10",
            "-G",
            "https://lyrics.kugou.com/download",
            "--data-urlencode",
            "ver=1",
            "--data-urlencode",
            "client=pc",
            "--data-urlencode",
            "fmt=lrc",
            "--data-urlencode",
            "charset=utf8",
            "--data-urlencode",
        ])
        .arg(format!("id={id}"))
        .arg("--data-urlencode")
        .arg(format!("accesskey={accesskey}"))
        .output()
        .await
        .map_err(|e| format!("Kugou download failed: {e}"))?;

    if !dl_out.status.success() {
        return Err(format!("Kugou download status {}", dl_out.status));
    }
    parse_kugou_download_json(&dl_out.stdout, duration_sec)
}

fn parse_kugou_candidate(bytes: &[u8]) -> Option<(String, String)> {
    let val: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let first = val.get("candidates")?.as_array()?.first()?;
    let id = first
        .get("id")
        .and_then(|v| {
            v.as_str()
                .map(ToString::to_string)
                .or_else(|| v.as_u64().map(|n| n.to_string()))
        })?;
    let accesskey = first.get("accesskey")?.as_str()?.to_string();
    Some((id, accesskey))
}

fn parse_kugou_download_json(bytes: &[u8], duration_sec: f64) -> Result<FetchedLyrics, String> {
    let val: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| format!("Invalid Kugou download JSON: {e}"))?;
    let content_b64 = val
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing Kugou lyric content".to_string())?;
    let decoded = decode_base64(content_b64)
        .ok_or_else(|| "Invalid base64 in Kugou lyrics".to_string())?;
    let text = String::from_utf8_lossy(&decoded);
    let synced = parse_lrc(&text);
    if !synced.is_empty() {
        return Ok(FetchedLyrics {
            source: "Kugou".into(),
            lines: synced,
        });
    }
    let plain = plain_text_to_timed_lines(&text, duration_sec);
    if !plain.is_empty() {
        return Ok(FetchedLyrics {
            source: "Kugou".into(),
            lines: plain,
        });
    }
    Err("Kugou returned empty lyrics".into())
}

/// Fetches plain lyrics from Lyrics.ovh (`https://api.lyrics.ovh/v1/<artist>/<title>`).
pub async fn fetch_lyrics_ovh(
    title: &str,
    artist: &str,
    duration_sec: f64,
) -> Result<FetchedLyrics, String> {
    let (clean_title, clean_artist) = clean_track_query(title, artist);
    if clean_artist.is_empty() || clean_title.is_empty() {
        return Err("Lyrics.ovh requires both artist and title".into());
    }

    let url = format!(
        "https://api.lyrics.ovh/v1/{}/{}",
        percent_encode_component(&clean_artist),
        percent_encode_component(&clean_title)
    );
    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", "10", &url])
        .output()
        .await
        .map_err(|e| format!("Lyrics.ovh curl failed: {e}"))?;

    if !output.status.success() {
        return Err(format!("Lyrics.ovh status {}", output.status));
    }
    parse_lyrics_ovh_json(&output.stdout, duration_sec)
}

fn parse_lyrics_ovh_json(bytes: &[u8], duration_sec: f64) -> Result<FetchedLyrics, String> {
    let val: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| format!("Invalid Lyrics.ovh JSON: {e}"))?;
    let raw = val
        .get("lyrics")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "No lyrics field in Lyrics.ovh response".to_string())?;
    let cleaned = if raw.starts_with("Paroles de la chanson ") {
        raw.split_once('\n').map(|(_, rest)| rest).unwrap_or(raw)
    } else {
        raw
    };
    let lines = plain_text_to_timed_lines(cleaned, duration_sec);
    if lines.is_empty() {
        return Err("Lyrics.ovh returned empty lyrics".into());
    }
    Ok(FetchedLyrics {
        source: "Lyrics.ovh".into(),
        lines,
    })
}

/// Translates lyric lines 1-to-1 into `target_lang` (e.g. `"en"`, `"tr"`, `"es"`, `"ja"`),
/// using Google Translate (`translate.googleapis.com`) with Lingva (`lingva.ml`) fallback.
pub async fn translate_lyrics_lines(
    lines: &[String],
    target_lang: &str,
) -> Result<TranslatedLyrics, String> {
    let lang = sanitize_lang_code(target_lang);
    let mut out = vec![String::new(); lines.len()];
    let active_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, s)| if s.trim().is_empty() { None } else { Some(i) })
        .collect();

    if active_indices.is_empty() {
        return Ok(TranslatedLyrics {
            detected_lang: None,
            target_lang: lang,
            provider: "None".into(),
            lines: out,
        });
    }

    let mut detected_lang: Option<String> = None;
    let mut provider_used = "Google Translate".to_string();

    for chunk in active_indices.chunks(28) {
        let joined = chunk
            .iter()
            .map(|&i| lines[i].trim().replace('\n', " "))
            .collect::<Vec<_>>()
            .join("\n");

        match translate_batch_google(&joined, &lang).await {
            Ok((translated_block, det)) => {
                if detected_lang.is_none() {
                    detected_lang = det;
                }
                let parts: Vec<&str> = translated_block.lines().collect();
                if parts.len() == chunk.len() {
                    for (&idx, tr) in chunk.iter().zip(parts) {
                        out[idx] = tr.trim().to_string();
                    }
                } else {
                    // Fallback per small sub-group if the translator merged/split newlines.
                    for &idx in chunk {
                        let single = lines[idx].trim();
                        if let Ok((tr_one, det_one)) = translate_batch_google(single, &lang).await {
                            if detected_lang.is_none() {
                                detected_lang = det_one;
                            }
                            out[idx] = tr_one.trim().to_string();
                        } else {
                            out[idx] = lines[idx].clone();
                        }
                    }
                }
            }
            Err(_) => {
                provider_used = "Lingva Translate".into();
                match translate_batch_lingva(&joined, &lang).await {
                    Ok((translated_block, det)) => {
                        if detected_lang.is_none() {
                            detected_lang = det;
                        }
                        let parts: Vec<&str> = translated_block.lines().collect();
                        if parts.len() == chunk.len() {
                            for (&idx, tr) in chunk.iter().zip(parts) {
                                out[idx] = tr.trim().to_string();
                            }
                        } else {
                            return Err("Translation provider line count mismatch".into());
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    Ok(TranslatedLyrics {
        detected_lang,
        target_lang: lang,
        provider: provider_used,
        lines: out,
    })
}

async fn translate_batch_google(
    text: &str,
    target_lang: &str,
) -> Result<(String, Option<String>), String> {
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={target_lang}&dt=t"
    );
    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", "12", "-X", "POST", &url, "--data-urlencode"])
        .arg(format!("q={text}"))
        .output()
        .await
        .map_err(|e| format!("Translate curl failed: {e}"))?;

    if !output.status.success() {
        return Err(format!("Google Translate status {}", output.status));
    }
    parse_google_translate_json(&output.stdout)
}

fn parse_google_translate_json(bytes: &[u8]) -> Result<(String, Option<String>), String> {
    let val: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| format!("Invalid Google Translate JSON: {e}"))?;
    let segments = val
        .get(0)
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Missing translation segments".to_string())?;
    let mut combined = String::new();
    for seg in segments {
        if let Some(piece) = seg.get(0).and_then(|s| s.as_str()) {
            combined.push_str(piece);
        }
    }
    let detected = val
        .get(2)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    if combined.trim().is_empty() {
        return Err("Empty translation returned".into());
    }
    Ok((combined, detected))
}

async fn translate_batch_lingva(
    text: &str,
    target_lang: &str,
) -> Result<(String, Option<String>), String> {
    let url = format!(
        "https://lingva.ml/api/v1/auto/{target_lang}/{}",
        percent_encode_component(text)
    );
    let output = Command::new("curl")
        .args(["-fsSL", "--max-time", "12", &url])
        .output()
        .await
        .map_err(|e| format!("Lingva curl failed: {e}"))?;
    if !output.status.success() {
        return Err(format!("Lingva status {}", output.status));
    }
    parse_lingva_json(&output.stdout)
}

fn parse_lingva_json(bytes: &[u8]) -> Result<(String, Option<String>), String> {
    let val: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| format!("Invalid Lingva JSON: {e}"))?;
    let tr = val
        .get("translation")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing Lingva translation".to_string())?
        .to_string();
    let detected = val
        .get("info")
        .and_then(|i| i.get("detectedSource"))
        .and_then(|s| s.as_str())
        .map(ToString::to_string);
    Ok((tr, detected))
}

fn sanitize_lang_code(raw: &str) -> String {
    let clean: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(10)
        .collect();
    if clean.is_empty() {
        "en".to_string()
    } else {
        clean.to_ascii_lowercase()
    }
}

fn normalize_artist(artist: &str) -> &str {
    if artist == "Unknown Artist" {
        ""
    } else {
        artist.trim()
    }
}

/// Cleans bracketed suffixes like `(Official Video)` / `[FLAC]` / `(feat. X)` for fuzzy lyrics lookup.
pub fn clean_track_query(title: &str, artist: &str) -> (String, String) {
    let strip_brackets = |input: &str| -> String {
        let mut out = String::with_capacity(input.len());
        let mut round_depth = 0usize;
        let mut sq_depth = 0usize;
        for ch in input.chars() {
            match ch {
                '(' => round_depth += 1,
                ')' => round_depth = round_depth.saturating_sub(1),
                '[' => sq_depth += 1,
                ']' => sq_depth = sq_depth.saturating_sub(1),
                _ if round_depth == 0 && sq_depth == 0 => out.push(ch),
                _ => {}
            }
        }
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    };

    let clean_t = strip_brackets(title);
    let base_a = normalize_artist(artist);
    let clean_a = strip_brackets(base_a);
    let final_t = if clean_t.is_empty() {
        title.trim().to_string()
    } else {
        clean_t
    };
    (final_t, clean_a)
}

fn percent_encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut buf = Vec::with_capacity((input.len() * 3) / 4);
    let mut val = 0u32;
    let mut bits = -8i32;
    for b in input.bytes() {
        if b.is_ascii_whitespace() || b == b'=' {
            continue;
        }
        let d = match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        val = (val << 6) | d;
        bits += 6;
        if bits >= 0 {
            buf.push(((val >> bits) & 0xFF) as u8);
            bits -= 8;
        }
    }
    Some(buf)
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

    #[test]
    fn test_clean_track_query_strips_brackets() {
        let (t, a) = clean_track_query("Nightcall (Official Video) [4K]", "Kavinsky (feat. Lovefoxxx)");
        assert_eq!(t, "Nightcall");
        assert_eq!(a, "Kavinsky");
    }

    #[test]
    fn test_parse_lrclib_search_prefers_synced_closest_duration() {
        let json = br#"[
            {"duration": 120.0, "syncedLyrics": "[00:01.00]Wrong duration"},
            {"duration": 210.0, "syncedLyrics": "[00:05.00]Matched synced"}
        ]"#;
        let res = parse_lrclib_search_json(json, 212.0).expect("lrclib search");
        assert_eq!(res.source, "LRCLIB Search");
        assert_eq!(res.lines.len(), 1);
        assert_eq!(res.lines[0].text, "Matched synced");
    }

    #[test]
    fn test_parse_netease_and_kugou_and_ovh() {
        let ne_search = br#"{"result":{"songs":[{"id":42,"duration":180000},{"id":99,"duration":250000}]}}"#;
        assert_eq!(parse_netease_song_id(ne_search, 182.0), Some(42));

        let ne_lrc = br#"{"lrc":{"lyric":"[00:10.50]Midnight drive\n[00:15:20]Neon sky"}}"#;
        let ne_res = parse_netease_lyric_json(ne_lrc, 180.0).expect("netease lyric");
        assert_eq!(ne_res.source, "NetEase");
        assert_eq!(ne_res.lines.len(), 2);
        assert_eq!(ne_res.lines[1].text, "Neon sky");

        // "[00:02.00]Hello" -> base64 "WzAwOjAyLjAwXUhlbGxv"
        let kg_search = br#"{"candidates":[{"id":"123","accesskey":"abc"}]}"#;
        assert_eq!(parse_kugou_candidate(kg_search), Some(("123".into(), "abc".into())));
        let kg_dl = br#"{"content":"WzAwOjAyLjAwXUhlbGxv"}"#;
        let kg_res = parse_kugou_download_json(kg_dl, 100.0).expect("kugou");
        assert_eq!(kg_res.source, "Kugou");
        assert_eq!(kg_res.lines[0].text, "Hello");

        let ovh = br#"{"lyrics":"Paroles de la chanson Test par Artist\nLine one\nLine two"}"#;
        let ovh_res = parse_lyrics_ovh_json(ovh, 40.0).expect("ovh");
        assert_eq!(ovh_res.source, "Lyrics.ovh");
        assert_eq!(ovh_res.lines.len(), 2);
        assert_eq!(ovh_res.lines[0].text, "Line one");
    }

    #[test]
    fn test_parse_translation_responses() {
        let gtx = br#"[[["Merhaba\n","Hello\n",null,null],["Dunya","World",null,null]],null,"en"]"#;
        let (tr, det) = parse_google_translate_json(gtx).expect("gtx parse");
        assert_eq!(det.as_deref(), Some("en"));
        assert_eq!(tr, "Merhaba\nDunya");

        let lingva = br#"{"translation":"Hola\nMundo","info":{"detectedSource":"en"}}"#;
        let (ltr, ldet) = parse_lingva_json(lingva).expect("lingva parse");
        assert_eq!(ldet.as_deref(), Some("en"));
        assert_eq!(ltr, "Hola\nMundo");
    }
}
