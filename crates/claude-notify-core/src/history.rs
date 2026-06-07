use crate::config::Config;
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};

// ─── Data Types ─────────────────────────────────────────────────────────────

/// One usage snapshot per monitor poll cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryRecord {
    /// UTC timestamp when this poll was recorded.
    pub polled_at: DateTime<Utc>,

    /// 5-hour utilization percentage (0–100).
    pub five_hour_pct: f64,

    /// Raw API reset timestamp for the 5-hour window (RFC3339 or ISO 8601 string).
    /// Stored as-is; window boundary detection uses string equality.
    pub five_hour_resets_at: Option<String>,

    /// 7-day utilization percentage (0–100).
    pub seven_day_pct: f64,

    /// Raw API reset timestamp for the 7-day window.
    pub seven_day_resets_at: Option<String>,

    /// Linear extrapolation of 5-hour usage to end-of-period.
    /// `None` when not enough time has elapsed to compute a meaningful prediction.
    pub five_hour_predicted_pct: Option<f64>,
}

/// Summary of a single 5-hour usage window (bounded by reset_at transitions).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WindowSummary {
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    /// Peak 5-hour utilization observed in this window.
    pub peak_five_hour_pct: f64,
    /// Whether any record in this window reached ≥ 100%.
    pub hit_100: bool,
    /// Duration from window_start to first record at ≥ 100%.
    pub time_to_100: Option<chrono::Duration>,
    /// 7-day utilization at the first poll in this window.
    pub seven_day_pct_at_start: f64,
    /// 7-day utilization at the last poll in this window.
    pub seven_day_pct_at_end: f64,
    /// Approximate cost to the 7-day limit for this window (pct_at_end − pct_at_start).
    pub seven_day_delta: f64,
}

/// Cross-limit prediction: how many more full 5-hour windows before the 7-day limit.
#[derive(Debug, Clone)]
pub struct CrossLimitPrediction {
    pub current_seven_day_pct: f64,
    /// Average 7-day % cost per fully-maxed 5-hour window.
    pub avg_window_cost_pct: f64,
    /// Estimated number of full windows remaining.
    pub windows_remaining: f64,
    /// Whether the cost figure is empirical (from history) or theoretical.
    pub data_source: PredictionSource,
    /// How many fully-maxed windows were observed (used for empirical calculation).
    pub windows_observed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PredictionSource {
    /// Derived from observed windows that actually hit 100%.
    Empirical,
    /// Fallback: 100% / 33.6 windows-per-week ≈ 2.976% per window.
    Theoretical,
}

// ─── Write ───────────────────────────────────────────────────────────────────

/// Append one record to the JSONL history file for the given instance.
/// Creates the file and any missing parent directories automatically.
/// After writing, trims the file to `config.history.max_records` if needed.
pub fn append_record(
    config: &Config,
    instance_name: &str,
    record: &HistoryRecord,
) -> anyhow::Result<()> {
    let path = config.history_path_for(instance_name)?;

    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Append the JSON line.
    let line = serde_json::to_string(record)?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{}", line)?;
    drop(file);

    // Trim to max_records if a limit is set.
    let max = config.history.max_records;
    if max > 0 {
        trim_to_max_records(&path, max)?;
    }

    Ok(())
}

/// Rewrite the file keeping only the last `max` non-empty lines.
fn trim_to_max_records(path: &std::path::Path, max: usize) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)?;
    let lines: Vec<&str> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();

    if lines.len() <= max {
        return Ok(());
    }

    let trimmed = lines[lines.len() - max..].join("\n") + "\n";
    std::fs::write(path, trimmed)?;
    Ok(())
}

// ─── Read ─────────────────────────────────────────────────────────────────────

/// Read all records for an instance, oldest first.
/// Lines that fail to parse are skipped with a warning.
pub fn load_records(config: &Config, instance_name: &str) -> anyhow::Result<Vec<HistoryRecord>> {
    let path = config.history_path_for(instance_name)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    parse_jsonl(&path)
}

/// Read the N most recent records for an instance.
#[allow(dead_code)]
pub fn load_recent_records(
    config: &Config,
    instance_name: &str,
    n: usize,
) -> anyhow::Result<Vec<HistoryRecord>> {
    let all = load_records(config, instance_name)?;
    if all.len() <= n {
        Ok(all)
    } else {
        Ok(all[all.len() - n..].to_vec())
    }
}

fn parse_jsonl(path: &std::path::Path) -> anyhow::Result<Vec<HistoryRecord>> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<HistoryRecord>(&line) {
            Ok(record) => records.push(record),
            Err(e) => {
                tracing::warn!("Skipping malformed history record at line {}: {}", i + 1, e);
            }
        }
    }

    Ok(records)
}

// ─── Analytics ────────────────────────────────────────────────────────────────

/// Group records into 5-hour windows by detecting transitions in `five_hour_resets_at`.
/// Returns one `WindowSummary` per detected window, oldest first.
pub fn summarize_windows(records: &[HistoryRecord]) -> Vec<WindowSummary> {
    if records.is_empty() {
        return Vec::new();
    }

    let mut windows = Vec::new();
    let mut window_start_idx = 0;
    let mut prev_resets_at = records[0].five_hour_resets_at.clone();

    for i in 1..records.len() {
        if records[i].five_hour_resets_at != prev_resets_at {
            windows.push(build_window_summary(&records[window_start_idx..i]));
            window_start_idx = i;
            prev_resets_at = records[i].five_hour_resets_at.clone();
        }
    }

    // Final (possibly incomplete) window.
    windows.push(build_window_summary(&records[window_start_idx..]));

    windows
}

fn build_window_summary(records: &[HistoryRecord]) -> WindowSummary {
    debug_assert!(!records.is_empty(), "window slice must be non-empty");

    let first = &records[0];
    let last = &records[records.len() - 1];

    let window_start = first.polled_at;
    let window_end = last.polled_at;

    let peak_five_hour_pct = records
        .iter()
        .map(|r| r.five_hour_pct)
        .fold(f64::NEG_INFINITY, f64::max);

    let hit_100 = records.iter().any(|r| r.five_hour_pct >= 100.0);

    let time_to_100 = if hit_100 {
        records
            .iter()
            .find(|r| r.five_hour_pct >= 100.0)
            .map(|r| r.polled_at.signed_duration_since(window_start))
    } else {
        None
    };

    let seven_day_pct_at_start = first.seven_day_pct;
    let seven_day_pct_at_end = last.seven_day_pct;
    let seven_day_delta = (seven_day_pct_at_end - seven_day_pct_at_start).max(0.0);

    WindowSummary {
        window_start,
        window_end,
        peak_five_hour_pct,
        hit_100,
        time_to_100,
        seven_day_pct_at_start,
        seven_day_pct_at_end,
        seven_day_delta,
    }
}

/// Bucket records by hour-of-day (0–23) and return average 5-hour utilization per bucket.
/// Hours with no data return `None`.
pub fn daily_pattern(records: &[HistoryRecord]) -> [Option<f64>; 24] {
    let mut sums = [0.0_f64; 24];
    let mut counts = [0_usize; 24];

    for r in records {
        let hour = r.polled_at.hour() as usize;
        sums[hour] += r.five_hour_pct;
        counts[hour] += 1;
    }

    let mut result = [None; 24];
    for h in 0..24 {
        if counts[h] > 0 {
            result[h] = Some(sums[h] / counts[h] as f64);
        }
    }
    result
}

/// Predict how many more fully-maxed 5-hour windows remain before the 7-day limit.
///
/// Uses empirical average cost when ≥ 2 windows hit 100%; otherwise falls back
/// to the theoretical value (100 / 33.6 ≈ 2.976% per window).
pub fn predict_windows_remaining(
    records: &[HistoryRecord],
    current_seven_day_pct: f64,
) -> CrossLimitPrediction {
    const THEORETICAL_COST: f64 = 100.0 / 33.6; // ≈ 2.976%

    let windows = summarize_windows(records);
    let maxed: Vec<&WindowSummary> = windows.iter().filter(|w| w.hit_100).collect();

    let (avg_window_cost_pct, data_source) = if maxed.len() >= 2 {
        let avg = maxed.iter().map(|w| w.seven_day_delta).sum::<f64>() / maxed.len() as f64;
        // Guard against zero/negative avg (shouldn't happen with real data).
        let avg = if avg > 0.0 { avg } else { THEORETICAL_COST };
        (avg, PredictionSource::Empirical)
    } else {
        (THEORETICAL_COST, PredictionSource::Theoretical)
    };

    let remaining = (100.0 - current_seven_day_pct).max(0.0);
    let windows_remaining = remaining / avg_window_cost_pct;

    CrossLimitPrediction {
        current_seven_day_pct,
        avg_window_cost_pct,
        windows_remaining,
        data_source,
        windows_observed: maxed.len(),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn make_record(
        polled_at: DateTime<Utc>,
        five_hour_pct: f64,
        five_hour_resets_at: Option<&str>,
        seven_day_pct: f64,
    ) -> HistoryRecord {
        HistoryRecord {
            polled_at,
            five_hour_pct,
            five_hour_resets_at: five_hour_resets_at.map(|s| s.to_string()),
            seven_day_pct,
            seven_day_resets_at: None,
            five_hour_predicted_pct: None,
        }
    }

    // ── append / load round-trip ─────────────────────────────────────────────

    /// Helper: write records to a temp file and parse them back.
    fn write_and_read(records: &[HistoryRecord]) -> Vec<HistoryRecord> {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.jsonl");

        for rec in records {
            let line = serde_json::to_string(rec).unwrap();
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(file, "{}", line).unwrap();
        }

        parse_jsonl(&path).unwrap()
    }

    #[test]
    fn round_trip_single_record() {
        let now = Utc::now();
        let rec = make_record(now, 42.5, Some("2026-05-27T14:00:00Z"), 15.0);
        let loaded = write_and_read(&[rec.clone()]);
        assert_eq!(loaded.len(), 1);
        assert!((loaded[0].five_hour_pct - 42.5).abs() < 0.001);
        assert_eq!(loaded[0].seven_day_pct, 15.0);
        assert_eq!(
            loaded[0].five_hour_resets_at.as_deref(),
            Some("2026-05-27T14:00:00Z")
        );
    }

    #[test]
    fn empty_file_returns_empty_vec() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("empty.jsonl");
        std::fs::write(&path, "").unwrap();
        let loaded = parse_jsonl(&path).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn malformed_line_is_skipped() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("malformed.jsonl");
        let now = Utc::now();
        let good = serde_json::to_string(&make_record(now, 50.0, None, 10.0)).unwrap();
        std::fs::write(&path, format!("{}\nNOT JSON\n{}\n", good, good)).unwrap();
        let loaded = parse_jsonl(&path).unwrap();
        assert_eq!(loaded.len(), 2, "two good lines should be returned");
    }

    #[test]
    fn retention_trim_keeps_newest() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("trim.jsonl");

        // Write 10 records with increasing five_hour_pct values.
        for i in 0..10u32 {
            let rec = make_record(
                Utc::now(),
                i as f64,
                Some("2026-01-01T00:00:00Z"),
                0.0,
            );
            let line = serde_json::to_string(&rec).unwrap();
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "{}", line).unwrap();
        }

        // Trim to 5.
        trim_to_max_records(&path, 5).unwrap();

        let loaded = parse_jsonl(&path).unwrap();
        assert_eq!(loaded.len(), 5);
        // The five_hour_pct values should be 5,6,7,8,9 (the newest 5).
        let values: Vec<f64> = loaded.iter().map(|r| r.five_hour_pct).collect();
        assert_eq!(values, vec![5.0, 6.0, 7.0, 8.0, 9.0]);
    }

    #[test]
    fn trim_no_op_when_under_limit() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("no_trim.jsonl");

        for i in 0..3u32 {
            let rec = make_record(Utc::now(), i as f64, None, 0.0);
            let line = serde_json::to_string(&rec).unwrap();
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap();
            writeln!(f, "{}", line).unwrap();
        }

        trim_to_max_records(&path, 10).unwrap();
        let loaded = parse_jsonl(&path).unwrap();
        assert_eq!(loaded.len(), 3);
    }

    // ── summarize_windows ────────────────────────────────────────────────────

    fn ts(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 27, hour, 0, 0).unwrap()
    }

    #[test]
    fn summarize_single_window() {
        let records = vec![
            make_record(ts(10), 20.0, Some("2026-05-27T15:00:00Z"), 10.0),
            make_record(ts(11), 50.0, Some("2026-05-27T15:00:00Z"), 12.0),
            make_record(ts(12), 80.0, Some("2026-05-27T15:00:00Z"), 14.0),
        ];
        let windows = summarize_windows(&records);
        assert_eq!(windows.len(), 1);
        let w = &windows[0];
        assert!((w.peak_five_hour_pct - 80.0).abs() < 0.001);
        assert!(!w.hit_100);
        assert!((w.seven_day_delta - 4.0).abs() < 0.001);
    }

    #[test]
    fn summarize_two_windows() {
        let records = vec![
            make_record(ts(10), 80.0, Some("2026-05-27T15:00:00Z"), 10.0),
            make_record(ts(11), 100.0, Some("2026-05-27T15:00:00Z"), 12.0),
            // Window boundary: resets_at changes
            make_record(ts(16), 5.0, Some("2026-05-27T21:00:00Z"), 12.5),
            make_record(ts(17), 40.0, Some("2026-05-27T21:00:00Z"), 13.0),
        ];
        let windows = summarize_windows(&records);
        assert_eq!(windows.len(), 2);

        let w0 = &windows[0];
        assert!(w0.hit_100);
        assert!(w0.time_to_100.is_some());

        let w1 = &windows[1];
        assert!(!w1.hit_100);
        assert!((w1.seven_day_pct_at_start - 12.5).abs() < 0.001);
    }

    #[test]
    fn summarize_none_to_some_boundary() {
        // None → Some counts as a boundary.
        let records = vec![
            make_record(ts(10), 50.0, None, 10.0),
            make_record(ts(11), 60.0, Some("2026-05-27T16:00:00Z"), 11.0),
        ];
        let windows = summarize_windows(&records);
        assert_eq!(windows.len(), 2);
    }

    // ── daily_pattern ────────────────────────────────────────────────────────

    #[test]
    fn daily_pattern_single_hour() {
        let records = vec![
            make_record(ts(10), 40.0, None, 5.0),
            make_record(ts(10), 60.0, None, 5.0),
        ];
        let pattern = daily_pattern(&records);
        assert!((pattern[10].unwrap() - 50.0).abs() < 0.001);
        assert!(pattern[11].is_none());
    }

    #[test]
    fn daily_pattern_empty() {
        let pattern = daily_pattern(&[]);
        for h in 0..24 {
            assert!(pattern[h].is_none());
        }
    }

    // ── predict_windows_remaining ────────────────────────────────────────────

    #[test]
    fn predict_theoretical_with_no_history() {
        let pred = predict_windows_remaining(&[], 40.0);
        assert_eq!(pred.data_source, PredictionSource::Theoretical);
        assert!((pred.avg_window_cost_pct - 100.0 / 33.6).abs() < 0.01);
        // 60% remaining / 2.976% ≈ 20.16 windows
        assert!(pred.windows_remaining > 20.0 && pred.windows_remaining < 21.0);
    }

    #[test]
    fn predict_empirical_with_two_maxed_windows() {
        // Two windows both hitting 100%, each costing 3% of the 7-day limit.
        let records = vec![
            make_record(ts(10), 100.0, Some("T1"), 10.0),
            make_record(ts(11), 100.0, Some("T1"), 13.0), // window 1: delta = 3%
            make_record(ts(16), 100.0, Some("T2"), 13.0),
            make_record(ts(17), 100.0, Some("T2"), 16.0), // window 2: delta = 3%
        ];
        let pred = predict_windows_remaining(&records, 16.0);
        assert_eq!(pred.data_source, PredictionSource::Empirical);
        assert!((pred.avg_window_cost_pct - 3.0).abs() < 0.001);
        assert_eq!(pred.windows_observed, 2);
        // (100 - 16) / 3 = 28 windows
        assert!((pred.windows_remaining - 28.0).abs() < 0.01);
    }

    #[test]
    fn predict_theoretical_with_one_maxed_window() {
        // Only 1 maxed window → not enough for empirical, fall back.
        let records = vec![
            make_record(ts(10), 100.0, Some("T1"), 10.0),
            make_record(ts(11), 100.0, Some("T1"), 13.0),
        ];
        let pred = predict_windows_remaining(&records, 13.0);
        assert_eq!(pred.data_source, PredictionSource::Theoretical);
        assert_eq!(pred.windows_observed, 1);
    }
}
