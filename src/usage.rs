use claude_notify_core::config::Config;
use claude_notify_core::pace;
use claude_notify_core::storage::SessionData;
use claude_notify_core::time_format;
use claude_notify_core::usage_fetcher::{UsageResponse, fetch_usage};
use chrono::{DateTime, Local, Utc};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

pub async fn run_usage(verbose: bool, instance_filter: Option<String>) -> anyhow::Result<()> {
    let config = Config::load()?;

    let now = Local::now();
    let timestamp = time_format::format_datetime_24h(&now, &config.general.timezone)
        .unwrap_or_else(|_| now.format("%H:%M %d/%m/%Y").to_string());

    println!("\nClaude.ai Usage");
    println!("{}", "━".repeat(50));
    println!("Fetched at: {}", timestamp.bright_black());
    println!();

    let instances = config.effective_instances();
    let show_headers = instances.len() > 1;

    let instances_to_show: Vec<_> = match &instance_filter {
        Some(name) => {
            let matching: Vec<_> = instances.into_iter().filter(|i| i.name == *name).collect();
            if matching.is_empty() {
                anyhow::bail!(
                    "Instance '{}' not found. Use 'claude-notify instances list' to see configured instances.",
                    name
                );
            }
            matching
        }
        None => instances,
    };

    for (idx, instance) in instances_to_show.iter().enumerate() {
        if show_headers {
            println!(
                "── {} {}",
                instance.name.bold(),
                "─".repeat(48 - instance.name.len())
            );
            println!();
        }

        if let Err(e) = display_instance_usage(&config, &instance.name, verbose).await {
            eprintln!("  Error fetching usage for '{}': {}", instance.name, e);
        }

        if idx < instances_to_show.len() - 1 {
            println!();
        }
    }

    Ok(())
}

async fn display_instance_usage(
    config: &Config,
    instance_name: &str,
    _verbose: bool,
) -> anyhow::Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );

    spinner.set_message("Fetching usage data...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));

    let session_path = config.session_path_for(instance_name)?;
    let session = SessionData::load(&session_path)?;
    let cookie_header = session.cookie_header_string();

    let usage_json = fetch_usage(&cookie_header, &session.org_id).await?;

    let usage: UsageResponse = serde_json::from_value(usage_json)
        .map_err(|e| anyhow::anyhow!("Failed to parse usage data: {}", e))?;

    spinner.finish_and_clear();

    display_usage_limit(
        "5-Hour Limit",
        usage.five_hour.utilization,
        usage.five_hour.resets_at.as_deref(),
        &config.thresholds.five_hour,
        300,
        &config.general.timezone,
    )?;

    println!();

    display_usage_limit(
        "7-Day Limit",
        usage.seven_day.utilization,
        usage.seven_day.resets_at.as_deref(),
        &config.thresholds.seven_day,
        10080,
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
    let percentage = utilization;

    let (color, status, emoji) = get_color_and_status(percentage, thresholds);

    let name_padded = format!("{:14}", name);
    let token_bar = create_progress_bar(percentage / 100.0);
    let percentage_str = format!("{:5.1}%", percentage).color(color).bold();

    match resets_at {
        Some(resets_at_str) => {
            let reset_time = DateTime::parse_from_rfc3339(resets_at_str)
                .or_else(|_| DateTime::parse_from_rfc3339(&format!("{}Z", resets_at_str)))
                .map_err(|e| anyhow::anyhow!("Failed to parse reset time: {}", e))?;

            let now = Utc::now();
            let duration = reset_time.signed_duration_since(now);

            let period_start = reset_time - chrono::Duration::minutes(period_minutes);
            let elapsed_duration = now.signed_duration_since(period_start);
            let elapsed_minutes = elapsed_duration.num_minutes();
            let time_percentage = (elapsed_minutes as f64 / period_minutes as f64 * 100.0)
                .min(100.0)
                .max(0.0);

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

            let absolute_time = time_format::format_reset_time_24h(&reset_time, timezone)
                .unwrap_or_else(|_| {
                    reset_time
                        .with_timezone(&Local)
                        .format("%b %d, %H:%M")
                        .to_string()
                });

            let time_bar = create_progress_bar(time_percentage / 100.0);
            let time_pct_str = format!("{:5.1}%", time_percentage).bright_black();

            let pace_text =
                pace::calculate_pace_info(percentage, reset_time.with_timezone(&Utc), period_minutes);
            let pace_color = if pace_text.contains("over pace") {
                colored::Color::Yellow
            } else if pace_text.contains("under pace") {
                colored::Color::Green
            } else {
                colored::Color::BrightBlack
            };

            println!(
                "{}  {} {} {}",
                name_padded.bold(),
                emoji,
                status.color(color),
                pace_text.color(pace_color)
            );
            println!("  ⏱  {}  {}", time_bar.bright_black(), time_pct_str);
            println!("  💬 {}  {}", token_bar.color(color), percentage_str);
            println!(
                "  Resets in: {} (at {})",
                relative_time.bright_black(),
                absolute_time.bright_black()
            );
        }
        None => {
            println!(
                "{}  💬 {}  {}   {}  {}",
                name_padded.bold(),
                token_bar.color(color),
                percentage_str,
                emoji,
                status.color(color)
            );
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

    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

#[cfg(test)]
fn create_overlapping_bar(time_percentage: f64, usage_percentage: f64) -> String {
    let bar_width = 20;
    let time_pos = ((time_percentage / 100.0) * bar_width as f64).round() as usize;
    let usage_pos = ((usage_percentage / 100.0) * bar_width as f64).round() as usize;
    let time_pos = time_pos.min(bar_width);
    let usage_pos = usage_pos.min(bar_width);

    let mut bar = String::from("[");

    for i in 0..bar_width {
        if i < usage_pos.min(time_pos) {
            bar.push('█');
        } else if i < time_pos {
            bar.push('░');
        } else if i < usage_pos {
            bar.push('▓');
        } else {
            bar.push('░');
        }
    }

    bar.push(']');
    bar
}

fn get_color_and_status(
    percentage: f64,
    thresholds: &[u8],
) -> (colored::Color, &'static str, &'static str) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_bar_zero() {
        let bar = create_progress_bar(0.0);
        assert_eq!(bar, "[░░░░░░░░░░]");
    }

    #[test]
    fn progress_bar_half() {
        let bar = create_progress_bar(0.5);
        assert_eq!(bar, "[█████░░░░░]");
    }

    #[test]
    fn progress_bar_full() {
        let bar = create_progress_bar(1.0);
        assert_eq!(bar, "[██████████]");
    }

    #[test]
    fn progress_bar_clamps_over_100() {
        let bar = create_progress_bar(1.5);
        assert_eq!(bar, "[██████████]");
    }

    #[test]
    fn overlapping_bar_under_pace() {
        let bar = create_overlapping_bar(50.0, 25.0);
        assert_eq!(bar, "[█████░░░░░░░░░░░░░░░]");
    }

    #[test]
    fn overlapping_bar_over_pace() {
        let bar = create_overlapping_bar(25.0, 50.0);
        assert_eq!(bar, "[█████▓▓▓▓▓░░░░░░░░░░]");
    }

    #[test]
    fn overlapping_bar_equal() {
        let bar = create_overlapping_bar(50.0, 50.0);
        assert_eq!(bar, "[██████████░░░░░░░░░░]");
    }

    #[test]
    fn overlapping_bar_zero() {
        let bar = create_overlapping_bar(0.0, 0.0);
        assert_eq!(bar, "[░░░░░░░░░░░░░░░░░░░░]");
    }

    #[test]
    fn overlapping_bar_full() {
        let bar = create_overlapping_bar(100.0, 100.0);
        assert_eq!(bar, "[████████████████████]");
    }

    #[test]
    fn color_status_below_all_thresholds() {
        let (color, status, _) = get_color_and_status(30.0, &[50, 70, 90]);
        assert_eq!(color, colored::Color::Green);
        assert_eq!(status, "OK");
    }

    #[test]
    fn color_status_at_50() {
        let (color, status, _) = get_color_and_status(55.0, &[50, 70, 90]);
        assert_eq!(color, colored::Color::Yellow);
        assert_eq!(status, "ELEVATED");
    }

    #[test]
    fn color_status_at_70() {
        let (color, status, _) = get_color_and_status(75.0, &[50, 70, 90]);
        assert_eq!(color, colored::Color::Yellow);
        assert_eq!(status, "WARNING");
    }

    #[test]
    fn color_status_at_90() {
        let (color, status, _) = get_color_and_status(95.0, &[50, 70, 90]);
        assert_eq!(color, colored::Color::Red);
        assert_eq!(status, "CRITICAL");
    }

    #[test]
    fn color_status_custom_thresholds() {
        let (color, status, _) = get_color_and_status(85.0, &[80]);
        assert_eq!(color, colored::Color::Yellow);
        assert_eq!(status, "WARNING");
    }

    #[test]
    fn color_status_empty_thresholds() {
        let (color, status, _) = get_color_and_status(99.0, &[]);
        assert_eq!(color, colored::Color::Green);
        assert_eq!(status, "OK");
    }
}
