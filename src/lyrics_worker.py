import os
import re
import sys

# Global variables for lazy loading to save RAM
cutlet_katsu = None
genius_api = None

# Genius Token
TOKEN = os.environ.get("GENIUS_ACCESS_TOKEN", "")


def contains_japanese(text):
    """Detect if text contains Hiragana, Katakana, or Kanji."""
    return bool(re.search(r"[\u3040-\u309F\u30A0-\u30FF\u4E00-\u9FFF]", text))


def get_genius_api():
    """Lazy-load lyricsgenius only when needed."""
    global genius_api
    if genius_api is None:
        try:
            import lyricsgenius

            genius_api = lyricsgenius.Genius(TOKEN, remove_section_headers=False)
            genius_api.verbose = False
        except ImportError:
            return None
    return genius_api


def get_genius_lyrics(artist, title):
    """Fetch lyrics from Genius."""
    genius = get_genius_api()
    if not genius:
        return None

    try:
        # Priority 1: Romanized version
        search_query = f"{title} (Romanized)"
        song = genius.search_song(search_query, artist)
        if not song:
            # Priority 2: Standard version
            song = genius.search_song(title, artist)
        return song.lyrics if song else None
    except:
        return None


def get_katsu():
    """Lazy-load cutlet (romanization engine) only when needed."""
    global cutlet_katsu
    if cutlet_katsu is None:
        try:
            import cutlet

            cutlet_katsu = cutlet.Cutlet()
            cutlet_katsu.use_foreign_spelling = False
        except ImportError:
            return None
    return cutlet_katsu


def romanize_text(text):
    """Convert Japanese text to Romaji."""
    katsu = get_katsu()
    if not katsu:
        return text
    try:
        return katsu.romaji(text).strip().capitalize()
    except:
        return text


class Aligner:
    def __init__(self, reference_text):
        self.reference_lines = []
        self.reference_norms = []
        if reference_text:
            for line in reference_text.splitlines():
                line = line.strip()
                # Skip headers and empty lines
                if line and not (line.startswith("[") and line.endswith("]")):
                    # Clean special unicode spaces
                    line = line.replace("\u2005", " ").replace("\u200b", "")
                    self.reference_lines.append(line)
                    self.reference_norms.append(self.normalize(line))

    def normalize(self, text):
        if not text:
            return ""
        # Romanize for comparison if it contains Japanese
        if contains_japanese(text):
            text = romanize_text(text)
        text = text.lower()
        text = re.sub(r"[^\w\s]", "", text)
        return " ".join(text.split())

    def find_best_match(self, lrc_norm, current_idx):
        if not lrc_norm:
            return None, current_idx
        import difflib

        best_match = None
        best_idx = -1
        max_score = -1

        # Look-ahead window (diperkecil jadi 8 agar tidak melompat sejauh 1 chorus)
        search_start = max(0, current_idx - 1)
        search_end = min(len(self.reference_lines), current_idx + 8)

        for i in range(search_start, search_end):
            g_line = self.reference_lines[i]
            g_norm = self.reference_norms[i]

            # --- KEMBALIKAN FITUR LOOK-AHEAD MERGE ---
            # Mencegah kalimat lirik terpotong jika Genius memecahnya jadi 2 baris
            if i + 1 < len(self.reference_lines):
                combined_line = f"{g_line} {self.reference_lines[i + 1]}"
                combined_norm = f"{g_norm} {self.reference_norms[i + 1]}"
                combined_score = difflib.SequenceMatcher(
                    None, lrc_norm, combined_norm
                ).ratio()

                if lrc_norm in combined_norm or combined_norm in lrc_norm:
                    len_ratio = min(len(lrc_norm), len(combined_norm)) / max(
                        len(lrc_norm), len(combined_norm)
                    )
                    combined_score = max(combined_score, 0.5 + (0.5 * len_ratio))

                if combined_score > 0.85:
                    dist_penalty = abs(i - current_idx) * 0.05
                    if (combined_score - dist_penalty) > max_score:
                        max_score = combined_score - dist_penalty
                        best_match = combined_line
                        best_idx = i + 1
                        continue

            # --- STANDARD SINGLE LINE MATCH ---
            score = difflib.SequenceMatcher(None, lrc_norm, g_norm).ratio()

            # Boost for substring matches
            if lrc_norm in g_norm or g_norm in lrc_norm:
                len_ratio = min(len(lrc_norm), len(g_norm)) / max(
                    len(lrc_norm), len(g_norm)
                )
                score = max(score, 0.5 + (0.5 * len_ratio))

            # PERBAIKAN: Penalti jarak dinaikkan dari 0.01 ke 0.05
            # Agar algoritma memprioritaskan baris terdekat, bukan lompat jauh ke kata yang kebetulan mirip
            dist_penalty = abs(i - current_idx) * 0.05
            effective_score = score - dist_penalty

            if effective_score > max_score:
                max_score = effective_score
                best_match = g_line
                best_idx = i

        if max_score > 0.55:
            return best_match, best_idx
        return None, current_idx


def adjust_timestamp(ts_str, offset):
    """Shift LRC timestamp by offset (seconds)."""
    match = re.match(r"\[(\d+):(\d+\.?\d*)\]", ts_str)
    if not match:
        return ts_str
    m, s = int(match.group(1)), float(match.group(2))
    total = m * 60 + s + offset
    if total < 0:
        total = 0.0
    return f"[{int(total // 60):02d}:{total % 60:05.2f}]"


def main():
    if len(sys.argv) < 2:
        print("Usage: lyrics_worker.py <mode> [offset] [artist] [title]")
        sys.exit(1)

    mode = sys.argv[1]  # 'fetch' or 'process'
    offset = float(sys.argv[2]) if len(sys.argv) > 2 else 0.0
    artist = sys.argv[3] if len(sys.argv) > 3 else ""
    title = sys.argv[4] if len(sys.argv) > 4 else ""

    # Mode 1: Fetch lyrics from Genius only (Fallback)
    if mode == "fetch":
        lyrics = get_genius_lyrics(artist, title)
        if lyrics:
            print(lyrics)
            sys.exit(0)
        sys.exit(1)

    # Mode 2: Process and Romanize LRC input from STDIN
    lrc_input = sys.stdin.read()
    if not lrc_input.strip():
        sys.exit(0)

    # Performance optimization: Check if we even need romanization
    needs_romaji = contains_japanese(lrc_input)

    reference_lyrics = None
    if needs_romaji:
        # Fetch reference from Genius for better Romaji accuracy
        reference_lyrics = get_genius_lyrics(artist, title)

    aligner = Aligner(reference_lyrics)
    current_ptr = 0

    for line in lrc_input.splitlines():
        match = re.match(r"^(\[[0-9:.]+\])(.*)", line)
        if match:
            timestamp, text_orig = match.groups()
            new_timestamp = adjust_timestamp(timestamp, offset)
            text = text_orig.strip()

            if not text:
                print(new_timestamp)
                continue

            # Normalized version for matching
            norm_lrc = aligner.normalize(text)
            best_match, found_idx = aligner.find_best_match(norm_lrc, current_ptr)

            if best_match:
                print(f"{new_timestamp} {best_match}")
                current_ptr = found_idx + 1
            else:
                # If no match, fallback to automated romanization
                if needs_romaji and contains_japanese(text):
                    print(f"{new_timestamp} {romanize_text(text)}")
                else:
                    print(f"{new_timestamp} {text}")
        else:
            # Metadata lines or other
            print(line)


if __name__ == "__main__":
    main()
