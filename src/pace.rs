use chrono::{DateTime, Utc};

/// Calculate pace information based on current usage and elapsed time in the period.
/// Returns emoji and text describing if usage is over, under, or on pace.
pub fn calculate_pace_info(percentage: f64, reset_time: DateTime<Utc>, period_minutes: i64) -> String {
    let now = Utc::now();
    let period_start = reset_time - chrono::Duration::minutes(period_minutes);
    let elapsed_minutes = now.signed_duration_since(period_start).num_minutes();
    let time_pct = (elapsed_minutes as f64 / period_minutes as f64 * 100.0).min(100.0).max(0.0);

    let pace_diff = percentage - time_pct;

    if pace_diff > 10.0 {
        format!("⚡ {:.1}% over pace", pace_diff)
    } else if pace_diff < -10.0 {
        format!("🐌 {:.1}% under pace", pace_diff.abs())
    } else {
        "✓ On pace".to_string()
    }
}
