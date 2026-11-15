"""Peek into a large or minified file without dumping it.

    python peek.py <file> <regex> [window] [limit]

Prints a window of text around every match of the regex, with byte offsets.
Written for the Raycast bundles, which are single-line minified files that no
ordinary search tool can show anything useful about.
"""
import re
import sys

path = sys.argv[1]
pattern = sys.argv[2]
window = int(sys.argv[3]) if len(sys.argv) > 3 else 400
limit = int(sys.argv[4]) if len(sys.argv) > 4 else 20

data = open(path, "rb").read()
print(f"{path}: {len(data)} bytes")
found = 0
for match in re.finditer(pattern.encode(), data):
    start = max(0, match.start() - window // 3)
    end = min(len(data), match.end() + window)
    print(f"\n--- offset {match.start()} ---")
    print(data[start:end].decode("utf-8", "replace"))
    found += 1
    if found >= limit:
        print(f"\n(stopped at {limit} matches)")
        break
if found == 0:
    print("no match")
