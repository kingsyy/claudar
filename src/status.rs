use crate::browser_auth::BrowserAuthenticator;
use crate::config::Config;
use crate::storage::SessionData;
use chrono::{DateTime, Utc};
use colored::Colorize;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct UsageResponse {
    five_hour: UsageLimit,
    seven_day: UsageLimit,
}

#[derive(Debug, Deserialize)]
struct UsageLimit {
    utilization: f64,
    resets_at: String,
}

pub fn run_status() -> anyhow::Result<()> {
    println!("\nClaude.ai Usage Status");
    println!("{}", "━".repeat(50));
    println!();

    // Load configuration and session data
    let config = Config::load()?;
    let session = SessionData::load(&config.auth.session_file)?;

    // Convert session data to cookie pairs for injection
    let cookie_pairs = session_to_cookie_pairs(&session);

    // Launch headless Chrome
    let browser = BrowserAuthenticator::new_headless()?;

    // Inject stored cookies
    browser.inject_cookies(cookie_pairs)?;

    // Fetch usage data
    let usage_json = browser.fetch_usage_api(&session.org_id)?;

    // Parse the response
    let usage: UsageResponse = serde_json::from_value(usage_json)
        .map_err(|e| anyhow::anyhow!("Failed to parse usage data: {}", e))?;

    println!();

    // Display 5-hour limit
    display_usage_limit(
        "5-Hour Limit",
        usage.five_hour.utilization,
        &usage.five_hour.resets_at,
        &config.thresholds.five_hour,
    )?;

    println!();

    // Display 7-day limit
    display_usage_limit(
        "7-Day Limit",
        usage.seven_day.utilization,
        &usage.seven_day.resets_at,
        &config.thresholds.seven_day,
    )?;

    println!();
    println!("{}  Use 'claude-notify run' to start monitoring", "💡".bright_blue());
    println!();

    Ok(())
}

fn display_usage_limit(
    name: &str,
    utilization: f64,
    resets_at: &str,
    thresholds: &[u8],
) -> anyhow::Result<()> {
    // Convert utilization to percentage
    // API may return either normalized (0.0-1.0) or percentage (0-100) values
    let percentage = if utilization <= 1.0 {
        utilization * 100.0
    } else {
        utilization
    };

    // Determine color based on thresholds
    let (color, status, emoji) = get_color_and_status(percentage, thresholds);

    // Create progress bar (needs normalized 0-1 value)
    let normalized_utilization = percentage / 100.0;
    let progress_bar = create_progress_bar(normalized_utilization);

    // Format the main line
    let name_padded = format!("{:14}", name);
    let progress_colored = progress_bar.color(color);
    let percentage_str = format!("{:5.1}%", percentage).color(color).bold();

    println!(
        "{}  {}  {}  {}  {}",
        name_padded.bold(),
        progress_colored,
        percentage_str,
        emoji,
        status.color(color)
    );

    // Parse and format reset time
    let reset_time = DateTime::parse_from_rfc3339(resets_at)
        .or_else(|_| {
            // Try ISO 8601 format without timezone
            DateTime::parse_from_rfc3339(&format!("{}Z", resets_at))
        })
        .map_err(|e| anyhow::anyhow!("Failed to parse reset time: {}", e))?;

    let now = Utc::now();
    let duration = reset_time.signed_duration_since(now);

    // Format relative time
    let relative_time = if duration.num_weeks() > 0 {
        let weeks = duration.num_weeks();
        let days = duration.num_days() % 7;
        format!("{}w {}d", weeks, days)
    } else if duration.num_days() > 0 {
        let days = duration.num_days();
        let hours = duration.num_hours() % 24;
        format!("{}d {}h", days, hours)
    } else if duration.num_hours() > 0 {
        let hours = duration.num_hours();
        let minutes = duration.num_minutes() % 60;
        format!("{}h {}m", hours, minutes)
    } else if duration.num_minutes() > 0 {
        let minutes = duration.num_minutes();
        format!("{}m", minutes)
    } else {
        "< 1m".to_string()
    };

    // Format absolute time
    let absolute_time = reset_time.format("%b %d, %I:%M %p");

    println!(
        "  Resets in: {} (at {})",
        relative_time.bright_black(),
        absolute_time.to_string().bright_black()
    );

    Ok(())
}

fn create_progress_bar(utilization: f64) -> String {
    let bar_width = 10;
    let filled = (utilization * bar_width as f64).round() as usize;
    let filled = filled.min(bar_width);
    let empty = bar_width - filled;

    format!(
        "[{}{}]",
        "█".repeat(filled),
        "░".repeat(empty)
    )
}

fn get_color_and_status(percentage: f64, thresholds: &[u8]) -> (colored::Color, &'static str, &'static str) {
    // Find the highest threshold that's been exceeded
    let mut highest_threshold = 0;
    for &threshold in thresholds {
        if percentage >= threshold as f64 {
            highest_threshold = threshold;
        }
    }

    match highest_threshold {
        90..=100 => (colored::Color::Red, "CRITICAL", "🔴"),
        70..=89 => (colored::Color::Yellow, "WARNING", "⚠️"),
        50..=69 => (colored::Color::Yellow, "ELEVATED", "⚠️"),
        _ => (colored::Color::Green, "OK", "✓"),
    }
}

fn session_to_cookie_pairs(session: &SessionData) -> Vec<(String, String)> {
    let mut cookies = vec![
        ("sessionKey".to_string(), session.session_key.clone()),
    ];

    if let Some(ref cf) = session.cf_clearance {
        cookies.push(("cf_clearance".to_string(), cf.clone()));
    }
    if let Some(ref org) = session.last_active_org {
        cookies.push(("lastActiveOrg".to_string(), org.clone()));
    }
    if let Some(ref device) = session.anthropic_device_id {
        cookies.push(("anthropic-device-id".to_string(), device.clone()));
    }
    if let Some(ref bm) = session.cf_bm {
        cookies.push(("__cf_bm".to_string(), bm.clone()));
    }
    if let Some(ref ssid) = session.ssid {
        cookies.push(("__ssid".to_string(), ssid.clone()));
    }

    cookies
}
