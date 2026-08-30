import { describe, expect, it } from "vitest";

import { authCommand, normalizeProvider, providerLabel } from "./provider";

describe("normalizeProvider", () => {
  it("passes through the two known providers", () => {
    expect(normalizeProvider("claude-web")).toBe("claude-web");
    expect(normalizeProvider("openai-web")).toBe("openai-web");
  });

  it("treats a missing provider as Claude, matching Provider::default()", () => {
    expect(normalizeProvider(undefined)).toBe("claude-web");
    expect(normalizeProvider(null)).toBe("claude-web");
  });
});

describe("authCommand", () => {
  // The bug this guards: every account got `start_auth`, so a ChatGPT account's
  // "Login" button opened claude.ai/login.
  it("sends a ChatGPT account to the ChatGPT login", () => {
    expect(authCommand("openai-web")).toBe("start_chatgpt_auth");
  });

  it("sends a Claude account to the Claude login", () => {
    expect(authCommand("claude-web")).toBe("start_auth");
  });

  it("falls back to the Claude login for an unknown provider", () => {
    expect(authCommand(undefined)).toBe("start_auth");
  });

  it("never returns the same command for both providers", () => {
    expect(authCommand("openai-web")).not.toBe(authCommand("claude-web"));
  });
});

describe("providerLabel", () => {
  it("names the service the user is logging in to", () => {
    expect(providerLabel("openai-web")).toBe("ChatGPT");
    expect(providerLabel("claude-web")).toBe("Claude.ai");
    expect(providerLabel(undefined)).toBe("Claude.ai");
  });
});
