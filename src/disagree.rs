//! Enumerating the date pairs where two conventions give different fractions.

use std::collections::HashMap;

use crate::{year_fraction, Context, Convention, Date, Error, Rational, Trace};

/// Which date pairs to examine: every `from <= start < end <= to`, optionally
/// limited to `end - start <= max_days`.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    pub from: Date,
    pub to: Date,
    pub max_days: Option<i64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Example {
    pub start: Date,
    pub end: Date,
    pub a: Rational,
    pub b: Rational,
}

impl Example {
    fn sort_key(&self) -> (i64, Date) {
        (self.start.days_until(self.end), self.start)
    }
}

/// Differing pairs that share the same branches taken by only one side.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Group {
    /// Branches convention A took and B did not.
    pub only_a: Trace,
    /// Branches convention B took and A did not.
    pub only_b: Trace,
    pub count: u64,
    /// The shortest differing period in the group, earliest start on ties.
    pub example: Example,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Report {
    pub a: Convention,
    pub b: Convention,
    pub pairs_checked: u64,
    pub differing: u64,
    /// Sorted by descending count, then by branch sets.
    pub groups: Vec<Group>,
}

type GroupKey = (Trace, Trace);

fn merge_into(groups: &mut HashMap<GroupKey, Group>, group: Group) {
    groups
        .entry((group.only_a, group.only_b))
        .and_modify(|g| {
            g.count += group.count;
            if group.example.sort_key() < g.example.sort_key() {
                g.example = group.example;
            }
        })
        .or_insert(group);
}

/// Pairs checked, and the groups found, by one worker.
type ScanResult = (u64, HashMap<GroupKey, Group>);

fn scan_starts(
    a: Convention,
    b: Convention,
    context: &Context,
    window: Window,
    starts: impl Iterator<Item = i64>,
) -> Result<ScanResult, Error> {
    let to = window.to.serial();
    let mut checked = 0u64;
    let mut groups = HashMap::new();
    for start_serial in starts {
        let start = Date::from_serial(start_serial)?;
        let last = window.max_days.map_or(to, |m| to.min(start_serial + m));
        for end_serial in start_serial + 1..=last {
            let end = Date::from_serial(end_serial)?;
            let fa = year_fraction(a, start, end, context)?;
            let fb = year_fraction(b, start, end, context)?;
            checked += 1;
            if fa.value != fb.value {
                merge_into(
                    &mut groups,
                    Group {
                        only_a: fa.trace.difference(fb.trace),
                        only_b: fb.trace.difference(fa.trace),
                        count: 1,
                        example: Example {
                            start,
                            end,
                            a: fa.value,
                            b: fb.value,
                        },
                    },
                );
            }
        }
    }
    Ok((checked, groups))
}

/// Compares conventions `a` and `b` on every pair in `window`, using up to
/// `threads` worker threads. The result does not depend on `threads`.
pub fn disagree(
    a: Convention,
    b: Convention,
    context: &Context,
    window: Window,
    threads: usize,
) -> Result<Report, Error> {
    if window.from > window.to {
        return Err(Error::StartAfterEnd {
            start: window.from,
            end: window.to,
        });
    }
    // Surface missing deal terms before spawning anything.
    year_fraction(a, window.from, window.from, context)?;
    year_fraction(b, window.from, window.from, context)?;

    let (from, to) = (window.from.serial(), window.to.serial());
    let threads = threads.clamp(1, 256) as i64;
    // Interleave start dates across workers so each gets a similar mix of
    // early starts (many pairs) and late starts (few pairs).
    let results: Vec<Result<ScanResult, Error>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                scope.spawn(move || {
                    let starts = (from..to).filter(move |s| (s - from) % threads == t);
                    scan_starts(a, b, context, window, starts)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("worker thread panicked"))
            .collect()
    });

    let mut pairs_checked = 0;
    let mut merged = HashMap::new();
    for result in results {
        let (checked, groups) = result?;
        pairs_checked += checked;
        for group in groups.into_values() {
            merge_into(&mut merged, group);
        }
    }
    let mut groups: Vec<Group> = merged.into_values().collect();
    groups.sort_by(|x, y| {
        y.count
            .cmp(&x.count)
            .then((x.only_a, x.only_b).cmp(&(y.only_a, y.only_b)))
    });
    let differing = groups.iter().map(|g| g.count).sum();
    Ok(Report {
        a,
        b,
        pairs_checked,
        differing,
        groups,
    })
}

/// A triple `a <= b <= c` with `f(a, c) != f(a, b) + f(b, c)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AdditivityCounterexample {
    pub a: Date,
    pub b: Date,
    pub c: Date,
    pub whole: Rational,
    pub sum_of_parts: Rational,
}

/// Searches triples inside `window` (with `c - a <= max_days` if given) for a
/// failure of additivity, shortest `c - a` first. `None` means the convention
/// is additive on every triple examined.
pub fn find_additivity_counterexample(
    convention: Convention,
    context: &Context,
    window: Window,
) -> Result<Option<AdditivityCounterexample>, Error> {
    let (from, to) = (window.from.serial(), window.to.serial());
    let max_span = window.max_days.unwrap_or(to - from).min(to - from);
    for span in 0..=max_span {
        for a_serial in from..=to - span {
            let c_serial = a_serial + span;
            let a = Date::from_serial(a_serial)?;
            let c = Date::from_serial(c_serial)?;
            let whole = year_fraction(convention, a, c, context)?.value;
            for b_serial in a_serial..=c_serial {
                let b = Date::from_serial(b_serial)?;
                let sum_of_parts = year_fraction(convention, a, b, context)?
                    .value
                    .checked_add(year_fraction(convention, b, c, context)?.value)?;
                if sum_of_parts != whole {
                    return Ok(Some(AdditivityCounterexample {
                        a,
                        b,
                        c,
                        whole,
                        sum_of_parts,
                    }));
                }
            }
        }
    }
    Ok(None)
}
