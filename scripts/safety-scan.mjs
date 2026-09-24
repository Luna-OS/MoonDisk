#!/usr/bin/env node
// MoonDisk safety scan.
//
// MoonDisk performs real disk writes (an explicit, informed project
// decision — see the PR this shipped in and docs/architecture.md), so
// this script's job isn't "prove no real write code exists" anymore.
// Instead it enforces the boundary that makes those real writes
// reviewable: every process invocation that can touch a disk lives in one
// of a short, explicit list of files — the ones that were actually hand-
// verified against a throwaway loop device (see
// src-tauri/tests/linux_write_path.rs) — and nowhere else. A `Command::new`
// call turning up in, say, `commands/` or a random new module would mean
// something is bypassing the plan -> validate -> confirm -> execute
// pipeline, which is exactly the case this scan exists to catch.
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = process.cwd();
const rustSrcDir = join(root, "src-tauri", "src");

// Files allowed to spawn processes, relative to src-tauri/src/.
const ALLOWED_PROCESS_FILES = new Set([
  "platform/linux.rs",
  "platform/linux_executor.rs",
  "platform/mod.rs", // run_powershell_script, Windows-only
  "platform/windows.rs",
  "platform/windows_executor.rs",
  "security/system_protection.rs",
]);

function walk(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) walk(full, out);
    else if (entry.endsWith(".rs")) out.push(full);
  }
  return out;
}

let violations = [];

for (const file of walk(rustSrcDir)) {
  const relPath = relative(rustSrcDir, file).replace(/\\/g, "/");
  const text = readFileSync(file, "utf8");
  const usesProcessCommand = /\bCommand::new\s*\(/.test(text) || /process::Command\b/.test(text);
  if (usesProcessCommand && !ALLOWED_PROCESS_FILES.has(relPath)) {
    violations.push(`${relPath}: spawns a process outside the reviewed executor allowlist`);
  }
}

// The one hard "never" that must never regress regardless of mode: no code
// path may flip the write barrier's own reasoning inline (i.e. no ad-hoc
// re-implementation of system-disk detection outside security::system_protection,
// which would be an easy place to accidentally get subtly wrong twice).
for (const file of walk(rustSrcDir)) {
  const relPath = relative(rustSrcDir, file).replace(/\\/g, "/");
  if (relPath === "security/system_protection.rs") continue;
  const text = readFileSync(file, "utf8");
  if (/proc\/mounts/.test(text) || /findmnt/.test(text)) {
    violations.push(
      `${relPath}: re-implements system-disk detection outside security::system_protection`,
    );
  }
}

if (violations.length > 0) {
  console.error("Safety scan failed:\n" + violations.map((v) => `  - ${v}`).join("\n"));
  process.exit(1);
}

console.log(`Safety scan passed (${ALLOWED_PROCESS_FILES.size} files allowed to spawn processes).`);
