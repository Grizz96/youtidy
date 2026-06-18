# youtidy 🎵

`youtidy` is a Rust-based TUI (Terminal User Interface) app that downloads audio from YouTube, automatically identifies tracks using acoustic fingerprinting (**AcoustID & MusicBrainz**), embeds album art, and organizes ID3v2 metadata.

## Features ✨

* 🔍 **Built-in YouTube Search**: Search for tracks directly from the terminal.
* 🚀 **Asynchronous Downloading**: Downloads audio via `yt-dlp` without blocking the TUI.
* 🧠 **Auto Track Identification**:
* Uses `fpcalc` to generate acoustic fingerprints.
* Queries the **AcoustID API** to fetch the MusicBrainz Recording ID.
* Queries the **MusicBrainz API** to match the best official release (Album/Single) based on heuristics.


* 🎨 **Album Art Retrieval & Embedding**:
* Fetches official cover art from the **Cover Art Archive** using MusicBrainz release data.
* Automatically falls back to the highest quality YouTube thumbnail (`maxresdefault` / `hqdefault`) if no official cover is found.
* Saves the cover image (`cover.jpg`/`cover.png`) directly into the album directory.
* Embeds the album art directly into the MP3 ID3v2 metadata.


* 🏷️ **Auto Tagging**: Writes ID3v2 tags (Title, Artist, Album) using the `lofty` crate.
* 📁 **Folder Organization**: Neatly saves downloaded files following a `Music/<Artist>/<Album>/<Title>.mp3` directory structure.

## Prerequisites 🛠️

Make sure you have the following system dependencies installed and available in your `$PATH`:

1. **yt-dlp**: For downloading audio from YouTube.
2. **ffmpeg**: Required by `yt-dlp` to convert the downloaded audio to MP3.
3. **fpcalc**: An AcoustID tool for calculating acoustic fingerprints.

### Installing Dependencies (Linux / Debian / Ubuntu)

```bash
sudo apt update
sudo apt install ffmpeg
# For yt-dlp (latest release recommended)
sudo wget https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -O /usr/local/bin/yt-dlp
sudo chmod a+rx /usr/local/bin/yt-dlp

# For fpcalc (Chromaprint)
sudo apt install libchromaprint-tools

```

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
