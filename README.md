# YouTube Notes: Captions and a Brief You Can Check

Paste a YouTube URL. This open Agent Skill from [Moosh Works](https://mooshworks.com/skills/) pulls the captions, writes a timestamped transcript, and can turn that tape into a short brief with claims you can jump back to.

It reads public captions. It does not download the video, and it does not invent speech when YouTube has no captions.

## Install

Install globally with the open `skills` CLI:

```bash
npx skills add mooshee/youtube-notes --skill youtube-notes -g
```

The installable payload lives in `skills/youtube-notes/`, so the CLI includes the caption fetcher rather than only the instruction file.

Restart the host application if the skill is not discovered in the current session.

## Use

```text
Use $youtube-notes to pull captions from this YouTube URL and write a short brief.
```

From a repository checkout:

```bash
python3 skills/youtube-notes/scripts/fetch_captions.py \
  "https://www.youtube.com/watch?v=VIDEO_ID" \
  --dir /tmp/VIDEO_ID
```

That writes `transcript.json`, `transcript-timed.txt`, and `transcript.txt`. Python 3.8+ is enough. `yt-dlp` on PATH is optional and only used for richer metadata.

Ask for the transcript alone when you want the files and nothing else.

## Need another skill?

[Scripted Product Demo](https://github.com/mooshee/product-demo) records one product walkthrough and renders it with camera moves, cursor motion, click feedback, privacy masks, and 6 supported layouts.

[ChatGPT Query Research](https://github.com/mooshee/chatgpt-search-query-mining) turns web-search queries from a ChatGPT session you control into a source-backed content plan.

See every install command on the [Moosh Works skills page](https://mooshworks.com/skills/).

## Author & License

Built by [Daniel Hallman](https://github.com/mooshee) at [Moosh Works](https://mooshworks.com/).

MIT licensed.
