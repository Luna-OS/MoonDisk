#!/usr/bin/env node
// Builds the `latest.json` manifest tauri-plugin-updater polls
// (see tauri.conf.json's `plugins.updater.endpoints`). Run by
// .github/workflows/release.yml's publish job, after the signed Windows
// (nsis) and Linux (AppImage) updater artifacts have been downloaded.
//
// Usage:
//   node generate-latest-json.mjs <version> <repo> <linuxDir> <windowsDir> <outFile>
//
// <version> may have a leading "v" (as in a git tag); it's stripped, since
// the updater's version comparison expects plain semver. <repo> is
// "owner/name", used to build the GitHub release download URLs.
import { readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [, , rawVersion, repo, linuxDir, windowsDir, outFile] = process.argv;
if (!rawVersion || !repo || !linuxDir || !windowsDir || !outFile) {
  console.error(
    "usage: generate-latest-json.mjs <version> <owner/repo> <linuxDir> <windowsDir> <outFile>",
  );
  process.exit(1);
}
const version = rawVersion.replace(/^v/, "");

function findFiles(dir, suffix) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      out.push(...findFiles(full, suffix));
    } else if (entry.endsWith(suffix)) {
      out.push(full);
    }
  }
  return out;
}

function platformEntry(dir, artifactSuffix) {
  const artifacts = findFiles(dir, artifactSuffix);
  if (artifacts.length !== 1) {
    throw new Error(
      `expected exactly one *${artifactSuffix} under ${dir}, found ${artifacts.length}`,
    );
  }
  const artifactPath = artifacts[0];
  const sigPath = `${artifactPath}.sig`;
  const signature = readFileSync(sigPath, "utf8").trim();
  const filename = artifactPath.split("/").pop();
  return {
    signature,
    url: `https://github.com/${repo}/releases/download/${rawVersion}/${filename}`,
  };
}

const manifest = {
  version,
  notes: "Siehe das GitHub-Release für Details.",
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": platformEntry(windowsDir, ".exe"),
    "linux-x86_64": platformEntry(linuxDir, ".AppImage"),
  },
};

writeFileSync(outFile, JSON.stringify(manifest, null, 2));
console.log(`wrote ${outFile}`);
