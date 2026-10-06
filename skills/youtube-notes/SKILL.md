---
name: youtube-notes
description: "Get a YouTube transcript, notes, digest or takeaways when the user asks to read a video."
---

# YouTube Notes

Pull captions from a YouTube URL, save a timestamped transcript, and write a short brief the user can check against the tape. Captions are the source of truth. If YouTube has no captions, stop and say so.

## Extract

Resolve `<skill-dir>` as the directory that contains this `SKILL.md`. Then run:

```bash
python3 <skill-dir>/scripts/fetch_captions.py "https://www.youtube.com/watch?v=VIDEO_ID" \
  --dir youtube-notes/VIDEO_ID
```

The script uses Python 3 stdlib only. It talks to YouTube's player API, reads timedtext XML, and joins nested word spans used by current auto-captions. Optional `yt-dlp` on PATH fills richer metadata. Do not re-download some other caption script.

`--dir` writes:

- `transcript.json` — metadata plus timed segments
- `transcript-timed.txt` — `[M:SS] cue` lines
- `transcript.txt` — cue text without timestamps

If the user names an output folder, use that. Otherwise `youtube-notes/<VIDEO_ID>/` in the current workspace is fine.

When they only asked for the transcript or captions, write those files and stop.

## Notes

When they asked for a summary, digest, notes, takeaways, or a watch/skip call, read the JSON and write `notes.md` next to the transcript using this shape:

```markdown
# <title>

**Channel:** <channel> · **Duration:** <H:MM:SS> · **Published:** <date>
**URL:** <watch url>
**Captions:** auto-generated | human

## Bottom line
One or two sentences. What the video is arguing, selling, or teaching.

## Use it for
- Who this is actually for, and what they can do after watching
- Skip this section if the user only wanted a recap

## Claims to check
- <claim> (<timestamp>)
- Mark opinion, anecdote, and unsourced numbers as such

## Scene list
| Time | What happens |
|------|----------------|
| 0:00 | ... |

## Quotes
> <short quote> — <timestamp>
```

Keep quotes short. Prefer paraphrase when a line is messy ASR. Auto-generated captions get a quality note in the header, not a lecture.

Watch/skip requests get Bottom line, Use it for, and a one-line verdict. Skip the scene list if the video is under a minute.

## Limits

- No captions → report that. Do not invent speech from the title.
- Local files → a transcription skill, not this one.
- Video download → out of scope.
- Publishing to YouTube → out of scope.
- Do not treat captions as proof of revenue, credentials, or product results.
