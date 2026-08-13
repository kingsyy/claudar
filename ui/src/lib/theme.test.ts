import { describe, expect, it } from "vitest";

import { normalizeThemePref, resolveThemePreference } from "./theme.svelte";

describe("normalizeThemePref", () => {
  it("keeps supported theme preferences", () => {
    expect(normalizeThemePref("system")).toBe("system");
    expect(normalizeThemePref("light")).toBe("light");
    expect(normalizeThemePref("dark")).toBe("dark");
    expect(normalizeThemePref("super-dark")).toBe("super-dark");
  });

  it("falls back to system for unknown stored values", () => {
    expect(normalizeThemePref("midnight")).toBe("system");
    expect(normalizeThemePref(null)).toBe("system");
  });
});

describe("resolveThemePreference", () => {
  it("resolves system from the OS preference", () => {
    expect(resolveThemePreference("system", false)).toBe("light");
    expect(resolveThemePreference("system", true)).toBe("dark");
  });

  it("keeps super dark separate from ordinary dark", () => {
    expect(resolveThemePreference("dark", false)).toBe("dark");
    expect(resolveThemePreference("super-dark", false)).toBe("super-dark");
    expect(resolveThemePreference("super-dark", true)).toBe("super-dark");
  });
});
