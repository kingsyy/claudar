use crate::browser_auth::BrowserAuthenticator;
use crate::config::Config;
use crate::storage::SessionData;

pub fn run_setup() -> anyhow::Result<()> {
    println!("\n=== Claude Code Usage Monitor - Setup Wizard ===\n");
    println!("This wizard will help you log in to Claude.ai and extract your session credentials.\n");

    // Step 0: Show Chrome launch instructions
    println!("Step 1: Start Chrome with Remote Debugging");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("To avoid bot detection, this tool connects to your real Chrome browser.");
    println!("Please start Chrome with the following command:\n");

    // Detect platform and show appropriate command
    #[cfg(target_os = "macos")]
    {
        println!("  /Applications/Google\\ Chrome.app/Contents/MacOS/Google\\ Chrome \\");
        println!("    --remote-debugging-port=9222 \\");
        println!("    --user-data-dir=/tmp/chrome-remote-profile \\");
        println!("    --no-first-run \\");
        println!("    --no-default-browser-check");
    }
    #[cfg(target_os = "linux")]
    {
        println!("  google-chrome \\");
        println!("    --remote-debugging-port=9222 \\");
        println!("    --user-data-dir=/tmp/chrome-remote-profile \\");
        println!("    --no-first-run \\");
        println!("    --no-default-browser-check");
    }
    #[cfg(target_os = "windows")]
    {
        println!("  \"C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe\" ^");
        println!("    --remote-debugging-port=9222 ^");
        println!("    --user-data-dir=%TEMP%\\chrome-remote-profile ^");
        println!("    --no-first-run ^");
        println!("    --no-default-browser-check");
    }

    println!("\nNotes:");
    println!("  • Close any existing Chrome windows first");
    println!("  • Chrome will open with a warning banner - this is normal");
    println!("  • Keep this Chrome window open during setup\n");

    // Wait for user to confirm Chrome is running
    use dialoguer::Input;
    Input::<String>::new()
        .with_prompt("Press Enter once you've started Chrome with remote debugging")
        .allow_empty(true)
        .interact_text()?;

    // Step 1: Connect to browser and perform manual login
    println!("\nStep 2: Browser Login");
    let authenticator = BrowserAuthenticator::new()?;

    let (org_id, cookies) = authenticator.login("")?;

    // Step 3: Build session data from cookies
    println!("\nStep 3: Building session data...");
    let session_data = build_session_data(org_id, cookies)?;

    // Step 4: Test API connection using the browser (before closing it)
    println!("\nStep 4: Testing API connection...");
    test_api_connection_via_browser(&authenticator, &session_data)?;
    println!("  ✓ API connection successful!");

    // Now we can safely close the browser
    drop(authenticator);

    // Step 5: Save configuration
    println!("\nStep 5: Saving configuration...");
    let config = Config::default();
    config.save()?;

    let session_path = config.auth.session_file;
    session_data.save(&session_path)?;

    println!("  ✓ Configuration saved!");
    println!("\nSetup complete! You can now run:");
    println!("  claude-notify status  - Check current usage");
    println!("  claude-notify run     - Start monitoring");

    Ok(())
}

/// Build SessionData from cookie list
fn build_session_data(org_id: String, cookies: Vec<(String, String)>) -> anyhow::Result<SessionData> {
    let mut session_key = None;
    let mut cf_clearance = None;
    let mut last_active_org = None;
    let mut anthropic_device_id = None;
    let mut cf_bm = None;
    let mut ssid = None;

    // Build full cookie string and extract individual values
    let mut cookie_parts = Vec::new();

    for (name, value) in cookies {
        cookie_parts.push(format!("{}={}", name, value));

        match name.as_str() {
            "sessionKey" => session_key = Some(value),
            "cf_clearance" => cf_clearance = Some(value),
            "lastActiveOrg" => last_active_org = Some(value),
            "anthropic-device-id" => anthropic_device_id = Some(value),
            "__cf_bm" => cf_bm = Some(value),
            "__ssid" => ssid = Some(value),
            _ => {}
        }
    }

    let full_cookie_string = cookie_parts.join("; ");

    let session_key = session_key
        .ok_or_else(|| anyhow::anyhow!("sessionKey cookie not found after login"))?;

    Ok(SessionData {
        org_id,
        session_key,
        cf_clearance,
        last_active_org,
        anthropic_device_id,
        cf_bm,
        ssid,
        full_cookie_string: Some(full_cookie_string),
    })
}

/// Test API connection using the browser to avoid Cloudflare detection
fn test_api_connection_via_browser(
    authenticator: &BrowserAuthenticator,
    session_data: &SessionData,
) -> anyhow::Result<()> {
    let url = format!(
        "https://claude.ai/api/organizations/{}/usage",
        session_data.org_id
    );

    println!("  → Making API request via browser: {}", url);

    // Make the API request using the browser's fetch API
    let result = authenticator.fetch_json(&url)?;

    // Verify the response has the expected structure
    if result.get("five_hour").is_some() || result.get("seven_day").is_some() {
        Ok(())
    } else {
        anyhow::bail!(
            "API response doesn't have expected structure. Got: {:?}",
            result
        );
    }
}

#[allow(dead_code)]
fn test_api_connection(session_data: &SessionData) -> anyhow::Result<()> {
    // Create a blocking HTTP client with more browser-like settings
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .gzip(true)
        .brotli(true)
        .use_rustls_tls() // Use rustls instead of native-tls for better compatibility
        .build()?;

    let url = format!(
        "https://claude.ai/api/organizations/{}/usage",
        session_data.org_id
    );

    let cookie_str = session_data.cookie_string();
    println!("  → Sending request with {} cookies", cookie_str.split(';').count());
    println!("  → URL: {}", url);

    // Build request with all necessary headers (matching real browser order)
    let request = client
        .get(&url)
        .header("Accept", "*/*")
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Accept-Encoding", "gzip, deflate, br")
        .header("Referer", "https://claude.ai/settings/usage")
        .header("Origin", "https://claude.ai")
        .header("Connection", "keep-alive")
        .header("Sec-Fetch-Dest", "empty")
        .header("Sec-Fetch-Mode", "cors")
        .header("Sec-Fetch-Site", "same-origin")
        .header("Sec-Ch-Ua", r#""Google Chrome";v="131", "Chromium";v="131", "Not_A Brand";v="24""#)
        .header("Sec-Ch-Ua-Mobile", "?0")
        .header("Sec-Ch-Ua-Platform", r#""macOS""#)
        .header("Cookie", cookie_str);

    let response = request.send()?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_else(|_| "Unable to read response".to_string());
        anyhow::bail!(
            "API test failed with status: {}. Response: {}",
            status,
            &body[..body.len().min(500)]
        );
    }

    // Try to parse the response to verify it's valid
    let json: serde_json::Value = response.json()?;

    // Verify the response has the expected structure
    if json.get("five_hour").is_some() || json.get("seven_day").is_some() {
        Ok(())
    } else {
        anyhow::bail!("API response doesn't have expected structure. Got: {:?}", json);
    }
}

