# ADR-0002: OTIO as the canonical export, FCP7 XML for Premiere, packages for handoff

Status: Accepted
Date: 2026-10-17
Deciders: yuvraj

## Context

Color and finishing stay in DaVinci Resolve and Premiere Pro. Neither app's native project
format (`.drp`, `.prproj`) can be written reliably by a third party. Timeline interchange
formats survive cuts, track structure and source ranges but not effects. Generated and 3D
clips are planned and have no meaning outside Studio. The Rust OpenTimelineIO bindings are
a placeholder crate plus Linux-only C bindings.

## Options

1. Write native project files
   + Opens directly  − Undocumented, version-specific, breaks silently
2. OTIO only
   + Open spec, read by Resolve  − Premiere support depends on plugins
3. OTIO canonical, FCP7 XML generated alongside, plus a conform package
   + Each app gets the format it reads best; the package travels between machines
   − Two writers to maintain

## Decision

Option 3. OTIO JSON is written directly with `serde_json` (no binding dependency) using
`Clip.2` and friends; FCP7 XML (`xmeml` v4) is written by a small writer. Both are produced
from the same validated project. The conform package copies only used media, makes names
unique, and records original paths and fingerprints in `manifest.json`. Studio ids ride in
OTIO metadata for a future importer.

Generated and 3D clips must bake to media files before export, so interchange stays
media-only and the receiving app never needs to understand them.

## Consequences

- Every export path is tested by reading the output with the reference OTIO Python library
  and its `fcp_xml` adapter during development.
- One-way handoff at picture lock is the supported workflow; round-trip import is not.
- Revisit if Premiere gains first-party OTIO import, which would let FCP7 XML retire.
