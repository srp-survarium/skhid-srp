//! Unix times as UTC calendar text.

/// `YYYY-MM-DD` (UTC) of a Unix time; [`clock`] is its time of day.
pub fn date(at: u64) -> String {
    let (year, month, day) = civil_from_days(at / 86_400);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `HH:MM` (UTC) of a Unix time.
pub fn clock(at: u64) -> String {
    let seconds = at % 86_400;
    format!("{:02}:{:02}", seconds / 3600, seconds / 60 % 60)
}

/// The date of a day counted from 1970-01-01 (the proleptic Gregorian
/// calendar; Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_utc_calendar_dates() {
        assert_eq!(date(0), "1970-01-01");
        assert_eq!(date(951_782_400), "2000-02-29");
        // 2026-10-18 12:34 UTC.
        let at = 1_792_326_840;
        assert_eq!((date(at), clock(at)), ("2026-10-18".into(), "12:34".into()));
    }
}
