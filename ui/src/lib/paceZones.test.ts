import { describe, expect, it } from "vitest";

import {
  clearOverrides,
  colorblindZones,
  contrastRatio,
  defaultZones,
  derivedZoneHex,
  hexToHsl,
  hslToHex,
  matchZone,
  mergeZone,
  moveBoundary,
  setZoneColor,
  setZoneLabel,
  setZoneOverride,
  splitZoneAt,
  zoneStyle,
  type PaceZone,
} from "./paceZones.svelte";

describe("defaultZones / colorblindZones", () => {
  it("cover 0% to open-ended with four contiguous zones", () => {
    for (const zones of [defaultZones(), colorblindZones()]) {
      expect(zones.map((z) => z.upTo)).toEqual([100, 110, 125, null]);
      expect(new Set(zones.map((z) => z.id)).size).toBe(4);
    }
  });

  it("matches the default projection into the sweet-spot zone", () => {
    expect(matchZone(105, defaultZones()).label).toBe("Sweet spot");
  });
});

describe("splitZoneAt", () => {
  const zones = defaultZones(); // upTo: 100, 110, 125, null

  it("splits a zone into two, copying its colour into the new lower zone", () => {
    const next = splitZoneAt(zones, zones[0].id, 50);
    expect(next.map((z) => z.upTo)).toEqual([50, 100, 110, 125, null]);
    expect(next[0].hue).toBe(zones[0].hue);
    expect(next[0].saturation).toBe(zones[0].saturation);
  });

  it("splits the open-ended last zone", () => {
    const next = splitZoneAt(zones, zones[3].id, 150);
    expect(next.map((z) => z.upTo)).toEqual([100, 110, 125, 150, null]);
  });

  it("refuses a split outside the target zone's own range", () => {
    expect(splitZoneAt(zones, zones[0].id, 105)).toBe(zones);
    expect(splitZoneAt(zones, zones[1].id, 100)).toBe(zones);
  });

  it("refuses a split that would leave a sliver thinner than the minimum gap", () => {
    expect(splitZoneAt(zones, zones[0].id, 0)).toBe(zones);
    expect(splitZoneAt(zones, zones[0].id, 100)).toBe(zones);
  });

  it("refuses to grow past the zone cap", () => {
    let many = defaultZones();
    for (let i = 0; i < 10; i++) {
      many = splitZoneAt(many, many[0].id, (i + 1) * 2);
    }
    expect(many.length).toBeLessThanOrEqual(8);
  });
});

describe("mergeZone", () => {
  const zones = defaultZones(); // upTo: 100, 110, 125, null

  it("merges a middle zone into its predecessor", () => {
    const next = mergeZone(zones, zones[1].id);
    expect(next.map((z) => z.upTo)).toEqual([110, 125, null]);
    expect(next[0].label).toBe(zones[0].label);
  });

  it("drops the first zone rather than merging it backward", () => {
    const next = mergeZone(zones, zones[0].id);
    expect(next.map((z) => z.upTo)).toEqual([110, 125, null]);
    expect(next[0].label).toBe(zones[1].label);
  });

  it("refuses to remove the last remaining zone", () => {
    const one: PaceZone[] = [{ id: "only", upTo: null, hue: 0, saturation: 0, label: "Only" }];
    expect(mergeZone(one, "only")).toBe(one);
  });
});

describe("setZoneColor / setZoneLabel", () => {
  it("update only the targeted zone", () => {
    const zones = defaultZones();
    const recolored = setZoneColor(zones, zones[2].id, 55, 40);
    expect(recolored[2]).toMatchObject({ hue: 55, saturation: 40 });
    expect(recolored[0]).toEqual(zones[0]);

    const relabeled = setZoneLabel(zones, zones[2].id, "Danger zone");
    expect(relabeled[2].label).toBe("Danger zone");
  });
});

describe("moveBoundary", () => {
  const zones = defaultZones(); // upTo: 100, 110, 125, null

  it("moves the boundary between a zone and the one after it", () => {
    const next = moveBoundary(zones, zones[1].id, 115);
    expect(next.map((z) => z.upTo)).toEqual([100, 115, 125, null]);
  });

  it("is a no-op on the open-ended last zone", () => {
    expect(moveBoundary(zones, zones[3].id, 200)).toBe(zones);
  });

  it("refuses a value that would collapse a neighbouring zone", () => {
    expect(moveBoundary(zones, zones[1].id, 100)).toBe(zones); // too close to the previous bound
    expect(moveBoundary(zones, zones[1].id, 125)).toBe(zones); // too close to the next bound
  });
});

describe("hexToHsl / hslToHex", () => {
  it("recovers the hue and saturation of a fully saturated colour", () => {
    expect(hexToHsl("#ff0000")).toEqual({ hue: 0, saturation: 100 });
    expect(hexToHsl("#00ff00")).toEqual({ hue: 120, saturation: 100 });
    expect(hexToHsl("#0000ff")).toEqual({ hue: 240, saturation: 100 });
  });

  it("treats grey as hueless", () => {
    expect(hexToHsl("#808080")).toEqual({ hue: 0, saturation: 0 });
  });

  it("round-trips a hue back through the swatch preview", () => {
    const hex = hslToHex(175, 70, 45);
    expect(hexToHsl(hex).hue).toBeCloseTo(175, 0);
  });
});

describe("setZoneOverride / clearOverrides", () => {
  it("ignores legacy text overrides while retaining bar colors", () => {
    const zone = { ...defaultZones()[0], overrides: { bg: "#112233", border: "#445566", text: "#ffffff" } };
    expect(zoneStyle(zone)).toContain("--zone-bg-override: #112233");
    expect(zoneStyle(zone)).toContain("--zone-border-override: #445566");
    expect(zoneStyle(zone)).not.toContain("--zone-text-override");
  });
  it("sets one channel without disturbing others already set", () => {
    const zones = defaultZones();
    const withBg = setZoneOverride(zones, zones[0].id, "bg", "#112233");
    const withBorder = setZoneOverride(withBg, zones[0].id, "border", "#445566");
    expect(withBorder[0].overrides).toEqual({ bg: "#112233", border: "#445566" });
    expect(withBorder[1]).toEqual(zones[1]); // other zones untouched
  });

  it("clearOverrides drops the whole overrides bag", () => {
    const zones = defaultZones();
    const withOverride = setZoneOverride(zones, zones[0].id, "bg", "#ffffff");
    const cleared = clearOverrides(withOverride, zones[0].id);
    expect(cleared[0].overrides).toBeUndefined();
  });
});

describe("derivedZoneHex", () => {
  it("returns the hue's swatch at each channel's representative lightness", () => {
    const zone: PaceZone = { id: "a", upTo: 100, hue: 142, saturation: 70, label: "Under pace" };
    expect(derivedZoneHex(zone, "bg")).toBe(hslToHex(142, 70, 95));
    expect(derivedZoneHex(zone, "border")).toBe(hslToHex(142, 70, 65));
  });
});

describe("contrastRatio", () => {
  it("matches the known WCAG extremes", () => {
    expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 0);
    expect(contrastRatio("#808080", "#808080")).toBeCloseTo(1, 5);
  });

});
