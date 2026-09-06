#!/usr/bin/env python3
"""Caption XML and URL parsing tests. No network."""

from __future__ import annotations

import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from fetch_captions import (  # noqa: E402
    format_timestamp,
    parse_timedtext,
    parse_video_id,
    timed_lines,
)


FORMAT3 = """<?xml version="1.0" encoding="utf-8" ?>
<timedtext format="3">
<body>
<p t="160" d="5599"><s>This</s><s t="240"> is</s><s t="400"> a</s><s t="640"> complete</s></p>
<p t="2710" d="3049" a="1">
</p>
<p t="2720" d="5360"><s>how</s><s t="240"> I</s><s t="560"> make</s></p>
</body>
</timedtext>
"""

FLAT = """<?xml version="1.0" encoding="utf-8" ?>
<timedtext>
<body>
<p t="1000" d="2000">Hello world</p>
<p t="3500" d="1500"></p>
</body>
</timedtext>
"""

SRV1 = """<?xml version="1.0" encoding="utf-8" ?>
<transcript>
<text start="1.5" dur="2.0">First cue</text>
<text start="4" dur="1">Second cue</text>
</transcript>
"""


class ParseTests(unittest.TestCase):
    def test_format3_keeps_spaces_and_drops_empty_windows(self):
        segs = parse_timedtext(FORMAT3)
        self.assertEqual(
            [s["text"] for s in segs],
            ["This is a complete", "how I make"],
        )
        self.assertEqual(segs[0]["start"], 0.2)
        self.assertEqual(segs[0]["duration"], 5.6)

    def test_flat_android_cues(self):
        self.assertEqual(
            parse_timedtext(FLAT),
            [{"start": 1.0, "duration": 2.0, "text": "Hello world"}],
        )

    def test_srv1_text_nodes(self):
        self.assertEqual(
            parse_timedtext(SRV1),
            [
                {"start": 1.5, "duration": 2.0, "text": "First cue"},
                {"start": 4.0, "duration": 1.0, "text": "Second cue"},
            ],
        )

    def test_video_ids(self):
        vid = "AdH49hw_BnA"
        self.assertEqual(parse_video_id(vid), vid)
        self.assertEqual(
            parse_video_id(f"https://youtu.be/{vid}?si=abc"), vid
        )
        self.assertEqual(
            parse_video_id(f"https://www.youtube.com/watch?v={vid}&t=12"),
            vid,
        )
        self.assertEqual(
            parse_video_id(f"https://www.youtube.com/embed/{vid}"), vid
        )
        self.assertEqual(
            parse_video_id(f"https://www.youtube.com/shorts/{vid}"), vid
        )

    def test_timestamp_and_timed_export(self):
        self.assertEqual(format_timestamp(5), "0:05")
        self.assertEqual(format_timestamp(3723), "1:02:03")
        payload = {
            "video_id": "AdH49hw_BnA",
            "url": "https://www.youtube.com/watch?v=AdH49hw_BnA",
            "auto_generated": True,
            "metadata": {"title": "Demo", "channel": "Moosh", "duration_formatted": "0:05"},
            "segments": [{"start": 1.2, "duration": 2.0, "text": "Hello there"}],
        }
        text = timed_lines(payload)
        self.assertIn("[0:01] Hello there", text)
        self.assertIn("Captions: auto-generated", text)


if __name__ == "__main__":
    unittest.main()
