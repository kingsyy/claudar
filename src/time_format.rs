use chrono::{DateTime, Local, TimeZone, Utc};
use chrono_tz::Tz;
use std::str::FromStr;

/// Parse a timezone string and convert a DateTime to that timezone
///
/// Supports:
/// - "local" - system local timezone
/// - "utc" or "UTC" - UTC timezone
/// - IANA timezone names like "America/New_York", "Europe/London", etc.
pub fn to_timezone(dt: &DateTime<impl TimeZone>, tz_str: &str) -> anyhow::Result<String> {
    let tz_lower = tz_str.to_lowercase();

    match tz_lower.as_str() {
        "local" => {
            // Convert to local timezone
            let local_dt: DateTime<Local> = dt.with_timezone(&Local);
            Ok(local_dt.to_rfc3339())
        }
        "utc" => {
            // Convert to UTC
            let utc_dt: DateTime<Utc> = dt.with_timezone(&Utc);
            Ok(utc_dt.to_rfc3339())
        }
        _ => {
            // Try to parse as IANA timezone
            let tz = Tz::from_str(tz_str)
                .map_err(|_| anyhow::anyhow!("Invalid timezone: '{}'. Use 'local', 'UTC', or a valid IANA timezone name (e.g., 'America/New_York', 'Europe/London')", tz_str))?;

            let converted_dt = dt.with_timezone(&tz);
            Ok(converted_dt.to_rfc3339())
        }
    }
}

/// Format a DateTime in 24-hour format with the specified timezone
///
/// Format: HH:MM (24-hour time)
pub fn format_time_24h(dt: &DateTime<impl TimeZone>, tz_str: &str) -> anyhow::Result<String> {
    let tz_lower = tz_str.to_lowercase();

    match tz_lower.as_str() {
        "local" => {
            let local_dt: DateTime<Local> = dt.with_timezone(&Local);
            Ok(local_dt.format("%H:%M").to_string())
        }
        "utc" => {
            let utc_dt: DateTime<Utc> = dt.with_timezone(&Utc);
            Ok(utc_dt.format("%H:%M").to_string())
        }
        _ => {
            let tz = Tz::from_str(tz_str)
                .map_err(|_| anyhow::anyhow!("Invalid timezone: '{}'. Use 'local', 'UTC', or a valid IANA timezone name", tz_str))?;

            let converted_dt = dt.with_timezone(&tz);
            Ok(converted_dt.format("%H:%M").to_string())
        }
    }
}

/// Format a DateTime with date and time in 24-hour format
///
/// Format: HH:MM DD/MM/YYYY
pub fn format_datetime_24h(dt: &DateTime<impl TimeZone>, tz_str: &str) -> anyhow::Result<String> {
    let tz_lower = tz_str.to_lowercase();

    match tz_lower.as_str() {
        "local" => {
            let local_dt: DateTime<Local> = dt.with_timezone(&Local);
            Ok(local_dt.format("%H:%M %d/%m/%Y").to_string())
        }
        "utc" => {
            let utc_dt: DateTime<Utc> = dt.with_timezone(&Utc);
            Ok(utc_dt.format("%H:%M %d/%m/%Y").to_string())
        }
        _ => {
            let tz = Tz::from_str(tz_str)
                .map_err(|_| anyhow::anyhow!("Invalid timezone: '{}'. Use 'local', 'UTC', or a valid IANA timezone name", tz_str))?;

            let converted_dt = dt.with_timezone(&tz);
            Ok(converted_dt.format("%H:%M %d/%m/%Y").to_string())
        }
    }
}

/// Format a reset time with date and time in 24-hour format
///
/// Format: MMM DD, HH:MM (e.g., "Dec 01, 15:45")
pub fn format_reset_time_24h(dt: &DateTime<impl TimeZone>, tz_str: &str) -> anyhow::Result<String> {
    let tz_lower = tz_str.to_lowercase();

    match tz_lower.as_str() {
        "local" => {
            let local_dt: DateTime<Local> = dt.with_timezone(&Local);
            Ok(local_dt.format("%b %d, %H:%M").to_string())
        }
        "utc" => {
            let utc_dt: DateTime<Utc> = dt.with_timezone(&Utc);
            Ok(utc_dt.format("%b %d, %H:%M").to_string())
        }
        _ => {
            let tz = Tz::from_str(tz_str)
                .map_err(|_| anyhow::anyhow!("Invalid timezone: '{}'. Use 'local', 'UTC', or a valid IANA timezone name", tz_str))?;

            let converted_dt = dt.with_timezone(&tz);
            Ok(converted_dt.format("%b %d, %H:%M").to_string())
        }
    }
}

/// Format a notification time in 24-hour format
///
/// Format: HH:MM DD-MM-YYYY
pub fn format_notification_time_24h(dt: &DateTime<impl TimeZone>, tz_str: &str) -> anyhow::Result<String> {
    let tz_lower = tz_str.to_lowercase();

    match tz_lower.as_str() {
        "local" => {
            let local_dt: DateTime<Local> = dt.with_timezone(&Local);
            Ok(local_dt.format("%H:%M %d-%m-%Y").to_string())
        }
        "utc" => {
            let utc_dt: DateTime<Utc> = dt.with_timezone(&Utc);
            Ok(utc_dt.format("%H:%M %d-%m-%Y").to_string())
        }
        _ => {
            let tz = Tz::from_str(tz_str)
                .map_err(|_| anyhow::anyhow!("Invalid timezone: '{}'. Use 'local', 'UTC', or a valid IANA timezone name", tz_str))?;

            let converted_dt = dt.with_timezone(&tz);
            Ok(converted_dt.format("%H:%M %d-%m-%Y").to_string())
        }
    }
}

/// Validate a timezone string
pub fn validate_timezone(tz_str: &str) -> anyhow::Result<()> {
    let tz_lower = tz_str.to_lowercase();

    match tz_lower.as_str() {
        "local" | "utc" => Ok(()),
        _ => {
            // Try to parse as IANA timezone
            Tz::from_str(tz_str)
                .map(|_| ())
                .map_err(|_| {
                    anyhow::anyhow!(
                        "Invalid timezone: '{}'\n\nValid options:\n  - 'local' (system timezone)\n  - 'UTC' (Coordinated Universal Time)\n  - IANA timezone name (e.g., 'America/New_York', 'Europe/London', 'Asia/Tokyo')\n\nSee https://en.wikipedia.org/wiki/List_of_tz_database_time_zones for a full list.",
                        tz_str
                    )
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_validate_timezone() {
        assert!(validate_timezone("local").is_ok());
        assert!(validate_timezone("LOCAL").is_ok());
        assert!(validate_timezone("utc").is_ok());
        assert!(validate_timezone("UTC").is_ok());
        assert!(validate_timezone("America/New_York").is_ok());
        assert!(validate_timezone("Europe/London").is_ok());
        assert!(validate_timezone("Asia/Tokyo").is_ok());
        assert!(validate_timezone("invalid_timezone").is_err());
    }

    #[test]
    fn test_format_time_24h() {
        let dt = Utc::now();

        // Should not panic for valid timezones
        assert!(format_time_24h(&dt, "local").is_ok());
        assert!(format_time_24h(&dt, "UTC").is_ok());
        assert!(format_time_24h(&dt, "America/New_York").is_ok());

        // Should error for invalid timezone
        assert!(format_time_24h(&dt, "invalid").is_err());
    }
}
