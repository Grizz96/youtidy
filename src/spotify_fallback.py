import sys
import json
import SpotipyFree

def search_spotify(query):
    try:
        sp = SpotipyFree.Spotify()
        results = sp.search(query, type="track")
        tracks = results.get("tracks", {}).get("items", [])
        if not tracks:
            return None
        track = tracks[0]
        
        # Get artist name(s)
        artists = [a["name"] for a in track.get("artists", [])]
        artist_name = ", ".join(artists) if artists else "Unknown Artist"
        
        album_name = track.get("album", {}).get("name", "Unknown Album")
        track_name = track.get("name", "Unknown Title")
        
        images = track.get("album", {}).get("images", [])
        cover_url = images[0]["url"] if images else None
        
        release_date = track.get("album", {}).get("release_date", "")
        
        return {
            "artist": artist_name,
            "album": album_name,
            "title": track_name,
            "cover_url": cover_url,
            "release_date": release_date
        }
    except Exception:
        return None

if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(1)
    
    query = sys.argv[1]
    metadata = search_spotify(query)
    if metadata:
        print(json.dumps(metadata))
    else:
        sys.exit(1)
