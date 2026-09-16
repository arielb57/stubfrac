//! Worked examples from published market documents, checked as exact
//! rationals. Where a document prints a rounded decimal, the exact value is
//! also checked to round to that decimal.

use stubfrac::{year_fraction, Branch, Context, Convention, Date, Error, Rational};

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

fn r(n: i64, den: i64) -> Rational {
    Rational::new(n, den).unwrap()
}

fn icma_context(frequency: u32, anchor: &str) -> Context {
    Context {
        frequency: Some(frequency),
        anchor: Some(d(anchor)),
        ..Context::default()
    }
}

/// One ACT/ACT example from ISDA, "EMU and Market Conventions: Recent
/// Developments" (1998): exact value, then the 5-decimal figure as printed.
struct ActActCase {
    name: &'static str,
    start: &'static str,
    end: &'static str,
    frequency: u32,
    anchor: &'static str,
    isda: (Rational, &'static str),
    icma: (Rational, &'static str),
    afb: (Rational, &'static str),
}

fn isda_1998_cases() -> Vec<ActActCase> {
    vec![
        ActActCase {
            name: "regular calculation period",
            start: "2003-11-01",
            end: "2004-05-01",
            frequency: 2,
            anchor: "2004-05-01",
            // 61 days of 2003 over 365, 121 days of 2004 over 366.
            isda: (r(61 * 366 + 121 * 365, 365 * 366), "0.49772"),
            icma: (r(182, 2 * 182), "0.50000"),
            afb: (r(182, 366), "0.49727"),
        },
        ActActCase {
            name: "short first calculation period",
            start: "1999-02-01",
            end: "1999-07-01",
            frequency: 1,
            anchor: "1999-07-01",
            isda: (r(150, 365), "0.41096"),
            // Reference period 1998-07-01 to 1999-07-01 has 365 days.
            icma: (r(150, 365), "0.41096"),
            afb: (r(150, 365), "0.41096"),
        },
        ActActCase {
            name: "long first calculation period",
            start: "2002-08-15",
            end: "2003-07-15",
            frequency: 2,
            anchor: "2003-07-15",
            isda: (r(334, 365), "0.91507"),
            // 153 of the 184 days in 2002-07-15..2003-01-15, then a full period.
            icma: (r(153, 2 * 184).checked_add(r(1, 2)).unwrap(), "0.91576"),
            afb: (r(334, 365), "0.91507"),
        },
        ActActCase {
            name: "short final calculation period: penultimate (regular) period",
            start: "1999-07-30",
            end: "2000-01-30",
            frequency: 2,
            anchor: "1999-07-30",
            isda: (r(155, 365).checked_add(r(29, 366)).unwrap(), "0.50389"),
            icma: (r(1, 2), "0.50000"),
            afb: (r(184, 365), "0.50411"),
        },
        ActActCase {
            name: "short final calculation period: final period",
            start: "2000-01-30",
            end: "2000-06-30",
            frequency: 2,
            anchor: "1999-07-30",
            isda: (r(152, 366), "0.41530"),
            // Notional period 2000-01-30 to 2000-07-30 has 182 days.
            icma: (r(152, 2 * 182), "0.41758"),
            afb: (r(152, 366), "0.41530"),
        },
    ]
}

#[test]
fn isda_1998_act_act_examples_match_exactly() {
    for case in isda_1998_cases() {
        let ctx = icma_context(case.frequency, case.anchor);
        for (conv, (exact, printed)) in [
            (Convention::ActActIsda, case.isda),
            (Convention::ActActIcma, case.icma),
            (Convention::ActActAfb, case.afb),
        ] {
            let got = year_fraction(conv, d(case.start), d(case.end), &ctx).unwrap();
            assert_eq!(got.value, exact, "{}: {}", case.name, conv.name());
            assert_eq!(
                got.value.to_decimal(5),
                printed,
                "{}: {} printed value",
                case.name,
                conv.name()
            );
        }
    }
}

#[test]
fn isda_1998_traces_name_the_stub_shape() {
    let trace = |start, end, freq, anchor| {
        year_fraction(
            Convention::ActActIcma,
            d(start),
            d(end),
            &icma_context(freq, anchor),
        )
        .unwrap()
        .trace
    };
    assert!(trace("2003-11-01", "2004-05-01", 2, "2004-05-01").contains(Branch::RegularPeriod));
    let short_first = trace("1999-02-01", "1999-07-01", 1, "1999-07-01");
    assert!(
        short_first.contains(Branch::ShortStub) && short_first.contains(Branch::StartOffSchedule)
    );
    let long_first = trace("2002-08-15", "2003-07-15", 2, "2003-07-15");
    assert!(long_first.contains(Branch::LongStub) && !long_first.contains(Branch::EndOffSchedule));
    let short_final = trace("2000-01-30", "2000-06-30", 2, "1999-07-30");
    assert!(
        short_final.contains(Branch::ShortStub) && short_final.contains(Branch::EndOffSchedule)
    );

    let isda = year_fraction(
        Convention::ActActIsda,
        d("2003-11-01"),
        d("2004-05-01"),
        &Context::default(),
    )
    .unwrap()
    .trace;
    assert!(isda.contains(Branch::IsdaSplitAtYearEnd));
    assert!(isda.contains(Branch::IsdaLeapYearDays) && isda.contains(Branch::IsdaCommonYearDays));
}

#[test]
fn icma_long_stub_is_not_confused_with_isda_when_stub_crosses_no_year_end() {
    // Same long first stub; the two notional periods have different lengths,
    // so ICMA differs from both ISDA and a naive days/(f * 365).
    let ctx = icma_context(2, "2003-07-15");
    let icma = year_fraction(
        Convention::ActActIcma,
        d("2002-08-15"),
        d("2003-07-15"),
        &ctx,
    )
    .unwrap()
    .value;
    assert_eq!(icma, r(337, 368));
    assert_ne!(icma, r(334, 365));
}

/// The 30/360-family table circulated with the ISDA 2006 Definitions §4.16:
/// start, end, day counts under 30/360, 30E/360 and 30E/360 (ISDA). No end
/// date in the table is the termination date.
const ISDA_2006_THIRTY_360: &[(&str, &str, i64, i64, i64)] = &[
    ("2007-01-15", "2007-01-30", 15, 15, 15),
    ("2007-01-15", "2007-02-15", 30, 30, 30),
    ("2007-01-15", "2007-07-15", 180, 180, 180),
    ("2007-09-30", "2008-03-31", 180, 180, 180),
    ("2007-09-30", "2007-10-31", 30, 30, 30),
    ("2007-09-30", "2008-09-30", 360, 360, 360),
    ("2007-01-15", "2007-01-31", 16, 15, 15),
    ("2007-01-31", "2007-02-28", 28, 28, 30),
    ("2007-02-28", "2007-03-31", 33, 32, 30),
    ("2006-08-31", "2007-02-28", 178, 178, 180),
    ("2007-02-28", "2007-08-31", 183, 182, 180),
    ("2007-02-14", "2007-02-28", 14, 14, 16),
    ("2007-02-26", "2008-02-29", 363, 363, 364),
    ("2008-02-29", "2009-02-28", 359, 359, 360),
    ("2008-02-29", "2008-03-30", 31, 31, 30),
    ("2008-02-29", "2008-03-31", 32, 31, 30),
    ("2007-02-28", "2007-03-05", 7, 7, 5),
    ("2007-10-31", "2007-11-28", 28, 28, 28),
    ("2007-08-31", "2008-02-29", 179, 179, 180),
    ("2008-02-29", "2008-08-31", 182, 181, 180),
    ("2008-08-31", "2009-02-28", 178, 178, 180),
    ("2009-02-28", "2009-08-31", 183, 182, 180),
];

#[test]
fn isda_2006_thirty_360_table_matches_exactly() {
    let ctx = Context {
        termination: Some(d("2030-06-15")),
        ..Context::default()
    };
    for &(start, end, bond, euro, euro_isda) in ISDA_2006_THIRTY_360 {
        for (conv, days) in [
            (Convention::Thirty360Bond, bond),
            (Convention::Thirty360European, euro),
            (Convention::Thirty360EuropeanIsda, euro_isda),
        ] {
            let got = year_fraction(conv, d(start), d(end), &ctx).unwrap().value;
            assert_eq!(got, r(days, 360), "{} {start} -> {end}", conv.name());
        }
    }
}

#[test]
fn thirty_e_360_isda_keeps_february_end_on_the_termination_date() {
    let at_termination = Context {
        termination: Some(d("2007-02-28")),
        ..Context::default()
    };
    let elsewhere = Context {
        termination: Some(d("2012-02-29")),
        ..Context::default()
    };
    let conv = Convention::Thirty360EuropeanIsda;

    let kept = year_fraction(conv, d("2007-01-31"), d("2007-02-28"), &at_termination).unwrap();
    assert_eq!(kept.value, r(28, 360));
    assert!(kept.trace.contains(Branch::D2FebEndIsTermination));

    let moved = year_fraction(conv, d("2007-01-31"), d("2007-02-28"), &elsewhere).unwrap();
    assert_eq!(moved.value, r(30, 360));
    assert!(moved.trace.contains(Branch::D2FebEndTo30NotTermination));

    // The start date is always adjusted, termination or not.
    let from_feb_end = year_fraction(conv, d("2012-02-29"), d("2012-03-15"), &elsewhere).unwrap();
    assert_eq!(from_feb_end.value, r(15, 360));
    assert!(from_feb_end.trace.contains(Branch::D1FebEndTo30));

    assert_eq!(
        year_fraction(conv, d("2007-01-31"), d("2007-02-28"), &Context::default()),
        Err(Error::MissingTermination)
    );
}

#[test]
fn thirty_u_360_sia_february_rules() {
    let ctx = Context::default();
    let sia = |s, e| year_fraction(Convention::Thirty360Us, d(s), d(e), &ctx).unwrap();
    // Rule 1 and 2: both February ends, so D2 = 30 and D1 = 30.
    assert_eq!(sia("2007-02-28", "2008-02-29").value, r(360, 360));
    // Rule 2 only: D1 = 30, D2 kept.
    assert_eq!(sia("2007-02-28", "2007-03-28").value, r(28, 360));
    // Rule 2 then rule 3: D1 becomes 30, so D2 = 31 also becomes 30.
    let feb_to_31 = sia("2007-02-28", "2007-03-31");
    assert_eq!(feb_to_31.value, r(30, 360));
    assert!(feb_to_31.trace.contains(Branch::D1FebEndTo30));
    assert!(feb_to_31.trace.contains(Branch::D2Is31To30SinceD1AtLeast30));
    // D2 on a February end is kept when D1 is not a February end.
    let into_feb = sia("2007-01-15", "2007-02-28");
    assert_eq!(into_feb.value, r(43, 360));
    assert!(into_feb.trace.contains(Branch::D2FebEndKept));
    // Rule 3 and 4 as in the bond basis.
    assert_eq!(sia("2007-01-31", "2007-03-31").value, r(60, 360));
    assert_eq!(sia("2007-01-29", "2007-03-31").value, r(62, 360));
}

#[test]
fn act_365_leap_and_no_leap() {
    let semi = Context {
        frequency: Some(2),
        ..Context::default()
    };
    let annual = Context {
        frequency: Some(1),
        ..Context::default()
    };
    let f = |conv, s, e, ctx: &Context| year_fraction(conv, d(s), d(e), ctx).unwrap().value;

    // Non-annual: denominator follows the end date's year.
    assert_eq!(
        f(Convention::Act365Leap, "2023-12-15", "2024-01-15", &semi),
        r(31, 366)
    );
    assert_eq!(
        f(Convention::Act365Leap, "2024-12-15", "2025-01-15", &semi),
        r(31, 365)
    );
    // Annual: denominator follows whether Feb 29 is inside (start, end].
    assert_eq!(
        f(Convention::Act365Leap, "2023-12-15", "2024-01-15", &annual),
        r(31, 365)
    );
    assert_eq!(
        f(Convention::Act365Leap, "2023-03-01", "2024-03-01", &annual),
        r(366, 366)
    );
    assert_eq!(
        f(Convention::Act365Leap, "2024-02-29", "2025-02-28", &annual),
        r(365, 365)
    );
    assert_eq!(
        year_fraction(
            Convention::Act365Leap,
            d("2024-01-01"),
            d("2024-02-01"),
            &Context::default()
        ),
        Err(Error::MissingFrequency(Convention::Act365Leap))
    );

    assert_eq!(
        f(Convention::NoLeap365, "2024-02-28", "2024-03-01", &semi),
        r(1, 365)
    );
    assert_eq!(
        f(Convention::NoLeap365, "2024-02-29", "2024-03-01", &semi),
        r(1, 365)
    );
    assert_eq!(
        f(Convention::NoLeap365, "2024-02-28", "2024-02-29", &semi),
        Rational::ZERO
    );
    assert_eq!(
        f(Convention::NoLeap365, "2020-01-01", "2030-01-01", &semi),
        r(3650, 365)
    );
}

#[test]
fn afb_counts_whole_years_back_with_the_february_rule() {
    let ctx = Context::default();
    let afb = |s, e| year_fraction(Convention::ActActAfb, d(s), d(e), &ctx).unwrap();
    // 1999-07-01 to 2000-07-01 is one whole year even though it holds 366 days.
    assert_eq!(afb("1999-07-01", "2000-07-01").value, Rational::ONE);
    // Two whole years back from 2005-02-28 land on 2004-02-29, then 2003-02-28.
    let from_feb = afb("2003-02-28", "2005-02-28");
    assert_eq!(from_feb.value, r(2, 1));
    let back_to_leap = afb("2004-02-29", "2005-02-28");
    assert_eq!(back_to_leap.value, Rational::ONE);
    assert!(back_to_leap.trace.contains(Branch::AfbFeb28BackToFeb29));
    // Without the rule this would be 1 whole year plus a remainder.
    assert_eq!(
        afb("2004-02-28", "2005-02-28").value,
        Rational::ONE.checked_add(r(1, 366)).unwrap()
    );
    // Remainder containing Feb 29 uses 366.
    assert_eq!(
        afb("2003-10-01", "2005-03-01").value,
        Rational::ONE.checked_add(r(152, 366)).unwrap()
    );
}

#[test]
fn canadian_bond_rule_worked_example() {
    // Semi-annual, coupons on Jan 15 / Jul 15. The 2020-01-15..2020-07-15
    // period has 182 days, below 365/2 = 182.5, so no partial piece reaches the
    // complement branch in it; 2020-07-15..2021-01-15 has 184 days.
    let ctx = icma_context(2, "2021-01-15");
    let can = |s, e| year_fraction(Convention::Act365Canadian, d(s), d(e), &ctx).unwrap();
    assert_eq!(can("2020-07-15", "2021-01-15").value, r(1, 2));
    assert_eq!(can("2020-07-15", "2021-01-13").value, r(182, 365));
    let long = can("2020-07-15", "2021-01-14");
    assert_eq!(long.value, r(1, 2).checked_sub(r(1, 365)).unwrap());
    assert!(long.trace.contains(Branch::CanadianLongPiece));
    assert_eq!(can("2020-01-15", "2020-07-14").value, r(181, 365));
    assert_eq!(can("2020-01-15", "2020-07-15").value, r(1, 2));
}

#[test]
fn invalid_inputs_are_errors() {
    let ctx = icma_context(2, "2021-01-15");
    assert_eq!(
        year_fraction(Convention::Act360, d("2021-01-02"), d("2021-01-01"), &ctx),
        Err(Error::StartAfterEnd {
            start: d("2021-01-02"),
            end: d("2021-01-01")
        })
    );
    let no_anchor = Context {
        frequency: Some(2),
        ..Context::default()
    };
    assert_eq!(
        year_fraction(
            Convention::ActActIcma,
            d("2021-01-01"),
            d("2021-01-01"),
            &no_anchor
        ),
        Err(Error::MissingAnchor(Convention::ActActIcma))
    );
    let bad_freq = Context {
        frequency: Some(5),
        anchor: Some(d("2021-01-15")),
        ..Context::default()
    };
    assert_eq!(
        year_fraction(
            Convention::Act365Canadian,
            d("2021-01-01"),
            d("2021-02-01"),
            &bad_freq
        ),
        Err(Error::InvalidFrequency(5))
    );
    assert!("act/999".parse::<Convention>().is_err());
    for conv in Convention::ALL {
        assert_eq!(conv.id().parse::<Convention>().unwrap(), *conv);
        assert_eq!(conv.name().parse::<Convention>().unwrap(), *conv);
    }
}
