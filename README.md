# youtidy 🎵

`youtidy` is a Rust-based TUI (Terminal User Interface) app that downloads audio from YouTube, automatically identifies tracks using acoustic fingerprinting (**AcoustID & MusicBrainz**), embeds album art, and organizes ID3v2 metadata.

## Features ✨

- 🔍 **Built-in YouTube Search**: Search for tracks directly from the terminal.
- 📋 **Batch Playlist Download**: Batch downloads entire YouTube playlists with interactive selection, concurrent downloading (up to 3 tracks simultaneously), and built-in API rate-limiting to avoid restrictions.
- ⚡ **Zero-Transcoding & Stream Copy**: Skips transcoding entirely when the downloaded format matches your target format (`m4a` or `opus`), or performs instant stream extraction (`webm` to `opus` copy) to save CPU and complete downloads instantly.
- 🚀 **Asynchronous Downloading**: Downloads audio via `yt-dlp` without blocking the TUI.
- 🧠 **Auto Track Identification**:
  - Uses `fpcalc` to generate acoustic fingerprints.
  - Queries the **AcoustID API** to fetch the MusicBrainz Recording ID.
  - Queries the **MusicBrainz API** to match the best official release (Album/Single) based on heuristics.
  - Falls back to Spotify search if AcoustID/MusicBrainz lookup fails (using `SpotipyFree`).
- 🎨 **Album Art Retrieval & Embedding**:
  - Fetches official cover art from the **Cover Art Archive** using MusicBrainz release data.
  - Falls back to fetching cover art from Spotify if the Spotify fallback was triggered.
  - Falls back to the highest quality YouTube thumbnail (`maxresdefault` / `hqdefault`) if no official cover is found.
  - Saves the cover image (`cover.jpg`/`cover.png`) directly into the album directory.
  - Embeds the album art directly into the MP3 ID3v2 metadata.
- 🏷️ **Auto Tagging**: Writes ID3v2 tags (Title, Artist, Album) using the `lofty` crate.
- 📁 **Folder Organization**: Neatly saves downloaded files following a `Music/<Artist>/<Album>/<Title>.mp3` directory structure.
- 💾 **Local Cache**: Caches download records at `cache/{video_id}.json`. Re-downloading the same video will be skipped if both the audio and target format extension match, and `.lrc` files still exist.
- 🎶 **Lyrics Romanization & Alignment**: Detects Japanese text in synced lyrics (LRCLIB), matches them against Genius lyrics using Romaji normalization, and shifts timestamps based on duration offsets.

## Prerequisites 🛠️

Make sure you have `yt-dlp`, `ffmpeg`, and `fpcalc` (AcoustID fingerprint tool) installed and available in your `$PATH`.

<details>
<summary><b>💡 Click to show installation guide for your OS</b></summary>

### Arch Linux
```bash
sudo pacman -S yt-dlp ffmpeg chromaprint
```

### Debian / Ubuntu
```bash
sudo apt update && sudo apt install ffmpeg libchromaprint-tools
# Install latest yt-dlp
sudo wget https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -O /usr/local/bin/yt-dlp
sudo chmod a+rx /usr/local/bin/yt-dlp
```

### Fedora / RHEL
```bash
sudo dnf install yt-dlp ffmpeg chromaprint
```

### macOS (Homebrew)
```bash
brew install yt-dlp ffmpeg chromaprint
```

### Windows
Using [Scoop](https://scoop.sh/):
```bash
scoop install yt-dlp ffmpeg
```
*Download the `fpcalc.exe` binary from [AcoustID](https://acoustid.org/chromaprint) and add it to your system's `Path` environment variable.*
</details>

## Configuration ⚙️

Create a `config.toml` in the app directory or `~/.config/youtidy/config.toml` (see `config.example.toml` for templates).

### Key Settings
- **AcoustID**: Set `acoustid_client_id` in `config.toml` (or `ACOUSTID_CLIENT_ID` in `.env`). Register at [AcoustID](https://acoustid.org/login) for a free key.
- **Output Folder**: Configure `[music] directory` (e.g. `~/Music`) and output formats like `mp3`, `m4a`, `opus`, or `flac`.

---

## Usage & Controls 🚀

### Getting Started
- **From Release Binary**: Download from [Releases](https://github.com/Grizz96/youtidy/releases), extract, set up your `config.toml`, and run `./youtidy`.
- **From Source**: Clone and run with `cargo run --release`.

### TUI Quick Reference

| Mode / Screen | Action & Keybindings |
| :--- | :--- |
| **Main Menu** | <ul><li>`↑ / ↓` to navigate modes</li><li>`Enter` to confirm selection</li></ul> |
| **Single Search** | <ul><li>Type query directly + `Enter` to search</li><li>`↑ / ↓` or mouse click to navigate results</li><li>`Enter` to start downloading & fingerprint tagging</li></ul> |
| **Playlist Downloader** | <ul><li>Paste Playlist URL + `Enter` to fetch tracks</li><li>`Space` to select/deselect track</li><li>`A` to toggle select/deselect all</li><li>`D` or `Enter` to start batch download (up to 3 concurrent)</li></ul> |
| **Real-time Logs** | <ul><li>Auto-scrolls by default</li><li>`↑ / ↓` to scroll manually line-by-line</li><li>`PageUp / PageDown` to page scroll</li></ul> |
| **Global Controls** | <ul><li>`Esc` to go back to previous screen</li><li>`q` to quit</li></ul> |

## License 📄

This project was created for personal use to help manage local music collections.
