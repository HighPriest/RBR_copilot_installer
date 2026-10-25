/// The UTC calendar date of a unix timestamp, by Howard Hinnant's
/// `civil_from_days`, to spare a date crate.
pub fn utc_date(unix: i64) -> (i64, u32, u32) {
    let z = unix.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// `conflicts-YYYY-MM-DD.jsonl` for the day `unix` falls on; ISO dates sort as
/// text, which the overlap summary relies on.
pub fn file_name(unix: i64) -> String {
    let (year, month, day) = utc_date(unix);
    format!("conflicts-{year:04}-{month:02}-{day:02}.jsonl")
}

#[cfg(test)]
mod tests {
    #[test]
    fn utc_dates_match_known_values() {
        assert_eq!(super::utc_date(0), (1970, 1, 1));
        assert_eq!(super::utc_date(951_782_400), (2000, 2, 29));
        assert_eq!(super::utc_date(1_792_886_400), (2026, 10, 25));
        assert_eq!(super::utc_date(1_792_886_400 + 86_399), (2026, 10, 25));
        assert_eq!(super::utc_date(4_107_542_400), (2100, 3, 1));
        assert_eq!(super::utc_date(-86_400), (1969, 12, 31));
    }

    #[test]
    fn a_days_reports_share_one_file() {
        assert_eq!(
            super::file_name(1_792_886_400),
            "conflicts-2026-10-25.jsonl"
        );
    }
}
