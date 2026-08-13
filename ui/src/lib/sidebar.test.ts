import { describe, expect, it } from "vitest";

import { normalizeSidebarCollapsed } from "./sidebar";

describe("normalizeSidebarCollapsed", () => {
  it("only treats an explicit true string as collapsed", () => {
    expect(normalizeSidebarCollapsed("true")).toBe(true);
    expect(normalizeSidebarCollapsed("false")).toBe(false);
    expect(normalizeSidebarCollapsed(null)).toBe(false);
  });
});
