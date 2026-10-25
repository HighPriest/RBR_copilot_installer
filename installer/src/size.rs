//! Byte counts as a person reads them, in binary units with the short labels,
//! so they match what Explorer says about the same file.

const KB: u64 = 1024;
const MB: u64 = 1024 * KB;
const GB: u64 = 1024 * MB;

/// `0 B`, `997 B`, `12.1 kB`, `41.6 MB`, `1.4 GB`: one decimal from kB up, in
/// integers, since one `{:.1}` on an `f64` links ~16 kB of float formatting.
pub fn human(bytes: u64) -> String {
    let (divisor, unit) = unit_for(bytes);
    if divisor == 1 {
        return format!("{bytes} {unit}");
    }
    let (whole, tenth) = tenths(bytes, divisor);
    format!("{whole}.{tenth} {unit}")
}

/// `12.4 / 41.6 MB`, both in the total's unit so they compare at a glance;
/// an unknown (zero) total shows only what arrived.
pub fn human_progress(done: u64, total: u64) -> String {
    if total == 0 {
        return human(done);
    }
    let (divisor, unit) = unit_for(total);
    if divisor == 1 {
        return format!("{done} / {total} {unit}");
    }
    let (done_whole, done_tenth) = tenths(done, divisor);
    let (total_whole, total_tenth) = tenths(total, divisor);
    format!("{done_whole}.{done_tenth} / {total_whole}.{total_tenth} {unit}")
}

/// `bytes / divisor` as a whole part and one decimal digit, rounded half up as `{:.1}` did.
fn tenths(bytes: u64, divisor: u64) -> (u64, u64) {
    let scaled = (bytes * 10 + divisor / 2) / divisor;
    (scaled / 10, scaled % 10)
}

fn unit_for(bytes: u64) -> (u64, &'static str) {
    match bytes {
        0..KB => (1, "B"),
        KB..MB => (KB, "kB"),
        MB..GB => (MB, "MB"),
        _ => (GB, "GB"),
    }
}

#[cfg(test)]
mod tests {
    /// 41.6 MB is what Explorer shows for the live catalogue's largest package.
    #[test]
    fn a_real_package_reads_as_explorer_shows_it() {
        assert_eq!(super::human(43_609_253), "41.6 MB");
    }

    #[test]
    fn a_small_package_is_counted_in_bytes_and_kb() {
        assert_eq!(super::human(5_861), "5.7 kB");
        assert_eq!(super::human(997), "997 B");
        assert_eq!(super::human(0), "0 B");
    }

    #[test]
    fn the_unit_turns_over_at_the_boundary() {
        assert_eq!(super::human(1023), "1023 B");
        assert_eq!(super::human(1024), "1.0 kB");
        assert_eq!(super::human(1024 * 1024 - 1), "1024.0 kB");
        assert_eq!(super::human(1024 * 1024), "1.0 MB");
        assert_eq!(super::human(1024 * 1024 * 1024), "1.0 GB");
    }

    /// Per-number units would show "996.0 kB / 1.0 MB" and read as no progress.
    #[test]
    fn a_transfer_shows_both_numbers_in_the_total_s_unit() {
        assert_eq!(
            super::human_progress(13_000_000, 43_609_253),
            "12.4 / 41.6 MB"
        );
        assert_eq!(super::human_progress(1_020_000, 1_048_576), "1.0 / 1.0 MB");
        assert_eq!(super::human_progress(500, 5_861), "0.5 / 5.7 kB");
    }

    #[test]
    fn an_unknown_total_still_reports_what_arrived() {
        assert_eq!(super::human_progress(43_609_253, 0), "41.6 MB");
    }
}
