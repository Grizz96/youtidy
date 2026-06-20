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

The application reads configurations from `config.toml` in the current directory or `~/.config/youtidy/config.toml`. Create a `config.toml` using `config.example.toml` as a reference.

### Environment Variables & Credentials
- **AcoustID**: Register at [AcoustID](https://acoustid.org/login) to get a Client ID. Add it as `acoustid_client_id` under `[api]` in `config.toml` or `ACOUSTID_CLIENT_ID` in `.env`.
- **Genius**: (Optional, for lyrics scraping fallbacks) Register at the [Genius Developer Portal](https://genius.com/api-clients) to get an access token. Add it as `genius_access_token` under `[api]` in `config.toml` or `GENIUS_ACCESS_TOKEN` in `.env`.

### Configurable Options
Configure these sections in your `config.toml`:

#### `[music]`
- `directory`: Base music directory to save tracks (e.g., `~/Music`).
- `format`: Audio file format (e.g., `mp3`).
- `save_cover_file`: If `true`, saves the cover image file in the album directory.
- `cache_directory`: Cache path for temporary downloads.
- `bitrate`: Specific target bitrate (e.g. `320k`, `192k`).
- `audio_quality`: ffmpeg VBR audio quality option (0-9, where 0 is best).
- `folder_structure`: Custom output path format (e.g. `{artist}/{album}`).
- `filename_template`: Custom filename template (e.g. `{title}` or `{artist} - {title}`).
- `embed_lyrics`: If `true`, embeds lyrics directly in the audio metadata tag.

#### `[tui]`
- `search_limit`: Maximum number of search results returned from YouTube (default: `10`).

#### `[network]`
- `proxy`: Custom proxy URL (e.g. `http://127.0.0.1:7890`) for API requests and downloads.
- `user_agent`: Custom User-Agent header string.

## Usage 🚀

### Running the Application

#### Option A: Running from Release Binary (Recommended)
1. Download the latest release tarball for your platform from the [Releases](https://github.com/Grizz96/youtidy/releases) page.
2. Extract the archive and navigate to the directory:
   ```bash
   tar -xzvf youtidy-v0.1.0-linux-x86_64.tar.gz
   cd youtidy-v0.1.0-linux-x86_64
   ```
3. Setup your configuration:
   - Copy `config.example.toml` to `config.toml` and fill in your AcoustID client API key and other desired settings.
   - Alternatively, copy `env.example` to `.env` and fill in your credentials.
4. Run the executable:
   ```bash
   ./youtidy
   ```

#### Option B: Building/Running from Source
1. Clone the repository:
   ```bash
   git clone https://github.com/Grizz96/youtidy.git
   cd youtidy
   ```
2. Setup your configuration files (`config.toml` or `.env`).
3. Run the application using Cargo:
   ```bash
   cargo run --release
   ```

---

### Step-by-Step Guide

#### 1. Main Menu Mode Selection
When you launch the app, you will be greeted by the mode selection menu:
- Use the **Up/Down Arrows** to select between **Search & Download Single Song** or **Download from Playlist**.
- Press **Enter** to open the selected mode.

#### 2. Search & Download Single Song
1. Type a query (song name, artist, etc.) directly into the search bar.
2. Press **Enter** to search. The search results will load asynchronously.
3. Use the **Up/Down Arrows** (or click with your mouse) to navigate the search results.
4. Press **Enter** to choose the desired song.
5. `youtidy` will now automatically download the audio, run AcoustID fingerprinting, query MusicBrainz for metadata/album art, fetch lyrics, and save the organized output file.
6. Press **Esc** to return to the search screen or **q** to quit.

#### 3. Download from Playlist (YouTube)
1. Paste or type the YouTube playlist URL into the input field (the URL must contain `list=`).
2. Press **Enter** to load the tracks.
3. An interactive list showing all the tracks will appear:
   - Use **Up/Down Arrows** to navigate the list.
   - Press **Space** to toggle (select/deselect) the highlighted track.
   - Press **A** to toggle select all / deselect all tracks in the playlist.
4. Press **D** or **Enter** to start downloading.
5. `youtidy` will download and process up to 3 tracks concurrently, displaying real-time download queues with status icons (`⏳ Pending`, `🔄 Processing`, `✅ Success`, `❌ Failed`) on the left panel, and detailed logs on the right.
6. Press **Esc** to return to the URL input field.

#### 4. Real-time Log Viewer & Scroll
During downloading and metadata processing, you can monitor output logs:
- By default, logs auto-scroll to the bottom.
- To scroll manually, use **Up/Down Arrows** to navigate line-by-line, or **PageUp/PageDown** to scroll by 10 lines.
- Scroll back to the bottom to resume auto-scroll.

### Keybindings:

#### General
* **q**: Quit the application.
* **Esc**: Go back to the previous screen (e.g., from search/logs/inputs back to menus).
* **Up / Down Arrow**: Navigate menus, search results, or playlist items.

#### Main Menu
* **Enter**: Confirm and select the highlighted mode.

#### Single Search Mode
* **Type text**: Enter keywords in the search bar.
* **Enter**: Trigger search or start processing the selected search result.
* **Mouse Click**: Navigate and select search results.

#### Playlist Mode
* **Type text**: Paste a YouTube playlist URL.
* **Enter**: Load the playlist.
* **Space**: Toggle selection of the highlighted track.
* **A**: Select or deselect all tracks in the playlist.
* **D / Enter**: Start batch downloading the selected tracks.

#### Log / Processing Viewer
* **Up / Down Arrow**: Scroll through logs line-by-line.
* **PageUp / PageDown**: Scroll through logs by page (10 lines).

## License 📄

This project was created for personal use to help manage local music collections.
