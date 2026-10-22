# Design system

Last verified: 2026-10-22

Every visual decision in Studio lives in one place, `src/theme/`. Components and panels
never hardcode a color, size, font or spacing value, and `npm run check:design` fails the
build if one slips in. A new look is a new theme file.

## Layers

```
src/theme/
  types.ts          the Theme shape: every role a theme must define
  primitives.ts     raw values (color ramps, font stacks); only themes import these
  themes/*.ts       a theme = one object of type Theme
  themes/index.ts   the registry of switchable themes and the default
  runtime.ts        applyTheme(): writes CSS variables, loads fonts, notifies the canvas
  base.css          document reset and base typography from tokens
  fonts/            self-hosted font files (the app is offline; no font CDNs)

src/ui/             primitives built only from tokens: Button, IconButton, Icon, Field,
                    PlainButton, TextInput, NumberInput, Select, Checkbox, RadioGroup,
                    Slider, List, Row, Panel,
                    Well, PanelToolbar, PanelBody, Spacer, Gap, EmptyState, Section, Note,
                    Actions, ProgressBar, Dialog, Timecode
src/panels/         compose primitives; their CSS is layout only
src/timeline/draw.ts paints the canvas from the theme object
```

Tiers inside a theme:

1. **Primitives**: raw values such as `graphite[850]`. Never referenced by components.
2. **Semantic roles**: what a value is for. `color.surface.raised`, `color.text.secondary`,
   `color.accent.base`, `color.state.hover`, `type.label`, `space[4]`, `radius.control`,
   `size.control`, `motion.quick`, `elevation.dialog`.
3. **Component tokens**: knobs specific to one part, kept in their own group. Today that
   is `timeline.*` (row and ruler height, header width, clip inset, band height, playhead
   and selection widths, ghost dash, alphas).

## Colour

Paper (light) is the default. It is three colours and where each one goes:

| | Where |
|---|---|
| `#e6e5e9` grey | the ground: the top bar, the status bar, the tab strips, the gaps between panels |
| `#2554ce` blue | the panel surface: the media bin, inspector and transform cards |
| `#181818` black | the stage: the timeline |

Text is `#ffffff` on the blue and on the black, and `#000000` on the grey, which is the
only surface where the ground is the backdrop for words. The blue is also the accent on
the grey (the Export button); on the blue card the accent is white, because blue on blue
says nothing, and on the stage it is the same blue lifted to `#5b8bf5` so it can hold a
playhead line against near black.

Ink (dark) is unfinished. Paper overrides `color`, `surfaces` and `elevation` and takes
everything else from Ink, so the pair cannot drift in type, density or timeline metrics.
Graphite and Crimson stay as alternates.

**The budget.** The whole palette is one neutral ramp plus five hues:

| | Values | What it is |
|---|---|---|
| `stone` | 18 steps | the warm neutral ramp, hue ~35 at 6-20% saturation. Both themes read it from opposite ends |
| `brass` | 5 | the one accent |
| `cobalt`, `verdigris`, `clay` | 2 each | media: video, audio, stills |
| `signal` | 2 | danger, one per scheme |

That is about 30 raw values behind 34 semantic roles, which is the normal shape: most of
an interface is neutral, and the hues are few enough to each mean one thing.

**Colour is rationed.** This is the whole idea, and it is what the references the palette
came from have in common more than any particular colour: the page is neutral and hue
appears only where it carries meaning. Three jobs, and nothing else gets a hue:

1. **Brass means time and the action that commits it.** The playhead, the timecode, the
   one primary button in a region. Nothing else.
2. **Cobalt, verdigris and clay mean media kind.** They appear on clips, bin swatches and
   tiles, never on chrome.
3. **Signal means danger.** Offline media, destructive menu items.

Selection is deliberately *not* the accent: it is plain `stone` at the far end of the
ramp, white on Ink and near-black on Paper. If selection were brass, "this is chosen" and
"this is now" would be the same colour on the same timeline.

**The viewer is not a card.** It sits on the ground, so the picture is on the grey and
only the frame itself is black, which is what letterboxing has to be. The blue is for
panels that hold controls.

## Depth

Four layers, and a thing belongs to exactly one:

| Layer | What is on it |
|---|---|
| ground | the top bar, the status bar, the tab strips, the gaps between cards, the viewer |
| card | a dock group: the panel surface or the stage |
| well | an inset inside a card: bin lists, inspector groups, overlays |
| raised | a control lifted on whatever it sits on |

A card is a card because you can see the ground around it: the gutter between two groups
is the ground showing through, not a line drawn over them. `.dv-groupview` gets that gap
as a transparent border, which costs no layout, and `radius.panel` gives it corners.

Stacking is `src/theme/layers.ts`, not a theme: `--z-raised`, `--z-sticky`,
`--z-overlay`. How things stack is a fact about the interface rather than a look, and a
theme that could reorder it would only be a way to break it. Dialogs are deliberately
absent from the scale: they are real `<dialog>` elements, so the browser puts them and
their backdrop in the top layer, above every z-index on the page.

## Surfaces

A theme can color regions differently. Every element marked `data-surface="<name>"` gets
the base colors merged with that surface's overrides from `theme.surfaces`:

| Surface | Where |
|---|---|
| `chrome` | top bar, status bar, panel tab strips, gaps between panels |
| `panel` | panel bodies (the default) |
| `well` | inset content: bin folders, list view, inspector groups, dialogs, tile captions |
| `stage` | picture and time: the viewer frame and the timeline |

Components keep using the same variables (`--color-surface-panel`,
`--color-text-primary`, `--color-accent-base`); the surface decides their values. Crimson
uses this for a red frame with dark text around near-black wells with a red accent;
Graphite defines no overrides. Mark a new region with `<Panel surface="...">`, `<Well>`,
`<Section surface="well">` or `data-surface` on any element; canvas code gets the
resolved colors from `surfaceColors(theme, "stage")`.

Panel header strips follow the active panel: `PANEL_SURFACES` in
`src/workspace/PanelHeader.tsx` maps a panel id to a surface (the timeline uses `stage`),
and everything else stays `chrome`. Each header has a title (the `heading` type role) and
an overflow menu built from that panel's entries plus the shared ones (maximize, reset
layout). Add a panel's actions to the `makeHeaderActions` call in `Workspace.tsx`.

## Component rules

These keep every panel reading as one product. The gallery shows each rule.

1. **One control height.** Every button, input, select and segmented control is
   `size.control` tall. Only icon buttons inside dense rows (track headers, list items)
   use `size="small"` (`size.controlSmall`).
2. **Button hierarchy.** `solid` is the one primary action in a region (Export, Import
   media in an empty bin, a dialog's confirm). `quiet` is the default action. `ghost`
   is for low-emphasis actions; square icon buttons are ghost. Toggles that are on show
   accent text on the accent wash, in both buttons and icon buttons. A control whose
   content is a block of layout rather than a label, such as a whole row that is one
   target, is a `PlainButton`: it strips the browser's chrome and adds nothing, so no
   screen has to write that reset again.
3. **Disabled fades.** Disabled controls use `opacity.disabled` instead of a color, so
   they stay legible on red, black or grey.
4. **Inputs.** `surface.input` fill, `border.subtle` outline, `border.strong` on hover,
   the accent focus ring on keyboard focus. Select triggers look like inputs.
5. **Toolbars.** `PanelToolbar` at the top or bottom of a panel: `size.toolbar` tall,
   `space.3` side padding, `space.2` between controls, `Gap` between groups. Prefer
   icon buttons with tooltips over text buttons in toolbars.
6. **Wells.** A panel body that holds wells uses `PanelBody layout="wells"`: `space.3`
   inset and `space.3` between wells. Wells never set their own margins.

7. **Lists.** Rows go in a `List`, and a row that can be picked is a `Row`. `Row` owns
   the hover fill, the selected fill and the selection marker (`edge` for a line in a
   list, `outline` for a tile); the caller's own class owns the grid, the padding and
   the radius, because a bin folder, a media row and an export choice differ there and
   nowhere else. Never restate selection in a panel's stylesheet.
8. **Overlays.** Menus, select lists, popovers and tooltips share one anatomy on the
   well surface: `radius.dialog`, `elevation.popover`, items `size.control` tall, a
   highlight fill instead of a focus ring, shortcuts in tertiary text on the right.
9. **Icon buttons always explain themselves.** `IconButton` requires a label, shown as
   a tooltip with the keyboard shortcut when there is one.
10. **Interaction fills are translucent.** What a thing *is* comes from
   `color.surface.*`: `raised` is the resting fill of anything lifted off the panel, a
   quiet button, a track header, a slider rail, the trough of a segmented control. What
   is *happening to it* comes from `color.state.*`: `hover`, and `selected` for the
   chosen row, tile, menu item or segment. The state fills are translucent so one value
   is right over a bare row and over a raised button alike, which is what stops the two
   from drifting into separate ladders of opaque greys. Where the element has no resting
   fill, set one as the background directly; where it has one, layer it as a
   `background-image` so the fill underneath survives. Selection also carries
   `color.selection` on a border or outline, never a different fill per panel.

11. **Menus mirror the toolbar.** A context menu or overflow menu offers the same action
   with the same label, icon and shortcut as its button, so `chord()` from
   `src/keyboard.ts` writes every shortcut. A right click selects what is under the
   pointer before the menu opens. Destructive items use `tone: "danger"` and sit last,
   after a separator; a disabled item says why in its label when that is not obvious.

## Naming

CSS variable names derive mechanically from the token path:

| Token | CSS variable |
|---|---|
| `color.text.secondary` | `--color-text-secondary` |
| `size.controlSmall` | `--size-control-small` |
| `space[4]` | `--space-4` |
| `radius.control` | `--radius-control` |
| `type.label` | `--type-label` (font shorthand), plus `--type-label-size`, `-weight`, `-line`, `-tracking`, `-family` |
| `font.numeric` | `--font-numeric` |
| `motion.quick` | `--motion-quick` |

In CSS, set type with the shorthand and tracking: `font: var(--type-label);
letter-spacing: var(--type-label-tracking);`. In TypeScript, read the object with
`useTheme()`; for canvas text use `canvasFont(theme, "canvas")`.

## Common changes

| To change | Edit |
|---|---|
| The palette | `color` in your theme file, and `surfaces` for per-region colors |
| Fonts | `font.ui` / `font.display` / `font.numeric`, plus `font.faces` for bundled files |
| Type scale or weights | `type` roles |
| Density | `size` (control and bar heights) and `timeline.rowHeight` / `rulerHeight` |
| Corner shape | `radius` |
| Icon weight | `border.iconStroke`; icon drawings are in `src/ui/Icon.tsx` |
| How a control looks in every place | its file in `src/ui/` |

## Adding a theme

1. Copy `src/theme/themes/graphite.ts` to a new file, change `id`, `name` and values. A
   variant can spread a base theme and override a few groups (see `graphiteCompact.ts`).
2. Add it to `themes` in `src/theme/themes/index.ts`.
3. Open the design gallery and pick it from the theme menu.

## Adding a font

Put the files in `src/theme/fonts/`, import them in the theme file so Vite bundles them,
and list them in `font.faces`:

```ts
import displayBold from "../fonts/Display-Bold.woff2";

font: {
  ui: '"Display", -apple-system, sans-serif',
  display: '"Display", -apple-system, sans-serif',
  numeric: '"Display", -apple-system, sans-serif',
  faces: [{ family: "Display", src: displayBold, weight: "700" }],
  numericVariant: "tabular-nums",
},
```

The runtime loads faces before notifying subscribers, so the timeline canvas repaints in
the right font.

## Design gallery

Open `#gallery` (link in the status bar) to see every color token, type role, spacing
step, radius, primitive, icon and the real timeline painter under the active theme, with
a theme switcher. It works in the desktop app and in the browser preview.

## Browser preview

Outside Tauri the UI installs a sample backend (`src/preview/`) with a fixed project and
bundled sample media, so `npm run dev` in any browser, or the hosted preview built with
`npm run build:preview`, shows the real interface. Editing, import and export are refused
with a clear message; selection, playback, layout and themes work.
