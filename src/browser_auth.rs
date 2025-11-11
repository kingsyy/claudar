use dialoguer::Input;
use headless_chrome::protocol::cdp::Network;
use headless_chrome::{Browser, LaunchOptions};
use std::ffi::OsStr;
use std::time::Duration;

pub struct BrowserAuthenticator {
    browser: Browser,
}

impl BrowserAuthenticator {
    pub fn new() -> anyhow::Result<Self> {
        println!("  → Connecting to running Chrome instance...");

        // Fetch the WebSocket debugger URL from Chrome
        let ws_url = Self::get_websocket_debugger_url().map_err(|e| {
            anyhow::anyhow!(
                "Failed to connect to Chrome on port 9222.\n\n\
                    Please start Chrome with remote debugging enabled:\n\n\
                    macOS:\n\
                    /Applications/Google\\ Chrome.app/Contents/MacOS/Google\\ Chrome \\\n  \
                      --remote-debugging-port=9222 \\\n  \
                      --user-data-dir=/tmp/chrome-remote-profile \\\n  \
                      --no-first-run \\\n  \
                      --no-default-browser-check\n\n\
                    Linux:\n\
                    google-chrome \\\n  \
                      --remote-debugging-port=9222 \\\n  \
                      --user-data-dir=/tmp/chrome-remote-profile \\\n  \
                      --no-first-run \\\n  \
                      --no-default-browser-check\n\n\
                    Windows:\n\
                    \"C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe\" ^\n  \
                      --remote-debugging-port=9222 ^\n  \
                      --user-data-dir=%TEMP%\\chrome-remote-profile ^\n  \
                      --no-first-run ^\n  \
                      --no-default-browser-check\n\n\
                    Original error: {}",
                e
            )
        })?;

        println!("  → Found Chrome at {}", ws_url);

        // Connect to Chrome using the WebSocket URL
        let browser = Browser::connect(ws_url)?;

        Ok(Self { browser })
    }

    /// Create a new BrowserAuthenticator with a headless Chrome instance
    /// This launches Chrome programmatically without a visible window
    pub fn new_headless(verbose: bool) -> anyhow::Result<Self> {
        if verbose {
            println!("  → Launching headless Chrome...");
        }

        // Use more realistic browser settings to avoid Cloudflare detection
        let launch_options = LaunchOptions::default_builder()
            .headless(true)
            .window_size(Some((1920, 1080)))
            .args(vec![
                OsStr::new("--disable-blink-features=AutomationControlled"),
                OsStr::new("--user-agent=Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"),
                OsStr::new("--disable-web-security"),
                OsStr::new("--disable-features=IsolateOrigins,site-per-process"),
                OsStr::new("--lang=en-US,en"),
            ])
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build launch options: {}", e))?;

        let browser = Browser::new(launch_options)
            .map_err(|e| anyhow::anyhow!("Failed to launch headless Chrome: {}\n\nPlease ensure Chrome is installed on your system.", e))?;

        if verbose {
            println!("  ✓ Headless Chrome launched successfully");
        }

        Ok(Self { browser })
    }

    /// Inject cookies into the browser session
    /// This allows us to use stored session cookies without re-authenticating
    pub fn inject_cookies(&self, cookies: Vec<(String, String)>, verbose: bool) -> anyhow::Result<()> {
        if verbose {
            println!("  → Injecting {} stored cookies...", cookies.len());
        }

        // Get or create a tab
        let tab = self.browser.new_tab()?;

        // Navigate to claude.ai first (cookies can only be set for the current domain)
        tab.navigate_to("https://claude.ai")?;
        std::thread::sleep(Duration::from_millis(500));

        // Build cookie parameters for CDP
        let mut cookie_params = Vec::new();
        for (name, value) in cookies {
            cookie_params.push(Network::CookieParam {
                name,
                value,
                url: Some("https://claude.ai".to_string()),
                domain: Some(".claude.ai".to_string()),
                path: Some("/".to_string()),
                secure: Some(true),
                http_only: Some(false),
                same_site: None,
                expires: None,
                priority: None,
                same_party: None,
                source_scheme: None,
                source_port: None,
                partition_key: None,
            });
        }

        // Set all cookies
        tab.call_method(Network::SetCookies {
            cookies: cookie_params,
        })?;

        // Inject JavaScript to mask automation indicators
        let _ = tab.evaluate(
            r#"
            Object.defineProperty(navigator, 'webdriver', {
                get: () => undefined
            });
            Object.defineProperty(navigator, 'plugins', {
                get: () => [1, 2, 3, 4, 5]
            });
            Object.defineProperty(navigator, 'languages', {
                get: () => ['en-US', 'en']
            });
            window.chrome = {
                runtime: {}
            };
            "#,
            false,
        );

        if verbose {
            println!("  ✓ Cookies injected successfully");
        }

        Ok(())
    }

    /// Fetch usage data from the Claude.ai API
    /// Returns the parsed JSON response with 5-hour and 7-day usage data
    pub fn fetch_usage_api(&self, org_id: &str, verbose: bool) -> anyhow::Result<serde_json::Value> {
        let url = format!("https://claude.ai/api/organizations/{}/usage", org_id);
        if verbose {
            println!("  → Fetching usage data from API...");
        }

        self.fetch_json(&url, verbose)
    }

    /// Fetch the WebSocket debugger URL from Chrome's debugging endpoint
    fn get_websocket_debugger_url() -> anyhow::Result<String> {
        let client = reqwest::blocking::Client::new();
        let response = client
            .get("http://localhost:9222/json/version")
            .send()
            .map_err(|e| anyhow::anyhow!("Failed to connect to Chrome debugging port: {}", e))?;

        let json: serde_json::Value = response
            .json()
            .map_err(|e| anyhow::anyhow!("Failed to parse Chrome debugging response: {}", e))?;

        let ws_url = json["webSocketDebuggerUrl"]
            .as_str()
            .ok_or_else(|| {
                anyhow::anyhow!("Chrome debugging response missing webSocketDebuggerUrl")
            })?
            .to_string();

        Ok(ws_url)
    }

    /// Complete login flow by letting user manually log in, then extract cookies and org_id
    pub fn login(&self, _email: &str) -> anyhow::Result<(String, Vec<(String, String)>)> {
        let tab = self.browser.new_tab()?;

        // Navigate to login page
        println!("  → Opening browser window at claude.ai/login...");
        tab.navigate_to("https://claude.ai/login")?;
        std::thread::sleep(Duration::from_secs(2));

        // Show instructions to user
        println!("\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║                   MANUAL LOGIN REQUIRED                       ║");
        println!("╚═══════════════════════════════════════════════════════════════╝");
        println!();
        println!("  A browser window has been opened for you.");
        println!();
        println!("  Please complete the following steps:");
        println!("    1. Log in to your Claude.ai account in the browser");
        println!("    2. Complete any verification steps (email code, etc.)");
        println!("    3. Wait until you see the main Claude chat interface");
        println!();
        println!("  Once you're logged in and see the Claude interface,");
        println!("  come back here and press Enter to continue...");
        println!();

        // Wait for user confirmation
        Input::<String>::new()
            .with_prompt("Press Enter when you've successfully logged in")
            .allow_empty(true)
            .interact_text()?;

        println!("\n  → Checking login status...");

        // Check if user is logged in by checking the URL
        let current_url = tab.get_url();
        println!("  → Current URL: {}", current_url);

        if current_url.contains("/login") {
            anyhow::bail!("You appear to still be on the login page. Please complete the login process first.");
        }

        println!("  ✓ Login detected! Extracting session data...");

        // Extract organization ID by intercepting usage API call
        println!("  → Extracting organization ID from usage API...");
        let org_id = self.get_org_id_from_usage_api(&tab)?;
        println!("  ✓ Found organization ID: {}", org_id);

        // Extract all cookies
        let cookies = self.get_cookies(&tab)?;
        println!("  ✓ Extracted {} cookies", cookies.len());

        Ok((org_id, cookies))
    }

    /// Get organization ID by intercepting the usage API call
    fn get_org_id_from_usage_api(&self, tab: &headless_chrome::Tab) -> anyhow::Result<String> {
        // Navigate to usage page first
        println!("  → Navigating to usage page...");
        tab.navigate_to("https://claude.ai/settings/usage")?;
        std::thread::sleep(Duration::from_secs(2));

        // Inject script to intercept fetch requests and capture org ID
        println!("  → Setting up network request interceptor...");
        tab.evaluate(
            r#"
            (function() {
                window.__capturedOrgId = null;
                const originalFetch = window.fetch;

                window.fetch = function(...args) {
                    const url = args[0];
                    if (typeof url === 'string' && url.includes('/organizations/')) {
                        const match = url.match(/\/organizations\/([a-f0-9-]{36})/i);
                        if (match) {
                            window.__capturedOrgId = match[1];
                        }
                    }
                    return originalFetch.apply(this, args);
                };
            })();
            "#,
            false,
        )?;

        println!("  ✓ Request interceptor installed");

        // Try clicking the refresh button to trigger API calls
        println!("  → Triggering usage API call...");
        let click_result = tab.evaluate(
            r#"
            const button = document.querySelector('[aria-label="Refresh usage limits"]');
            if (button) {
                button.click();
                'clicked';
            } else {
                'not_found';
            }
            "#,
            false,
        );

        let button_clicked = click_result
            .ok()
            .and_then(|r| r.value)
            .and_then(|v| v.as_str().map(|s| s == "clicked"))
            .unwrap_or(false);

        if button_clicked {
            println!("  ✓ Clicked refresh button");
            std::thread::sleep(Duration::from_secs(2));
        } else {
            println!(
                "  ⚠ Refresh button not found, page load might have triggered API calls already"
            );
            std::thread::sleep(Duration::from_secs(1));
        }

        // Check if we captured the org ID
        let result = tab.evaluate("window.__capturedOrgId", false)?;
        if let Some(value) = result.value {
            if let Some(org_id_str) = value.as_str() {
                if !org_id_str.is_empty() && org_id_str != "null" {
                    return Ok(org_id_str.to_string());
                }
            }
        }

        // If fetch interception didn't work, try checking current page URL and other sources
        println!("  → Trying alternative extraction methods...");

        // Check if the current URL contains org ID
        let current_url = tab.get_url();
        if let Some(org_id) = extract_org_id_from_url(&current_url) {
            return Ok(org_id);
        }

        // Try extracting from page context (localStorage, sessionStorage, etc.)
        let result = tab.evaluate(
            r#"
            // Try to find org ID in various places
            const cookies = document.cookie;
            const orgMatch = cookies.match(/organizationId[=:]([a-f0-9-]{36})/i);
            if (orgMatch) return orgMatch[1];

            // Try localStorage
            const orgId = localStorage.getItem('organizationId') ||
                         localStorage.getItem('orgId') ||
                         localStorage.getItem('org_id');
            if (orgId && orgId.match(/^[a-f0-9-]{36}$/i)) return orgId;

            // Try sessionStorage
            const sessionOrgId = sessionStorage.getItem('organizationId') ||
                                sessionStorage.getItem('orgId') ||
                                sessionStorage.getItem('org_id');
            if (sessionOrgId && sessionOrgId.match(/^[a-f0-9-]{36}$/i)) return sessionOrgId;

            // Try to find it in the page HTML as a data attribute or in scripts
            const htmlText = document.documentElement.innerHTML;
            const htmlMatch = htmlText.match(/organizationId["']?\s*:\s*["']([a-f0-9-]{36})/i);
            if (htmlMatch) return htmlMatch[1];

            null;
            "#,
            false,
        )?;

        if let Some(value) = result.value {
            if let Some(org_id_str) = value.as_str() {
                if !org_id_str.is_empty() && org_id_str != "null" {
                    return Ok(org_id_str.to_string());
                }
            }
        }

        anyhow::bail!(
            "Could not extract organization ID from usage page. \n\
            The page format may have changed or the API may not have been called yet.\n\
            Please try:\n\
            1. Manually navigate to claude.ai/usage in your browser\n\
            2. Check your browser's Network tab (F12) for requests to /organizations/\n\
            3. Report this issue with any error details"
        )
    }

    /// Make a request via the browser by navigating to the URL and reading the JSON response
    pub fn fetch_json(&self, url: &str, verbose: bool) -> anyhow::Result<serde_json::Value> {
        // Get or create a tab
        let tabs = self.browser.get_tabs().lock().unwrap();
        let tab = tabs.first()
            .ok_or_else(|| anyhow::anyhow!("No browser tabs available"))?;

        // Simply navigate to the API URL - the browser will display the JSON response
        tab.navigate_to(url)?;

        // Wait for navigation to complete
        tab.wait_until_navigated()?;

        // Additional wait to ensure page is fully loaded
        std::thread::sleep(Duration::from_secs(3));

        // Extract the JSON from the page body
        let result = tab.evaluate(
            r#"document.body.innerText || document.body.textContent"#,
            false,
        )?;

        // Parse the JSON string
        if let Some(value) = result.value {
            if let Some(json_str) = value.as_str() {
                // Debug: Print the raw response if it's short
                if verbose {
                    if json_str.len() < 500 {
                        tracing::debug!("API Response: {}", json_str);
                    } else {
                        tracing::debug!("API Response length: {} bytes", json_str.len());
                    }
                }

                let parsed: serde_json::Value = serde_json::from_str(json_str)
                    .map_err(|e| {
                        // Provide more context in the error
                        let preview = if json_str.len() > 200 {
                            format!("{}...", &json_str[..200])
                        } else {
                            json_str.to_string()
                        };
                        anyhow::anyhow!(
                            "Failed to parse JSON response: {}\nResponse preview: {}",
                            e,
                            preview
                        )
                    })?;
                return Ok(parsed);
            }
        }

        anyhow::bail!("Could not extract JSON from page");
    }

    /// Get all cookies from the browser
    fn get_cookies(&self, tab: &headless_chrome::Tab) -> anyhow::Result<Vec<(String, String)>> {
        // Get cookies using CDP
        let cookies_result = tab.call_method(Network::GetCookies {
            urls: Some(vec!["https://claude.ai".to_string()]),
        })?;

        let mut cookie_pairs = Vec::new();

        for cookie in cookies_result.cookies {
            cookie_pairs.push((cookie.name, cookie.value));
        }

        Ok(cookie_pairs)
    }
}

/// Extract organization ID from a claude.ai URL
fn extract_org_id_from_url(url: &str) -> Option<String> {
    // URLs like: https://claude.ai/settings/59b2da95-7ec0-4540-95fa-cea7c7f333c6
    // or: https://claude.ai/new/59b2da95-7ec0-4540-95fa-cea7c7f333c6

    let parts: Vec<&str> = url.split('/').collect();

    // Look for a part that looks like a UUID
    for part in parts {
        if part.len() == 36 && part.matches('-').count() == 4 {
            return Some(part.to_string());
        }
    }

    None
}
