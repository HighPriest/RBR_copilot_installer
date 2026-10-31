//! UTC dates as text, with Howard Hinnant's civil-from-days arithmetic,
//! hand-rolled to spare a date crate.

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// `2026-10-25T18-15-00Z`: sortable, and without ISO 8601's colons, which
/// Windows forbids in a file or directory name.
pub fn file_stamp(unix_seconds: i64) -> String {
    let civil = Civil::of(unix_seconds);
    format!(
        "{}T{:02}-{:02}-{:02}Z",
        civil.date(),
        civil.hours,
        civil.minutes,
        civil.seconds
    )
}

/// `2026-10-25T18:15:00Z`, the shape the server's log writes, so the two line up.
pub fn iso_stamp(unix_seconds: i64) -> String {
    let civil = Civil::of(unix_seconds);
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        civil.date(),
        civil.hours,
        civil.minutes,
        civil.seconds
    )
}

struct Civil {
    year: i64,
    month: i64,
    day: i64,
    hours: i64,
    minutes: i64,
    seconds: i64,
}

impl Civil {
    fn of(unix_seconds: i64) -> Civil {
        let days = unix_seconds.div_euclid(86_400);
        let of_day = unix_seconds.rem_euclid(86_400);

        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        Civil {
            year: yoe + era * 400 + i64::from(month <= 2),
            month,
            day: doy - (153 * mp + 2) / 5 + 1,
            hours: of_day / 3600,
            minutes: (of_day % 3600) / 60,
            seconds: of_day % 60,
        }
    }

    fn date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(test)]
mod tests {
    /// Expected values come from .NET's `DateTimeOffset.FromUnixTimeSeconds`,
    /// since a hand-worked expectation passes however broken the code is.
    #[test]
    fn stamps_match_the_utc_calendar() {
        assert_eq!(super::file_stamp(0), "1970-01-01T00-00-00Z");
        assert_eq!(super::file_stamp(1_761_000_000), "2025-10-20T22-40-00Z");
        assert_eq!(super::file_stamp(1_709_164_800), "2024-02-29T00-00-00Z");
        assert_eq!(super::iso_stamp(1_761_000_000), "2025-10-20T22:40:00Z");
        assert_eq!(super::iso_stamp(1_709_164_799), "2024-02-28T23:59:59Z");
    }

    #[test]
    fn file_stamps_sort_chronologically_and_are_legal_on_windows() {
        let earlier = super::file_stamp(1_761_000_000);
        let later = super::file_stamp(1_761_000_001);
        assert!(earlier < later);
        for forbidden in [':', '/', '\\', '*', '?', '"', '<', '>', '|'] {
            assert!(!later.contains(forbidden), "{later}");
        }
    }
}
