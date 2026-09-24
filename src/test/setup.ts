import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => {
  cleanup();
});

// The real Tauri IPC bridge (`window.__TAURI_INTERNALS__`) only exists
// inside a Tauri webview. Component tests run in jsdom, so any call to
// `@tauri-apps/api` must be mocked per-test; this just gives every test a
// clean, predictable starting point instead of an unhandled rejection.
vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn().mockResolvedValue("0.0.0-test"),
}));
