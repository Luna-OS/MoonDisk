import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import App from "./App";

describe("App", () => {
  it("renders the MoonDisk heading and slogan", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "MoonDisk" })).toBeInTheDocument();
    expect(screen.getByText("Deine Laufwerke. Sicher im Mondlicht.")).toBeInTheDocument();

    // Let the mocked getVersion() promise settle inside act() so React
    // doesn't warn about an update outside of it once the test unmounts.
    await screen.findByText("MoonDisk 0.0.0-test");
  });

  it("shows the resolved app version once the Tauri IPC bridge responds", async () => {
    render(<App />);

    expect(await screen.findByText("MoonDisk 0.0.0-test")).toBeInTheDocument();
  });
});
