import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => {
  cleanup();
});

// The real Tauri IPC bridge only exists inside a Tauri webview. Component
// tests run in jsdom, so `invoke` is mocked per-test via
// `vi.mocked(invoke)` — this just gives every test a clean, predictable
// starting point instead of an unhandled rejection when a component calls
// it during mount.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockRejectedValue(new Error("invoke() not mocked for this test")),
}));
