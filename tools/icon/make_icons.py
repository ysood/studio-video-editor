#!/usr/bin/env python3
"""Render Studio's app icon set from the monogram master.

    python3 tools/icon/make_icons.py [--variant ink|crimson] [--dest DIR]

This is a design tool, run by hand when the mark or the plate changes. It is not part
of the build and not covered by the check suite: it needs Python 3 with Pillow, which
the repo-local toolchain in .toolchain/ does not carry. See README.md.
"""

import argparse
import os
import struct
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, ImageChops, ImageFilter

HERE = Path(__file__).resolve().parent
MASTER = HERE / "monogram.png"
DEST = HERE.parent.parent / "src-tauri" / "icons"

# Plate fills, top to bottom. Ink is the shipping icon; crimson is the signal colour
# from the theme, kept here because the choice is worth revisiting, not relitigating.
PLATES = {
    "ink": [(0.0, (0x1A, 0x1B, 0x1F)), (0.55, (0x0D, 0x0E, 0x10)), (1.0, (0x06, 0x06, 0x07))],
    "crimson": [(0.0, (0xFF, 0x2A, 0x3E)), (0.55, (0xD8, 0x02, 0x1C)), (1.0, (0x8E, 0x01, 0x0E))],
}

# The mark is refilled rather than reusing the original render's chrome, whose baked
# highlights assume a white ground and go muddy on a dark plate. The stop at 0.57 is
# the glint that keeps it reading as metal instead of flat grey.
SILVER = [
    (0.00, (0xFB, 0xFC, 0xFD)), (0.12, (0xDD, 0xE3, 0xEA)), (0.30, (0xA6, 0xAF, 0xBB)),
    (0.46, (0x7C, 0x85, 0x91)), (0.57, (0xF0, 0xF4, 0xF8)), (0.67, (0xC0, 0xC8, 0xD2)),
    (0.83, (0x81, 0x89, 0x95)), (1.00, (0xBD, 0xC4, 0xCE)),
]

SQUIRCLE_N = 5.0  # superellipse exponent approximating the macOS corner

# pixel size -> (thicken px, mark height as a fraction of the plate, plate side as a
# fraction of the icon box). Small sizes give up the Dock's breathing room and carry a
# heavier mark: the serif tips and the counter between stem and bowl go first.
SIZES = {
    1024: (0, 0.70, 0.82), 512: (0, 0.70, 0.82), 256: (1, 0.71, 0.82),
    128: (2, 0.72, 0.83), 64: (4, 0.74, 0.88), 32: (6, 0.78, 0.92),
    16: (10, 0.82, 0.97),
}


def vgrad(w, h, stops):
    """A vertical gradient through (position, rgb) stops."""
    col = Image.new("RGB", (1, h))
    px = col.load()
    for y in range(h):
        t = y / max(1, h - 1)
        for i in range(len(stops) - 1):
            t0, c0 = stops[i]
            t1, c1 = stops[i + 1]
            if t0 <= t <= t1:
                f = (t - t0) / (t1 - t0) if t1 > t0 else 0.0
                px[0, y] = tuple(round(c0[j] + (c1[j] - c0[j]) * f) for j in range(3))
                break
    return col.resize((w, h), Image.NEAREST)


def squircle(side, n=SQUIRCLE_N):
    """Superellipse mask, antialiased along each row."""
    m = Image.new("L", (side, side), 0)
    px = m.load()
    a = side / 2.0
    for y in range(side):
        v = 1.0 - abs((y + 0.5 - a) / a) ** n
        if v <= 0:
            continue
        nx = v ** (1.0 / n)
        x0, x1 = a - nx * a, a + nx * a
        for x in range(max(0, int(x0)), min(side, int(-(-x1 // 1)))):
            cov = min(x + 1.0, x1) - max(float(x), x0)
            if cov > 0:
                px[x, y] = round(255 * min(1.0, cov))
    return m


def grow(mask, n, kernel):
    for _ in range(n):
        mask = mask.filter(kernel)
    return mask


def render(master, px, layout, stops):
    """Draw the icon at `px` pixels using the composition chosen for size `layout`.

    A 16pt icon on a Retina display is 32 pixels of the 16pt drawing, not the 32pt
    one, so the plate and mark proportions come from the point size while the stroke
    weighting comes from the pixel count that has to carry it.
    """
    thicken = SIZES[px][0]
    _, mark_frac, plate_frac = SIZES[layout]

    ss = 4 if px <= 256 else 2  # supersample, then resolve down
    n = px * ss
    side = int(round(n * plate_frac))

    sq = squircle(side)
    plate = vgrad(side, side, stops).convert("RGBA")

    # One device pixel of light along the top edge, so a dark icon does not read as a
    # hole punched in the Dock.
    rim = ImageChops.subtract(sq, grow(sq, max(1, ss), ImageFilter.MinFilter(3)))
    rim_ramp = vgrad(side, side, [(0.0, (255, 255, 255)), (0.45, (150, 150, 150)), (1.0, (70, 70, 70))])
    plate = Image.alpha_composite(plate, Image.merge("RGBA", (
        *Image.new("RGB", (side, side), (255, 255, 255)).split(),
        ImageChops.multiply(rim, rim_ramp.convert("L")).point(lambda v: round(v * 0.42)),
    )))

    mark = grow(master, thicken, ImageFilter.MaxFilter(3))
    th = int(round(side * mark_frac))
    tw = max(1, int(round(th * mark.width / mark.height)))
    mark = mark.resize((tw, th), Image.LANCZOS)
    mx, my = (side - tw) // 2, (side - th) // 2

    shadow = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    shadow.paste(Image.new("RGBA", (tw, th), (0, 0, 0, 120)),
                 (mx, my + ss * 2), mark.filter(ImageFilter.GaussianBlur(ss * 1.6)))
    plate = Image.alpha_composite(plate, shadow)

    layer = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    layer.paste(Image.merge("RGBA", (*vgrad(tw, th, SILVER).split(), mark)), (mx, my))
    plate = Image.alpha_composite(plate, layer)

    # Clip to the plate so the mark's shadow cannot spill past the corner.
    plate.putalpha(ImageChops.multiply(plate.getchannel("A"), sq))
    canvas = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    off = (n - side) // 2
    canvas.paste(plate, (off, off), plate)
    return canvas.resize((px, px), Image.LANCZOS)


def write_ico(path, images):
    """Minimal ICO writer with PNG-compressed entries, which Windows has taken since Vista."""
    blobs = []
    for im in images:
        with tempfile.TemporaryDirectory() as tmp:
            f = os.path.join(tmp, "e.png")
            im.save(f)
            blobs.append((im.width, Path(f).read_bytes()))
    out = struct.pack("<HHH", 0, 1, len(blobs))
    offset = 6 + 16 * len(blobs)
    for w, b in blobs:
        d = 0 if w >= 256 else w  # 0 means 256 in an icon directory entry
        out += struct.pack("<BBBBHHII", d, d, 0, 0, 1, 32, len(b), offset)
        offset += len(b)
    Path(path).write_bytes(out + b"".join(b for _, b in blobs))


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--variant", choices=sorted(PLATES), default="ink")
    ap.add_argument("--master", type=Path, default=MASTER)
    ap.add_argument("--dest", type=Path, default=DEST)
    args = ap.parse_args()

    master = Image.open(args.master).convert("L")
    stops = PLATES[args.variant]
    args.dest.mkdir(parents=True, exist_ok=True)

    def draw(px, layout):
        return render(master, px, layout, stops)

    # The five paths tauri.conf.json lists, plus the 512 master Tauri keeps beside them.
    draw(32, 32).save(args.dest / "32x32.png")
    draw(128, 128).save(args.dest / "128x128.png")
    draw(256, 128).save(args.dest / "128x128@2x.png")
    draw(512, 512).save(args.dest / "icon.png")

    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / "icon.iconset"
        iconset.mkdir()
        for pt in (16, 32, 128, 256, 512):
            draw(pt, pt).save(iconset / f"icon_{pt}x{pt}.png")
            draw(pt * 2, pt).save(iconset / f"icon_{pt}x{pt}@2x.png")
        subprocess.run(["iconutil", "-c", "icns", str(iconset),
                        "-o", str(args.dest / "icon.icns")], check=True)

    write_ico(args.dest / "icon.ico", [draw(s, s) for s in (16, 32, 64, 128, 256)])
    print(f"wrote the {args.variant} icon set into {args.dest}")


if __name__ == "__main__":
    main()
