use crate::browser_auth::BrowserAuthenticator;
use crate::config::Config;
use crate::storage::SessionData;

pub fn run_setup(verbose: bool, instance_name: Option<String>) -> anyhow::Result<()> {
    println!("\n=== Claude Code Usage Monitor - Setup Wizard ===\n");
    println!("This wizard will help you log in to Claude.ai and extract your session credentials.\n");

    // Determine instance name
    let mut config = Config::load()?;
    let instance_name = match instance_name {
        Some(name) => name,
        None => {
            if config.has_instances() {
                // Prompt user to pick from existing instances or enter a new one
                let mut names: Vec<String> = config.instances.iter().map(|i| i.name.clone()).collect();
                names.push("(new instance)".to_string());

                use dialoguer::Select;
                let selection = Select::new()
                    .with_prompt("Which instance do you want to set up?")
                    .items(&names)
                    .default(0)
                    .interact()?;

                if selection == names.len() - 1 {
                    // New instance
                    use dialoguer::Input;
                    Input::<String>::new()
                        .with_prompt("Enter a name for the new instance")
                        .interact_text()?
                } else {
                    names[selection].clone()
                }
            } else {
                "default".to_string()
            }
        }
    };

    if instance_name != "default" {
        println!("Setting up instance: {}\n", instance_name);
    }

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
    test_api_connection_via_browser(&authenticator, &session_data, verbose)?;
    println!("  ✓ API connection successful!");

    // Now we can safely close the browser
    drop(authenticator);

    // Step 5: Save configuration
    println!("\nStep 5: Saving configuration...");

    // Add instance to config if it's new and not "default"
    if instance_name != "default" && !config.instances.iter().any(|i| i.name == instance_name) {
        config.instances.push(crate::config::InstanceConfig { name: instance_name.clone() });
    }
    config.save()?;

    let session_path = config.session_path_for(&instance_name)?;
    session_data.save(&session_path)?;

    println!("  ✓ Configuration saved!");
    println!("\nSetup complete! You can now run:");
    println!("  claude-notify usage   - Check current usage");
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
    verbose: bool,
) -> anyhow::Result<()> {
    let url = format!(
        "https://claude.ai/api/organizations/{}/usage",
        session_data.org_id
    );

    if verbose {
        println!("  → Making API request via browser: {}", url);
    }

    // Make the API request using the browser's fetch API
    let result = authenticator.fetch_json(&url, verbose)?;

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


