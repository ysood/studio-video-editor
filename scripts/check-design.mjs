// Fails when a visual value is hardcoded outside src/theme, so every color, size and
// font stays a theme decision. Add `design-allow` in a comment on a line to exempt it
// (reserve that for values that are not design, such as a 100% width).

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

const root = new URL("..", import.meta.url).pathname;
const src = join(root, "src");
const skip = [join(src, "theme"), join(src, "bindings")];

const rules = [
  { name: "hex color", pattern: /#[0-9a-fA-F]{3,8}\b/, in: ["css", "ts", "tsx"] },
  { name: "rgb/hsl color", pattern: /\b(?:rgba?|hsla?)\(/, in: ["css", "ts", "tsx"] },
  { name: "pixel literal", pattern: /(?<![\w-])\d+(?:\.\d+)?px\b/, in: ["css", "ts", "tsx"] },
  { name: "font literal", pattern: /(?<![\w-])font-(?:size|weight|family)\s*:(?!\s*(?:var\(|inherit))/, in: ["css"] },
  { name: "letter-spacing literal", pattern: /(?<![\w-])letter-spacing\s*:(?!\s*(?:var\(|inherit))/, in: ["css"] },
  { name: "line-height literal", pattern: /(?<![\w-])line-height\s*:(?!\s*(?:var\(|inherit))/, in: ["css"] },
  { name: "canvas font string", pattern: /\.font\s*=\s*[`"']/, in: ["ts", "tsx"] },
];

function walk(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (skip.some((s) => path === s || path.startsWith(s + sep))) return [];
    if (statSync(path).isDirectory()) return walk(path);
    return [path];
  });
}

const problems = [];
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
  });
}

if (problems.length > 0) {
  console.error("Hardcoded design values found. Move them into a theme token (src/theme):\n");
  for (const p of problems) console.error("  " + p);
  process.exit(1);
}
console.log("design values all come from the theme");
