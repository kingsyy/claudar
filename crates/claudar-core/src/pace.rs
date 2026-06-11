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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    const PERIOD: i64 = 300; // 5-hour window in minutes

    /// Build a reset time such that exactly `time_pct` of the period has elapsed
    /// as of "now". With a 300-minute period, 50% elapsed means the reset is
    /// 150 minutes in the future.
    fn reset_at_elapsed(time_pct: f64) -> DateTime<Utc> {
        let elapsed = (PERIOD as f64 * time_pct / 100.0) as i64;
        Utc::now() + Duration::minutes(PERIOD - elapsed)
    }

    #[test]
    fn over_pace_when_usage_outruns_time() {
        // 50% of the period elapsed but 80% used → ~30% over pace.
        let result = calculate_pace_info(80.0, reset_at_elapsed(50.0), PERIOD);
        assert!(result.starts_with('⚡'), "expected over-pace, got {result:?}");
        assert!(result.contains("over pace"), "got {result:?}");
    }

    #[test]
    fn under_pace_when_time_outruns_usage() {
        // 50% of the period elapsed but only 20% used → ~30% under pace.
        let result = calculate_pace_info(20.0, reset_at_elapsed(50.0), PERIOD);
        assert!(result.starts_with('🐌'), "expected under-pace, got {result:?}");
        assert!(result.contains("under pace"), "got {result:?}");
    }

    #[test]
    fn on_pace_within_ten_percent_band() {
        // 50% elapsed, 55% used → only 5% over, inside the ±10 dead band.
        let result = calculate_pace_info(55.0, reset_at_elapsed(50.0), PERIOD);
        assert_eq!(result, "✓ On pace");
    }

    #[test]
    fn boundary_just_inside_band_is_on_pace() {
        // ~50% elapsed, 59% used → ~9% over, still on pace (threshold is > 10).
        let result = calculate_pace_info(59.0, reset_at_elapsed(50.0), PERIOD);
        assert_eq!(result, "✓ On pace");
    }

    #[test]
    fn time_pct_clamped_when_period_already_elapsed() {
        // Reset is in the past: elapsed > period, so time_pct clamps to 100.
        // 40% used vs 100% time → strongly under pace, never a panic or >100 value.
        let past_reset = Utc::now() - Duration::minutes(60);
        let result = calculate_pace_info(40.0, past_reset, PERIOD);
        assert!(result.contains("under pace"), "got {result:?}");
    }
}
