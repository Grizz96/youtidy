# ytupdate 🎵

`ytupdate` adalah aplikasi TUI (Terminal User Interface) berbasis Rust untuk mengunduh audio dari YouTube, mencocokkannya secara otomatis menggunakan sidik jari akustik (**AcoustID & MusicBrainz**), menyematkan gambar sampul album (album art), dan merapikan metadata ID3v2 secara otomatis.

## Fitur Utama ✨

- 🔍 **Pencarian YouTube Langsung**: Cari lagu langsung dari antarmuka terminal.
- 🚀 **Pengunduhan Asinkron**: Mengunduh audio menggunakan `yt-dlp` tanpa memblokir TUI.
- 🧠 **Identifikasi Lagu Otomatis**:
  - Menggunakan `fpcalc` untuk menghitung sidik jari audio (acoustic fingerprint).
  - Melakukan lookup ke **AcoustID API** untuk menemukan MusicBrainz Recording ID.
  - Melakukan query ke **MusicBrainz API** untuk mencocokkan rilis resmi terbaik (Album/Single) berdasarkan heuristik.
- 🎨 **Album Art Retrieval & Embedding**:
  - Mencari sampul album resmi di **Cover Art Archive** berdasarkan rilis MusicBrainz.
  - Menyediakan fallback otomatis ke thumbnail YouTube kualitas tertinggi (`maxresdefault` / `hqdefault`) jika sampul resmi tidak ditemukan.
  - Menyimpan gambar sampul (`cover.jpg`/`cover.png`) ke dalam folder album.
  - Menyematkan (embed) album art ke dalam tag metadata file MP3.
- 🏷️ **Tagging Otomatis**: Menulis tag ID3v2 (Title, Artist, Album) menggunakan library `lofty`.
- 📁 **Organisasi Folder**: Menyimpan hasil unduhan secara rapi dengan struktur `Music/<Artist>/<Album>/<Title>.mp3`.

## Prasyarat 🛠️

Pastikan perangkat Anda sudah terinstal beberapa dependency sistem berikut yang harus tersedia di dalam `$PATH`:

1. **yt-dlp**: Untuk mengunduh audio dari YouTube.
2. **ffmpeg**: Diperlukan oleh `yt-dlp` untuk konversi audio ke format MP3.
3. **fpcalc**: Alat dari AcoustID untuk menghitung sidik jari akustik lagu.

### Instalasi Dependency (Linux / Debian / Ubuntu)
```bash
sudo apt update
sudo apt install ffmpeg
# Untuk yt-dlp (disarankan versi terbaru)
sudo wget https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -O /usr/local/bin/yt-dlp
sudo chmod a+rx /usr/local/bin/yt-dlp

# Untuk fpcalc (Chromaprint)
sudo apt install libchromaprint-tools
```

## Konfigurasi ⚙️

Aplikasi memerlukan API Key dari **AcoustID**.
1. Dapatkan API Key gratis (Client ID) dengan masuk menggunakan akun MusicBrainz Anda di [AcoustID](https://acoustid.org/login).
2. Buat file `.env` di direktori utama proyek dan tambahkan Client ID Anda:

```env
ACOUSTID_CLIENT_ID=your_acoustid_client_id_here
```

## Cara Menjalankan 🚀

1. Pastikan file `.env` sudah terisi dengan benar.
2. Jalankan aplikasi menggunakan `cargo`:

```bash
cargo run
```

### Kontrol Antarmuka:
- **Ketik karakter**: Untuk mencari judul lagu di kolom pencarian.
- **Enter**: Untuk memulai pencarian ketika di mode input, atau memilih hasil pencarian untuk diproses.
- **Up / Down Arrow / Mouse Click**: Navigasi dan pilih hasil pencarian YouTube yang sesuai.
- **Esc**: Kembali ke pencarian dari halaman pemrosesan log.
- **q**: Keluar dari aplikasi.

## Lisensi 📄

Proyek ini dibuat untuk penggunaan personal dalam mengelola koleksi musik lokal.
