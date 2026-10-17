---
name: refactoring-ui
description: Visual design principles for building and reviewing interfaces, adapted from Adam Wathan and Steve Schoger's book "Refactoring UI". Use this whenever you are writing or changing UI code, picking sizes, spacing, colors, font sizes, weights, borders, shadows or radii, adding a new panel or control, or reacting to feedback like "this looks off", "it doesn't feel polished", "something about this is ugly", "make it look better" or "the spacing is weird". Also use it before reviewing a screenshot of an interface, and when adding tokens to a design system. Reach for it even when the request sounds like a small tweak, because most "small tweaks" are really hierarchy or spacing problems and fixing the symptom makes the real problem worse.
---

# Refactoring UI

Design judgement that would otherwise take years of practice, compressed into rules you
can apply deliberately. Adapted from **Refactoring UI** by Adam Wathan and Steve Schoger
(refactoringui.com); the reasoning is theirs, the wording and the Studio-specific
guidance here are not from the book.

The reason to work from principles rather than taste is that "make it look nicer" is
paralysing, while "the secondary text is competing with the primary text" is a bug you
can fix. Almost everything that reads as ugly is one of a handful of named problems.

## Start here: it is almost always hierarchy

When an interface feels wrong but you cannot say why, the usual cause is that everything
is shouting at once. Nothing recedes, so nothing stands out.

The fix is rarely to make the important thing bigger. Font size is a blunt instrument:
leaning on it gives you oversized headings and unreadably small captions. Reach for
**weight and color first**, and change size last.

- Primary content: normal size, heavier weight (600-700), darkest text color.
- Secondary content: same or slightly smaller size, normal weight, a softer color.
- Tertiary content: softer color again. Resist going below about 12px.

Two weights and three text colors cover nearly every interface. Avoid weights under 400
for UI text; if something needs de-emphasising, make it lighter in color, not thinner.

**Emphasise by de-emphasising.** When the thing you want to stand out will not, stop
adding emphasis to it and start removing emphasis from what surrounds it. A nav item that
will not pop usually means the inactive items are too loud. A sidebar that competes with
the content usually needs its background removed, not the content strengthened.

**Semantics are secondary.** A heading element does not have to be large; a destructive
action does not have to be red and prominent. Choose the element for meaning, then style
it for its actual place in the hierarchy. Most section titles are closer to labels than
headings and should be small and quiet, because the content under them is the point.

**Actions sit in a pyramid.** One primary action with a solid high-contrast fill, a
couple of secondary actions outlined or low-contrast, and tertiary actions styled as
plain links. If a page appears to have three primary buttons, two of them are lying.

**Labels are a last resort.** `label: value` pairs flatten hierarchy by giving both equal
weight. Often the format already says what the data is, or you can fold the label into
the value ("12 left in stock" rather than "In stock: 12"). When you genuinely need a
label, treat it as supporting content and let the data dominate, unless users are
scanning for the label itself, as in a specifications table.

## Spacing is the cheapest improvement available

Start with more white space than feels right and remove it until you are happy. Adding
space only until something stops looking actively bad reliably lands you short of where
it should be.

Dense layouts are legitimate when a lot of information must be visible at once, which is
true of an editor's timeline and inspector. Make that a deliberate decision rather than
the default, because it is much easier to notice that you need less space than that you
need more.

**Use a non-linear scale.** "Multiples of 4" does not help you choose between 120 and
125. For a scale to actually make decisions easier, adjacent values should differ by
roughly 25% or more, so that two of any three candidates are obviously wrong. Small
values can sit close together; large ones should spread out.

**Avoid ambiguous spacing.** When a label sits as far from its own field as from the next
group, the eye cannot tell what belongs to what. Related things need noticeably less
space between them than unrelated things. This single rule fixes a lot of forms.

**You do not have to fill the screen**, and you do not have to make one element wide just
because its neighbour is.

## Type

Pick sizes from a small hand-built scale rather than computing a modular scale. Ratios
produce fractional pixel values and tend to leave gaps exactly where interfaces need
sizes. Hand-picked values avoid both problems.

- Use px or rem, never em, for the scale itself. Because em compounds with nesting, a
  nested element lands on a computed size that is not in your scale at all.
- **Line height is inversely proportional to font size.** Small text needs more leading
  so the eye can find the next line; large headings often need none at all (1.0-1.2).
  Wider columns also need more leading than narrow ones.
- Keep body text to roughly 45-75 characters per line.
- **Align mixed sizes on the baseline**, not the center. Your eye already uses the
  baseline as a reference, so centering two different sizes reads as misaligned.
- **Right-align numbers** in tables so the decimal points line up and values compare at a
  glance. Use tabular figures where the typeface offers them.
- Add letter-spacing to all-caps text. Capitals are uniform in height and lose the shape
  cues that make lowercase readable at default tracking.
- When nearly everything is a link, do not style links to "pop". Use a heavier weight or
  darker color, and save underlines and color for links inside prose.

## Color

Work in HSL. Hue, saturation and lightness map onto what your eye actually perceives, so
related colors look related in the code; hex does not have that property.

**You need far more colors than a five-swatch palette.** In practice: 8-10 greys, 5-10
shades of each primary, and a few accent colors with shades of their own for state.

**Define the shades up front** rather than generating them with lighten/darken at the
point of use, which is how you end up with thirty near-identical blues. Pick the base
first, then the darkest and lightest ends, then fill the middle by halving the gaps.

Two things that stop shades looking washed out:

- **Increase saturation as lightness moves away from 50%.** Saturation has less visible
  effect near the extremes, so a constant saturation value looks progressively greyer.
- **Rotate hue to change brightness.** Hues have different inherent brightness. Rotating
  toward yellow, cyan or magenta lightens; toward red, green or blue darkens. Keep
  rotations under about 20-30 degrees or it reads as a different color. This is how you
  get dark shades that stay rich instead of turning to mud.

**Greys do not have to be grey.** A little blue makes them cool, a little yellow or
orange makes them warm. Keep the temperature consistent across the ramp.

**Never put grey text on a colored background.** What works on white is reduced contrast,
not greyness. Hand-pick a color at the same hue as the background and adjust saturation
and lightness. Semi-transparent white looks dull, and over an image the background shows
through the glyphs.

For accessible color that is not ugly: when white-on-color needs an uncomfortably dark
background to reach 4.5:1, flip it and use dark colored text on a light colored tint
instead. And never let color be the only carrier of meaning, or colorblind users lose it
entirely. Pair it with an icon, a label or a contrast difference.

## Depth

Depth comes from consistently simulating a light source above the interface.

- Raised: a lighter top edge, and a small dark shadow below with a tight blur. Keep the
  blur small; real shadows cast by a nearby surface have fairly sharp edges.
- Inset: a dark shadow at the top and a lighter bottom edge, the inverse.
- Hand-pick the lighter edge color rather than overlaying translucent white, which drains
  saturation from whatever is underneath.

**Shadows express elevation**, so treat them as a scale of about five steps rather than a
decorative flourish. Small for buttons, medium for dropdowns, large for modals. The
higher something sits, the more it demands attention, so let the z-position follow the
importance. Changing elevation on interaction (a dragged item lifting) is a strong,
cheap affordance.

**Good shadows often have two parts**: a large soft one for the cast shadow from a direct
light, and a tighter darker one for the ambient occlusion right under the object. As
elevation increases, the tight shadow should fade, because an object further from a
surface has less of it.

Flat designs still convey depth. Lighter than the background reads as nearer, darker
reads as recessed. A short vertical offset with no blur gives depth without gloss.

**Overlapping elements across a boundary** creates layers cheaply and is often more
effective than any shadow.

## Finishing touches

- **Use fewer borders.** They accumulate into visual noise fast. Distinguish regions with
  a shadow, a slightly different background, or simply more space. If you have both a
  border and a background difference, try deleting the border.
- **Supercharge the defaults.** Icons in place of bullets, styled selection states,
  custom controls. Small upgrades to things already on screen.
- **Accent borders** (a colored bar along an edge) add visual interest without needing
  any illustration skill.
- **Do not neglect empty states.** They are the first thing a user sees. Lead with the
  call to action and hide supporting chrome, such as filters and tabs, that does nothing
  until content exists.
- **Question the obvious form of a component.** A dropdown is just a floating box; a
  table cell can hold two combined fields with internal hierarchy.

## Applying this in Studio

This repo already enforces the book's central argument, which is that systems beat
one-off decisions. Honour that:

- **Every visual value is a theme token.** Colors, sizes, spacing, radii, fonts, type
  roles, motion and timeline dimensions live in `src/theme/` only. `npm run check:design`
  fails on hardcoded values, so a literal in a component is a bug, not a shortcut.
- When a value you need does not exist, **add a token rather than a literal**, and place
  it on the existing scale instead of inventing a nearby one. Two tokens 4% apart defeat
  the purpose of having a scale.
- If two panels disagree about a dimension, the answer is usually one shared token, not
  two matching literals. Matching literals drift.
- Studio's Crimson theme is a deliberately saturated red frame around near-black content,
  with surface contexts for chrome, wells and stage. When adding a surface, decide which
  context it belongs to first; that choice carries the text and border colors with it.
- Dark UI inverts some intuitions: on dark surfaces, elevation usually reads better as a
  *lighter* surface than as a heavier shadow, because shadows disappear against black.
- See `docs/design-system.md` for the tokens and primitives, and the design gallery
  (linked in the status bar) for seeing them together.

## Reviewing an interface

When asked why something looks wrong, work down this list before touching anything. The
order matters, because a spacing fix applied to a hierarchy problem just moves the mess.

1. Can you tell what the most important thing on screen is? If not, it is hierarchy, and
   the fix is probably de-emphasising competitors rather than amplifying the subject.
2. Is anything emphasised by size that should be emphasised by weight or color instead?
3. Is related content grouped more tightly than unrelated content?
4. Are there values that are not on a scale, or two values so close together that nobody
   could tell them apart?
5. Count the borders. How many could become spacing or a background change?
6. Do things that sit at the same conceptual elevation share a shadow and a surface?
7. Is any meaning carried by color alone?
8. Are mixed font sizes on one line aligned to the baseline?

Then make the smallest change that addresses the highest item on the list, and look
again. Several small corrections in sequence beat one sweeping restyle, because you can
see which one actually helped.

## Source

Adam Wathan and Steve Schoger, *Refactoring UI* (refactoringui.com). This skill is a
condensed adaptation written for agents working in this repository. It omits the book's
chapter on working with images, which is about photography and is less relevant to
application chrome. Consult the book itself for the worked visual examples, which are
most of its value and cannot be reproduced in text.
