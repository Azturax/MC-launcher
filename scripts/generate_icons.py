"""Generate simple orange/gold Aureum PNG + ICO icons. No third-party deps."""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "src-tauri" / "icons"
PRIMARY = (255, 167, 38, 255)  # #FFA726
SECONDARY = (255, 213, 79, 255)  # #FFD54F
ON_PRIMARY = (66, 36, 0, 255)


def png(width: int, height: int, pixels: list[tuple[int, int, int, int]]) -> bytes:
    raw = bytearray()
    for y in range(height):
        raw.append(0)
        for x in range(width):
            raw.extend(pixels[y * width + x])

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            chunk(b"IHDR", ihdr),
            chunk(b"IDAT", zlib.compress(bytes(raw), 9)),
            chunk(b"IEND", b""),
        ]
    )


def paint(size: int) -> list[tuple[int, int, int, int]]:
    pixels = []
    cx = cy = (size - 1) / 2
    r_outer = size * 0.46
    r_inner = size * 0.30
    for y in range(size):
        for x in range(size):
            dx = x - cx
            dy = y - cy
            dist = (dx * dx + dy * dy) ** 0.5
            # rounded-square window
            nx = abs(dx) / cx
            ny = abs(dy) / cy
            rounded = (nx**4 + ny**4) ** 0.25
            if rounded > 1.02:
                pixels.append((0, 0, 0, 0))
            elif dist <= r_inner:
                pixels.append(SECONDARY)
            elif dist <= r_outer:
                pixels.append(ON_PRIMARY)
            else:
                pixels.append(PRIMARY)
    return pixels


def ico(pngs: list[tuple[int, bytes]]) -> bytes:
    count = len(pngs)
    header = struct.pack("<HHH", 0, 1, count)
    offset = 6 + 16 * count
    entries = b""
    payloads = b""
    for size, data in pngs:
        w = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", w, w, 0, 0, 1, 32, len(data), offset)
        payloads += data
        offset += len(data)
    return header + entries + payloads


def main() -> None:
    ROOT.mkdir(parents=True, exist_ok=True)
    sizes = (32, 128, 256)
    files: dict[int, bytes] = {}
    for size in sizes:
        data = png(size, size, paint(size))
        files[size] = data
        (ROOT / f"{size}x{size}.png").write_bytes(data)
    (ROOT / "icon.ico").write_bytes(ico([(32, files[32]), (128, files[128]), (256, files[256])]))
    print(f"Wrote icons to {ROOT}")


if __name__ == "__main__":
    main()
