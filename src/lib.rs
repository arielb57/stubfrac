//! Day-count year fractions as exact rationals, with a rule trace for every
//! result and an enumerator of the date pairs where two conventions disagree.
//!
//! ```
//! use stubfrac::{year_fraction, Context, Convention, Date, Rational};
//!
//! let start: Date = "2002-08-15".parse().unwrap();
//! let end: Date = "2003-07-15".parse().unwrap();
//! let context = Context {
//!     frequency: Some(2),
//!     anchor: Some(end),
//!     ..Context::default()
//! };
//! let icma = year_fraction(Convention::ActActIcma, start, end, &context).unwrap();
//! assert_eq!(icma.value, Rational::new(337, 368).unwrap());
//! ```

mod convention;
mod date;
pub mod disagree;
mod rational;
mod schedule;

use std::fmt;

pub use convention::{year_fraction, Branch, Context, Convention, Trace, YearFraction};
pub use date::{days_in_month, is_leap_year, Date};
pub use rational::Rational;
pub use schedule::{Piece, Schedule};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    InvalidDate(String),
    StartAfterEnd { start: Date, end: Date },
    UnknownConvention(String),
    InvalidFrequency(u32),
    MissingFrequency(Convention),
    MissingAnchor(Convention),
    MissingTermination,
    ZeroDenominator,
    Overflow,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidDate(s) => write!(
                f,
                "invalid date '{s}' (expected YYYY-MM-DD, years 0001-9999)"
            ),
            Error::StartAfterEnd { start, end } => write!(f, "start {start} is after end {end}"),
            Error::UnknownConvention(s) => {
                write!(f, "unknown convention '{s}' (see `stubfrac list`)")
            }
            Error::InvalidFrequency(n) => write!(
                f,
                "frequency {n} does not divide 12 (use 1, 2, 3, 4, 6 or 12)"
            ),
            Error::MissingFrequency(c) => write!(f, "{c} needs a coupon frequency (--freq)"),
            Error::MissingAnchor(c) => write!(f, "{c} needs a schedule anchor date (--anchor)"),
            Error::MissingTermination => {
                write!(f, "30E/360 ISDA needs the termination date (--termination)")
            }
            Error::ZeroDenominator => write!(f, "zero denominator"),
            Error::Overflow => write!(f, "result does not fit in an i64 rational"),
        }
    }
}

impl std::error::Error for Error {}
