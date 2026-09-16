use std::collections::BTreeMap;

use stubfrac::disagree::{disagree, find_additivity_counterexample, Window};
use stubfrac::{year_fraction, Branch, Context, Convention, Date, Rational, Trace};

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

fn context() -> Context {
    Context {
        frequency: Some(2),
        anchor: Some(d("2021-08-31")),
        end_of_month: true,
        termination: Some(d("2021-02-28")),
    }
}

/// Count, then the first pair in (span, start) order with both fractions.
type OracleGroup = (u64, Date, Date, Rational, Rational);

/// Oracle: materialise every differing pair, sort, and group by key.
fn brute_force(
    a: Convention,
    b: Convention,
    ctx: &Context,
    window: Window,
) -> (u64, BTreeMap<(Trace, Trace), OracleGroup>) {
    let mut checked = 0;
    let mut rows = Vec::new();
    let mut start = window.from;
    while start < window.to {
        let mut end = start.add_days(1).unwrap();
        while end <= window.to && window.max_days.is_none_or(|m| start.days_until(end) <= m) {
            let fa = year_fraction(a, start, end, ctx).unwrap();
            let fb = year_fraction(b, start, end, ctx).unwrap();
            checked += 1;
            if fa.value != fb.value {
                let key = (fa.trace.difference(fb.trace), fb.trace.difference(fa.trace));
                rows.push((key, start.days_until(end), start, end, fa.value, fb.value));
            }
            end = end.add_days(1).unwrap();
        }
        start = start.add_days(1).unwrap();
    }
    rows.sort_by_key(|row| (row.0, row.1, row.2));
    let mut groups = BTreeMap::new();
    for (key, _, start, end, va, vb) in rows {
        groups
            .entry(key)
            .and_modify(|g: &mut OracleGroup| g.0 += 1)
            .or_insert((1, start, end, va, vb));
    }
    (checked, groups)
}

#[test]
fn enumerator_matches_brute_force_over_one_year() {
    let ctx = context();
    let pairs = [
        (Convention::Thirty360Bond, Convention::Thirty360European),
        (Convention::Thirty360Bond, Convention::Thirty360Us),
        (Convention::Thirty360Us, Convention::Thirty360EuropeanIsda),
        (
            Convention::Thirty360European,
            Convention::Thirty360EuropeanIsda,
        ),
        (Convention::ActActIsda, Convention::ActActAfb),
        (Convention::ActActIcma, Convention::Act365Canadian),
        (Convention::Act365Fixed, Convention::NoLeap365),
    ];
    for max_days in [None, Some(40)] {
        let window = Window {
            from: d("2020-06-01"),
            to: d("2021-05-31"),
            max_days,
        };
        for (a, b) in pairs {
            let (checked, expected) = brute_force(a, b, &ctx, window);
            for threads in [1, 7] {
                let report = disagree(a, b, &ctx, window, threads).unwrap();
                let label = format!(
                    "{} vs {} threads={threads} max_days={max_days:?}",
                    a.name(),
                    b.name()
                );
                assert_eq!(report.pairs_checked, checked, "{label}");
                assert_eq!(
                    report.differing,
                    expected.values().map(|g| g.0).sum::<u64>(),
                    "{label}"
                );
                assert_eq!(report.groups.len(), expected.len(), "{label}");
                for group in &report.groups {
                    let &(count, start, end, va, vb) = expected
                        .get(&(group.only_a, group.only_b))
                        .unwrap_or_else(|| panic!("{label}: unexpected group {}", group.only_a));
                    assert_eq!(group.count, count, "{label}");
                    assert_eq!(
                        (
                            group.example.start,
                            group.example.end,
                            group.example.a,
                            group.example.b
                        ),
                        (start, end, va, vb),
                        "{label}"
                    );
                }
                assert!(
                    report.groups.windows(2).all(|w| w[0].count >= w[1].count),
                    "{label}"
                );
            }
        }
    }
}

#[test]
fn bond_basis_and_30e_differ_only_on_day_31_ends() {
    let window = Window {
        from: d("2020-01-01"),
        to: d("2020-12-31"),
        max_days: None,
    };
    let report = disagree(
        Convention::Thirty360Bond,
        Convention::Thirty360European,
        &context(),
        window,
        4,
    )
    .unwrap();
    assert_eq!(report.groups.len(), 1);
    let group = report.groups[0];
    assert!(group.only_a.contains(Branch::D2Is31KeptSinceD1Below30));
    assert!(group.only_b.contains(Branch::D2Is31To30));
    assert_eq!(
        (group.example.start, group.example.end),
        (d("2020-01-29"), d("2020-01-31"))
    );
    // Every date pair ending on a 31st with a start day below 30 in the window.
    let mut expected = 0;
    let mut start = window.from;
    while start < window.to {
        if start.day() < 30 {
            expected += (1..=12)
                .filter_map(|m| Date::new(2020, m, 31).ok())
                .filter(|&end| end > start)
                .count() as u64;
        }
        start = start.add_days(1).unwrap();
    }
    assert_eq!(group.count, expected);
}

#[test]
fn identical_conventions_never_disagree() {
    let window = Window {
        from: d("2020-01-01"),
        to: d("2020-12-31"),
        max_days: Some(100),
    };
    let report = disagree(
        Convention::ActActIsda,
        Convention::ActActIsda,
        &context(),
        window,
        3,
    )
    .unwrap();
    assert_eq!(report.differing, 0);
    assert!(report.groups.is_empty());
    assert!(report.pairs_checked > 0);
}

#[test]
fn enumerator_reports_missing_terms_and_bad_windows() {
    let window = Window {
        from: d("2020-01-01"),
        to: d("2020-03-01"),
        max_days: None,
    };
    assert!(disagree(
        Convention::ActActIcma,
        Convention::Act360,
        &Context::default(),
        window,
        2
    )
    .is_err());
    let backwards = Window {
        from: d("2020-03-01"),
        to: d("2020-01-01"),
        max_days: None,
    };
    assert!(disagree(
        Convention::Act365Fixed,
        Convention::Act360,
        &context(),
        backwards,
        2
    )
    .is_err());
}

fn assert_is_counterexample(conv: Convention, ctx: &Context, window: Window) {
    let cx = find_additivity_counterexample(conv, ctx, window)
        .unwrap()
        .unwrap_or_else(|| panic!("{} should not be additive", conv.name()));
    assert!(cx.a <= cx.b && cx.b <= cx.c);
    let f = |x, y| year_fraction(conv, x, y, ctx).unwrap().value;
    assert_eq!(cx.whole, f(cx.a, cx.c));
    assert_eq!(
        cx.sum_of_parts,
        f(cx.a, cx.b).checked_add(f(cx.b, cx.c)).unwrap()
    );
    assert_ne!(cx.whole, cx.sum_of_parts, "{}", conv.name());
}

#[test]
fn thirty_360_variants_with_date_dependent_rules_are_not_additive() {
    let ctx = context();
    let window = Window {
        from: d("2020-01-01"),
        to: d("2021-12-31"),
        max_days: Some(60),
    };
    for conv in [
        Convention::Thirty360Bond,
        Convention::Thirty360Us,
        Convention::Thirty360EuropeanIsda,
    ] {
        assert_is_counterexample(conv, &ctx, window);
    }
    // The shortest counterexample for the bond basis is a two-day period
    // split on the 30th.
    let bond = find_additivity_counterexample(Convention::Thirty360Bond, &ctx, window)
        .unwrap()
        .unwrap();
    assert_eq!(
        (bond.a, bond.b, bond.c),
        (d("2020-01-29"), d("2020-01-30"), d("2020-01-31"))
    );
    // Leap-year denominators break additivity outside the 30/360 family too.
    assert_is_counterexample(
        Convention::ActActAfb,
        &ctx,
        Window {
            max_days: Some(400),
            ..window
        },
    );
}

#[test]
fn thirty_e_360_and_actual_conventions_have_no_counterexample() {
    let ctx = context();
    let window = Window {
        from: d("2019-12-01"),
        to: d("2020-04-30"),
        max_days: Some(75),
    };
    for conv in [
        Convention::Thirty360European,
        Convention::Act365Fixed,
        Convention::ActActIsda,
    ] {
        assert_eq!(
            find_additivity_counterexample(conv, &ctx, window).unwrap(),
            None,
            "{}",
            conv.name()
        );
    }
}
