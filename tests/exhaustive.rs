//! Every (start, end) pair with both dates in 2019-2021, through every
//! convention, under schedules that exercise month-end rolls and Feb 29.

use stubfrac::{year_fraction, Context, Convention, Date, Rational};

fn contexts() -> Vec<Context> {
    let d = |s: &str| s.parse::<Date>().unwrap();
    vec![
        Context {
            frequency: Some(2),
            anchor: Some(d("2020-02-29")),
            end_of_month: true,
            termination: Some(d("2021-02-28")),
        },
        Context {
            frequency: Some(12),
            anchor: Some(d("2021-01-31")),
            end_of_month: false,
            termination: Some(d("2020-02-29")),
        },
        Context {
            frequency: Some(1),
            anchor: Some(d("2019-03-15")),
            end_of_month: false,
            termination: Some(d("2019-12-31")),
        },
    ]
}

#[test]
fn every_pair_in_2019_to_2021_through_every_convention() {
    let from = "2019-01-01".parse::<Date>().unwrap().serial();
    let to = "2021-12-31".parse::<Date>().unwrap().serial();
    let mut evaluated = 0u64;
    for ctx in contexts() {
        for s in from..=to {
            let start = Date::from_serial(s).unwrap();
            for e in s..=to {
                let end = Date::from_serial(e).unwrap();
                let days = e - s;
                for &conv in Convention::ALL {
                    let result = year_fraction(conv, start, end, &ctx)
                        .unwrap_or_else(|err| panic!("{} {start} {end}: {err}", conv.name()));
                    let v = result.value;
                    evaluated += 1;
                    assert!(v.denom() > 0);
                    assert!(
                        !result.trace.is_empty(),
                        "{} {start} {end}: empty trace",
                        conv.name()
                    );
                    if days == 0 {
                        assert_eq!(v, Rational::ZERO);
                        continue;
                    }
                    let ratio = |n: i64| Rational::new(days, n).unwrap();
                    let in_range = |lo: Rational, hi: Rational| lo <= v && v <= hi;
                    let ok = match conv {
                        Convention::Act360 => v == ratio(360),
                        Convention::Act365Fixed => v == ratio(365),
                        Convention::Act365Leap => v == ratio(365) || v == ratio(366),
                        Convention::NoLeap365 => {
                            in_range(Rational::new(days - 1, 365).unwrap(), ratio(365))
                        }
                        Convention::ActActIsda | Convention::ActActAfb => {
                            in_range(ratio(366), ratio(365))
                        }
                        Convention::ActActIcma | Convention::Act365Canadian => {
                            in_range(ratio(372), ratio(336))
                        }
                        _ => {
                            // Day adjustments move the 30/360 count at most 4
                            // days from the unadjusted calendar difference.
                            let raw = 360 * (end.year() - start.year()) as i64
                                + 30 * (end.month() as i64 - start.month() as i64)
                                + (end.day() as i64 - start.day() as i64);
                            let count = v.numer() * (360 / v.denom());
                            v >= Rational::ZERO && 360 % v.denom() == 0 && (count - raw).abs() <= 4
                        }
                    };
                    assert!(
                        ok,
                        "{} {start} -> {end} ({days} days) gave {v}",
                        conv.name()
                    );
                }
            }
        }
    }
    assert_eq!(evaluated, 3 * 12 * (1096 * 1097 / 2));
}
