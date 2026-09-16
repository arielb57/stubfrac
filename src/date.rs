use std::fmt;
use std::str::FromStr;

use crate::Error;

/// A proleptic Gregorian calendar date, years 1 to 9999.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Date {
    year: i32,
    month: u8,
    day: u8,
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap_year(year) => 29,
        _ => 28,
    }
}

impl Date {
    pub fn new(year: i32, month: u8, day: u8) -> Result<Date, Error> {
        if !(1..=9999).contains(&year)
            || !(1..=12).contains(&month)
            || day == 0
            || day > days_in_month(year, month)
        {
            return Err(Error::InvalidDate(format!("{year:04}-{month:02}-{day:02}")));
        }
        Ok(Date { year, month, day })
    }

    /// Builds a date from a year and a zero-based month count that may overflow
    /// into later or earlier years, clamping the day to the month's length.
    /// Used for schedule roll dates, which may fall just outside 1..=9999.
    pub(crate) fn from_month_index(month_index: i64, day: u8, end_of_month: bool) -> Date {
        let year = month_index.div_euclid(12) as i32;
        let month = (month_index.rem_euclid(12) + 1) as u8;
        let dim = days_in_month(year, month);
        let day = if end_of_month { dim } else { day.min(dim) };
        Date { year, month, day }
    }

    pub fn year(self) -> i32 {
        self.year
    }

    pub fn month(self) -> u8 {
        self.month
    }

    pub fn day(self) -> u8 {
        self.day
    }

    pub(crate) fn month_index(self) -> i64 {
        self.year as i64 * 12 + (self.month as i64 - 1)
    }

    pub fn is_last_day_of_month(self) -> bool {
        self.day == days_in_month(self.year, self.month)
    }

    pub fn is_last_day_of_february(self) -> bool {
        self.month == 2 && self.is_last_day_of_month()
    }

    /// Days since 1970-01-01 (Howard Hinnant's days_from_civil).
    pub fn serial(self) -> i64 {
        let y = self.year as i64 - if self.month <= 2 { 1 } else { 0 };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = self.month as i64;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }

    /// Inverse of [`Date::serial`]. Returns an error outside years 1..=9999.
    pub fn from_serial(serial: i64) -> Result<Date, Error> {
        let z = serial + 719468;
        let era = z.div_euclid(146097);
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
        if !(1..=9999).contains(&year) {
            return Err(Error::InvalidDate(format!("serial {serial}")));
        }
        Date::new(year as i32, month, day)
    }

    pub fn days_until(self, later: Date) -> i64 {
        later.serial() - self.serial()
    }

    pub fn add_days(self, days: i64) -> Result<Date, Error> {
        Date::from_serial(self.serial() + days)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl FromStr for Date {
    type Err = Error;

    /// Parses `YYYY-MM-DD`.
    fn from_str(s: &str) -> Result<Date, Error> {
        let bad = || Error::InvalidDate(s.to_string());
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3
            || parts[0].len() != 4
            || parts[1].len() != 2
            || parts[2].len() != 2
            || !parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(bad());
        }
        let year = parts[0].parse().map_err(|_| bad())?;
        let month = parts[1].parse().map_err(|_| bad())?;
        let day = parts[2].parse().map_err(|_| bad())?;
        Date::new(year, month, day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_round_trips_across_centuries() {
        let start = Date::new(1600, 1, 1).unwrap().serial();
        let end = Date::new(2400, 12, 31).unwrap().serial();
        let mut prev: Option<Date> = None;
        for s in start..=end {
            let d = Date::from_serial(s).unwrap();
            assert_eq!(d.serial(), s);
            if let Some(p) = prev {
                assert!(p < d);
            }
            prev = Some(d);
        }
    }

    #[test]
    fn known_serials() {
        assert_eq!(Date::new(1970, 1, 1).unwrap().serial(), 0);
        assert_eq!(Date::new(2000, 3, 1).unwrap().serial(), 11017);
        assert_eq!(
            Date::new(2004, 1, 1)
                .unwrap()
                .days_until(Date::new(2004, 5, 1).unwrap()),
            121
        );
    }

    #[test]
    fn leap_rules() {
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(1900));
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(2023));
        assert!(Date::new(2023, 2, 29).is_err());
        assert!(Date::new(2024, 2, 29).unwrap().is_last_day_of_february());
        assert!(!Date::new(2024, 2, 28).unwrap().is_last_day_of_february());
    }

    #[test]
    fn parsing_rejects_malformed_input() {
        assert_eq!(
            "2003-07-15".parse::<Date>().unwrap(),
            Date::new(2003, 7, 15).unwrap()
        );
        for bad in [
            "2003-7-15",
            "2003-07-32",
            "20030715",
            "2003-13-01",
            "abcd-01-01",
            "0000-01-01",
            "+003-01-01",
        ] {
            assert!(bad.parse::<Date>().is_err(), "{bad} should be rejected");
        }
    }
}
