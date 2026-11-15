"""Extracts the theme's design tokens from Raycast's built stylesheets.

    python css-tokens.py <frontend folder>

Every CSS custom property is a token the theme can set. This finds them all,
with the values they take (light and dark usually differ) and the selector
that sets them.
"""
import collections
import os
import re
import sys

folder = sys.argv[1]

DECLARATION = re.compile(r"(--[A-Za-z0-9_-]+)\s*:\s*([^;{}]+)")
SELECTOR = re.compile(r"([^{}]+)\{")

tokens = collections.defaultdict(lambda: collections.defaultdict(set))
files = 0
for name in sorted(os.listdir(folder)):
    if not name.endswith(".css"):
        continue
    path = os.path.join(folder, name)
    try:
        text = open(path, encoding="utf-8", errors="replace").read()
    except OSError:
        continue
    files += 1
    for match in DECLARATION.finditer(text):
        prop = match.group(1)
        value = " ".join(match.group(2).split())
        # Look backwards for the selector that owns this declaration.
        head = text[max(0, match.start() - 300):match.start()]
        selectors = SELECTOR.findall(head)
        owner = selectors[-1].strip() if selectors else "?"
        owner = " ".join(owner.split())
        if len(owner) > 90:
            owner = owner[-90:]
        tokens[prop][f"{owner} :: {value}"].add(name)

print(f"read {files} stylesheets, found {len(tokens)} custom properties\n")
for prop in sorted(tokens):
    entries = tokens[prop]
    print(f"{prop}")
    for entry in sorted(entries)[:6]:
        where = sorted(entries[entry])
        shown = ", ".join(w[:28] for w in where[:3])
        print(f"    {entry}")
        print(f"        in: {shown}")
    if len(entries) > 6:
        print(f"    ... and {len(entries) - 6} more forms")
    print()
