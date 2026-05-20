// SPDX-License-Identifier: BUSL-1.1

//! Pure timezone-aware UTC time computation for post scheduling.

use crate::app_state::DefaultPostTime;
use chrono::{Datelike, TimeZone};

/// Compute a UTC RFC 3339 schedule string for the given default post time and
/// timezone. If the target time has already passed today in the configured
/// timezone, the next occurrence (tomorrow) is returned.
///
/// Pass `now` explicitly so callers (and tests) can control the reference instant.
pub fn compute_schedule_utc(
    dpt: &DefaultPostTime,
    timezone: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<String, String> {
    let tz: chrono_tz::Tz = if timezone.is_empty() {
        chrono_tz::UTC
    } else {
        timezone.parse().map_err(|_| format!("Unknown timezone: '{}'", timezone))?
    };

    let now_local = now.with_timezone(&tz);
    let today = now_local.date_naive();
    let target_utc = schedule_on_date_utc(&tz, today, dpt)?;

    if target_utc > now {
        Ok(target_utc.to_rfc3339())
    } else {
        let tomorrow = today + chrono::Duration::days(1);
        Ok(schedule_on_date_utc(&tz, tomorrow, dpt)?.to_rfc3339())
    }
}

fn schedule_on_date_utc(
    tz: &chrono_tz::Tz,
    date: chrono::NaiveDate,
    dpt: &DefaultPostTime,
) -> Result<chrono::DateTime<chrono::Utc>, String> {
    tz.with_ymd_and_hms(
        date.year(), date.month(), date.day(),
        u32::from(dpt.hour), u32::from(dpt.minute), 0,
    )
    .single()
    .ok_or_else(|| format!("Ambiguous or invalid local time on {}", date))
    .map(|dt| dt.with_timezone(&chrono::Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn utc(y: i32, mo: u32, d: u32, h: u32, min: u32) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc.with_ymd_and_hms(y, mo, d, h, min, 0).unwrap()
    }

    #[test]
    fn test_schedule_utc_returns_today_when_time_not_yet_passed() {
        let dpt = DefaultPostTime { hour: 9, minute: 30, timezone: String::new() };
        let result = compute_schedule_utc(&dpt, "UTC", utc(2026, 5, 5, 8, 0)).unwrap();
        assert!(result.starts_with("2026-05-05T09:30:00"), "got: {}", result);
    }

    #[test]
    fn test_schedule_utc_returns_tomorrow_when_time_has_passed() {
        let dpt = DefaultPostTime { hour: 9, minute: 30, timezone: String::new() };
        let result = compute_schedule_utc(&dpt, "UTC", utc(2026, 5, 5, 10, 0)).unwrap();
        assert!(result.starts_with("2026-05-06T09:30:00"), "got: {}", result);
    }

    #[test]
    fn test_schedule_utc_europe_london_converts_correctly() {
        let dpt = DefaultPostTime { hour: 9, minute: 30, timezone: String::new() };
        let result = compute_schedule_utc(&dpt, "Europe/London", utc(2026, 5, 5, 8, 0)).unwrap();
        assert!(result.starts_with("2026-05-05T08:30:00"), "got: {}", result);
    }

    #[test]
    fn test_schedule_utc_europe_london_tomorrow_when_passed() {
        let dpt = DefaultPostTime { hour: 9, minute: 30, timezone: String::new() };
        let result = compute_schedule_utc(&dpt, "Europe/London", utc(2026, 5, 5, 10, 0)).unwrap();
        assert!(result.starts_with("2026-05-06T08:30:00"), "got: {}", result);
    }

    #[test]
    fn test_schedule_utc_empty_timezone_defaults_to_utc() {
        let dpt = DefaultPostTime { hour: 9, minute: 30, timezone: String::new() };
        let result = compute_schedule_utc(&dpt, "", utc(2026, 5, 5, 8, 0)).unwrap();
        assert!(result.starts_with("2026-05-05T09:30:00"), "got: {}", result);
    }

    #[test]
    fn test_schedule_utc_invalid_timezone_returns_error() {
        let dpt = DefaultPostTime { hour: 9, minute: 30, timezone: String::new() };
        let result = compute_schedule_utc(&dpt, "Not/A/Timezone", utc(2026, 5, 5, 8, 0));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown timezone"));
    }
}
