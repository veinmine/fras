#!/usr/bin/env python3
"""Placeholder item textures for FRAS 0.1 (original, GPL-3.0-or-later like the
rest of the mod). 16×16 pixel art drawn from tiny maps. Run from the
repository root: python3 tools/gen_textures.py"""
import os
import struct
import zlib

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
OUT = os.path.join(ROOT, "assets/fras/textures/item")


def png(path, pixels):
    h, w = len(pixels), len(pixels[0])
    raw = b"".join(b"\x00" + b"".join(struct.pack("BBBB", *p) for p in row) for row in pixels)

    def chunk(kind, data):
        c = kind + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)

    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
    data += chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(data)


def draw(art, palette):
    rows = [r.ljust(16, ".")[:16] for r in art]
    rows += ["." * 16] * (16 - len(rows))
    return [[palette.get(c, (0, 0, 0, 0)) for c in row] for row in rows]


WRENCH = [
    "................",
    "..........##....",
    ".........#ww#...",
    "........#w..#...",
    "........#w.#....",
    ".......#ww#.....",
    "......#ww#......",
    ".....#ww#.......",
    "....#ww#........",
    "...#ww#.........",
    "..#ww#..........",
    ".#w#w#..........",
    ".#w.w#..........",
    "..#..#..........",
]
HAMMER = [
    "................",
    ".......####.....",
    "......#wwww#....",
    ".....#wwwwww#...",
    "......#wwwww#...",
    ".......#bwww#...",
    "......#b##ww#...",
    ".....#b#..##....",
    "....#b#.........",
    "...#b#..........",
    "..#b#...........",
    ".#b#............",
    ".##.............",
]
BELT = [
    "................",
    "................",
    "....########....",
    "...#llllllll#...",
    "..#llddddddll#..",
    "..#ld######dl#..",
    "..#ld#....#dl#..",
    "..#ld#....#dl#..",
    "..#ld######dl#..",
    "..#llddddddll#..",
    "...#llllllll#...",
    "....########....",
]
CRUSHED = [
    "................",
    "................",
    "....h...........",
    "...hm#....h.....",
    "....#....hm#....",
    "..........#.....",
    ".....hm.........",
    "....hmm#...h....",
    ".....##...hm#...",
    "..h........#....",
    ".hm#....hm......",
    "..#....hmm#.....",
    "........##......",
]
PLATE = [
    "................",
    "................",
    "................",
    "...##########...",
    "..#hhhhhhhhhh#..",
    "..#hmmmmmmmmm#..",
    "..#hmmmmmmmmm#..",
    "..#hmmmmmmmmm#..",
    "..#hmmmmmmmmm#..",
    "..#hmmmmmmmmm#..",
    "..#hmmmmmmmmd#..",
    "...##########...",
]

DARK = (40, 40, 44, 255)
METALS = {
    "iron": ((236, 236, 236, 255), (190, 190, 196, 255), (120, 120, 128, 255)),
    "copper": ((240, 160, 120, 255), (200, 110, 70, 255), (130, 70, 45, 255)),
    "gold": ((255, 240, 120, 255), (230, 190, 50, 255), (160, 120, 20, 255)),
}


FLAKES = [
    "................",
    "................",
    "...r............",
    "..rdr.....o.....",
    "...r.....odr....",
    "..........r.....",
    "......ro........",
    ".....rddr...r...",
    "......rr...rdo..",
    "..o.........r...",
    ".rdr...or.......",
    "..r...rddr......",
    ".......rr.......",
]
PIGMENT = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "......rrr.......",
    "....rrlrrrr.....",
    "...rrlllrrrr....",
    "..rrrrlrrrrrd...",
    ".rrrrrrrrrrrdd..",
    ".dddddddddddd...",
]
PAINT = [
    "................",
    "......####......",
    ".....#hhhh#.....",
    "....########....",
    "...#wwwwwwww#...",
    "...#rrrrrrrr#...",
    "...#rlrrrrrr#...",
    "...#rlrrrrrr#...",
    "...#rrrrrrrr#...",
    "...#rrrrrrrd#...",
    "...#wwwwwwww#...",
    "...#wwwwwwww#...",
    "....########....",
]
RUST = ((150, 70, 30, 255), (120, 52, 24, 255), (180, 95, 45, 255))


def rust_block(stage):
    """Iron with rust: a grey base, rust spots by stage (deterministic)."""
    import random
    rng = random.Random(stage)
    share = {"spotty": 0.18, "rusty": 0.6, "leaking": 0.85}.get(stage, 0.0)
    rows = []
    for y in range(16):
        row = []
        for x in range(16):
            if stage == "painted":
                v = rng.randint(-8, 8)
                base = (140 + v, 48 + v // 2, 34 + v // 2, 255)
                if x in (0, 15) or y in (0, 15):
                    base = (110, 38, 28, 255)
                row.append(base)
                continue
            g = 200 + rng.randint(-14, 10)
            px = (g, g, g + 4, 255)
            if x in (0, 15) or y in (0, 15):
                px = (150, 150, 156, 255)
            if rng.random() < share:
                px = rng.choice(RUST)
            row.append(px)
        rows.append(row)
    if stage == "leaking":
        # A dark hole and a drip below it.
        for (x, y) in [(7, 6), (8, 6), (7, 7), (8, 7)]:
            rows[y][x] = (40, 22, 14, 255)
        for y in range(8, 13):
            rows[y][8] = (90, 40, 20, 255)
    return rows


def main():
    os.makedirs(OUT, exist_ok=True)
    block = os.path.join(ROOT, "assets/fras/textures/block")
    os.makedirs(block, exist_ok=True)
    for stage in ["spotty", "rusty", "leaking", "painted"]:
        png(os.path.join(block, f"rust_{stage}.png"), rust_block(stage))
    r, d, o = RUST
    png(os.path.join(OUT, "rust_flakes.png"), draw(FLAKES, {"r": r, "d": d, "o": o}))
    png(os.path.join(OUT, "red_oxide_pigment.png"), draw(PIGMENT, {"r": (160, 55, 35, 255), "l": (200, 85, 60, 255), "d": (110, 35, 25, 255)}))
    png(os.path.join(OUT, "red_oxide_paint.png"), draw(PAINT, {"#": DARK, "h": (120, 85, 50, 255), "w": (176, 176, 184, 255), "r": (150, 45, 32, 255), "l": (200, 85, 60, 255), "d": (100, 30, 22, 255)}))
    png(os.path.join(OUT, "wrench.png"), draw(WRENCH, {"#": DARK, "w": (176, 176, 184, 255)}))
    png(os.path.join(OUT, "hand_hammer.png"), draw(HAMMER, {"#": DARK, "w": (170, 170, 178, 255), "b": (120, 85, 50, 255)}))
    png(os.path.join(OUT, "leather_belt.png"), draw(BELT, {"#": (60, 35, 20, 255), "l": (150, 90, 50, 255), "d": (110, 65, 35, 255)}))
    for metal, (h, m, d) in METALS.items():
        png(os.path.join(OUT, f"crushed_{metal}.png"), draw(CRUSHED, {"h": h, "m": m, "#": d}))
        if metal != "gold":
            png(os.path.join(OUT, f"{metal}_plate.png"), draw(PLATE, {"h": h, "m": m, "d": d, "#": DARK}))


if __name__ == "__main__":
    main()
