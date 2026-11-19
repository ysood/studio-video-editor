# Design system

Last verified: 2026-11-19

Every visual decision in Studio lives in one place, `src/theme/`. Components and panels
never hardcode a color, size, font or spacing value, and `npm run check:design` fails the
build if one slips in.

Studio is dark, and it ships one theme. A video editor is dark for a reason that is not
fashion: every judgement made in it is a judgement about a picture, and a bright interface
around a picture changes what you think the picture is doing. Premiere, Resolve and Avid
all arrive at the same answer.

## Layers

```
src/theme/
  types.ts          the Theme shape: every role a theme must define
  primitives.ts     raw values (the colour ramp, the hues, font stacks)
  themes/studio.ts  the theme
  themes/index.ts   the registry
  vocabulary.ts     which Tailwind utility names exist, as data
  tailwind.css      what each of those names means, mapped onto the tokens
  runtime.ts        applyTheme(): writes CSS variables, loads fonts, notifies the canvas
  base.css          document reset and base typography, in the `base` cascade layer
  layers.ts         the z-index scale
  fonts/            self-hosted font files (the app is offline; no font CDNs)

src/lib/cn.ts       shadcn's `cn`, taught Studio's vocabulary
src/ui/             the components, built on shadcn/ui and Radix, styled in tokens
src/panels/         compose those components; their CSS is layout only
src/timeline/draw.ts paints the canvas from the theme object
```

Tiers inside a theme:

1. **Primitives**: raw values such as `ash[850]`. Never referenced by components.
2. **Semantic roles**: what a value is for. `color.surface.raised`, `color.text.secondary`,
   `color.accent.base`, `color.state.hover`, `type.label`, `space[4]`, `radius.control`,
   `size.control`, `motion.quick`, `elevation.dialog`.
3. **Component tokens**: knobs specific to one part, kept in their own group. Today that
   is `timeline.*` (row and ruler height, header width, clip inset, band height, playhead
   and selection widths, ghost dash, alphas).

## Tailwind and shadcn/ui

Components are shadcn/ui's, which means they are Radix primitives with known-good
accessibility, keyboard handling and focus management, written in Tailwind. **If shadcn
has the component, we use shadcn's** and adapt it; we do not write our own. The place
that rule earns its keep is the small controls: a hand-rolled checkbox is where a focus
ring and an ARIA role quietly fail to exist.

shadcn components are not a dependency. `npx shadcn@latest add <name>` copies source into
`src/ui/`, where it becomes ours to edit, and two edits make it Studio's rather than
generic: its Tailwind classes become Studio's tokens, and its lucide icons become
`src/ui/Icon.tsx`.

Tailwind is wired up as **a second spelling of the tokens, not a second set of them.**
`src/theme/tailwind.css` maps every utility namespace onto the variables `applyTheme()`
already writes, using `@theme inline`, which emits the variable reference into the
utility and writes nothing to `:root`. So `bg-panel` compiles to
`background-color: var(--color-surface-panel)` and resolves wherever it lands. Two things
follow for free:

- Runtime theme changes reach every utility, because the utility holds a reference and
  not a value.
- `[data-surface]` keeps working. A well restates the colour variables, so the same
  `bg-panel` is the well's panel colour inside a well and the panel's everywhere else.

Tailwind's own 4px spacing multiplier is switched off in favour of the theme's 2px steps,
so a utility can only ask for a value the theme defines. Zero is defined explicitly
alongside them: it is not a design decision, but it is a value a utility has to be able to
ask for, and while `--spacing-0` was missing every `px-0`, `p-0`, `min-w-0` and `min-h-0`
in the app compiled to nothing and failed silently.

Preflight is deliberately not imported: `base.css` owns the document reset, and
Tailwind's would fight the panel stylesheets. `base.css` sits in the `base` cascade layer,
which matters more than it looks: **unlayered rules beat layered ones whatever their
specificity**, so an unlayered `button { color: inherit }` would outrank every `text-*`
utility in the app and no component could colour a button.

### The vocabulary, and why it is checked

`src/theme/vocabulary.ts` lists the names that exist; `tailwind.css` says what each means.
Two consumers need the list in a form CSS cannot give them:

- `src/lib/cn.ts` hands it to tailwind-merge. Out of the box tailwind-merge only knows
  Tailwind's stock scale, where `text-body` and `text-fg-muted` look like the same kind of
  class, so merging the two silently drops the colour.
- `scripts/check-design.mjs` uses the spacing names to tell a utility that resolves to a
  value from one that resolves to nothing.

Both failures are invisible: the class list looks right in the source and the style is
simply absent in the browser. So `check:design` asserts that `vocabulary.ts` and the
`@theme inline` block name exactly the same tokens, and a mismatch fails the build.

## Colour

The whole idea of the palette is that **colour is rationed**, and the chrome gets none of
it at all.

| | Values | What it is |
|---|---|---|
| `neutral` | 12 steps | shadcn/ui's stock dark surfaces, and the whole of the chrome |
| `accentWhite` | 3 | the one accent, which is white |
| `red` | 2 | the playhead, and danger |
| `mediaBlue`, `mediaGreen`, `mediaViolet` | 2 each | video, audio, stills |

**Neutral** is shadcn/ui's dark scaffold, which is Tailwind's `neutral` scale, and the
important thing about it is that its chroma is exactly zero. An earlier ramp here carried
about 8% of blue on the theory that a trace of hue reads as an instrument rather than a
utility. It does not. At these lightnesses a tinted grey reads as a grey that has gone
wrong, the whole window looks lifted toward blue, and on a tool whose job is judging
colour that is the one mistake worth never making.

The named steps map onto shadcn's semantic tokens, which is what lets a pasted component
land correctly: `950` is `background`, `900` is `card`, `800` is `secondary` and `muted`,
`400` is `muted-foreground`, `200` is `primary`, `50` is `foreground`.

**The accent is white**, as the primary is in any shadcn dark theme. It was amber, and
amber turned up on the Export button, the timecode, every toggle that was on and the zoom
slider, which is a brand hue sprinkled across the chrome of a tool whose whole job is
showing you someone else's colours.

Three things have a hue, and nothing else gets one:

1. **Blue, green and violet mean media kind**, on clips, bin swatches and tiles, never on
   chrome. The assignment follows Premiere and Resolve rather than taste: video is blue
   (Premiere's Iris), audio is green (Caribbean, and Resolve's audio green), stills are
   violet (Lavender). An editor arriving from either already knows what a green bar is.
   **These are the only saturated fills in the product**, which is the point: with the
   chrome at chroma zero they are the only thing the eye is pulled to, so they can afford
   to be vivid.
2. **Red means now.** The playhead is a role of its own rather than the accent, because
   with a neutral accent there would be no colour left saying "this is the current frame",
   and that is the one thing on a timeline that has to be found without being looked for.
   Red is what Resolve uses, and against blue, green and violet clips it is the one hue
   none of them occupy.
3. **Red also means danger**: offline media, destructive menu items, in a softer shade,
   because an alarm set in type wants less saturation than a one pixel line. The two never
   share a surface.

**Selection is white**, and that is not a collision with the accent. "This is now" is the
red one, so the only two whites in play are "this is chosen" and "this is the action",
which never appear on the same element.

Each media kind is a pair. `base` fills the clip; `band` is the same hue taken down hard
and carries the clip's name. The band **darkens** rather than lightening, which is what
makes a vivid fill affordable: white type at 11px sits at about 3:1 on a saturated fill
and clears 7:1 on the band. Premiere draws its clip names on a darker header for the same
reason.

**Borders are the text they separate, faded**, and they are alphas rather than greys:
shadcn's own white at 10% and 15%. One value is then correct on the panel, inside a well
and on the stage, and a hairline never has to be re-picked per surface.

## Depth

The workspace is flat and black. The window and every panel in it are one surface
(`neutral[950]`), and a panel is told from its neighbour by a hairline, which is how
Premiere and Resolve divide a workspace. Panels used to be grey cards on a black ground
with a gutter and a radius: that spent a surface step on saying "this is a panel", left
the window mostly grey, and rounded a corner against every edge of the window.

| Surface | What is on it |
|---|---|
| panel | the window, the bars, every panel body |
| raised (`neutral[900]`) | things that are actually lifted: a quiet button, an input, a track header, a menu |
| stage (`neutral[1000]`) | picture and time: the viewer's frame and the timeline canvas |

Grey means raised. If a region is grey and is not a control, an input or an overlay, it
is wrong.

## Panel anatomy

Every panel is the same three parts, and the numbers are the same in all of them:

| | Value |
|---|---|
| header and footer height | `size.toolbar`, 44px, the same as the top bar |
| inset at the sides of a bar, and of anything in the body | `space[6]`, 12px |
| rule between a bar and the body | `border.width` at `border.subtle` |
| control height inside a bar | `size.control`, 28px, leaving 8px above and below |
| corner | none: a panel meets its neighbours and the window square |

The header is `PanelToolbar`, or `Tabs fill`, or the dock's own title strip, and all three
come out as the same bar. Whatever a panel puts in its body starts at the 12px inset, so
the first thing in a bar and the first thing under it share a left edge in every panel.
That shared edge is most of what makes four panels read as one product.

Three spacing rules hold inside a panel, taken from how real product UI is built:

- **Words keep 12px from any edge**, and neighbouring values keep a fixed 8 to 12px gap.
  Never leftover `space-between`.
- **No near-tangents.** Two edges 1 to 3px apart read as an accident. Line them up
  exactly or part them by at least 8px. This is why a bar is 44 and not 40: a 34px
  button in a 40px bar sat 3px off its rule.
- **A table has one column definition**, shared by its heading row and its body rows,
  with fixed tracks. It keeps a minimum width and scrolls sideways under it rather than
  dropping a column or crushing one.

Stacking is `src/theme/layers.ts`, not a theme: `--z-raised`, `--z-sticky`, `--z-overlay`,
reached as `z-raised`, `z-sticky`, `z-overlay`. How things stack is a fact about the
interface rather than a look, and a theme that could reorder it would only be a way to
break it. Dialogs are deliberately absent from the scale: they are real `<dialog>`
elements, so the browser puts them and their backdrop in the top layer.

## Type

Geist and Geist Mono, bundled. The scale is four sizes and it is closed:

| Size | Role | Used for |
|---|---|---|
| 11 | `caption` | what annotates something else: column headings, hints, the status bar |
| 12 | `label` | the interface itself: every control, row, field, tab and timecode |
| 13 | `body`, `title` | prose meant to be read in sentences, and the project's name |
| 20 | `timecodeDisplay` | the one large readout |

Almost everything in an editor is 12, and `Panel` sets it. Hierarchy inside a panel comes
from colour (primary, secondary, tertiary) and the two weights (400, 500), never from a
size picked for the occasion: that is how a list ends up with a 13px name under a 12px
heading beside an 11px value. Mono is for timecode and number fields only; a value in a
table is sans with tabular figures.

## Surfaces

A theme colors regions differently. Every element marked `data-surface="<name>"` gets the
base colors merged with that surface's overrides from `theme.surfaces`:

Components keep using the same variables (`--color-surface-panel`, `--color-text-primary`)
and the same utilities (`bg-panel`, `text-fg`); the surface decides their values. Mark a
new region with `<Panel surface="...">`, `<Well>`, `<Section surface="well">` or
`data-surface` on any element; canvas code gets the resolved colors from
`surfaceColors(theme, "stage")` rather than from the DOM, which is why the timeline paints
stage colours whatever surface its panel is on.

Panel header strips follow the active panel: `PANEL_SURFACES` in
`src/workspace/PanelHeader.tsx` maps a panel id to a surface, because a header belongs to
the panel under it rather than to the frame around it. A panel listed in `HEADLESS` there
shows no strip at all, for a panel whose own toolbar already says what it is. Add a
panel's actions to the `makeHeaderActions` call in `Workspace.tsx`.

## Component rules

These keep every panel reading as one product. The gallery shows each rule.

1. **One control height.** Every button, input, select and segmented control is
   `size.control` tall. Only icon buttons inside dense rows (track headers, list items)
   use `size="small"`. `controlBase` in `src/ui/Button.tsx` holds the height, radius,
   focus ring, transition and disabled treatment every control shares; a new control
   spreads it rather than restating parts of it.
2. **Button hierarchy.** `solid` is the one primary action in a region, and it is white. `quiet` is the
   default action. `ghost` is for low-emphasis actions; square icon buttons are ghost.
   There is no fourth: a region that seems to need two solid buttons has two things
   claiming to be the main one, which is a layout problem rather than a variant problem.
   Toggles that are on show accent text on the accent wash. A control whose content is a
   block of layout rather than a label is a `PlainButton`.
3. **Disabled fades.** Disabled controls use `opacity.disabled` instead of a color, so
   they stay legible on any surface.
4. **Inputs.** `inputBase` in `src/ui/Field.tsx`: `surface.input` fill, `border.subtle`
   outline, `border.strong` on hover, the one focus ring. Select triggers are the same
   `inputBase`, because a thing that sits beside an input has to read as its equal.
5. **Toolbars.** `PanelToolbar` at the top or bottom of a panel: `size.toolbar` tall, the
   panel inset at the sides, `space.2` between controls, `Gap` between groups. Prefer icon
   buttons with tooltips over text buttons. A bar draws a hairline against the body and
   takes no margin: its height already clears its controls from both edges.
6. **Wells.** A panel body that holds wells uses `PanelBody layout="wells"`. Wells never
   set their own margins.
7. **Lists.** Rows go in a `List`, and a row that can be picked is a `Row`. `Row` owns the
   hover fill, the selected fill and the selection marker (`edge` for a line in a list,
   `outline` for a tile); the caller's own class owns the grid, the padding and the
   radius, because a bin folder, a media row and an export choice differ there and nowhere
   else. Never restate selection in a panel's stylesheet.
8. **Overlays.** Menus, select lists, popovers and tooltips share one anatomy, in
   `src/ui/overlayStyles.ts` rather than in four stylesheets that drift apart a line at a
   time: the overlay surface, `radius.dialog`, `elevation.popover`, items `size.control`
   tall, a highlight fill instead of a focus ring, shortcuts in tertiary text on the
   right. Overlay rows take a highlight rather than a ring because the keyboard and the
   pointer move the same highlight, and a ring on top would say the same thing twice.
9. **Icon buttons always explain themselves.** `IconButton` requires a label, shown as a
   tooltip with the keyboard shortcut when there is one.
10. **Interaction fills are translucent, and they layer.** What a thing *is* comes from
    `color.surface.*`. What is *happening to it* comes from `color.state.*`. The state
    fills are translucent so one value is right over a bare row and over a raised button
    alike, which is what stops the two from drifting into separate ladders of opaque
    greys. Use the `state-hover` and `state-selected` utilities, which apply the fill as a
    background image so the resting fill underneath survives; `bg-hover` would replace it
    and show the panel through. Selection also carries `color.selection` on a border or
    outline, never a different fill per panel.
11. **Two views of one panel is a tab strip**, not a control in a toolbar. `Tabs` with
    `fill` sits flush at the panel's top edge and gives the chosen view the rest of the
    height. A segmented control alone in the middle of an otherwise empty bar reads as a
    cutout in the panel rather than as a way to change what the panel is showing, which
    is what the properties panel used to do. A `Segmented` is for a choice that sits
    among other controls in a toolbar, and it ranges left with them.
12. **Menus mirror the toolbar.** A context menu or overflow menu offers the same action
    with the same label, icon and shortcut as its button, so `chord()` from
    `src/keyboard.ts` writes every shortcut. A right click selects what is under the
    pointer before the menu opens. Destructive items use `tone: "danger"` and sit last,
    after a separator; a disabled item says why in its label when that is not obvious.

## Naming

The Tailwind class name is the token path, so there is one thing to learn rather than two:

| Token | CSS variable | Utility |
|---|---|---|
| `color.surface.panel` | `--color-surface-panel` | `bg-panel` |
| `color.text.secondary` | `--color-text-secondary` | `text-fg-muted` |
| `color.border.subtle` | `--color-border-subtle` | `border-edge` |
| `color.media.videoBand` | `--color-media-video-band` | `bg-media-video-band` |
| `space[4]` | `--space-4` | `p-4`, `gap-4` |
| `size.control` | `--size-control` | `h-control`, `size-control` |
| `radius.button` | `--radius-button` | `rounded-button` |
| `type.label` | `--type-label` | `text-label` |

In plain CSS, set type with the shorthand and tracking: `font: var(--type-label);
letter-spacing: var(--type-label-tracking);`. In TypeScript, read the object with
`useTheme()`; for canvas text use `canvasFont(theme, "canvas")`.

## Common changes

| To change | Edit |
|---|---|
| The palette | `color` in `themes/studio.ts`, and `surfaces` for per-region colors |
| Fonts | `font.ui` / `font.display` / `font.numeric`, plus `font.faces` |
| Type scale or weights | `type` roles |
| Density | `size` (control and bar heights) and `timeline.rowHeight` / `rulerHeight` |
| Corner shape | `radius` |
| Icon weight | `border.iconStroke`; icon drawings are in `src/ui/Icon.tsx` |
| How a control looks everywhere | its file in `src/ui/` |
| Adding a Tailwind utility name | `vocabulary.ts` and `tailwind.css`, together |

## Adding a theme

Usually, do not. Studio ships one theme and the registry exists because the canvas
painters and the gallery read a theme as an object rather than as the document's current
variables. A look that needs a new theme is usually a change to this one.

If a second is genuinely wanted: copy `themes/studio.ts`, change `id`, `name` and values,
and add it to `themes` in `themes/index.ts`. The gallery's theme picker appears by itself
once there is more than one.

## Adding a font

Put the files in `src/theme/fonts/`, import them in the theme file so Vite bundles them,
and list them in `font.faces`. The runtime loads faces before notifying subscribers, so
the timeline canvas repaints in the right font.

## Design gallery

Open `#gallery` (link in the status bar) to see every color token, type role, spacing
step, radius, primitive, icon and the real timeline painter under the active theme. It
works in the desktop app and in the browser preview.

## Browser preview

Outside Tauri the UI installs a sample backend (`src/preview/`) with a fixed project and
bundled sample media, so `npm run dev` in any browser, or the hosted preview built with
`npm run build:preview`, shows the real interface. Editing, import and export are refused
with a clear message; selection, playback and layout work.
