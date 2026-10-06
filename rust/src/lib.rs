// Author: Daniel Hallman
#![allow(clippy::collapsible_if)]

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::LazyLock;

static VIDEO_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:v=|/v/|youtu\.be/|embed/|shorts/)([A-Za-z0-9_-]{11})").unwrap()
});

static API_KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""INNERTUBE_API_KEY":\s*"([A-Za-z0-9_-]+)""#).unwrap()
});

pub const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub duration: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
    pub title: String,
    pub channel: String,
    pub duration: i64,
    pub duration_formatted: String,
    pub published: String,
    pub view_count: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_live: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chapters: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptionPayload {
    pub video_id: String,
    pub url: String,
    pub language: String,
    pub auto_generated: bool,
    pub metadata: Metadata,
    pub segments: Vec<Segment>,
    pub total_segments: usize,
}

pub fn parse_video_id(url_or_id: &str) -> Result<String, String> {
    let text = url_or_id.trim();
    if text.len() == 11 && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Ok(text.to_string());
    }
    if let Some(caps) = VIDEO_ID_RE.captures(text) {
        if let Some(m) = caps.get(1) {
            return Ok(m.as_str().to_string());
        }
    }
    Err(format!("Could not read a YouTube video id from: {url_or_id}"))
}

pub fn format_timestamp(seconds: f64) -> String {
    let total = seconds.max(0.0) as i64;
    let hours = total / 3600;
    let rem = total % 3600;
    let minutes = rem / 60;
    let secs = rem % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

pub fn parse_timedtext(xml_text: &str) -> Vec<Segment> {
    let mut reader = Reader::from_str(xml_text);
    reader.config_mut().trim_text(false);

    let mut segments: Vec<Segment> = Vec::new();
    let mut srv1_segments: Vec<Segment> = Vec::new();

    let mut in_p = false;
    let mut current_p_t: Option<f64> = None;
    let mut current_p_d: Option<f64> = None;
    let mut current_p_text = String::new();

    let mut in_text = false;
    let mut current_text_start: Option<f64> = None;
    let mut current_text_dur: Option<f64> = None;
    let mut current_text_content = String::new();

    let mut buf = Vec::new();

    while let Ok(event) = reader.read_event_into(&mut buf) {
        match event {
            Event::Start(e) => {
                let name = e.name();
                if name.as_ref() == b"p" {
                    in_p = true;
                    current_p_text.clear();
                    current_p_t = None;
                    current_p_d = None;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"t" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                current_p_t = v.parse::<f64>().ok();
                            }
                        } else if attr.key.as_ref() == b"d" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                current_p_d = v.parse::<f64>().ok();
                            }
                        }
                    }
                } else if name.as_ref() == b"text" {
                    in_text = true;
                    current_text_content.clear();
                    current_text_start = None;
                    current_text_dur = None;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"start" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                current_text_start = v.parse::<f64>().ok();
                            }
                        } else if attr.key.as_ref() == b"dur" {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                current_text_dur = v.parse::<f64>().ok();
                            }
                        }
                    }
                }
            }
            Event::Empty(e) => {
                let name = e.name();
                if name.as_ref() == b"p" {
                    // Empty <p ... />
                } else if name.as_ref() == b"text" {
                    // Empty <text ... />
                }
            }
            Event::Text(e) => {
                if in_p {
                    if let Ok(s) = e.unescape() {
                        current_p_text.push_str(&s);
                    }
                } else if in_text {
                    if let Ok(s) = e.unescape() {
                        current_text_content.push_str(&s);
                    }
                }
            }
            Event::End(e) => {
                let name = e.name();
                if name.as_ref() == b"p" {
                    in_p = false;
                    if let Some(t) = current_p_t {
                        let text = html_escape::decode_html_entities(&current_p_text).trim().to_string();
                        if !text.is_empty() {
                            let dur = current_p_d.unwrap_or(0.0);
                            segments.push(Segment {
                                start: (t / 100.0).round() / 10.0,
                                duration: (dur / 100.0).round() / 10.0,
                                text,
                            });
                        }
                    }
                } else if name.as_ref() == b"text" {
                    in_text = false;
                    let text = html_escape::decode_html_entities(&current_text_content).trim().to_string();
                    if !text.is_empty() {
                        let start = current_text_start.unwrap_or(0.0);
                        let dur = current_text_dur.unwrap_or(0.0);
                        srv1_segments.push(Segment {
                            start: (start * 10.0).round() / 10.0,
                            duration: (dur * 10.0).round() / 10.0,
                            text,
                        });
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if !segments.is_empty() {
        segments
    } else {
        srv1_segments
    }
}

pub fn timed_lines(payload: &CaptionPayload) -> String {
    let mut out = String::new();
    let title = if !payload.metadata.title.is_empty() {
        &payload.metadata.title
    } else {
        &payload.video_id
    };
    out.push_str(&format!("# {title}\n"));
    out.push_str(&format!("Channel: {}\n", payload.metadata.channel));
    out.push_str(&format!("Duration: {}\n", payload.metadata.duration_formatted));
    out.push_str(&format!("URL: {}\n", payload.url));
    let cap_kind = if payload.auto_generated { "auto-generated" } else { "human" };
    out.push_str(&format!("Captions: {cap_kind}\n\n---\n\n"));

    for seg in &payload.segments {
        out.push_str(&format!("[{}] {}\n", format_timestamp(seg.start), seg.text));
    }
    out
}

pub fn plain_lines(payload: &CaptionPayload) -> String {
    let mut out = String::new();
    let title = if !payload.metadata.title.is_empty() {
        &payload.metadata.title
    } else {
        &payload.video_id
    };
    out.push_str(&format!("# {title}\n"));
    out.push_str(&format!("Channel: {}\n", payload.metadata.channel));
    out.push_str(&format!("Duration: {}\n", payload.metadata.duration_formatted));
    out.push_str(&format!("URL: {}\n\n---\n\n", payload.url));

    for seg in &payload.segments {
        out.push_str(&format!("{}\n", seg.text));
    }
    out
}

fn with_fmt(url: &str, fmt: &str) -> String {
    if url.contains("fmt=") {
        let re = Regex::new(r"fmt=[^&]+").unwrap();
        re.replace(url, format!("fmt={fmt}")).to_string()
    } else if url.contains('?') {
        format!("{url}&fmt={fmt}")
    } else {
        format!("{url}?fmt={fmt}")
    }
}

fn ytdlp_metadata(video_id: &str) -> Option<Metadata> {
    let output = Command::new("yt-dlp")
        .args([
            "--dump-json",
            "--no-download",
            "--no-warnings",
            &format!("https://www.youtube.com/watch?v={video_id}"),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let val: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let duration = val.get("duration").and_then(|v| v.as_i64()).unwrap_or(0);
    Some(Metadata {
        title: val.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        channel: val.get("channel").or_else(|| val.get("uploader")).and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        duration,
        duration_formatted: if duration > 0 { format_timestamp(duration as f64) } else { String::new() },
        published: val.get("upload_date").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        view_count: val.get("view_count").and_then(|v| v.as_i64()).unwrap_or(0),
        is_live: val.get("is_live").and_then(|v| v.as_bool()),
        chapters: val.get("chapters").and_then(|v| v.as_array()).cloned().unwrap_or_default(),
        language: val.get("language").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
    })
}

pub fn fetch_captions(client: &reqwest::blocking::Client, url_or_id: &str, language: &str) -> Result<CaptionPayload, String> {
    let video_id = parse_video_id(url_or_id)?;

    // 1. Fetch watch page
    let watch_url = format!("https://www.youtube.com/watch?v={video_id}");
    let watch_resp = client
        .get(&watch_url)
        .header("User-Agent", USER_AGENT)
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .map_err(|e| format!("Network error for {watch_url}: {e}"))?;

    let html = watch_resp.text().map_err(|e| format!("Failed to read watch page: {e}"))?;
    let api_key = API_KEY_RE
        .captures(&html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .ok_or_else(|| "Watch page did not include an InnerTube API key".to_string())?;

    // 2. Fetch player payload
    let player_url = format!("https://www.youtube.com/youtubei/v1/player?key={api_key}");
    let payload = serde_json::json!({
        "context": {
            "client": {
                "clientName": "ANDROID",
                "clientVersion": "20.10.38"
            }
        },
        "videoId": video_id
    });

    let player_resp = client
        .post(&player_url)
        .header("User-Agent", USER_AGENT)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .map_err(|e| format!("Failed to post to innertube: {e}"))?;

    let player: serde_json::Value = player_resp.json().map_err(|e| format!("Failed to parse player JSON: {e}"))?;

    let tracks = player
        .pointer("/captions/playerCaptionsTracklistRenderer/captionTracks")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "This video has no captions".to_string())?;

    if tracks.is_empty() {
        return Err("This video has no captions".to_string());
    }

    let track = tracks
        .iter()
        .find(|t| t.get("languageCode").and_then(|v| v.as_str()) == Some(language))
        .or_else(|| {
            tracks.iter().find(|t| {
                t.get("languageCode")
                    .and_then(|v| v.as_str())
                    .map(|code| code.starts_with(language))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(&tracks[0]);

    let track_url = track
        .get("baseUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Caption track is missing a URL".to_string())?;

    let xml_resp = client
        .get(track_url)
        .header("User-Agent", USER_AGENT)
        .send()
        .map_err(|e| format!("Failed to fetch caption xml: {e}"))?;

    let xml_text = xml_resp.text().unwrap_or_default();
    let mut segments = parse_timedtext(&xml_text);

    if segments.is_empty() {
        let srv_url = with_fmt(track_url, "srv1");
        if let Ok(srv_resp) = client.get(&srv_url).header("User-Agent", USER_AGENT).send() {
            let srv_text = srv_resp.text().unwrap_or_default();
            segments = parse_timedtext(&srv_text);
        }
    }

    if segments.is_empty() {
        return Err("Caption XML had no readable text".to_string());
    }

    let metadata = ytdlp_metadata(&video_id).unwrap_or_else(|| {
        let details = player.get("videoDetails").cloned().unwrap_or_default();
        let micro = player.pointer("/microformat/playerMicroformatRenderer").cloned().unwrap_or_default();
        let duration = details.get("lengthSeconds").and_then(|v| v.as_str()).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
        let published = micro.get("publishDate").or_else(|| micro.get("uploadDate")).and_then(|v| v.as_str()).unwrap_or_default().to_string();

        Metadata {
            title: details.get("title").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            channel: details.get("author").and_then(|v| v.as_str()).unwrap_or_default().to_string(),
            duration,
            duration_formatted: if duration > 0 { format_timestamp(duration as f64) } else { String::new() },
            published,
            view_count: details.get("viewCount").and_then(|v| v.as_str()).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0),
            is_live: details.get("isLiveContent").and_then(|v| v.as_bool()),
            chapters: Vec::new(),
            language: language.to_string(),
        }
    });

    let kind = track.get("kind").and_then(|v| v.as_str()).unwrap_or_default();
    let vss_id = track.get("vssId").and_then(|v| v.as_str()).unwrap_or_default();
    let auto_generated = kind == "asr" || vss_id.contains("asr");

    let total = segments.len();
    Ok(CaptionPayload {
        video_id: video_id.clone(),
        url: format!("https://www.youtube.com/watch?v={video_id}"),
        language: track.get("languageCode").and_then(|v| v.as_str()).unwrap_or(language).to_string(),
        auto_generated,
        metadata,
        segments,
        total_segments: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORMAT3: &str = r#"<?xml version="1.0" encoding="utf-8" ?>
<timedtext format="3">
<body>
<p t="160" d="5599"><s>This</s><s t="240"> is</s><s t="400"> a</s><s t="640"> complete</s></p>
<p t="2710" d="3049" a="1">
</p>
<p t="2720" d="5360"><s>how</s><s t="240"> I</s><s t="560"> make</s></p>
</body>
</timedtext>"#;

    const FLAT: &str = r#"<?xml version="1.0" encoding="utf-8" ?>
<timedtext>
<body>
<p t="1000" d="2000">Hello world</p>
<p t="3500" d="1500"></p>
</body>
</timedtext>"#;

    const SRV1: &str = r#"<?xml version="1.0" encoding="utf-8" ?>
<transcript>
<text start="1.5" dur="2.0">First cue</text>
<text start="4" dur="1">Second cue</text>
</transcript>"#;

    #[test]
    fn test_format3_keeps_spaces_and_drops_empty_windows() {
        let segs = parse_timedtext(FORMAT3);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["This is a complete", "how I make"]);
        assert_eq!(segs[0].start, 0.2);
        assert_eq!(segs[0].duration, 5.6);
    }

    #[test]
    fn test_flat_android_cues() {
        let segs = parse_timedtext(FLAT);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "Hello world");
        assert_eq!(segs[0].start, 1.0);
        assert_eq!(segs[0].duration, 2.0);
    }

    #[test]
    fn test_srv1_cues() {
        let segs = parse_timedtext(SRV1);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["First cue", "Second cue"]);
        assert_eq!(segs[0].start, 1.5);
        assert_eq!(segs[0].duration, 2.0);
        assert_eq!(segs[1].start, 4.0);
        assert_eq!(segs[1].duration, 1.0);
    }

    #[test]
    fn test_parse_video_id() {
        assert_eq!(parse_video_id("dQw4w9WgXcQ").unwrap(), "dQw4w9WgXcQ");
        assert_eq!(parse_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ").unwrap(), "dQw4w9WgXcQ");
        assert_eq!(parse_video_id("https://youtu.be/dQw4w9WgXcQ").unwrap(), "dQw4w9WgXcQ");
        assert_eq!(parse_video_id("https://www.youtube.com/shorts/dQw4w9WgXcQ").unwrap(), "dQw4w9WgXcQ");
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0.0), "0:00");
        assert_eq!(format_timestamp(45.0), "0:45");
        assert_eq!(format_timestamp(65.0), "1:05");
        assert_eq!(format_timestamp(3665.0), "1:01:05");
    }
}
