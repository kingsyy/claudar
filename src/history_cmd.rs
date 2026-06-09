use crate::cli::HistoryView;
use claudar_core::config::Config;
use claudar_core::history::{self, HistoryRecord, PredictionSource};
use chrono::{DateTime, Local, Utc};
use colored::Colorize;

pub fn run_history(
    config: &Config,
    instance_filter: Option<String>,
    view: HistoryView,
    days: u32,
    json_output: bool,
) -> anyhow::Result<()> {
    if !config.history.enabled {
        println!(
            "{} History recording is disabled.",
            "ℹ".bright_black()
        );
        println!("Enable it with:");
        println!("  claudar config set history.enabled true");
        return Ok(());
    }

    let instances = config.effective_instances();

    // Determine which instances to show.
    let instances_to_show: Vec<_> = match &instance_filter {
        Some(name) => {
            let matching: Vec<_> = instances.into_iter().filter(|i| i.name == *name).collect();
            if matching.is_empty() {
                anyhow::bail!(
                    "Instance '{}' not found. Use 'claudar instances list' to see configured instances.",
                    name
                );
            }
            matching
        }
        None => instances,
    };

    let show_headers = instances_to_show.len() > 1;

    for (idx, instance) in instances_to_show.iter().enumerate() {
        if show_headers {
            let header = format!("── {} ", instance.name.bold());
            let line_len = 50_usize.saturating_sub(instance.name.len() + 4);
            println!("{}{}", header, "─".repeat(line_len));
            println!();
        }

        if let Err(e) = display_instance_history(config, &instance.name, &view, days, json_output)
        {
            eprintln!(
                "  Error loading history for '{}': {}",
                instance.name, e
            );
        }

        if idx < instances_to_show.len() - 1 {
            println!();
        }
    }

    Ok(())
}

fn display_instance_history(
    config: &Config,
    instance_name: &str,
    view: &HistoryView,
    days: u32,
    json_output: bool,
) -> anyhow::Result<()> {
    let all_records = history::load_records(config, instance_name)?;

    if all_records.is_empty() {
        println!(
            "  {} No history yet for '{}'. Records are written after each monitor poll.",
            "ℹ".bright_black(),
            instance_name
        );
        return Ok(());
    }

    // Filter to the requested day window.
    let cutoff = Utc::now() - chrono::Duration::days(days as i64);
    let records: Vec<HistoryRecord> = all_records
        .into_iter()
        .filter(|r| r.polled_at >= cutoff)
        .collect();

    if records.is_empty() {
        println!(
            "  {} No records in the last {} days for '{}'.",
            "ℹ".bright_black(),
            days,
            instance_name
        );
        return Ok(());
    }

    if json_output {
        println!("{}", serde_json::to_string_pretty(&records)?);
        return Ok(());
    }

    match view {
        HistoryView::Timeline => display_timeline(&records, config),
        HistoryView::Windows => display_windows(&records, config),
        HistoryView::Daily => display_daily(&records),
        HistoryView::Predict => display_predict(&records),
        HistoryView::All => {
            display_timeline(&records, config);
            println!();
            display_windows(&records, config);
            println!();
            display_daily(&records);
            println!();
            display_predict(&records);
        }
    }

    Ok(())
}

// ─── Timeline view ───────────────────────────────────────────────────────────

fn display_timeline(records: &[HistoryRecord], config: &Config) {
    println!("{}", "Usage Timeline".bold());
    println!("{}", "━".repeat(70));

    // Show at most 96 records (24 h at 15-min polls) unless the set is smaller.
    let display_records: Vec<&HistoryRecord> = if records.len() > 96 {
        records[records.len() - 96..].iter().collect()
    } else {
        records.iter().collect()
    };

    // Find peak 5-hour record index for highlighting.
    let peak_idx = display_records
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.five_hour_pct.partial_cmp(&b.five_hour_pct).unwrap())
        .map(|(i, _)| i);

    println!(
        "  {:<20}  {:>6}  {:>6}  {}",
        "Timestamp".bright_black(),
        "5h %".bright_black(),
        "7d %".bright_black(),
        "5-hour bar".bright_black()
    );
    println!("  {}", "─".repeat(65).bright_black());

    for (i, rec) in display_records.iter().enumerate() {
        let ts = format_ts_local(rec.polled_at, &config.general.timezone);
        let bar = mini_bar(rec.five_hour_pct / 100.0);
        let five_str = format!("{:5.1}%", rec.five_hour_pct);
        let seven_str = format!("{:5.1}%", rec.seven_day_pct);

        let (five_colored, bar_colored) = color_pct(rec.five_hour_pct, five_str, bar);

        let peak_marker = if Some(i) == peak_idx { " ← peak" } else { "" };

        println!(
            "  {:<20}  {}  {}  {}{}",
            ts.bright_black(),
            five_colored,
            seven_str.bright_black(),
            bar_colored,
            peak_marker.bright_black()
        );
    }

    println!(
        "\n  {} records shown ({} total in window)",
        display_records.len(),
        records.len()
    );
}

// ─── Windows view ────────────────────────────────────────────────────────────

fn display_windows(records: &[HistoryRecord], config: &Config) {
    let windows = history::summarize_windows(records);

    println!("{}", "5-Hour Window Summaries".bold());
    println!("{}", "━".repeat(70));

    if windows.is_empty() {
        println!("  No complete windows in this time range.");
        return;
    }

    println!(
        "  {:<32}  {:>7}  {:>8}  {:>9}  {:>8}",
        "Window".bright_black(),
        "Peak 5h".bright_black(),
        "Hit 100?".bright_black(),
        "Time→100".bright_black(),
        "7d cost".bright_black()
    );
    println!("  {}", "─".repeat(68).bright_black());

    for w in &windows {
        let start = format_ts_local(w.window_start, &config.general.timezone);
        let end_time = format_ts_local(w.window_end, &config.general.timezone);
        let range = format!("{} → {}", start, end_time);

        let peak_str = format!("{:5.1}%", w.peak_five_hour_pct);
        let (peak_colored, _) = color_pct(w.peak_five_hour_pct, peak_str.clone(), String::new());

        let hit_str = if w.hit_100 {
            "yes".green().to_string()
        } else {
            "no".bright_black().to_string()
        };

        let t100_str = w
            .time_to_100
            .map(|d| format_duration_short(d))
            .unwrap_or_else(|| "—".to_string());

        let cost_str = format!("+{:.1}%", w.seven_day_delta);
        let cost_colored = if w.seven_day_delta >= 3.0 {
            cost_str.yellow()
        } else {
            cost_str.bright_black()
        };

        println!(
            "  {:<32}  {}  {:>8}  {:>9}  {}",
            range.bright_black(),
            peak_colored,
            hit_str,
            t100_str.bright_black(),
            cost_colored
        );
    }

    println!(
        "\n  {} windows detected",
        windows.len()
    );
}

// ─── Daily pattern view ──────────────────────────────────────────────────────

fn display_daily(records: &[HistoryRecord]) {
    let pattern = history::daily_pattern(records);

    println!("{}", "Average 5-Hour Usage by Hour of Day".bold());
    println!("{}", "━".repeat(60));

    let max_avg = pattern
        .iter()
        .filter_map(|v| *v)
        .fold(f64::NEG_INFINITY, f64::max);

    if max_avg <= 0.0 {
        println!("  Not enough data.");
        return;
    }

    for h in 0..24 {
        match pattern[h] {
            None => {
                println!("  {:02}  {:<20}  (no data)", h, "·".repeat(20).bright_black());
            }
            Some(avg) => {
                let bar_len = ((avg / max_avg) * 20.0).round() as usize;
                let bar = format!("{}{}", "█".repeat(bar_len), "░".repeat(20 - bar_len));
                let pct_str = format!("{:5.1}%", avg);
                let (bar_colored, _) = color_pct(avg, bar, String::new());
                let (pct_colored, _) = color_pct(avg, pct_str, String::new());
                println!("  {:02}  {}  {}", h, bar_colored, pct_colored);
            }
        }
    }

    // Find peak hour.
    if let Some((peak_h, _)) = pattern
        .iter()
        .enumerate()
        .filter_map(|(h, v)| v.map(|avg| (h, avg)))
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
    {
        println!("\n  Peak hour: {:02}:00 UTC", peak_h);
    }
}

// ─── Predict view ────────────────────────────────────────────────────────────

fn display_predict(records: &[HistoryRecord]) {
    // Use the most recent 7-day percentage for the prediction.
    let current_7d = records.last().map(|r| r.seven_day_pct).unwrap_or(0.0);
    let pred = history::predict_windows_remaining(records, current_7d);

    println!("{}", "7-Day Limit Prediction".bold());
    println!("{}", "━".repeat(60));

    let seven_bar = mini_bar(pred.current_seven_day_pct / 100.0);
    let (seven_str, _) = color_pct(
        pred.current_seven_day_pct,
        format!("{:.1}%", pred.current_seven_day_pct),
        String::new(),
    );
    println!("  Current 7-day usage:  {} {}", seven_str, seven_bar.bright_black());
    println!(
        "  Remaining capacity:   {:.1}%",
        (100.0 - pred.current_seven_day_pct).max(0.0)
    );
    println!();

    match pred.data_source {
        PredictionSource::Empirical => {
            println!(
                "  Cost per full 5h window:  {:.2}%  {}",
                pred.avg_window_cost_pct,
                format!("(empirical, {} windows observed)", pred.windows_observed).bright_black()
            );
        }
        PredictionSource::Theoretical => {
            let obs_note = if pred.windows_observed == 1 {
                "(only 1 maxed window observed — need ≥ 2 for empirical)".to_string()
            } else {
                "(no maxed windows observed yet)".to_string()
            };
            println!(
                "  Cost per full 5h window:  {:.2}%  {}",
                pred.avg_window_cost_pct,
                format!("(theoretical — {})", obs_note).bright_black()
            );
        }
    }

    println!();

    let windows_floor = pred.windows_remaining.floor() as u64;
    let prediction_str = format!("≈ {} full 5-hour windows", windows_floor);
    let (colored_prediction, _) = color_windows(pred.windows_remaining, prediction_str);

    println!("  {}", "Estimated windows remaining:".bold());
    println!("    {}", colored_prediction);
    println!();

    if pred.windows_remaining < 5.0 {
        println!(
            "  {} You are close to your weekly limit!",
            "⚠".yellow()
        );
    } else {
        println!(
            "  At your {} pace, you can max out approximately {} more",
            if pred.data_source == PredictionSource::Empirical {
                "historical"
            } else {
                "estimated"
            },
            windows_floor
        );
        println!("  5-hour windows before hitting your weekly limit.");
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Format a UTC timestamp in local time (or the configured timezone).
fn format_ts_local(dt: DateTime<Utc>, timezone: &str) -> String {
    let tz_lower = timezone.to_lowercase();
    match tz_lower.as_str() {
        "utc" => dt.format("%b %d %H:%M").to_string(),
        _ => {
            use std::str::FromStr;
            if let Ok(tz) = chrono_tz::Tz::from_str(timezone) {
                dt.with_timezone(&tz).format("%b %d %H:%M").to_string()
            } else {
                dt.with_timezone(&Local).format("%b %d %H:%M").to_string()
            }
        }
    }
}

/// 10-char ASCII progress bar: `[████████░░]`
fn mini_bar(fraction: f64) -> String {
    let width = 10usize;
    let filled = (fraction.clamp(0.0, 1.0) * width as f64).round() as usize;
    format!("[{}{}]", "█".repeat(filled), "░".repeat(width - filled))
}

/// Apply a colour to a percentage string and an associated bar based on severity.
fn color_pct(pct: f64, pct_str: String, bar: String) -> (String, String) {
    if pct >= 90.0 {
        (pct_str.red().bold().to_string(), bar.red().to_string())
    } else if pct >= 70.0 {
        (pct_str.yellow().to_string(), bar.yellow().to_string())
    } else if pct >= 50.0 {
        (pct_str.bright_yellow().to_string(), bar.bright_yellow().to_string())
    } else {
        (pct_str.green().to_string(), bar.green().to_string())
    }
}

/// Colour a windows-remaining count (red when low, green when plenty).
fn color_windows(count: f64, label: String) -> (String, String) {
    if count < 3.0 {
        (label.red().bold().to_string(), String::new())
    } else if count < 10.0 {
        (label.yellow().to_string(), String::new())
    } else {
        (label.green().to_string(), String::new())
    }
}

/// Format a chrono::Duration as e.g. "3h 15m" or "42m".
fn format_duration_short(d: chrono::Duration) -> String {
    let total_minutes = d.num_minutes().max(0);
    let h = total_minutes / 60;
    let m = total_minutes % 60;
    if h > 0 {
        format!("{}h {:02}m", h, m)
    } else {
        format!("{}m", m)
    }
}
