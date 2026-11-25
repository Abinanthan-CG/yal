"""Rasterize YAL's official Launch Ring vector icon (`assets/yal.svg`) into:
  - `assets/yal.png` (512x512 transparent PNG)
  - `assets/yal.ico` (multi-resolution 16..256px Windows icon)
  - `apps/yal/icons/icon.ico`

Run:  python scripts/make-icon.py
Requires: Microsoft Edge (or Chrome) + Pillow (`pip install pillow`).
"""

from __future__ import annotations

import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from PIL import Image


def find_browser() -> Path:
    candidates = [
        Path(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
        Path(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"),
        Path(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
    ]
    for path in candidates:
        if path.is_file():
            return path
    for name in ("msedge", "chrome", "chromium"):
        found = shutil.which(name)
        if found:
            return Path(found)
    raise RuntimeError("Microsoft Edge or Chrome is required to rasterize assets/yal.svg")


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    svg_path = root / "assets" / "yal.svg"
    if not svg_path.is_file():
        raise FileNotFoundError(f"missing vector source: {svg_path}")

    svg = svg_path.read_text(encoding="utf-8")
    browser = find_browser()

    with tempfile.TemporaryDirectory(prefix="yal-icon-") as tmp:
        tmp_dir = Path(tmp)
        html_path = tmp_dir / "render.html"
        raw_png = tmp_dir / "yal-1024.png"
        profile_dir = tmp_dir / "profile"
        html_path.write_text(
            f"""<!doctype html>
<html><head><meta charset="utf-8"><style>
html, body {{ margin: 0; padding: 0; width: 1024px; height: 1024px; background: transparent; overflow: hidden; }}
svg {{ width: 1024px; height: 1024px; display: block; }}
</style></head><body>{svg}</body></html>""",
            encoding="utf-8",
        )
        subprocess.run(
            [
                str(browser),
                "--headless=new",
                "--disable-gpu",
                "--default-background-color=00000000",
                "--force-device-scale-factor=1",
                "--window-size=1024,1024",
                f"--user-data-dir={profile_dir}",
                f"--screenshot={raw_png}",
                html_path.as_uri(),
            ],
            check=True,
        )

        raw = Image.open(raw_png).convert("RGBA")
        if raw.getpixel((0, 0))[3] != 0:
            raise RuntimeError(f"expected transparent corner pixel, got {raw.getpixel((0, 0))}")

        png_out = root / "assets" / "yal.png"
        raw.resize((512, 512), Image.Resampling.LANCZOS).save(png_out, format="PNG", optimize=True)
        print(f"wrote {png_out}")

        sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
        for ico_out in (root / "assets" / "yal.ico", root / "apps" / "yal" / "icons" / "icon.ico"):
            ico_out.parent.mkdir(parents=True, exist_ok=True)
            raw.save(ico_out, format="ICO", sizes=sizes)
            print(f"wrote {ico_out}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
