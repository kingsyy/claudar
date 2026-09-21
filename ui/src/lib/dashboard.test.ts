import { describe, expect, it } from "vitest";

import {
  formatResetTimestamp,
  hasShortWindow,
  matchZone,
  projectedPeak,
  type PaceZone,
  windowLabel,
  windowMs,
} from "./dashboard";

describe("projectedPeak", () => {
  it("projects a week once enough time has elapsed to avoid startup noise", () => {
    expect(projectedPeak(7, 7.47, null)).toBeCloseTo(93.71, 1);
  });

  it("does not extrapolate a genuinely new window", () => {
    expect(projectedPeak(1, 0.5, null)).toBe(1);
  });

  it("prefers a prediction supplied by the backend", () => {
    expect(projectedPeak(20, 50, 42)).toBe(42);
  });
});

describe("matchZone", () => {
  const zones: PaceZone[] = [
    { id: "a", upTo: 100, hue: 142, saturation: 70, label: "Under pace" },
    { id: "b", upTo: 110, hue: 175, saturation: 70, label: "Sweet spot" },
    { id: "c", upTo: 125, hue: 38, saturation: 90, label: "Over pace" },
    { id: "d", upTo: null, hue: 0, saturation: 80, label: "Critical" },
  ];

  it("finds the zone whose upper bound the projection falls under", () => {
    expect(matchZone(80, zones).id).toBe("a");
    expect(matchZone(105, zones).id).toBe("b");
    expect(matchZone(120, zones).id).toBe("c");
  });

  it("falls into the open-ended last zone above every bound", () => {
    expect(matchZone(200, zones).id).toBe("d");
  });

  it("treats a bound as exclusive: the value at a boundary belongs to the next zone", () => {
    expect(matchZone(100, zones).id).toBe("b");
    expect(matchZone(110, zones).id).toBe("c");
  });
});

describe("formatResetTimestamp", () => {
  it("formats an exact, copyable timestamp with timezone", () => {
    const formatted = formatResetTimestamp(
      "2026-08-12T22:00:00Z",
      "en-GB",
      "UTC",
    );

    expect(formatted).toContain("12 Aug 2026");
    expect(formatted).toContain("22:00:00");
    expect(formatted).toContain("UTC");
  });

  it("returns an em dash without a reset timestamp", () => {
    expect(formatResetTimestamp(null)).toBe("—");
  });
});

describe("windowLabel", () => {
  it("names the two windows both providers report", () => {
    expect(windowLabel(5 * 60 * 60 * 1000)).toBe("5-hour");
    expect(windowLabel(7 * 24 * 60 * 60 * 1000)).toBe("7-day");
  });

  // The point of labelling from the duration: a plan with a different period
  // gets named for what it is instead of borrowing Claude's wording.
  it("names an unfamiliar window after its own length", () => {
    expect(windowLabel(3 * 60 * 60 * 1000)).toBe("3-hour");
    expect(windowLabel(30 * 24 * 60 * 60 * 1000)).toBe("30-day");
  });
});

describe("windowMs", () => {
  it("uses the reported length", () => {
    expect(windowMs(18000, 999)).toBe(18_000_000);
  });

  it("falls back for payloads from a backend without the field", () => {
    expect(windowMs(undefined, 999)).toBe(999);
    expect(windowMs(null, 999)).toBe(999);
  });
});

describe("hasShortWindow", () => {
  it("always shows it for Claude, which reports both windows", () => {
    expect(hasShortWindow("claude-web", null)).toBe(true);
    expect(hasShortWindow(undefined, undefined)).toBe(true);
  });

  // The bug this guards: a ChatGPT account reporting only a weekly window would
  // otherwise render a 5-hour bar at 0%, which was never true.
  it("hides it for a ChatGPT account that reported no short window", () => {
    expect(hasShortWindow("openai-web", null)).toBe(false);
    expect(hasShortWindow("openai-web", 18000)).toBe(true);
  });
});
