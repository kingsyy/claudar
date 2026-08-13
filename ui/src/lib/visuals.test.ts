import { describe, expect, it } from "vitest";

import { visualClasses } from "./visuals.svelte";

describe("visualClasses", () => {
  it("adds the explicit accessibility overrides to the app root", () => {
    expect(visualClasses({ reduceMotion: true, highContrast: true, statusPalette: "colorblind" }))
      .toEqual(["reduce-motion", "high-contrast", "status-colorblind"]);
  });

  it("keeps the default presentation free of override classes", () => {
    expect(visualClasses({ reduceMotion: false, highContrast: false, statusPalette: "standard" }))
      .toEqual([]);
  });
});
