use std::fmt;
use std::str::FromStr;

use crate::date::is_leap_year;
use crate::{Date, Error, Rational, Schedule};

macro_rules! branches {
    ($($variant:ident => $label:expr,)*) => {
        /// One rule branch a day-count calculation can take.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        #[repr(u8)]
        pub enum Branch { $($variant,)* }

        impl Branch {
            pub const ALL: &'static [Branch] = &[$(Branch::$variant,)*];

            pub fn label(self) -> &'static str {
                match self { $(Branch::$variant => $label,)* }
            }
        }
    };
}

branches! {
    EmptyPeriod => "start equals end: zero",
    ActualOver360 => "actual days / 360",
    ActualOver365 => "actual days / 365",
    ThirtyDayMonths => "30-day months: 360*dY + 30*dM + dD, / 360",
    LeapAnnualFeb29 => "annual: Feb 29 in (start, end], denominator 366",
    LeapAnnualNoFeb29 => "annual: no Feb 29 in (start, end], denominator 365",
    LeapEndInLeapYear => "end date in a leap year, denominator 366",
    LeapEndInCommonYear => "end date in a common year, denominator 365",
    NoLeapDayRemoved => "Feb 29 removed from the day count",
    IsdaOneCalendarYear => "period inside one calendar year",
    IsdaSplitAtYearEnd => "period split at year boundaries",
    IsdaLeapYearDays => "days in a leap year counted /366",
    IsdaCommonYearDays => "days in a common year counted /365",
    AfbWholeYears => "whole years counted back from the end date",
    AfbFeb28BackToFeb29 => "counting back from Feb 28 landed on Feb 29",
    AfbFeb29InRemainder => "Feb 29 in the remaining period, denominator 366",
    AfbNoFeb29InRemainder => "no Feb 29 in the remaining period, denominator 365",
    RegularPeriod => "exactly one regular reference period",
    WholeRegularPeriods => "several whole regular reference periods",
    ShortStub => "stub inside a single reference period",
    LongStub => "stub spanning several reference periods",
    StartOffSchedule => "start date is not a roll date",
    EndOffSchedule => "end date is not a roll date",
    EndOfMonthRoll => "roll dates forced to month ends",
    FullReferencePeriod => "full reference period: 1/f",
    IcmaPartialPiece => "partial piece: days / (f * reference period days)",
    CanadianShortPiece => "piece under 365/f days: days/365",
    CanadianLongPiece => "partial piece of at least 365/f days: 1/f - (period days - days)/365",
    D1Is31To30 => "D1 = 31 -> 30",
    D1FebEndTo30 => "D1 last day of Feb -> 30",
    D1FebEndKept => "D1 last day of Feb kept",
    D2Is31To30 => "D2 = 31 -> 30",
    D2Is31To30SinceD1AtLeast30 => "D2 = 31 and D1 is 30 or 31 -> 30",
    D2Is31KeptSinceD1Below30 => "D2 = 31 kept because D1 < 30",
    D2FebEndTo30NotTermination => "D2 last day of Feb, not the termination date -> 30",
    D2FebEndIsTermination => "D2 last day of Feb is the termination date: kept",
    D2FebEndTo30SinceD1FebEnd => "D1 and D2 both last day of Feb -> D2 = 30",
    D2FebEndKept => "D2 last day of Feb kept",
}

impl fmt::Display for Branch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// The set of rule branches taken by one calculation, stored as a bitset so
/// that enumerating hundreds of millions of periods never allocates.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug, PartialOrd, Ord)]
pub struct Trace(u64);

impl Trace {
    pub fn insert(&mut self, branch: Branch) {
        self.0 |= 1 << branch as u8;
    }

    pub fn contains(self, branch: Branch) -> bool {
        self.0 & (1 << branch as u8) != 0
    }

    pub fn difference(self, other: Trace) -> Trace {
        Trace(self.0 & !other.0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn branches(self) -> impl Iterator<Item = Branch> {
        Branch::ALL
            .iter()
            .copied()
            .filter(move |b| self.contains(*b))
    }
}

impl fmt::Display for Trace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return f.write_str("(none)");
        }
        let labels: Vec<&str> = self.branches().map(Branch::label).collect();
        f.write_str(&labels.join("; "))
    }
}

macro_rules! conventions {
    ($($variant:ident => ($id:expr, $name:expr),)*) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub enum Convention { $($variant,)* }

        impl Convention {
            pub const ALL: &'static [Convention] = &[$(Convention::$variant,)*];

            /// Short identifier used on the command line.
            pub fn id(self) -> &'static str {
                match self { $(Convention::$variant => $id,)* }
            }

            pub fn name(self) -> &'static str {
                match self { $(Convention::$variant => $name,)* }
            }
        }
    };
}

conventions! {
    Act360 => ("act360", "ACT/360"),
    Act365Fixed => ("act365f", "ACT/365F"),
    Act365Leap => ("act365l", "ACT/365L"),
    NoLeap365 => ("nl365", "NL/365"),
    ActActIsda => ("actact-isda", "ACT/ACT ISDA"),
    ActActIcma => ("actact-icma", "ACT/ACT ICMA"),
    ActActAfb => ("actact-afb", "ACT/ACT AFB"),
    Thirty360Bond => ("30-360", "30/360 Bond Basis"),
    Thirty360Us => ("30u-360", "30U/360 (SIA)"),
    Thirty360European => ("30e-360", "30E/360"),
    Thirty360EuropeanIsda => ("30e-360-isda", "30E/360 ISDA"),
    Act365Canadian => ("act365-canadian", "ACT/365 Canadian Bond"),
}

impl Convention {
    /// What `Context` fields this convention reads.
    pub fn requirements(self) -> &'static str {
        match self {
            Convention::Act365Leap => "--freq",
            Convention::ActActIcma | Convention::Act365Canadian => "--freq --anchor [--eom]",
            Convention::Thirty360EuropeanIsda => "--termination",
            _ => "",
        }
    }
}

impl fmt::Display for Convention {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Convention {
    type Err = Error;

    fn from_str(s: &str) -> Result<Convention, Error> {
        Convention::ALL
            .iter()
            .copied()
            .find(|c| c.id().eq_ignore_ascii_case(s) || c.name().eq_ignore_ascii_case(s))
            .ok_or_else(|| Error::UnknownConvention(s.to_string()))
    }
}

/// Deal terms some conventions need beyond the two accrual dates.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Context {
    /// Coupons per year (ACT/365L, ACT/ACT ICMA, Canadian).
    pub frequency: Option<u32>,
    /// A roll date of the coupon schedule, typically maturity (ICMA, Canadian).
    pub anchor: Option<Date>,
    /// Roll on month ends when the anchor is a month end (ICMA, Canadian).
    pub end_of_month: bool,
    /// The deal's termination date (30E/360 ISDA).
    pub termination: Option<Date>,
}

impl Context {
    pub fn schedule(&self, convention: Convention) -> Result<Schedule, Error> {
        let frequency = self.frequency.ok_or(Error::MissingFrequency(convention))?;
        let anchor = self.anchor.ok_or(Error::MissingAnchor(convention))?;
        Schedule::new(frequency, anchor, self.end_of_month)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct YearFraction {
    pub value: Rational,
    pub trace: Trace,
}

/// Year fraction for the accrual period from `start` to `end`.
pub fn year_fraction(
    convention: Convention,
    start: Date,
    end: Date,
    context: &Context,
) -> Result<YearFraction, Error> {
    if start > end {
        return Err(Error::StartAfterEnd { start, end });
    }
    // Validate requirements even for empty periods, so a missing input is
    // reported regardless of the dates.
    let schedule = match convention {
        Convention::ActActIcma | Convention::Act365Canadian => Some(context.schedule(convention)?),
        _ => None,
    };
    match convention {
        Convention::Act365Leap if context.frequency.is_none() => {
            return Err(Error::MissingFrequency(convention))
        }
        Convention::Thirty360EuropeanIsda if context.termination.is_none() => {
            return Err(Error::MissingTermination)
        }
        _ => {}
    }
    let mut trace = Trace::default();
    if start == end {
        trace.insert(Branch::EmptyPeriod);
        return Ok(YearFraction {
            value: Rational::ZERO,
            trace,
        });
    }
    let days = start.days_until(end);
    let value = match convention {
        Convention::Act360 => {
            trace.insert(Branch::ActualOver360);
            Rational::new(days, 360)?
        }
        Convention::Act365Fixed => {
            trace.insert(Branch::ActualOver365);
            Rational::new(days, 365)?
        }
        Convention::Act365Leap => act_365_leap(start, end, days, context, &mut trace)?,
        Convention::NoLeap365 => {
            trace.insert(Branch::ActualOver365);
            let leap_days = feb29_count_through(end) - feb29_count_through(start);
            if leap_days > 0 {
                trace.insert(Branch::NoLeapDayRemoved);
            }
            Rational::new(days - leap_days, 365)?
        }
        Convention::ActActIsda => act_act_isda(start, end, &mut trace)?,
        Convention::ActActAfb => act_act_afb(start, end, &mut trace)?,
        Convention::ActActIcma => {
            let schedule = schedule.expect("schedule built above");
            act_act_icma(start, end, &schedule, &mut trace)?
        }
        Convention::Act365Canadian => {
            let schedule = schedule.expect("schedule built above");
            canadian(start, end, &schedule, &mut trace)?
        }
        Convention::Thirty360Bond
        | Convention::Thirty360Us
        | Convention::Thirty360European
        | Convention::Thirty360EuropeanIsda => {
            thirty_360(convention, start, end, context.termination, &mut trace)?
        }
    };
    Ok(YearFraction { value, trace })
}

/// Number of Feb 29 dates on or before `date` (from year 1).
fn feb29_count_through(date: Date) -> i64 {
    let prior = date.year() as i64 - 1;
    let mut count = prior / 4 - prior / 100 + prior / 400;
    if is_leap_year(date.year()) && (date.month() > 2 || (date.month() == 2 && date.day() == 29)) {
        count += 1;
    }
    count
}

fn contains_feb29(start: Date, end: Date) -> bool {
    feb29_count_through(end) > feb29_count_through(start)
}

fn act_365_leap(
    start: Date,
    end: Date,
    days: i64,
    context: &Context,
    trace: &mut Trace,
) -> Result<Rational, Error> {
    let frequency = context
        .frequency
        .ok_or(Error::MissingFrequency(Convention::Act365Leap))?;
    if !matches!(frequency, 1 | 2 | 3 | 4 | 6 | 12) {
        return Err(Error::InvalidFrequency(frequency));
    }
    let denominator = if frequency == 1 {
        if contains_feb29(start, end) {
            trace.insert(Branch::LeapAnnualFeb29);
            366
        } else {
            trace.insert(Branch::LeapAnnualNoFeb29);
            365
        }
    } else if is_leap_year(end.year()) {
        trace.insert(Branch::LeapEndInLeapYear);
        366
    } else {
        trace.insert(Branch::LeapEndInCommonYear);
        365
    };
    Rational::new(days, denominator)
}

fn act_act_isda(start: Date, end: Date, trace: &mut Trace) -> Result<Rational, Error> {
    let (mut leap_days, mut common_days) = (0i64, 0i64);
    for year in start.year()..=end.year() {
        let year_start = Date::from_month_index(year as i64 * 12, 1, false).serial();
        let next_year_start = Date::from_month_index((year as i64 + 1) * 12, 1, false).serial();
        let from = start.serial().max(year_start);
        let to = end.serial().min(next_year_start);
        if to > from {
            if is_leap_year(year) {
                leap_days += to - from;
            } else {
                common_days += to - from;
            }
        }
    }
    let spans_years = start.year() != end.year()
        && !(end.year() == start.year() + 1 && end.month() == 1 && end.day() == 1);
    trace.insert(if spans_years {
        Branch::IsdaSplitAtYearEnd
    } else {
        Branch::IsdaOneCalendarYear
    });
    if leap_days > 0 {
        trace.insert(Branch::IsdaLeapYearDays);
    }
    if common_days > 0 {
        trace.insert(Branch::IsdaCommonYearDays);
    }
    Rational::new(common_days, 365)?.checked_add(Rational::new(leap_days, 366)?)
}

fn act_act_afb(start: Date, end: Date, trace: &mut Trace) -> Result<Rational, Error> {
    let mut whole_years = 0i64;
    let mut remainder_end = end;
    let end_is_common_feb28 = end.is_last_day_of_february() && end.day() == 28;
    for k in 1.. {
        let year = end.year() - k;
        if year < 1 {
            break;
        }
        let candidate = if end.month() == 2 && end.day() >= 28 {
            // Feb 29 falls back to Feb 28 in a common year; per the AFB
            // rule, Feb 28 of a common year goes back to Feb 29 of a leap year.
            let day = if is_leap_year(year) && (end_is_common_feb28 || end.day() == 29) {
                29
            } else {
                28
            };
            Date::new(year, 2, day)?
        } else {
            Date::new(year, end.month(), end.day())?
        };
        if candidate < start {
            break;
        }
        if candidate.day() == 29 && end_is_common_feb28 {
            trace.insert(Branch::AfbFeb28BackToFeb29);
        }
        whole_years = k as i64;
        remainder_end = candidate;
    }
    if whole_years > 0 {
        trace.insert(Branch::AfbWholeYears);
    }
    let remainder_days = start.days_until(remainder_end);
    let denominator = if remainder_days == 0 {
        365
    } else if contains_feb29(start, remainder_end) {
        trace.insert(Branch::AfbFeb29InRemainder);
        366
    } else {
        trace.insert(Branch::AfbNoFeb29InRemainder);
        365
    };
    Rational::integer(whole_years).checked_add(Rational::new(remainder_days, denominator)?)
}

fn classify_stub(start: Date, end: Date, schedule: &Schedule, trace: &mut Trace) {
    let start_on = schedule.is_roll_date(start);
    let end_on = schedule.is_roll_date(end);
    let periods = schedule.period_index(end) - schedule.period_index(start) + i64::from(!end_on);
    trace.insert(match (start_on && end_on, periods) {
        (true, 1) => Branch::RegularPeriod,
        (true, _) => Branch::WholeRegularPeriods,
        (false, 1) => Branch::ShortStub,
        (false, _) => Branch::LongStub,
    });
    if !start_on {
        trace.insert(Branch::StartOffSchedule);
    }
    if !end_on {
        trace.insert(Branch::EndOffSchedule);
    }
    if schedule.rolls_on_month_end() {
        trace.insert(Branch::EndOfMonthRoll);
    }
}

fn act_act_icma(
    start: Date,
    end: Date,
    schedule: &Schedule,
    trace: &mut Trace,
) -> Result<Rational, Error> {
    classify_stub(start, end, schedule, trace);
    let frequency = schedule.frequency() as i64;
    let mut total = Rational::ZERO;
    for piece in schedule.pieces(start, end) {
        let part = if piece.is_full_period() {
            trace.insert(Branch::FullReferencePeriod);
            Rational::new(1, frequency)?
        } else {
            trace.insert(Branch::IcmaPartialPiece);
            Rational::new(piece.days(), frequency * piece.period_days())?
        };
        total = total.checked_add(part)?;
    }
    Ok(total)
}

fn canadian(
    start: Date,
    end: Date,
    schedule: &Schedule,
    trace: &mut Trace,
) -> Result<Rational, Error> {
    classify_stub(start, end, schedule, trace);
    let frequency = schedule.frequency() as i64;
    let mut total = Rational::ZERO;
    for piece in schedule.pieces(start, end) {
        let days = piece.days();
        let part = if piece.is_full_period() {
            trace.insert(Branch::FullReferencePeriod);
            Rational::new(1, frequency)?
        } else if days * frequency < 365 {
            trace.insert(Branch::CanadianShortPiece);
            Rational::new(days, 365)?
        } else {
            trace.insert(Branch::CanadianLongPiece);
            Rational::new(1, frequency)?
                .checked_sub(Rational::new(piece.period_days() - days, 365)?)?
        };
        total = total.checked_add(part)?;
    }
    Ok(total)
}

fn thirty_360(
    convention: Convention,
    start: Date,
    end: Date,
    termination: Option<Date>,
    trace: &mut Trace,
) -> Result<Rational, Error> {
    trace.insert(Branch::ThirtyDayMonths);
    let (mut d1, mut d2) = (start.day() as i64, end.day() as i64);
    let d1_feb_end = start.is_last_day_of_february();
    let d2_feb_end = end.is_last_day_of_february();
    match convention {
        Convention::Thirty360Bond => {
            if d1_feb_end {
                trace.insert(Branch::D1FebEndKept);
            }
            if d2_feb_end {
                trace.insert(Branch::D2FebEndKept);
            }
            if d2 == 31 {
                if d1 >= 30 {
                    trace.insert(Branch::D2Is31To30SinceD1AtLeast30);
                    d2 = 30;
                } else {
                    trace.insert(Branch::D2Is31KeptSinceD1Below30);
                }
            }
            if d1 == 31 {
                trace.insert(Branch::D1Is31To30);
                d1 = 30;
            }
        }
        Convention::Thirty360Us => {
            if d1_feb_end && d2_feb_end {
                trace.insert(Branch::D2FebEndTo30SinceD1FebEnd);
                d2 = 30;
            } else if d2_feb_end {
                trace.insert(Branch::D2FebEndKept);
            }
            if d1_feb_end {
                trace.insert(Branch::D1FebEndTo30);
                d1 = 30;
            }
            if d2 == 31 {
                if d1 >= 30 {
                    trace.insert(Branch::D2Is31To30SinceD1AtLeast30);
                    d2 = 30;
                } else {
                    trace.insert(Branch::D2Is31KeptSinceD1Below30);
                }
            }
            if d1 == 31 {
                trace.insert(Branch::D1Is31To30);
                d1 = 30;
            }
        }
        Convention::Thirty360European => {
            if d1_feb_end {
                trace.insert(Branch::D1FebEndKept);
            }
            if d2_feb_end {
                trace.insert(Branch::D2FebEndKept);
            }
            if d1 == 31 {
                trace.insert(Branch::D1Is31To30);
                d1 = 30;
            }
            if d2 == 31 {
                trace.insert(Branch::D2Is31To30);
                d2 = 30;
            }
        }
        Convention::Thirty360EuropeanIsda => {
            if d1_feb_end {
                trace.insert(Branch::D1FebEndTo30);
                d1 = 30;
            }
            if d1 == 31 {
                trace.insert(Branch::D1Is31To30);
                d1 = 30;
            }
            if d2_feb_end {
                if Some(end) == termination {
                    trace.insert(Branch::D2FebEndIsTermination);
                } else {
                    trace.insert(Branch::D2FebEndTo30NotTermination);
                    d2 = 30;
                }
            }
            if d2 == 31 {
                trace.insert(Branch::D2Is31To30);
                d2 = 30;
            }
        }
        _ => unreachable!("thirty_360 called with {convention:?}"),
    }
    let day_count = 360 * (end.year() as i64 - start.year() as i64)
        + 30 * (end.month() as i64 - start.month() as i64)
        + (d2 - d1);
    Rational::new(day_count, 360)
}
