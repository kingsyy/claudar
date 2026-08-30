/** Which service an account monitors. Mirrors `claudar_core::config::Provider`,
 *  which serialises kebab-case. */
export type Provider = "claude-web" | "openai-web";

/** Absent on payloads from a backend older than the multi-provider change; those
 *  instances are all Claude, matching `Provider::default()` on the Rust side. */
export function normalizeProvider(provider: Provider | undefined | null): Provider {
  return provider === "openai-web" ? "openai-web" : "claude-web";
}

/** The service's own name, for buttons and headings the user reads. */
export function providerLabel(provider: Provider | undefined | null): string {
  return normalizeProvider(provider) === "openai-web" ? "ChatGPT" : "Claude.ai";
}

/** The Tauri command that opens this provider's login flow.
 *
 *  The two are not interchangeable: each drives a browser to its own login URL
 *  and stores a different credential shape. Picking by provider is what keeps a
 *  ChatGPT account off claude.ai/login.
 */
export function authCommand(provider: Provider | undefined | null): string {
  return normalizeProvider(provider) === "openai-web" ? "start_chatgpt_auth" : "start_auth";
}
