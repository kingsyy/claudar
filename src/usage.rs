use crate::browser_auth::BrowserAuthenticator;
use crate::config::Config;
use crate::storage::SessionData;
use crate::time_format;
use chrono::{DateTime, Local, Utc};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct UsageResponse {
    five_hour: UsageLimit,
    seven_day: UsageLimit,
}

#[derive(Debug, Deserialize)]
struct UsageLimit {
    utilization: f64,
    #[serde(default)]
    resets_at: Option<String>,
}

pub fn run_usage(verbose: bool) -> anyhow::Result<()> {
    // Load configuration first to get timezone setting
    let config = Config::load()?;

    let now = Local::now();
    let timestamp = time_format::format_datetime_24h(&now, &config.general.timezone)
        .unwrap_or_else(|_| now.format("%H:%M %d/%m/%Y").to_string());

    println!("\nClaude.ai Usage");
    println!("{}", "━".repeat(50));
    println!("Fetched at: {}", timestamp.bright_black());
    println!();

    // Create loading spinner
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.cyan} {msg}")
            .unwrap()
    );

    spinner.set_message("Loading configuration...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    // Load session data (config already loaded above)
    let session = SessionData::load(&config.auth.session_file)?;

    // Convert session data to cookie pairs for injection
    let cookie_pairs = session_to_cookie_pairs(&session);

    spinner.set_message("Launching browser...");

    // Launch headless Chrome
    let browser = BrowserAuthenticator::new_headless(verbose)?;

    spinner.set_message("Authenticating...");

    // Inject stored cookies
    browser.inject_cookies(cookie_pairs, verbose)?;

    spinner.set_message("Fetching usage data...");

    // Fetch usage data
    let usage_json = browser.fetch_usage_api(&session.org_id, verbose)?;

    // Parse the response
    let usage: UsageResponse = serde_json::from_value(usage_json)
        .map_err(|e| anyhow::anyhow!("Failed to parse usage data: {}", e))?;

    spinner.finish_and_clear();
    println!();

    // Display 5-hour limit (5 hours = 300 minutes)
    display_usage_limit(
        "5-Hour Limit",
        usage.five_hour.utilization,
        usage.five_hour.resets_at.as_deref(),
        &config.thresholds.five_hour,
        300, // 5 hours in minutes
        &config.general.timezone,
    )?;

    println!();

    // Display 7-day limit (7 days = 10080 minutes)
    display_usage_limit(
        "7-Day Limit",
        usage.seven_day.utilization,
        usage.seven_day.resets_at.as_deref(),
        &config.thresholds.seven_day,
        10080, // 7 days in minutes
        &config.general.timezone,
    )?;

    println!();

    Ok(())
}

fn display_usage_limit(
    name: &str,
    utilization: f64,
    resets_at: Option<&str>,
    thresholds: &[u8],
    period_minutes: i64,
    timezone: &str,
) -> anyhow::Result<()> {
    // API returns utilization as percentage already
    let percentage = utilization;

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

    // Parse and format reset time (handle null gracefully)
    match resets_at {
        Some(resets_at_str) => {
            let reset_time = DateTime::parse_from_rfc3339(resets_at_str)
                .or_else(|_| {
                    // Try ISO 8601 format without timezone
                    DateTime::parse_from_rfc3339(&format!("{}Z", resets_at_str))
                })
                .map_err(|e| anyhow::anyhow!("Failed to parse reset time: {}", e))?;

            let now = Utc::now();
            let duration = reset_time.signed_duration_since(now);

            // Calculate time elapsed in the period
            let period_start = reset_time - chrono::Duration::minutes(period_minutes);
            let elapsed_duration = now.signed_duration_since(period_start);
            let elapsed_minutes = elapsed_duration.num_minutes();
            let time_percentage = (elapsed_minutes as f64 / period_minutes as f64 * 100.0).min(100.0).max(0.0);

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

            // Format absolute time in 24-hour format with configured timezone
            let absolute_time = time_format::format_reset_time_24h(&reset_time, timezone)
                .unwrap_or_else(|_| reset_time.with_timezone(&Local).format("%b %d, %H:%M").to_string());

            println!(
                "  Resets in: {} (at {})",
                relative_time.bright_black(),
                absolute_time.bright_black()
            );

            // Display time elapsed vs usage comparison with overlapping bar
            if time_percentage > 0.0 {
                let expected_usage = time_percentage; // If perfectly paced, usage should match time elapsed
                let pace_diff = percentage - expected_usage;

                // Create the overlapping bar
                let overlap_bar = create_overlapping_bar(time_percentage, percentage);

                // Determine pace status and color
                let (pace_emoji, pace_text, pace_color) = if pace_diff > 10.0 {
                    ("⚡", format!("{:.1}% over pace", pace_diff), colored::Color::Yellow)
                } else if pace_diff < -10.0 {
                    ("🐌", format!("{:.1}% under pace", pace_diff.abs()), colored::Color::Green)
                } else {
                    ("✓", "On pace".to_string(), colored::Color::BrightBlack)
                };

                // Display the visualization
                println!(
                    "  {} ⏱ {:.1}% │ 💬 {:.1}%",
                    overlap_bar.bright_white(),
                    time_percentage,
                    percentage
                );
                println!(
                    "  {} {}",
                    pace_emoji,
                    pace_text.color(pace_color)
                );
            }
        }
        None => {
            // API returned null for reset time - show fallback message
            println!(
                "  Resets in: {} (API did not provide reset time)",
                "Unknown".bright_black()
            );
        }
    }

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

fn create_overlapping_bar(time_percentage: f64, usage_percentage: f64) -> String {
    let bar_width = 20;
    let time_pos = ((time_percentage / 100.0) * bar_width as f64).round() as usize;
    let usage_pos = ((usage_percentage / 100.0) * bar_width as f64).round() as usize;
    let time_pos = time_pos.min(bar_width);
    let usage_pos = usage_pos.min(bar_width);

    let mut bar = String::from("[");

    for i in 0..bar_width {
        if i < usage_pos.min(time_pos) {
            // Both time and usage have passed this point
            bar.push('█');
        } else if i < time_pos {
            // Only time has passed, usage hasn't reached here yet (under pace)
            bar.push('░');
        } else if i < usage_pos {
            // Usage has passed but time hasn't (over pace) - show with different char
            bar.push('▓');
        } else {
            // Neither has reached here yet
            bar.push('░');
        }
    }

    bar.push(']');
    bar
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
