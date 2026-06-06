# Obscura Research: A Lightweight Alternative to Headless Chrome

This document explores using [Obscura](https://github.com/h4ckf0r0day/obscura) (and other alternatives) as a replacement for `headless_chrome` in `claude-notify` to solve the process-bloat and macOS binary cloning issues.

## 1. What is Obscura?

Obscura is a Rust-native headless browser engine. Unlike `headless_chrome` (which controls the massive Google Chrome binary), Obscura is built on top of:
- **V8 Engine:** For real JavaScript execution.
- **Chrome DevTools Protocol (CDP):** For compatibility with tools like Puppeteer.
- **Lurking Logic:** Built-in stealth to avoid detection (Cloudflare, etc.).

### Pros
- **Zero Binary Clones:** It's a single Rust binary, not an app bundle. macOS won't create 1.3GB clones in `code_sign_clone`.
- **Lightweight:** No GPU process, no Audio service, no crashpad handlers. 
- **Native Rust:** Deeply integrates with the Rust ecosystem.

### Cons
- **Headless Only:** Obscura does *not* have a UI. You cannot "pop open" a window for manual login.

---

## 2. The "Not Headless" Problem

The current `claude-notify` flow relies on manual login via a visible Chrome window to capture cookies:
```rust
// src/browser_auth.rs
pub fn login(&self, _email: &str) -> anyhow::Result<(String, Vec<(String, String)>)> {
    let tab = self.browser.new_tab()?; // Opens a visible window
    tab.navigate_to("https://claude.ai/login")?;
    // ... wait for user to press Enter ...
}
```

Since Obscura is headless-only, it **cannot perform the initial login step** directly.

---

## 3. Possible Solutions

### Solution A: Hybrid Approach (Recommended)
Use Google Chrome *only* for the one-time `setup` command, and use Obscura for the background `monitor` service.
- **Setup:** User runs `claude-notify setup`. The app launches real Chrome, user logs in, app saves cookies to `sessions/*.json`.
- **Monitor:** The background daemon (Launchd/Systemd) uses Obscura. It injects the saved cookies into Obscura's V8 context and fetches the JSON.
- **Benefit:** Stops 100% of background process bloat and clone accumulation.

### Solution B: Cookie Import from Local Browser
Instead of launching a browser for login, the app could read cookies directly from the user's installed browser (Chrome/Arc/Edge).
- **How:** Use a crate like `browser-cookie-extractor`.
- **Benefit:** No browser launch needed at all.
- **Risk:** macOS "Sandboxing" and "Full Disk Access" make reading Chrome's SQLite cookie database increasingly difficult without a password/permission prompt.

### Solution C: Local Proxy / Auth Interceptor
Launch a tiny local web server during setup.
1. The user opens their *actual* browser to `localhost:9000`.
2. The page redirects them to Claude.ai.
3. After login, the user runs a Bookmarklet (JS snippet) that sends the cookies back to the local server.
- **Benefit:** Extremely "lightweight" and bypasses all headless detection.

### Solution D: Playwright-Rust
Switch from `headless_chrome` to the `playwright-rust` crate.
- Playwright's "Hermetic" browsers are downloaded into `~/Library/Caches` and are often better behaved than the system Google Chrome, though they still use the same process model.

---

## 4. Technical Feasibility of Obscura

If we moved the monitor to Obscura, the logic in `fetch_json` would look like this:

```rust
// Pseudocode for Obscura integration
let browser = Obscura::launch().await?;
let page = browser.new_page().await?;

// Inject cookies via CDP
page.set_cookies(saved_cookies).await?;

// Fetch JSON
let json_str = page.goto_and_wait("https://claude.ai/api/...").await?;
```

Obscura is still in early development compared to Chrome, but for **JSON-only API fetching**, it is highly capable and significantly more "stealthy."
