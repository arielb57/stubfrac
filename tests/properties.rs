use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use stubfrac::{year_fraction, Context, Convention, Date, Error, Rational, Schedule};

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 512,
        max_shrink_iters: 2_000,
        max_shrink_time: 20_000,
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..ProptestConfig::default()
    }
}

const FROM: i64 = 3_652; // 1980-01-01
const TO: i64 = 32_872; // 2060-01-01

fn date() -> impl Strategy<Value = Date> {
    (FROM..=TO).prop_map(|s| Date::from_serial(s).unwrap())
}

fn frequency() -> impl Strategy<Value = u32> {
    prop::sample::select(vec![1u32, 2, 3, 4, 6, 12])
}

fn context() -> impl Strategy<Value = Context> {
    (frequency(), date(), any::<bool>(), date()).prop_map(|(f, anchor, eom, termination)| Context {
        frequency: Some(f),
        anchor: Some(anchor),
        end_of_month: eom,
        termination: Some(termination),
    })
}

fn ordered_triple() -> impl Strategy<Value = (Date, Date, Date)> {
    (date(), date(), date()).prop_map(|(x, y, z)| {
        let mut v = [x, y, z];
        v.sort();
        (v[0], v[1], v[2])
    })
}

fn f(conv: Convention, a: Date, b: Date, ctx: &Context) -> Rational {
    year_fraction(conv, a, b, ctx).unwrap().value
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn icma_regular_periods_are_exactly_one_over_frequency(
        ctx in context(), k in -100i64..100, n in 1i64..8,
    ) {
        let schedule = ctx.schedule(Convention::ActActIcma).unwrap();
        let freq = ctx.frequency.unwrap() as i64;
        let start = schedule.roll_date(k);
        let one = f(Convention::ActActIcma, start, schedule.roll_date(k + 1), &ctx);
        prop_assert_eq!(one, Rational::new(1, freq).unwrap());
        let several = f(Convention::ActActIcma, start, schedule.roll_date(k + n), &ctx);
        prop_assert_eq!(several, Rational::new(n, freq).unwrap());
    }

    #[test]
    fn icma_stub_is_days_over_frequency_times_reference_days(
        ctx in context(), k in -100i64..100, cut in 0.0f64..1.0,
    ) {
        let schedule = ctx.schedule(Convention::ActActIcma).unwrap();
        let freq = ctx.frequency.unwrap() as i64;
        let (p0, p1) = (schedule.roll_date(k), schedule.roll_date(k + 1));
        let period = p0.days_until(p1);
        let days = ((period as f64) * cut) as i64;
        // Short first stub ending on a roll date, and short final stub starting on one.
        let first = f(Convention::ActActIcma, p1.add_days(-days).unwrap(), p1, &ctx);
        let last = f(Convention::ActActIcma, p0, p0.add_days(days).unwrap(), &ctx);
        let expected = Rational::new(days, freq * period).unwrap();
        prop_assert_eq!(first, expected);
        prop_assert_eq!(last, expected);
    }

    #[test]
    fn additive_conventions_are_additive((a, b, c) in ordered_triple(), ctx in context()) {
        for conv in [
            Convention::Act360,
            Convention::Act365Fixed,
            Convention::NoLeap365,
            Convention::ActActIsda,
            Convention::ActActIcma,
            Convention::Thirty360European,
        ] {
            let whole = f(conv, a, c, &ctx);
            let parts = f(conv, a, b, &ctx).checked_add(f(conv, b, c, &ctx)).unwrap();
            prop_assert_eq!(whole, parts, "{} on {} {} {}", conv.name(), a, b, c);
        }
    }

    #[test]
    fn empty_periods_are_zero_and_ordered_periods_non_negative(
        (a, b, _) in ordered_triple(), ctx in context(),
    ) {
        for &conv in Convention::ALL {
            prop_assert_eq!(f(conv, a, a, &ctx), Rational::ZERO, "{}", conv.name());
            prop_assert!(f(conv, a, b, &ctx) >= Rational::ZERO, "{} {} {}", conv.name(), a, b);
            if a < b {
                let reversed = year_fraction(conv, b, a, &ctx);
                prop_assert_eq!(reversed, Err(Error::StartAfterEnd { start: b, end: a }));
            }
        }
    }

    #[test]
    fn canadian_rule_on_and_inside_a_reference_period(
        ctx in context(), k in -100i64..100, shortfall_seed in 0i64..400,
    ) {
        let schedule: Schedule = ctx.schedule(Convention::Act365Canadian).unwrap();
        let freq = ctx.frequency.unwrap() as i64;
        let (p0, p1) = (schedule.roll_date(k), schedule.roll_date(k + 1));
        let period = p0.days_until(p1);
        prop_assert_eq!(
            f(Convention::Act365Canadian, p0, p1, &ctx),
            Rational::new(1, freq).unwrap()
        );
        // Accrue from the period start for `days`, strictly inside the period.
        let shortfall = 1 + shortfall_seed % (period - 1);
        let days = period - shortfall;
        let got = f(Convention::Act365Canadian, p0, p0.add_days(days).unwrap(), &ctx);
        let expected = if days * freq < 365 {
            Rational::new(days, 365).unwrap()
        } else {
            Rational::new(1, freq).unwrap().checked_sub(Rational::new(shortfall, 365).unwrap()).unwrap()
        };
        prop_assert_eq!(got, expected, "{} days of {}", days, period);
        // The same stub placed at the end of the period follows the same rule.
        let got_first = f(Convention::Act365Canadian, p1.add_days(-days).unwrap(), p1, &ctx);
        prop_assert_eq!(got_first, expected);
    }

    #[test]
    fn act_act_isda_matches_a_day_by_day_sum(a in date(), length in 0i64..1_500) {
        let b = a.add_days(length).unwrap();
        let (mut leap, mut common) = (0, 0);
        let mut day = a;
        while day < b {
            if stubfrac::is_leap_year(day.year()) { leap += 1 } else { common += 1 }
            day = day.add_days(1).unwrap();
        }
        let expected = Rational::new(common, 365).unwrap().checked_add(Rational::new(leap, 366).unwrap()).unwrap();
        prop_assert_eq!(f(Convention::ActActIsda, a, b, &Context::default()), expected);
    }

    #[test]
    fn afb_is_one_for_every_whole_calendar_year(a in date(), years in 1i32..20) {
        // Feb 28/29 starts are excluded: the AFB rule sends a Feb 28 end back
        // to Feb 29, so e.g. 2020-02-28 -> 2021-02-28 is 1 + 1/366 by design.
        prop_assume!(!(a.month() == 2 && a.day() >= 28));
        let end = Date::new(a.year() + years, a.month(), a.day()).unwrap();
        let expected = Rational::integer(years as i64);
        prop_assert_eq!(f(Convention::ActActAfb, a, end, &Context::default()), expected);
    }
}
