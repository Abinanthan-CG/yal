"""Draw YAL's original icon.

The upstream icon is not clearly reusable, so this draws an original mark: a
rounded square in the port's blue with a lighter ring in the middle, which
reads as a search lens at 16 pixels.

Run:  python scripts/make-icon.py
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

BRAND = (0xE8, 0x8A, 0x3A)  # B, G, R - a warm blue.
INK = (0xFF, 0xFF, 0xFF)
SIZE = 32


def rounded_square(x: int, y: int, size: int, radius: int) -> bool:
    """Whether (x, y) is inside a rounded square of `size` with `radius` corners."""
    left, top = 0, 0
    right, bottom = size - 1, size - 1
    cx = min(max(x, left + radius), right - radius)
    cy = min(max(y, top + radius), bottom - radius)
    dx, dy = x - cx, y - cy
    return dx * dx + dy * dy <= radius * radius


def ring(x: int, y: int, size: int, centre: float, outer: float, inner: float) -> bool:
    dx, dy = x - centre, y - centre
    distance = (dx * dx + dy * dy) ** 0.5
    return inner <= distance <= outer


def build_pixels() -> list[list[tuple[int, int, int, int]]]:
    centre = (SIZE - 1) / 2
    pixels: list[list[tuple[int, int, int, int]]] = []
    for y in range(SIZE):
        row: list[tuple[int, int, int, int]] = []
        for x in range(SIZE):
            if not rounded_square(x, y, SIZE, 7):
                row.append((0, 0, 0, 0))
                continue
            if ring(x, y, SIZE, centre, 9.5, 6.5):
                row.append((*INK, 255))
            elif ring(x, y, SIZE, centre, 12.5, 11.0):
                # The handle of the lens, running down and to the right.
                row.append((*INK, 255))
            else:
                row.append((*BRAND, 255))
        pixels.append(row)
    # The lens handle.
    for offset in range(0, 7):
        x = int(centre + 8 + offset)
        y = int(centre + 8 + offset)
        for dy in range(3):
            for dx in range(3):
                px, py = x + dx, y + dy
                if 0 <= px < SIZE and 0 <= py < SIZE and rounded_square(px, py, SIZE, 7):
                    pixels[py][px] = (*INK, 255)
    return pixels


def write_ico(path: Path) -> None:
    pixels = build_pixels()
    # A 32-bit icon stores its rows bottom to top, in BGRA.
    raw = bytearray()
    for row in reversed(pixels):
        for blue, green, red, alpha in row:
            raw += bytes((blue, green, red, alpha))

    header = struct.pack("<IiiHHIIiiII", 40, SIZE, SIZE * 2, 1, 32, 0, len(raw), 0, 0, 0, 0)
    image = header + bytes(raw)
    mask = bytes(SIZE * 4)  # The AND mask, unused for a 32-bit icon.

    entry = struct.pack("<BBBBHHII", SIZE, SIZE, 0, 0, 1, 32, len(image), 22)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("wb") as handle:
        handle.write(struct.pack("<HHH", 0, 1, 1))
        handle.write(entry)
        handle.write(image)
        handle.write(mask)


def png_chunk(kind: bytes, payload: bytes) -> bytes:
    import struct
    import zlib

    return (
        struct.pack(">I", len(payload))
        + kind
        + payload
        + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
    )


def write_png(path: Path) -> None:
    import struct
    import zlib

    pixels = build_pixels()
    raw = bytearray()
    for row in pixels:
        raw.append(0)  # No filter for this row.
        for blue, green, red, alpha in row:
            raw += bytes((red, green, blue, alpha))

    header = struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0)
    data = (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", header)
        + png_chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + png_chunk(b"IEND", b"")
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("wb") as handle:
        handle.write(data)


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    for target in [
        root / "apps" / "yal" / "icons" / "icon.ico",
        root / "assets" / "yal.ico",
    ]:
        write_ico(target)
        print(f"wrote {target}")
    png = root / "assets" / "yal.png"
    write_png(png)
    print(f"wrote {png}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
