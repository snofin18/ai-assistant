#!/usr/bin/env python3
"""Generate the deterministic Paint T3.2 source fixture.

The PNG is emitted with stored DEFLATE blocks so its bytes do not depend on
the local zlib compression level. CI can therefore compare a frozen SHA-256,
byte count, and IHDR dimensions without carrying a large binary in git.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import zlib
from pathlib import Path

WIDTH_PX = 2048
HEIGHT_PX = 1536
BORDER_PX = 4
DIAGONAL_HALF_WIDTH_PX = 1
WHITE = (255, 255, 255)
BLUE = (0, 162, 232)
BLACK = (0, 0, 0)

EXPECTED_PATH = Path(__file__).with_name("expected.json")


def _chunk(kind: bytes, payload: bytes) -> bytes:
    body = kind + payload
    return struct.pack(">I", len(payload)) + body + struct.pack(">I", zlib.crc32(body))


def _stored_zlib(payload: bytes) -> bytes:
    """Wrap payload in a deterministic zlib stream using stored deflate blocks."""
    output = bytearray(b"\x78\x01")
    for offset in range(0, len(payload), 65_535):
        block = payload[offset : offset + 65_535]
        final = offset + len(block) >= len(payload)
        output.append(0x01 if final else 0x00)
        output.extend(struct.pack("<H", len(block)))
        output.extend(struct.pack("<H", (~len(block)) & 0xFFFF))
        output.extend(block)
    output.extend(struct.pack(">I", zlib.adler32(payload) & 0xFFFFFFFF))
    return bytes(output)


def _pixel_is_blue(x_px: int, y_px: int) -> bool:
    return (
        x_px < BORDER_PX
        or y_px < BORDER_PX
        or x_px >= WIDTH_PX - BORDER_PX
        or y_px >= HEIGHT_PX - BORDER_PX
    )


def _pixel_is_black(x_px: int, y_px: int) -> bool:
    descending = abs(y_px - x_px) <= DIAGONAL_HALF_WIDTH_PX
    ascending = abs(y_px - (HEIGHT_PX - 1 - x_px)) <= DIAGONAL_HALF_WIDTH_PX
    return descending or ascending


def generate_png() -> bytes:
    raw = bytearray()
    for y_px in range(HEIGHT_PX):
        raw.append(0)  # PNG filter type 0 (None).
        for x_px in range(WIDTH_PX):
            if _pixel_is_blue(x_px, y_px):
                color = BLUE
            elif _pixel_is_black(x_px, y_px):
                color = BLACK
            else:
                color = WHITE
            raw.extend(color)
    ihdr = struct.pack(
        ">IIBBBBB",
        WIDTH_PX,
        HEIGHT_PX,
        8,
        2,
        0,
        0,
        0,
    )
    return (
        b"\x89PNG\r\n\x1a\n"
        + _chunk(b"IHDR", ihdr)
        + _chunk(b"IDAT", _stored_zlib(bytes(raw)))
        + _chunk(b"IEND", b"")
    )


def metadata_for(png: bytes) -> dict[str, int | str]:
    return {
        "sha256": hashlib.sha256(png).hexdigest(),
        "bytes": len(png),
        "width_px": WIDTH_PX,
        "height_px": HEIGHT_PX,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--print-expected", action="store_true")
    args = parser.parse_args()

    png = generate_png()
    metadata = metadata_for(png)

    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(png)
    if args.print_expected:
        print(json.dumps(metadata, indent=2, sort_keys=True))
    if args.check:
        expected_document = json.loads(EXPECTED_PATH.read_text(encoding="utf-8"))
        expected = expected_document.get("fixture", expected_document)
        for key, actual in metadata.items():
            if expected.get(key) != actual:
                raise SystemExit(
                    f"fixture mismatch: expected {key}={expected.get(key)!r}, "
                    f"actual={actual!r}"
                )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
