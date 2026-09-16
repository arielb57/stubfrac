use crate::{Date, Error};

/// A coupon schedule reduced to what day counts need: a frequency and one
/// anchor date (usually maturity, or the first/last regular coupon date).
///
/// Roll date `k` is computed directly as `anchor + k * (12 / frequency)`
/// months for any integer `k`, never by stepping from a previous roll date.
/// Rolling backward from maturity and forward from an issue-side anchor
/// therefore yield the same grid whenever they share an anchor, and a day-31
/// anchor does not decay to 30 after passing through a 30-day month.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Schedule {
    frequency: u32,
    anchor: Date,
    end_of_month: bool,
}

/// The overlap of an accrual period with one notional reference period.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Piece {
    pub period_start: Date,
    pub period_end: Date,
    pub from: Date,
    pub to: Date,
}

impl Piece {
    pub fn days(&self) -> i64 {
        self.from.days_until(self.to)
    }

    pub fn period_days(&self) -> i64 {
        self.period_start.days_until(self.period_end)
    }

    pub fn is_full_period(&self) -> bool {
        self.from == self.period_start && self.to == self.period_end
    }
}

impl Schedule {
    /// `frequency` is coupons per year and must divide 12. With
    /// `end_of_month`, an anchor on a month end makes every roll date a month
    /// end; otherwise roll dates keep the anchor's day, clamped to the month.
    pub fn new(frequency: u32, anchor: Date, end_of_month: bool) -> Result<Schedule, Error> {
        if !matches!(frequency, 1 | 2 | 3 | 4 | 6 | 12) {
            return Err(Error::InvalidFrequency(frequency));
        }
        Ok(Schedule {
            frequency,
            anchor,
            end_of_month,
        })
    }

    pub fn frequency(&self) -> u32 {
        self.frequency
    }

    pub fn anchor(&self) -> Date {
        self.anchor
    }

    /// True when roll dates are forced to month ends.
    pub fn rolls_on_month_end(&self) -> bool {
        self.end_of_month && self.anchor.is_last_day_of_month()
    }

    fn months(&self) -> i64 {
        12 / self.frequency as i64
    }

    pub fn roll_date(&self, k: i64) -> Date {
        Date::from_month_index(
            self.anchor.month_index() + k * self.months(),
            self.anchor.day(),
            self.rolls_on_month_end(),
        )
    }

    /// The `k` with `roll_date(k) <= date < roll_date(k + 1)`.
    pub fn period_index(&self, date: Date) -> i64 {
        let mut k = (date.month_index() - self.anchor.month_index()).div_euclid(self.months());
        while self.roll_date(k) > date {
            k -= 1;
        }
        while self.roll_date(k + 1) <= date {
            k += 1;
        }
        k
    }

    pub fn is_roll_date(&self, date: Date) -> bool {
        self.roll_date(self.period_index(date)) == date
    }

    /// Splits `[start, end)` into its overlaps with consecutive reference
    /// periods. Empty when `start >= end`.
    pub fn pieces(&self, start: Date, end: Date) -> Pieces<'_> {
        Pieces {
            schedule: self,
            k: self.period_index(start),
            cursor: start,
            end,
        }
    }
}

pub struct Pieces<'a> {
    schedule: &'a Schedule,
    k: i64,
    cursor: Date,
    end: Date,
}

impl Iterator for Pieces<'_> {
    type Item = Piece;

    fn next(&mut self) -> Option<Piece> {
        if self.cursor >= self.end {
            return None;
        }
        let period_start = self.schedule.roll_date(self.k);
        let period_end = self.schedule.roll_date(self.k + 1);
        let to = period_end.min(self.end);
        let piece = Piece {
            period_start,
            period_end,
            from: self.cursor,
            to,
        };
        self.cursor = to;
        self.k += 1;
        Some(piece)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    #[test]
    fn rejects_frequencies_that_do_not_divide_a_year() {
        for f in [0, 5, 7, 24] {
            assert_eq!(
                Schedule::new(f, d("2020-01-01"), false),
                Err(Error::InvalidFrequency(f))
            );
        }
    }

    #[test]
    fn day_31_anchor_does_not_decay() {
        let s = Schedule::new(4, d("2021-08-31"), false).unwrap();
        assert_eq!(s.roll_date(-1), d("2021-05-31"));
        assert_eq!(s.roll_date(-2), d("2021-02-28"));
        assert_eq!(s.roll_date(-3), d("2020-11-30"));
        assert_eq!(s.roll_date(-4), d("2020-08-31"));
    }

    #[test]
    fn end_of_month_flag_moves_feb_anchor_to_month_ends() {
        let plain = Schedule::new(2, d("2023-02-28"), false).unwrap();
        let eom = Schedule::new(2, d("2023-02-28"), true).unwrap();
        assert_eq!(plain.roll_date(1), d("2023-08-28"));
        assert_eq!(eom.roll_date(1), d("2023-08-31"));
        assert_eq!(eom.roll_date(2), d("2024-02-29"));
        // A mid-month anchor ignores the flag.
        let mid = Schedule::new(2, d("2023-02-15"), true).unwrap();
        assert!(!mid.rolls_on_month_end());
        assert_eq!(mid.roll_date(1), d("2023-08-15"));
    }

    #[test]
    fn period_index_brackets_every_date() {
        let s = Schedule::new(2, d("2003-07-15"), false).unwrap();
        let mut date = d("1999-01-01");
        while date < d("2006-01-01") {
            let k = s.period_index(date);
            assert!(
                s.roll_date(k) <= date && date < s.roll_date(k + 1),
                "{date}"
            );
            date = date.add_days(1).unwrap();
        }
    }

    #[test]
    fn pieces_cover_the_accrual_exactly() {
        let s = Schedule::new(2, d("2003-07-15"), false).unwrap();
        let pieces: Vec<Piece> = s.pieces(d("2002-08-15"), d("2003-07-15")).collect();
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].period_start, d("2002-07-15"));
        assert_eq!(pieces[0].period_end, d("2003-01-15"));
        assert_eq!((pieces[0].days(), pieces[0].period_days()), (153, 184));
        assert!(pieces[1].is_full_period());
        assert_eq!(s.pieces(d("2003-07-15"), d("2003-07-15")).count(), 0);
    }
}
