"""Lists the colour roles a theme can set, with their literal values.

    python css-colors.py <frontend folder>

Only declarations whose value is a literal colour are kept; the ones that
merely reference another variable are aliases and would only add noise.
"""
import collections
import os
import re
import sys

folder = sys.argv[1]

DECLARATION = re.compile(r"(--[A-Za-z0-9_-]+)\s*:\s*([^;{}]+)")
LITERAL = re.compile(
    r"^(#[0-9a-fA-F]{3,8}|rgba?\([^)]*\)|oklch\([^)]*\)|color-mix\([^)]*)\s*$"
)

roles = collections.defaultdict(set)
files = 0
for name in sorted(os.listdir(folder)):
    if not name.endswith(".css"):
        continue
    text = open(os.path.join(folder, name), encoding="utf-8", errors="replace").read()
    files += 1
    for match in DECLARATION.finditer(text):
        prop = match.group(1)
        value = " ".join(match.group(2).split())
        if LITERAL.match(value):
            roles[prop].add(value)

print(f"read {files} stylesheets\n")
print(f"{len(roles)} colour roles with literal values\n")
for prop in sorted(roles):
    values = sorted(roles[prop])
    print(f"{prop:44} {', '.join(values)}")
