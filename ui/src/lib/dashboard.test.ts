import { describe, expect, it } from "vitest";

import {
  formatResetTimestamp,
  paceVerdict,
  projectedPeak,
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

describe("paceVerdict", () => {
  it("keeps a projected overage of up to 10% acceptable", () => {
    expect(paceVerdict(72, 110)).toBe("ok");
  });

  it("warns when the projected overage exceeds the acceptable 10% buffer", () => {
    expect(paceVerdict(72, 110.1)).toBe("warn");
  });

  it("reserves critical for a significantly over-pace projection or a capped limit", () => {
    expect(paceVerdict(72, 125)).toBe("crit");
    expect(paceVerdict(99, 99)).toBe("crit");
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
