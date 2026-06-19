#!/usr/bin/env python3
"""Generate assets/icon.png — a 512x512 archive-box icon, no third-party deps."""
import struct, zlib, os

W = H = 512

def pixel(x, y):
    # Rounded-ish blue tile with a lighter centered square (a stylized box).
    border = 40
    inner = 150 <= x < 362 and 150 <= y < 362
    in_tile = border <= x < W - border and border <= y < H - border
    if not in_tile:
        return (0, 0, 0, 0)            # transparent margin
    if inner:
        return (235, 240, 252, 255)    # light face
    return (45, 108, 223, 255)         # blue body

raw = bytearray()
for y in range(H):
    raw.append(0)  # PNG filter type 0 for this scanline
    for x in range(W):
        raw += bytes(pixel(x, y))

def chunk(typ, data):
    body = typ + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

png = b"\x89PNG\r\n\x1a\n"
png += chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
png += chunk(b"IEND", b"")

os.makedirs("assets", exist_ok=True)
with open("assets/icon.png", "wb") as f:
    f.write(png)
print("wrote assets/icon.png", len(png), "bytes")
