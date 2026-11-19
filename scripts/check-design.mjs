// Fails when a visual value is hardcoded outside src/theme, so every color, size and
// font stays a theme decision. Add `design-allow` in a comment on a line to exempt it
// (reserve that for values that are not design, such as a 100% width).

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const src = join(root, "src");
const skip = [join(src, "theme"), join(src, "bindings")];

// The Tailwind vocabulary. `src/theme/vocabulary.ts` lists the names that exist and
// `src/theme/tailwind.css` says what each one means; neither is allowed to drift from
// the other, because a name in one and not the other fails silently. A utility naming a
// token the theme does not define simply compiles to nothing, and a token tailwind-merge
// has not been told about gets merged away.
const vocabulary = readFileSync(join(src, "theme", "vocabulary.ts"), "utf8");
const tailwindCss = readFileSync(join(src, "theme", "tailwind.css"), "utf8");

/** The names in one exported list of `vocabulary.ts`. */
function declaredNames(constName) {
  const body = vocabulary.split(`export const ${constName} = [`)[1]?.split("] as const")[0] ?? "";
  return new Set([...body.matchAll(/"([^"]+)"/g)].map((m) => m[1]));
}

/** The names `tailwind.css` maps in one namespace, ignoring Tailwind's `--x--y` modifiers. */
function mappedNames(namespace) {
  const theme = tailwindCss.split("@theme inline {")[1]?.split("\n}")[0] ?? "";
  const re = new RegExp(`^\\s*--${namespace}-([a-z0-9-]+):`, "gm");
  return new Set([...theme.matchAll(re)].map((m) => m[1]).filter((n) => !n.includes("--")));
}

const NAMESPACES = [
  ["COLOR_TOKENS", "color"],
  ["TEXT_ROLES", "text"],
  ["SPACING_TOKENS", "spacing"],
  ["RADIUS_TOKENS", "radius"],
  ["FONT_TOKENS", "font"],
  ["SHADOW_TOKENS", "shadow"],
];

const vocabularyProblems = [];
for (const [constName, namespace] of NAMESPACES) {
  const declared = declaredNames(constName);
  const mapped = mappedNames(namespace);
  for (const name of declared) {
    if (!mapped.has(name)) {
      vocabularyProblems.push(
        `vocabulary.ts lists ${constName} "${name}", which tailwind.css does not map to a token`,
      );
    }
  }
  for (const name of mapped) {
    if (!declared.has(name)) {
      vocabularyProblems.push(
        `tailwind.css maps --${namespace}-${name}, which vocabulary.ts does not list in ${constName}`,
      );
    }
  }
}

// Spacing utilities resolve against that same list. One naming a step the theme does not
// define compiles to no class at all, so it fails silently in the browser rather than
// loudly here.
const SPACING_UTILITIES =
  "p|px|py|pt|pr|pb|pl|ps|pe|m|mx|my|mt|mr|mb|ml|ms|me|gap|gap-x|gap-y|space-x|space-y|w|h|size|min-w|min-h|max-w|max-h|inset|top|right|bottom|left|translate-x|translate-y|basis|indent|scroll-m|scroll-p";
const SPACING_SCALE = new Set([
  ...declaredNames("SPACING_TOKENS"),
  // Not spacing: Tailwind's own keywords, which take no theme value.
  "auto", "full", "px", "min", "max", "fit", "screen", "dvh", "dvw", "lvh", "lvw", "svh", "svw",
]);

const rules = [
  { name: "hex color", pattern: /#[0-9a-fA-F]{3,8}\b/, in: ["css", "ts", "tsx"] },
  { name: "rgb/hsl color", pattern: /\b(?:rgba?|hsla?)\(/, in: ["css", "ts", "tsx"] },
  { name: "pixel literal", pattern: /(?<![\w-])\d+(?:\.\d+)?px\b/, in: ["css", "ts", "tsx"] },
  { name: "font literal", pattern: /(?<![\w-])font-(?:size|weight|family)\s*:(?!\s*(?:var\(|inherit))/, in: ["css"] },
  { name: "letter-spacing literal", pattern: /(?<![\w-])letter-spacing\s*:(?!\s*(?:var\(|inherit))/, in: ["css"] },
  { name: "line-height literal", pattern: /(?<![\w-])line-height\s*:(?!\s*(?:var\(|inherit))/, in: ["css"] },
  { name: "canvas font string", pattern: /\.font\s*=\s*[`"']/, in: ["ts", "tsx"] },
  // A Tailwind arbitrary value is a hardcoded design value wearing a class name.
  // An arbitrary *variant* is not: `data-[state=open]:` selects, it does not style,
  // so a trailing colon exempts the match.
  {
    name: "Tailwind arbitrary value",
    pattern: /(?<![\w-])(?:[a-z][a-z0-9]*-)+\[[^\]]+\](?!:)/,
    in: ["ts", "tsx", "css"],
  },
];

/** Tailwind spacing utilities that ask for a step the theme does not define. */
function offScaleSpacing(line) {
  const found = [];
  const re = new RegExp(`(?<![\\w-])-?(${SPACING_UTILITIES})-([a-z0-9-]+)`, "g");
  for (const m of line.matchAll(re)) {
    if (!SPACING_SCALE.has(m[2])) found.push(m[0]);
  }
  return found;
}

function walk(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (skip.some((s) => path === s || path.startsWith(s + sep))) return [];
    if (statSync(path).isDirectory()) return walk(path);
    return [path];
  });
}

const problems = [...vocabularyProblems];
for (const file of walk(src)) {
  const ext = file.split(".").pop();
  if (!["css", "ts", "tsx"].includes(ext) || file.endsWith(".test.ts") || file.endsWith(".d.ts")) continue;
  const lines = readFileSync(file, "utf8").split("\n");
  lines.forEach((line, i) => {
    if (line.includes("design-allow")) return;
    // Ignore comment-only lines; prose may mention values.
    const trimmed = line.trim();
    if (trimmed.startsWith("//") || trimmed.startsWith("*") || trimmed.startsWith("/*")) return;
    for (const rule of rules) {
      if (rule.in.includes(ext) && rule.pattern.test(line)) {
        problems.push(`${relative(root, file)}:${i + 1}  ${rule.name}: ${trimmed}`);
      }
    }
    if (ext === "tsx") {
      for (const match of line.matchAll(/className=(?:"([^"]*)"|\{`([^`]*)`\}|\{"([^"]*)"\})/g)) {
        const classes = match[1] ?? match[2] ?? match[3] ?? "";
        for (const bad of offScaleSpacing(classes)) {
          problems.push(
            `${relative(root, file)}:${i + 1}  off-scale spacing: ${bad} (theme.space has steps 1-12)`,
          );
        }
      }
    }
  });
}

if (problems.length > 0) {
  console.error("Hardcoded design values found. Move them into a theme token (src/theme):\n");
  for (const p of problems) console.error("  " + p);
  process.exit(1);
}
console.log("design values all come from the theme");
