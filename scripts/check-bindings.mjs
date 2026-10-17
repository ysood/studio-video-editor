// Regenerates TypeScript bindings from the Rust types and fails if they differ from what
// is committed, so the UI can never drift from the core schema.
import { execFileSync } from "node:child_process";

const run = (cmd, args) => execFileSync(cmd, args, { stdio: "inherit" });

run("cargo", ["test", "--workspace", "--quiet", "export_bindings"]);
const status = execFileSync("git", ["status", "--porcelain", "--", "src/bindings"], {
  encoding: "utf8",
});
if (status.trim()) {
  console.error("\nsrc/bindings is out of date with the Rust types:\n" + status);
  console.error("Commit the regenerated files.");
  process.exit(1);
}
console.log("bindings are up to date");
