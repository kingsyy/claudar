import { describe, expect, it } from "vitest";

import { nextTabIndex } from "./tabs";

describe("nextTabIndex", () => {
  it("steps forward and back", () => {
    expect(nextTabIndex("ArrowRight", 2, 7)).toBe(3);
    expect(nextTabIndex("ArrowLeft", 2, 7)).toBe(1);
  });

  it("wraps around both ends", () => {
    expect(nextTabIndex("ArrowRight", 6, 7)).toBe(0);
    expect(nextTabIndex("ArrowLeft", 0, 7)).toBe(6);
  });

  it("jumps to the first and last tab", () => {
    expect(nextTabIndex("Home", 4, 7)).toBe(0);
    expect(nextTabIndex("End", 4, 7)).toBe(6);
  });

  it("ignores keys that aren't tab movement", () => {
    expect(nextTabIndex("Enter", 2, 7)).toBeNull();
    expect(nextTabIndex("a", 2, 7)).toBeNull();
  });

  it("returns null rather than dividing by zero on an empty tablist", () => {
    expect(nextTabIndex("ArrowRight", 0, 0)).toBeNull();
  });
});
