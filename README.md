# youtidy 🎵

`youtidy` is a Rust-based TUI (Terminal User Interface) app that downloads audio from YouTube, automatically identifies tracks using acoustic fingerprinting (**AcoustID & MusicBrainz**), embeds album art, and organizes ID3v2 metadata.

## Features ✨

- 🔍 **Built-in YouTube Search**: Search for tracks directly from the terminal.
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
- 💾 **Local Cache**: Caches download records at `cache/{video_id}.json`. Re-downloading the same video will be skipped if both the audio and `.lrc` files still exist.
- 🎶 **Lyrics Romanization & Alignment**: Detects Japanese text in synced lyrics (LRCLIB), matches them against Genius lyrics using Romaji normalization, and shifts timestamps based on duration offsets.

## Prerequisites 🛠️

Make sure you have the following system dependencies installed and available in your `$PATH`:

1. **yt-dlp**: For downloading audio from YouTube.
2. **ffmpeg**: Required by `yt-dlp` to convert the downloaded audio to MP3.
3. **fpcalc**: An AcoustID tool for calculating acoustic fingerprints.

### Installing Dependencies

**Arch Linux**
```bash
sudo pacman -S yt-dlp ffmpeg chromaprint

```

**Debian / Ubuntu**

```bash
sudo apt update
sudo apt install ffmpeg libchromaprint-tools
# For yt-dlp (latest release recommended over APT repo)
sudo wget [https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp](https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp) -O /usr/local/bin/yt-dlp
sudo chmod a+rx /usr/local/bin/yt-dlp

```

**Fedora / RHEL**

```bash
sudo dnf install yt-dlp ffmpeg chromaprint

```

**macOS (Homebrew)**

```bash
brew install yt-dlp ffmpeg chromaprint

```

**Windows**
It is highly recommended to use a package manager like [Scoop](https://scoop.sh/) or [Winget](https://learn.microsoft.com/en-us/windows/package-manager/winget/) to install the dependencies easily, or download them manually:

* **yt-dlp**: Run `scoop install yt-dlp` (or `winget install yt-dlp`), or download `yt-dlp.exe` from the [official releases](https://github.com/yt-dlp/yt-dlp/releases).
* **ffmpeg**: Run `scoop install ffmpeg` (or `winget install ffmpeg`), or download from [ffmpeg.org](https://ffmpeg.org/download.html).
* **fpcalc**: Download the `fpcalc.exe` binary from the [AcoustID / Chromaprint website](https://acoustid.org/chromaprint).
* **Important for Windows**: If downloading manually, ensure all `.exe` files are extracted and placed in a folder that is added to your system's **Environment Variables (`Path`)** so they can be accessed globally from the terminal.

## Configuration ⚙️

The application requires an **AcoustID** API Key.

1. Get a free API Key (Client ID) by logging in with your MusicBrainz account at [AcoustID](https://acoustid.org/login).
2. Create an `.env` file in the project root and add your Client ID:

```env
ACOUSTID_CLIENT_ID=your_acoustid_client_id_here

```

## Usage 🚀

1. Ensure your `.env` file is properly configured.
2. Run the app using `cargo`:

```bash
cargo run

```

### Keybindings:

* **Type anything**: Enter keywords in the search bar.
* **Enter**: Trigger a search, or select a search result to start processing.
* **Up / Down Arrow / Mouse Click**: Navigate through the YouTube search results.
* **Esc**: Return to the search view from the processing logs.
* **q**: Quit the application.

## License 📄

This project was created for personal use to help manage local music collections.
