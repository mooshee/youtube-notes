#!/usr/bin/env python3
"""Pull YouTube captions and light metadata with Python stdlib.

Author: Daniel Hallman

Usage:
    python3 fetch_captions.py URL -o transcript.json
    python3 fetch_captions.py URL --dir /tmp/video-id
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import urllib.error
import urllib.request
import xml.etree.ElementTree as ET
from html import unescape
from pathlib import Path
from urllib.parse import parse_qsl, urlencode, urlparse, urlunparse

USER_AGENT = (
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
    "AppleWebKit/537.36 (KHTML, like Gecko) "
    "Chrome/120.0.0.0 Safari/537.36"
)
ANDROID_CONTEXT = {
    "client": {"clientName": "ANDROID", "clientVersion": "20.10.38"}
}
VIDEO_ID_RE = re.compile(
    r"(?:v=|/v/|youtu\.be/|embed/|shorts/)([A-Za-z0-9_-]{11})"
)


class CaptionError(RuntimeError):
    """Captions or metadata could not be fetched."""


def parse_video_id(url_or_id: str) -> str:
    text = url_or_id.strip()
    if re.fullmatch(r"[A-Za-z0-9_-]{11}", text):
        return text
    match = VIDEO_ID_RE.search(text)
    if match:
        return match.group(1)
    raise CaptionError(f"Could not read a YouTube video id from: {url_or_id}")


def format_timestamp(seconds: float) -> str:
    total = max(0, int(seconds))
    hours, rem = divmod(total, 3600)
    minutes, secs = divmod(rem, 60)
    if hours:
        return f"{hours}:{minutes:02d}:{secs:02d}"
    return f"{minutes}:{secs:02d}"


def _request(url: str, data: bytes | None = None, timeout: int = 20) -> bytes:
    headers = {
        "User-Agent": USER_AGENT,
        "Accept-Language": "en-US,en;q=0.9",
    }
    if data is not None:
        headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.read()
    except urllib.error.HTTPError as exc:
        raise CaptionError(f"HTTP {exc.code} for {url}") from exc
    except urllib.error.URLError as exc:
        raise CaptionError(f"Network error for {url}: {exc.reason}") from exc


def cue_text(elem: ET.Element) -> str:
    """Join all visible text in a cue, including nested word spans.

    YouTube timedtext format=3 stores each word on a child <s> node and
    keeps a leading space on later words. Strip only the finished cue.
    """
    parts: list[str] = []
    if elem.text:
        parts.append(elem.text)
    for child in elem:
        parts.append(cue_text(child))
        if child.tail:
            parts.append(child.tail)
    return "".join(parts)


def parse_timedtext(xml_text: str) -> list[dict]:
    root = ET.fromstring(xml_text)
    segments: list[dict] = []

    for elem in root.iter("p"):
        start_ms = elem.get("t")
        if start_ms is None:
            continue
        duration_ms = float(elem.get("d") or 0)
        text = unescape(cue_text(elem)).strip()
        if not text:
            continue
        segments.append(
            {
                "start": round(float(start_ms) / 1000, 1),
                "duration": round(duration_ms / 1000, 1),
                "text": text,
            }
        )
    if segments:
        return segments

    for elem in root.iter("text"):
        text = unescape(cue_text(elem)).strip()
        if not text:
            continue
        segments.append(
            {
                "start": round(float(elem.get("start") or 0), 1),
                "duration": round(float(elem.get("dur") or 0), 1),
                "text": text,
            }
        )
    return segments


def _with_fmt(url: str, fmt: str) -> str:
    parts = urlparse(url)
    query = dict(parse_qsl(parts.query, keep_blank_values=True))
    query["fmt"] = fmt
    return urlunparse(parts._replace(query=urlencode(query)))


def _watch_page(video_id: str) -> str:
    return _request(f"https://www.youtube.com/watch?v={video_id}").decode(
        "utf-8", "replace"
    )


def _api_key(page_html: str) -> str:
    match = re.search(r'"INNERTUBE_API_KEY":\s*"([A-Za-z0-9_-]+)"', page_html)
    if not match:
        raise CaptionError("Watch page did not include an InnerTube API key")
    return match.group(1)


def _player_payload(video_id: str, api_key: str) -> dict:
    body = json.dumps(
        {"context": ANDROID_CONTEXT, "videoId": video_id}
    ).encode("utf-8")
    raw = _request(
        f"https://www.youtube.com/youtubei/v1/player?key={api_key}",
        data=body,
    )
    return json.loads(raw.decode("utf-8"))


def _caption_tracks(player: dict) -> list[dict]:
    captions = player.get("captions") or {}
    renderer = captions.get("playerCaptionsTracklistRenderer") or {}
    tracks = renderer.get("captionTracks") or []
    if not tracks:
        raise CaptionError("This video has no captions")
    return tracks


def _pick_track(tracks: list[dict], language: str) -> dict:
    for track in tracks:
        if track.get("languageCode") == language:
            return track
    for track in tracks:
        code = str(track.get("languageCode") or "")
        if code.startswith(language):
            return track
    return tracks[0]


def _player_metadata(player: dict) -> dict:
    details = player.get("videoDetails") or {}
    micro = (
        (player.get("microformat") or {}).get("playerMicroformatRenderer") or {}
    )
    duration = int(details.get("lengthSeconds") or 0)
    published = str(micro.get("publishDate") or micro.get("uploadDate") or "")
    return {
        "title": details.get("title") or "",
        "channel": details.get("author") or "",
        "duration": duration,
        "duration_formatted": format_timestamp(duration) if duration else "",
        "published": published,
        "view_count": int(details.get("viewCount") or 0),
        "is_live": bool(details.get("isLiveContent")),
    }


def _ytdlp_metadata(video_id: str) -> dict:
    try:
        result = subprocess.run(
            [
                "yt-dlp",
                "--dump-json",
                "--no-download",
                "--no-warnings",
                f"https://www.youtube.com/watch?v={video_id}",
            ],
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return {}
    if result.returncode != 0:
        return {}
    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError:
        return {}
    duration = int(data.get("duration") or 0)
    return {
        "title": data.get("title") or "",
        "channel": data.get("channel") or data.get("uploader") or "",
        "duration": duration,
        "duration_formatted": format_timestamp(duration) if duration else "",
        "published": data.get("upload_date") or "",
        "view_count": int(data.get("view_count") or 0),
        "chapters": data.get("chapters") or [],
        "language": data.get("language") or "",
    }


def fetch_captions(url_or_id: str, language: str = "en") -> dict:
    video_id = parse_video_id(url_or_id)
    page_html = _watch_page(video_id)
    player = _player_payload(video_id, _api_key(page_html))
    track = _pick_track(_caption_tracks(player), language)
    track_url = track.get("baseUrl")
    if not track_url:
        raise CaptionError("Caption track is missing a URL")

    xml_text = _request(track_url).decode("utf-8", "replace")
    segments = parse_timedtext(xml_text)
    if not segments:
        xml_text = _request(_with_fmt(track_url, "srv1")).decode(
            "utf-8", "replace"
        )
        segments = parse_timedtext(xml_text)
    if not segments:
        raise CaptionError("Caption XML had no readable text")

    metadata = _ytdlp_metadata(video_id) or _player_metadata(player)
    kind = track.get("kind") or ""
    generated = kind == "asr" or "asr" in str(track.get("vssId") or "")
    return {
        "video_id": video_id,
        "url": f"https://www.youtube.com/watch?v={video_id}",
        "language": track.get("languageCode") or language,
        "auto_generated": generated,
        "metadata": metadata,
        "segments": segments,
        "total_segments": len(segments),
    }


def timed_lines(payload: dict) -> str:
    meta = payload.get("metadata") or {}
    header = [
        f"# {meta.get('title') or payload['video_id']}",
        f"Channel: {meta.get('channel') or ''}",
        f"Duration: {meta.get('duration_formatted') or ''}",
        f"URL: {payload['url']}",
        f"Captions: {'auto-generated' if payload.get('auto_generated') else 'human'}",
        "",
        "---",
        "",
    ]
    body = [
        f"[{format_timestamp(seg['start'])}] {seg['text']}"
        for seg in payload["segments"]
    ]
    return "\n".join(header + body) + "\n"


def plain_lines(payload: dict) -> str:
    meta = payload.get("metadata") or {}
    header = [
        f"# {meta.get('title') or payload['video_id']}",
        f"Channel: {meta.get('channel') or ''}",
        f"Duration: {meta.get('duration_formatted') or ''}",
        f"URL: {payload['url']}",
        "",
        "---",
        "",
    ]
    body = [seg["text"] for seg in payload["segments"]]
    return "\n".join(header + body) + "\n"


def write_dir(payload: dict, directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "transcript.json").write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
    )
    (directory / "transcript-timed.txt").write_text(timed_lines(payload))
    (directory / "transcript.txt").write_text(plain_lines(payload))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Fetch YouTube captions as JSON or text files"
    )
    parser.add_argument("url", help="YouTube URL or 11-character video id")
    parser.add_argument("-o", "--output", help="Write JSON to this path")
    parser.add_argument("--dir", help="Write json, timed, and plain files here")
    parser.add_argument("--timed", help="Write a [M:SS] transcript here")
    parser.add_argument("--plain", help="Write caption text without timestamps")
    parser.add_argument(
        "-l",
        "--language",
        default="en",
        help="Preferred caption language (default: en)",
    )
    args = parser.parse_args(argv)

    try:
        payload = fetch_captions(args.url, language=args.language)
    except CaptionError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1

    dumped = json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
    if args.dir:
        write_dir(payload, Path(args.dir))
        print(f"wrote {args.dir}", file=sys.stderr)
    if args.output:
        Path(args.output).write_text(dumped)
        print(f"wrote {args.output}", file=sys.stderr)
    if args.timed:
        Path(args.timed).write_text(timed_lines(payload))
        print(f"wrote {args.timed}", file=sys.stderr)
    if args.plain:
        Path(args.plain).write_text(plain_lines(payload))
        print(f"wrote {args.plain}", file=sys.stderr)
    if not any([args.dir, args.output, args.timed, args.plain]):
        sys.stdout.write(dumped)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
