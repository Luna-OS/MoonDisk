#!/usr/bin/env node
// MoonDisk icon rendering pipeline.
//
// Renders assets/branding/moondisk-icon.svg into the full PNG size set plus
// a multi-resolution .ico, and copies the sizes Tauri expects into
// src-tauri/icons/. See docs/icon-conversion.md for the exact steps this
// script automates and how to reproduce them manually if the pipeline ever
// needs to be redone by hand.
//
// Usage:
//   node scripts/render-icons.mjs                  # render everything
//   node scripts/render-icons.mjs --preview <in.svg> <out.png> [size] [bgHex]
//                                                    # ad-hoc SVG -> PNG,
//                                                    # used while designing
//                                                    # the branding SVGs
import { Resvg } from "@resvg/resvg-js";
import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

function renderPng(svgPath, size, bg) {
  const svg = readFileSync(svgPath, "utf8");
  const resvg = new Resvg(svg, {
    fitTo: { mode: "width", value: size },
    background: bg ? bg : "rgba(0,0,0,0)",
  });
  return resvg.render().asPng();
}

// --- Minimal, dependency-free ICO writer (Vista-style PNG-in-ICO, valid per
// the ICO spec for any size, supported by Windows since Vista and by every
// modern icon viewer). Avoids pulling in a native ICO-encoding dependency
// for what is otherwise a very small amount of binary framing. ---
function makeIco(pngBuffers) {
  const count = pngBuffers.length;
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(count, 4);

  const entries = [];
  const images = [];
  let offset = 6 + 16 * count;
  for (const { size, png } of pngBuffers) {
    const entry = Buffer.alloc(16);
    entry[0] = size >= 256 ? 0 : size;
    entry[1] = size >= 256 ? 0 : size;
    entry[2] = 0;
    entry[3] = 0;
    entry.writeUInt16LE(1, 4);
    entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(png.length, 8);
    entry.writeUInt32LE(offset, 12);
    offset += png.length;
    entries.push(entry);
    images.push(png);
  }
  return Buffer.concat([header, ...entries, ...images]);
}

function main() {
  const args = process.argv.slice(2);

  if (args[0] === "--preview") {
    const [, inPath, outPath, sizeArg, bgArg] = args;
    const size = Number(sizeArg) || 512;
    const bg = bgArg ? `#${bgArg.replace(/^#/, "")}` : undefined;
    const png = renderPng(inPath, size, bg);
    writeFileSync(outPath, png);
    console.log(`Preview: ${inPath} -> ${outPath} @ ${size}px (${png.length} bytes)`);
    return;
  }

  const iconSvg = path.join(root, "assets/branding/moondisk-icon.svg");
  const iconsDir = path.join(root, "assets/icons");
  const tauriIconsDir = path.join(root, "src-tauri/icons");
  const publicDir = path.join(root, "public");
  mkdirSync(iconsDir, { recursive: true });
  mkdirSync(tauriIconsDir, { recursive: true });

  const pngSizes = [16, 32, 48, 128, 256, 512];
  const rendered = new Map();
  for (const size of pngSizes) {
    const png = renderPng(iconSvg, size);
    rendered.set(size, png);
    writeFileSync(path.join(iconsDir, `icon-${size}.png`), png);
    console.log(`assets/icons/icon-${size}.png (${png.length} bytes)`);
  }

  // Windows .ico: a small multi-resolution set is standard practice (covers
  // taskbar, title bar, and Explorer's various view sizes).
  const icoSizes = [16, 32, 48, 256];
  const ico = makeIco(icoSizes.map((size) => ({ size, png: rendered.get(size) })));
  writeFileSync(path.join(iconsDir, "icon.ico"), ico);
  console.log(`assets/icons/icon.ico (${ico.length} bytes)`);

  // Tauri's own icons/ directory (referenced by src-tauri/tauri.conf.json's
  // bundle.icon list) uses its own naming convention, not assets/icons/'s.
  const tauriMap = {
    "32x32.png": 32,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "icon.png": 512,
  };
  for (const [name, size] of Object.entries(tauriMap)) {
    writeFileSync(path.join(tauriIconsDir, name), rendered.get(size));
    console.log(`src-tauri/icons/${name} (from ${size}px)`);
  }
  copyFileSync(path.join(iconsDir, "icon.ico"), path.join(tauriIconsDir, "icon.ico"));
  console.log("src-tauri/icons/icon.ico (copied)");

  // Browser favicon used by index.html.
  mkdirSync(publicDir, { recursive: true });
  writeFileSync(path.join(publicDir, "moondisk-icon.png"), rendered.get(128));
  console.log("public/moondisk-icon.png (from 128px)");
}

main();
