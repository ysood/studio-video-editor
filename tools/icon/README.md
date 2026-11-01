# App icon

`make_icons.py` renders every icon file `src-tauri/tauri.conf.json` lists, from one
master, in one pass:

```sh
python3 tools/icon/make_icons.py                  # the shipping icon
python3 tools/icon/make_icons.py --variant crimson
```

It writes `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, a ten slot
`icon.icns` and a five entry `icon.ico` into `src-tauri/icons/`. What is committed
there is this script's output, so a run with no arguments should leave the tree clean.

## Running it

Needs Python 3 with Pillow, and `iconutil` for the `.icns`, so it is macOS only. The
repo-local toolchain in `.toolchain/` carries Rust and Node but no Python, so this is
deliberately outside the build and outside the check suite: a design tool to run by
hand when the mark or the plate changes, not a build step.

## The master

`monogram.png` is the mark as a grayscale alpha master, cropped to its bounding box.
Everything else is drawn around it.

It was keyed out of a chrome render of the monogram on a white field. That keying was
fiddly and is not reproducible here, because the render itself was not kept: its field
sat at 251-254 with a soft shadow trailing down to about 247, so a cut anywhere in that
band dragged the shadow in and silted up the counters until the letterform read as a
blob. The cut that worked went at 225, with the field flood-filled inward from the
corners so the chrome's own near-white highlights stayed solid instead of letting the
flood punch holes through the T's crossbar, then a median pass for contour speckle and
a 4px morphological close to heal the nicks the reflections cut into the edges.

The master in this directory was recovered from the shipped 1024px icon rather than
from that render, so it is about 3% smaller than the original and differs from it
slightly along the antialiased contour. If the original render turns up, re-keying it
with those numbers gives a cleaner master; nothing else in the pipeline changes.

## What the script decides

- **The squircle is part of the artwork.** macOS draws a plain PNG from an `.icns` as
  given and does not round the corners for you, so the plate is a superellipse drawn
  into every size.
- **The mark is refilled, not reused.** The original chrome's highlights assume a white
  ground and go muddy on a dark plate, so the silhouette is filled with a synthetic
  silver gradient whose glint sits at 57%.
- **Weight and inset are per size.** The serif tips and the counter between stem and
  bowl are what the resampler eats first, so small sizes thicken the mark and let the
  plate fill more of the box. See `SIZES`.
- **@2x slots use the point size's composition.** A 16pt icon on a Retina display is 32
  pixels of the 16pt drawing, not the 32pt drawing.

At 16px only the S and stem survive. That is as much as a two-letter monogram carries
at that size, and it is why the Finder sidebar will not show a legible one.

## Changing the plate

`PLATES` holds the fills. `ink` ships; `crimson` is the theme's signal colour, kept
because the choice is worth revisiting. Switching means re-running the script and
committing the six files it rewrites.
