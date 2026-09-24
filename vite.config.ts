/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

// MoonDisk frontend build configuration.
//
// The dev-server settings below (fixed port, strictPort, watch ignores)
// follow the official Tauri + Vite integration guide: Tauri's webview
// expects a stable, predictable dev server and must not be disrupted by
// file changes happening inside src-tauri/ (which is rebuilt by cargo,
// not vite). See docs/architecture.md §6 for the IPC/capability boundary
// this frontend talks to.
const tauriDevHost = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  // Prevent Vite from obscuring Rust errors.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: tauriDevHost || false,
    hmr: tauriDevHost
      ? {
          protocol: "ws",
          host: tauriDevHost,
          port: 1421,
        }
      : undefined,
    watch: {
      // Tell Vite to ignore watching the Rust crate, which has its own
      // (much slower) rebuild cycle driven by cargo/tauri, not Vite.
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    // No fixed legacy `target` override: both webview runtimes MoonDisk
    // targets are evergreen (WebView2 on Windows, WebKitGTK 4.1+ on Linux),
    // so Vite's own modern default is a better fit than older Tauri
    // templates' "chrome105"/"safari13" pins.
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },

  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    css: true,
    reporters: ["default"],
    coverage: {
      provider: "v8",
      reporter: ["text", "html"],
      exclude: ["src/types/generated/**", "src-tauri/**"],
    },
  },
});
