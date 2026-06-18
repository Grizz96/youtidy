import sys
import cutlet

def main():
    katsu = cutlet.Cutlet()
    katsu.use_foreign_spelling = False
    
    # Read lines from stdin, romanize, and print to stdout
    for line in sys.stdin:
        # Strip newline character, but keep original spacing
        text = line.rstrip('\r\n')
        if not text:
            print("")
            sys.stdout.flush()
            continue
            
        try:
            romaji = katsu.romaji(text).strip().capitalize()
            print(romaji)
        except Exception:
            print(text)
        sys.stdout.flush()

if __name__ == "__main__":
    main()
