import { describe, expect, it } from "vitest";

import { visualClasses } from "./visuals.svelte";

describe("visualClasses", () => {
  it("adds the explicit accessibility overrides to the app root", () => {
    expect(
      visualClasses({
        reduceMotion: true,
        highContrast: true,
        statusPalette: "colorblind",
        textSize: "larger",
      }),
    ).toEqual(["reduce-motion", "high-contrast", "status-colorblind", "text-larger"]);
  });

  it("keeps the default presentation free of override classes", () => {
    expect(
      visualClasses({
        reduceMotion: false,
        highContrast: false,
        statusPalette: "standard",
        textSize: "default",
      }),
    ).toEqual([]);
  });

  it("emits a class for every non-default text size", () => {
    const classesFor = (textSize: "small" | "large" | "larger") =>
      visualClasses({
        reduceMotion: false,
        highContrast: false,
        statusPalette: "standard",
        textSize,
      });

    expect(classesFor("small")).toEqual(["text-small"]);
    expect(classesFor("large")).toEqual(["text-large"]);
    expect(classesFor("larger")).toEqual(["text-larger"]);
  });
});
