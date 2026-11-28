---
name: captae
description: Use Captae on the user's computer to download videos, audio and images from web pages, convert formats, make subtitles, cut clips, search the media library, or turn a video into a transcript plus key frames so you can understand it. Use it when the user gives you a video link or says things like “download this”, “convert it to MP3”, “what is this video about” or “find the one I downloaded before”.
---

# Captae

Captae is a local media tool installed on the user's computer, and all files stay on the user's own disk. You call it through the MCP server `captae`, started with `captae mcp --stdio`. When the Captae desktop app is open, requests are handed to it and the user can watch the progress there; when it isn't, the command-line process does the work itself.

## Tools

| Tool | When to use it |
|---|---|
| `download` | The user gives you a link and wants to save it. Pages on YouTube, Bilibili and other sites, direct media links and m3u8 all work. By default it waits until the download finishes and returns `path`, `asset_id` and `type` (video / audio / image). For playlists and posts with several videos, see “Playlists and live streams” below |
| `library_query` | The user says “the one I downloaded before” or “find xx”. `q` searches titles, descriptions, tags and speech transcripts; you can filter by `type`, `source` (source site), `days`, `tags` and `category` |
| `share_video` | You need to **understand** what a video is about. Returns a Markdown transcript (with timestamps) and several key frame images, so you don't have to watch the video yourself; the prepared files are in `bundle_dir` |
| `subtitle` | The user wants a subtitle file. It uses the site's own subtitles first, and transcribes locally if there are none |
| `convert` | Change the format: MP4 / WebM / MOV / ProRes 422 / MP3 / AAC / FLAC / WAV / PNG / JPG / WebP / AVIF |
| `trim` | Cut out a section. Times can be seconds or `mm:ss`; pass an audio item id or a file path in `replace_audio` to replace the sound |
| `concat` | Join several videos into one, in order. `inputs` is an array of item ids or local paths, at least two |
| `library_add_tag` | Tag an item so it's easier to find later |
| `library_categories` | See how the user's library is organized: categories (like folders, each item in at most one), item counts, how many are uncategorized |
| `library_set_category` | The user asks you to file an item somewhere. Use an existing category name from `library_categories` when one fits; a new name creates a category. Captae's automatic sorting won't move items filed this way |
| `preset_list` / `preset_run` | One-click workflows the user has saved, such as “Douyin footage” or “Podcast to text”. When the user mentions a preset by name, check it with `preset_list` first |

Most tools accept either an `asset_id` (from what `download` or `library_query` returned) or a local `path`. Prefer `asset_id`.

The user can turn off AI access in Captae, or turn off individual tools. Tools that are turned off don't appear in the tool list, and Captae sends `notifications/tools/list_changed` when the list changes. In the Captae interface these tools are shown with names like `captae.library.query` and `captae.library.add_tag`; when you call them, use the names in the table above.

`convert`, `trim` and `subtitle` name their results automatically and put them next to the original file (or in `output_dir`), adding “(2)” when the name is taken. To choose the file name, pass the full path in `output` (the extension must match the result; if you leave it out, it's added for you). If the file in `output` already exists, the call fails with `OUTPUT_EXISTS` and nothing is overwritten.

## How to work

- Check before downloading. The video the user means may already be in the library, so try `library_query` first, and only `download` if it isn't there.
- When you want to know what a video says, use `share_video`. Don't just download it and tell the user “downloaded”.
- For large downloads, or several at once, pass `wait: false`. Once you have the job id you can reply to the user, and they can follow the progress in the Captae window.
- Describe paths to the user in plain words, for example “saved to Movies › Captae”, unless they ask for the full path.
- Don't decide to delete or overwrite files for the user. Captae doesn't offer a delete tool, and in normal mode it never overwrites existing files.

## Playlists and live streams

- When the link is a playlist, a post with several videos, or a Bilibili video with several parts, `download` doesn't download everything right away. It returns `status: "choose"` and `entries` (each with `id`, number `n`, title and duration).
  Ask the user which ones they want (unless they already said), then call it again with the same `url` plus `items`: `items` takes `entries[].id` values or numbers, and `["all"]` means all of them.
- While a live stream is in progress, `download` fails with `code` `live`. If the user wants to record it, add `record_live: true` to record from now on, with `max_minutes` as the maximum length in minutes:
  it defaults to 10 minutes and can be at most 180, and decimals are allowed (more than 180, or a value that isn't positive, fails with `BAD_ARGS`). Recording stops automatically when the limit is reached, and the recorded part is saved as a playable file in the library.
  It returns `path` and `asset_id`, plus `recording`: `seconds` (how many seconds were recorded), `max_seconds` (the maximum number of seconds) and `note` (why it stopped: the limit was reached, the live stream ended, or the user clicked “Stop and save” in Captae).
  If the user wants to record for a long time, ask how long first. If you don't want to wait, pass `wait: false`; the user can “Stop and save” in Captae at any time. The recorded file runs a few seconds longer than the set length (recording starts from the newest few segments in the live playlist).
- If the live stream hasn't started yet, `code` is `live_upcoming` and `startsAt` is the start time.
- Each recording of the same live stream link has different content: an earlier recording of this link in the library doesn't mean you don't need to record now (when Captae checks “already downloaded?”, recorded live streams don't count either).

## Joining videos (concat)

- The order of `inputs` is the playback order in the result, for example `{"inputs": [12, 15, "~/Movies/outro.mp4"]}`.
- If the videos all have the same codec, resolution and frame rate, they're joined directly without re-encoding, which is fast and loses no quality. If not, the first video sets the format: the others are scaled (with black bars where needed) and matched to its frame rate, and the result is re-encoded to MP4, which is slower. Silence is added for clips without sound.
- Only videos can be joined; audio and images cause an error. The result goes into the library, and `path` and `asset_id` are returned.

## Basic mode and Advanced mode

Basic mode is the default: only the parameters above are available, and Captae picks the codecs and the download method itself.

After the user turns on Advanced mode on the “AI assistant” page in Captae, you can also pass:

- `download.ytdlp_args`, `download.aria2_args`, `download.aria2_threads`: web page links go through yt-dlp and only accept `ytdlp_args`; direct media links go through aria2 and only accept `aria2_args` and `aria2_threads`; m3u8 is downloaded by Captae itself and accepts neither. Passing the wrong ones fails with `BAD_ARGS`
- `convert.ffmpeg_args`, `trim.ffmpeg_args`: added before the output file
- `convert.overwrite`, `trim.overwrite`, `subtitle.overwrite`: used together with `output`, they overwrite it if it already exists. The original file itself can never be overwritten

These parameters are passed as they are to yt-dlp / aria2 / ffmpeg. Use them only when the user explicitly asks for a specific detail, and confirm with the user before overwriting a file. In basic mode they're rejected with error code `ADVANCED_MODE_OFF`; tell the user where to turn it on, and don't keep retrying.

## Errors

When a tool fails, `isError` is true and the content is JSON: `error` (a code for machines) and `message` (a sentence for people, in the language of the user's Captae interface). Common ones:

| error | Meaning | What to do |
|---|---|---|
| `ACCESS_OFF` | The user turned off “Let AI assistants use Captae” | Tell the user. Don't retry |
| `TOOL_OFF` | The user turned off this particular tool | Same as above |
| `ADVANCED_MODE_OFF` | You used advanced parameters | Retry without them, or ask the user to turn on Advanced mode |
| `BAD_ARGS` | Advanced parameters were given to the wrong engine | Switch to the right parameters as `message` says |
| `OUTPUT_EXISTS` | The file in `output` already exists | Use another file name. If the user explicitly wants to overwrite it, add `overwrite: true` in Advanced mode |
| `BAD_OUTPUT` | `output` isn't a full path, has the wrong extension, or points to the original file itself | Fix it as `message` says |
| `OUTPUT_NOT_WRITABLE` | The save location can't be written to (for example, an external drive isn't connected) | Tell the user, or use another location |
| `FILE_NOT_FOUND` | The file in `path` doesn't exist | Check the path, or use `library_query` first |
| `DOWNLOAD_FAILED` | The download didn't succeed. Also includes `code` (the reason), `next` (what to do next) and `raw` (the original error); see the table below | Pass `message` and `next` on to the user |
| `PRESET_NOT_FOUND` / `PRESET_NOT_ALLOWED` / `PRESET_FAILED` | No such preset / the user hasn't allowed AI to use it / it failed while running | Check which presets exist with `preset_list` first |
| `CANCELLED` | The user cancelled this job in Captae | Tell the user it was cancelled. Don't retry on your own |
| `FAILED` | Other failures, such as an item's file having been moved | Pass `message` on to the user; if an item can't be found, run `library_query` again |

`code` values for `DOWNLOAD_FAILED`:

| code | Meaning | What to do |
|---|---|---|
| `need_login` | The user has to sign in to watch it (age restriction, membership, private; without signing in, paid content only gives a preview clip) | Ask the user to sign in to this site in Captae (“Sign in to X” on the job, or “Settings › Signed-in sites”), or to download it with the Captae extension in their browser (which uses the browser's sign-in), then try again. You don't get, and don't need, the user's sign-in |
| `need_verify` | The site wants to confirm it's not a bot | Ask the user to click “Verify” on the job in Captae, then try again |
| `drm` | Protected by DRM; Captae can't download it | Tell the user. Don't retry |
| `geo` / `blocked` | Not available in the user's region / blocked by the network | Ask the user to switch networks or check the proxy, then try again |
| `unavailable` | The video was removed, doesn't exist, or the link no longer works | Tell the user |
| `unsupported` | The download engine can't read this site (or it's an image post) | Ask the user to open the page in their browser and download it with the Captae extension |
| `no_media` | There's no video or audio on the page | Tell the user |
| `live` / `live_upcoming` | Live now / hasn't started yet | See “Playlists and live streams” above |
| `rate_limited` | The site asked to slow down | Captae retries automatically after a while; you don't need to retry |
| `network` | The network is unstable | You can retry once |
| `disk` | The save location is full or not writable | Tell the user |
| `tool` | A Captae download component has a problem | Ask the user to “Check for updates” or “Repair” in Captae's settings |
| `unknown` | Other | Pass `message` on to the user |

Every call is recorded in “Recent calls” in Captae, so the user can see what you did.
