# Design system

Last verified: 2026-10-17

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
                    TextInput, NumberInput, Select, Checkbox, Slider, Panel, PanelToolbar,
                    PanelBody, Spacer, Gap, EmptyState, Section, Note, Actions,
                    ProgressBar, Dialog, Timecode
src/panels/         compose primitives; their CSS is layout only
src/timeline/draw.ts paints the canvas from the theme object
```

Tiers inside a theme:

1. **Primitives**: raw values such as `graphite[850]`. Never referenced by components.
2. **Semantic roles**: what a value is for. `color.surface.raised`, `color.text.secondary`,
   `color.accent.base`, `type.label`, `space[4]`, `radius.control`, `size.control`,
   `motion.quick`, `elevation.dialog`.
3. **Component tokens**: knobs specific to one part, kept in their own group. Today that
   is `timeline.*` (row and ruler height, header width, clip inset, band height, playhead
   and selection widths, ghost dash, alphas).

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
| The palette | `color` in your theme file |
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
