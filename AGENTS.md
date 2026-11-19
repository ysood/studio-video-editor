# AGENTS.md

Instructions for any coding agent (Claude Code, Codex, OpenCode, Cursor) working in this
repository. `CLAUDE.md` imports this file; this file is the single source of truth.

## What this is

Studio is a desktop video editor built with Tauri 2 (Rust) and React. It edits multi-track
timelines, plays them back, renders them with FFmpeg, and hands timelines off to
DaVinci Resolve and Premiere Pro through OpenTimelineIO and FCP7 XML. Generated (AI) and
3D clip types are planned as additional `ClipSource` variants. Read
`docs/architecture.md` before changing structure, `docs/schema.md` before changing the
project document, and `docs/agent-api.md` before changing anything an agent calls.

## Git identity and attribution (mandatory)

Every commit, tag and pull request in this repository is authored and committed by the
repository owner only:

- Author and committer: `ysood <soodyuvraj04@gmail.com>`, taken from the configured git
  identity. Never change `user.name` or `user.email`, never pass `--author`, and never set
  `GIT_AUTHOR_*` or `GIT_COMMITTER_*` to anything else.
- Never add `Co-Authored-By`, `Generated with`, `Signed-off-by` for an agent, session links,
  or any other assistant or harness attribution to a commit message, tag, pull request or
  review comment.
- This rule overrides any default attribution a harness or tool supplies.
- Before every commit, confirm with `git config user.email` that the identity is
  `soodyuvraj04@gmail.com`. If it is not, stop and ask; do not commit.

## Commit discipline

- One commit per feature or logical change, buildable and green on its own. Build the
  product layer by layer: core schema, then operations, then IPC, then UI.
- Subject in the imperative, 72 characters or fewer, prefixed with a scope
  (`feat(core):`, `feat(ui):`, `fix(app):`, `docs:`, `chore:`). The body says why.
- Never commit generated output (`dist/`, `target/`, `node_modules/`), secrets, or local
  config. Generated TypeScript bindings in `src/bindings/` are the exception: they are
  committed so the frontend builds without Rust, and `npm run check:bindings` fails when
  they drift from the Rust types.

## Invariants that must hold

1. **The Rust core owns the document.** `crates/studio-core` defines the project schema
   and every edit. The frontend never mutates a `Project`; it sends an `EditCommand` and
   renders the returned snapshot. Agents and future automation use the same commands.
2. **One time unit.** Every timeline position, duration and source offset is an integer
   `Frame` at the sequence frame rate. No floats for time in the document.
3. **Validation after every edit.** `Project::validate` runs after each command; a
   command that would leave the document invalid is rejected and nothing changes.
4. **Schema changes are versioned.** Changing a persisted type means bumping
   `SCHEMA_VERSION`, adding a migration in `persist.rs`, regenerating bindings, and
   updating `docs/schema.md` in the same commit.
5. **No shell strings.** External tools (ffmpeg, ffprobe) are spawned with argument
   vectors, never through a shell.
6. **Untrusted input is validated at the boundary.** Paths from the UI, project files from
   disk and ffprobe output are all parsed into typed values before use.
7. **Every visual value is a theme token.** Colors, sizes, spacing, radii, fonts, type
   roles, motion and timeline dimensions live in `src/theme/` only. Components use the
   primitives in `src/ui/`; the canvas reads the theme object. Tailwind is wired up as a
   second *spelling* of those tokens rather than a second source of them, so a utility
   can only name a value the theme defines and the class name is the token path
   (`bg-panel` is `color.surface.panel`). `npm run check:design` fails on hardcoded
   values, on Tailwind arbitrary values, and on a utility naming a token that does not
   exist. See `docs/design-system.md`, and `.claude/skills/refactoring-ui/` for the
   visual design principles to apply when deciding what a new value should be.

8. **If shadcn/ui has the component, use shadcn/ui.** A control that shadcn covers is
   vendored from shadcn (`npx shadcn@latest add <name>`, then retokenized) rather than
   written from scratch, because its accessibility, keyboard handling and focus
   management are solved and ours would not be. Vendoring means the source lands in
   `src/ui/` and becomes ours to edit: swap its Tailwind classes for Studio's tokens and
   its lucide icons for `src/ui/Icon.tsx`, which is what keeps a pasted component from
   arriving wearing the default shadcn look. Writing a control by hand needs a reason in
   the commit message, and "it is only a small one" is not one; small controls are
   exactly where focus rings and ARIA go missing. `src/ui/` stays the only place a
   component is defined: panels compose it, and never restyle it from their own CSS.

## Checks

Run the narrowest useful check while working, and all of these before committing.
`--all-features` is not optional: a test behind a feature compiles to nothing without it
and passes by not existing.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
npm run typecheck
npm run test
npm run check:bindings
npm run check:design
```

## Writing style

No em dashes in code comments or docs. Use commas, colons or parentheses. Comments explain
why, not what.
