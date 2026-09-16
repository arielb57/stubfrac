# stubfrac

Day-count year fractions as exact rationals, with irregular ICMA stubs, the Canadian bond rule, a trace of every rule branch taken, and a tool that maps where two conventions disagree.

## The problem

Almost every rates library ships "30/360" and "ACT/ACT", but they quietly implement different variants: bond basis or 30E, ISDA or ICMA or AFB, with or without the February end-of-month rules. Few handle ACT/ACT ICMA long and short stubs correctly, and fewer handle the Canadian ACT/365 bond rule at all. When two systems disagree on accrued interest by one day's worth, floating-point output and half-remembered rules make it impossible to say which rule each side applied. Reconciling bond cash flows needs exact fractions and a precise list of the dates where two conventions part ways.

## How it works

**Exact arithmetic.** Every result is a `Rational` with an `i64` numerator and denominator, always in lowest terms. Additions go through `i128` and return `Error::Overflow` rather than wrapping if the reduced result does not fit. ACT/ACT ISDA over 2003-11-01..2004-05-01 is `66491/133590`, not `0.4977243805`.

**A trace per result.** Each calculation records the rule branches it took ("D2 = 31 kept because D1 < 30", "counting back from Feb 28 landed on Feb 29", "stub spanning several reference periods") in a 64-bit set. It never allocates, which is what makes the enumeration below affordable.

**ICMA and Canadian reference periods.** A coupon schedule is reduced to a frequency `f`, one anchor date (usually maturity) and an end-of-month flag. Roll date `k` is computed in closed form as `anchor + k·12/f` months, with the day clamped to the month length, or forced to month end when the anchor is a month end and `--eom` is set. `period_index(d)` finds the `k` with `roll(k) <= d < roll(k+1)`, and an accrual period is split into its overlaps ("pieces") with consecutive reference periods:

- ACT/ACT ICMA sums `days / (f · reference days)` over the pieces, and gives exactly `1/f` for each full piece. That covers short and long stubs, at either end.
- ACT/365 Canadian Bond, per piece: `1/f` for a full period, `days/365` when `days < 365/f`, otherwise `1/f − (reference days − days)/365`.

Worked example, the long first coupon from the ISDA 1998 paper (semi-annual, first coupon 2003-07-15):

```
accrual 2002-08-15 -> 2003-07-15
  piece 1: 153 days of reference period 2002-07-15 -> 2003-01-15 (184 days)  153 / (2·184)
  piece 2: full reference period 2003-01-15 -> 2003-07-15                    1/2
  total                                                                      337/368 = 0.91576
ACT/ACT ISDA gives 334/365 = 0.91507: all 334 days fall in the common year 2002/2003.
```

**The other conventions.** ACT/360, ACT/365F, ACT/365L (annual: 366 if Feb 29 is in (start, end]; otherwise 366 if the end date is in a leap year), NL/365 (Feb 29s removed), ACT/ACT ISDA (days in each calendar year over that year's length), ACT/ACT AFB (whole years counted back from the end date, Feb 28 going back to Feb 29 in a leap year, remainder over 366 if it contains Feb 29), and the 30/360 family: Bond Basis, 30U/360 with the SIA February rules, 30E/360, and 30E/360 ISDA, which needs the termination date.

**Disagreement map.** `stubfrac disagree A B` evaluates both conventions on every pair `from <= start < end <= to`. For each pair where the fractions differ it takes the branches only A took and the branches only B took. That pair of sets is the group key, so branches both sides share, such as "D1 = 31 -> 30", drop out and the key names the rule that caused the split. Each group keeps a count and its minimal example: the shortest period, earliest start on ties. Start dates are dealt round-robin to worker threads (`std::thread::scope`), and the per-thread group maps are merged with the same min-example rule, so the report does not depend on the thread count. 2000-01-01..2040-12-31 is 112,132,800 pairs.

## Install and usage

Requires Rust 1.85 or newer. No runtime dependencies. From a checkout of this repository:

```sh
cargo test                       # the full suite, about 10 s
cargo build --release
./target/release/stubfrac list
```

```
id               name                   needs
act360           ACT/360
act365f          ACT/365F
act365l          ACT/365L               --freq
nl365            NL/365
actact-isda      ACT/ACT ISDA
actact-icma      ACT/ACT ICMA           --freq --anchor [--eom]
actact-afb       ACT/ACT AFB
30-360           30/360 Bond Basis
30u-360          30U/360 (SIA)
30e-360          30E/360
30e-360-isda     30E/360 ISDA           --termination
act365-canadian  ACT/365 Canadian Bond  --freq --anchor [--eom]
```

### `frac`: one period, with its trace

```sh
stubfrac frac actact-icma 2002-08-15 2003-07-15 --freq 2 --anchor 2003-07-15
```

```
convention: ACT/ACT ICMA
period:     2002-08-15 -> 2003-07-15 (334 days)
fraction:   337/368 = 0.9157608696
trace:
  - stub spanning several reference periods
  - start date is not a roll date
  - full reference period: 1/f
  - partial piece: days / (f * reference period days)
reference periods:
  - 2002-07-15 -> 2003-01-15 (184 days): accrues 2002-08-15 -> 2003-01-15 (153 days)
  - 2003-01-15 -> 2003-07-15 (181 days): accrues 2003-01-15 -> 2003-07-15 (181 days)
```

```sh
stubfrac frac act365-canadian 2020-07-15 2021-01-14 --freq 2 --anchor 2021-01-15
```

```
convention: ACT/365 Canadian Bond
period:     2020-07-15 -> 2021-01-14 (183 days)
fraction:   363/730 = 0.4972602740
trace:
  - stub inside a single reference period
  - end date is not a roll date
  - partial piece of at least 365/f days: 1/f - (period days - days)/365
reference periods:
  - 2020-07-15 -> 2021-01-15 (184 days): accrues 2020-07-15 -> 2021-01-14 (183 days)
```

### `disagree`: where two conventions part ways

```sh
stubfrac disagree 30e-360 30e-360-isda --from 2000-01-01 --to 2040-12-31 --termination 2040-02-29
```

```
30E/360 vs 30E/360 ISDA, 2000-01-01 to 2040-12-31
598046 of 112132800 pairs differ, in 4 groups

group 1 (311246 pairs)
  30E/360 only: D1 last day of Feb kept
  30E/360 ISDA only: D1 last day of Feb -> 30
  minimal example: 2000-02-29 -> 2000-03-01 (1 day): 30E/360 = 1/180, 30E/360 ISDA = 1/360

group 2 (286460 pairs)
  30E/360 only: D2 last day of Feb kept
  30E/360 ISDA only: D2 last day of Feb, not the termination date -> 30
  minimal example: 2000-02-28 -> 2000-02-29 (1 day): 30E/360 = 1/360, 30E/360 ISDA = 1/180

group 3 (300 pairs)
  30E/360 only: D1 last day of Feb kept; D2 last day of Feb kept
  30E/360 ISDA only: D1 last day of Feb -> 30; D2 last day of Feb, not the termination date -> 30
  minimal example: 2000-02-29 -> 2001-02-28 (365 days): 30E/360 = 359/360, 30E/360 ISDA = 1

group 4 (40 pairs)
  30E/360 only: D1 last day of Feb kept; D2 last day of Feb kept
  30E/360 ISDA only: D1 last day of Feb -> 30; D2 last day of Feb is the termination date: kept
  minimal example: 2039-02-28 -> 2040-02-29 (366 days): 30E/360 = 361/360, 30E/360 ISDA = 359/360
```

Options: `--max-days N` limits period length, and `--threads N` defaults to all cores. `--termination` defaults to `--to`.

### `additive`: search for f(a,c) ≠ f(a,b) + f(b,c)

```sh
stubfrac additive 30e-360-isda --from 2020-01-01 --to 2021-12-31 --termination 2021-02-28 --max-days 60
```

```
30E/360 ISDA is not additive: f(2021-02-27, 2021-03-01) = 1/90 but f(2021-02-27, 2021-02-28) + f(2021-02-28, 2021-03-01) = 1/180
```

### As a library

```rust
use stubfrac::{year_fraction, Context, Convention, Date, Rational};

let start: Date = "2002-08-15".parse()?;
let end: Date = "2003-07-15".parse()?;
let ctx = Context { frequency: Some(2), anchor: Some(end), ..Context::default() };
let icma = year_fraction(Convention::ActActIcma, start, end, &ctx)?;
assert_eq!(icma.value, Rational::new(337, 368)?);
```

`stubfrac::disagree::{disagree, find_additivity_counterexample}` expose the enumerators.

## Results

### Disagreement groups, 2000-01-01 to 2040-12-31

Every pair `2000-01-01 <= start < end <= 2040-12-31` was checked (112,132,800 pairs), except where a span limit is shown. 30E/360 ISDA used termination date 2040-02-29. Each row is one group from `stubfrac disagree`, and each example is the shortest differing period in that group.

| A vs B | Pairs that differ | A only | B only | Minimal example | A | B |
|---|---|---|---|---|---|---|
| 30/360 Bond Basis vs 30E/360 | 2,047,834 | D2 = 31 kept because D1 < 30 | D2 = 31 -> 30 | 2000-01-29 -> 2000-01-31 | 1/180 | 1/360 |
| 30/360 Bond Basis vs 30U/360 (SIA) | 305,260 | D1 last day of Feb kept | D1 last day of Feb -> 30 | 2000-02-29 -> 2000-03-01 | 1/180 | 1/360 |
| | 5,986 | D1 Feb end kept; D2 = 31 kept because D1 < 30 | D1 Feb end -> 30; D2 = 31 and D1 is 30 or 31 -> 30 | 2000-02-29 -> 2000-03-31 | 4/45 | 1/12 |
| | 330 | D1 Feb end kept; D2 Feb end kept | D1 Feb end -> 30; D1 and D2 both Feb end -> D2 = 30 | 2000-02-29 -> 2001-02-28 | 359/360 | 1 |
| 30E/360 vs 30E/360 ISDA | 311,246 | D1 last day of Feb kept | D1 last day of Feb -> 30 | 2000-02-29 -> 2000-03-01 | 1/180 | 1/360 |
| | 286,460 | D2 last day of Feb kept | D2 Feb end, not termination -> 30 | 2000-02-28 -> 2000-02-29 | 1/360 | 1/180 |
| | 300 | D1 and D2 Feb end kept | D1 -> 30; D2 not termination -> 30 | 2000-02-29 -> 2001-02-28 | 359/360 | 1 |
| | 40 | D1 and D2 Feb end kept | D1 -> 30; D2 is termination: kept | 2039-02-28 -> 2040-02-29 | 361/360 | 359/360 |
| 30U/360 (SIA) vs 30E/360 ISDA | 2,041,848 | D2 = 31 kept because D1 < 30 | D2 = 31 -> 30 | 2000-01-29 -> 2000-01-31 | 1/180 | 1/360 |
| | 286,460 | D2 last day of Feb kept | D2 Feb end, not termination -> 30 | 2000-02-28 -> 2000-02-29 | 1/360 | 1/180 |
| | 40 | D1 and D2 both Feb end -> D2 = 30 | D2 is termination: kept | 2039-02-28 -> 2040-02-29 | 1 | 359/360 |
| ACT/ACT ISDA vs ACT/ACT AFB, periods <= 366 days (5,414,055 pairs) | 838,710 | split at year end; leap and common days | no Feb 29, /365 | 2000-12-31 -> 2001-01-02 | 731/133590 | 2/365 |
| | 538,572 | inside one year; leap days /366 | no Feb 29, /365 | 2000-01-01 -> 2000-01-02 | 1/366 | 1/365 |
| | 486,240 | split at year end; leap and common days | Feb 29 in remainder, /366 | 2003-12-31 -> 2004-02-29 | 21901/133590 | 10/61 |
| | 7,280 | split at year end; leap and common days | whole years counted back | 2000-03-01 -> 2001-03-01 | 22214/22265 | 1 |
| | 3,650 | split at year end; leap and common days | whole years; no Feb 29, /365 | 2000-02-29 -> 2001-03-01 | 133649/133590 | 366/365 |
| | 10 | split at year end; leap and common days | whole years; Feb 28 back to Feb 29 | 2000-02-29 -> 2001-02-28 | 133283/133590 | 1 |
| | 10 | split at year end; leap and common days | whole years; Feb 28 back to Feb 29; Feb 29 in remainder | 2000-02-28 -> 2001-02-28 | 66824/66795 | 367/366 |
| ACT/ACT ICMA vs ACT/365 Canadian, semi-annual Jan/Jul 15, periods <= 366 days | 5,399,174 | partial piece: days/(f·ref days) | piece under 365/f days: days/365 | 2000-01-01 -> 2000-01-02 | 1/368 | 1/365 |
| | 14,480 | partial piece | under 365/f; and at least 365/f | 2000-07-14 -> 2001-01-14 | 16745/33488 | 1/2 |
| | 160 | partial piece | at least 365/f: 1/f − (ref − days)/365 | 2000-07-15 -> 2001-01-14 | 183/368 | 363/730 |

Branch labels are shortened in the table ("Feb end" = last day of February); the CLI prints them in full.

Commands that reproduce the table (each prints the full group labels):

```sh
B=./target/release/stubfrac
$B disagree 30-360  30e-360      --from 2000-01-01 --to 2040-12-31
$B disagree 30-360  30u-360      --from 2000-01-01 --to 2040-12-31
$B disagree 30e-360 30e-360-isda --from 2000-01-01 --to 2040-12-31 --termination 2040-02-29
$B disagree 30u-360 30e-360-isda --from 2000-01-01 --to 2040-12-31 --termination 2040-02-29
$B disagree actact-isda actact-afb --from 2000-01-01 --to 2040-12-31 --max-days 366
$B disagree actact-icma act365-canadian --from 2000-01-01 --to 2040-12-31 --freq 2 --anchor 2040-07-15 --max-days 366
```

Each full 112-million-pair 30/360 run took about 2 s of wall time (12.5 s CPU) on a multi-core macOS laptop. This is not a performance claim, just what to expect when you run them.

Things the map shows:

- **30/360 Bond Basis and 30E/360 differ for exactly one reason**: an end date on the 31st with a start day below 30. The test suite counts those pairs independently for 2020 and checks the count.
- **The Canadian rule is discontinuous by design.** 2000-07-14 -> 2001-01-14 is not a coupon period, yet it accrues exactly 1/2. The 1-day piece before the roll date gives 1/365, and the 183-day piece gives 1/2 − 1/365.
- **AFB can exceed 1 on a one-year period.** 2000-02-28 -> 2001-02-28 counts back to 2000-02-29 and leaves one day over 366, giving 367/366.

### What the tests prove

`cargo test` runs 43 tests, about 10 s on a laptop:

- **Published vectors** (`tests/published_vectors.rs`). The ISDA 1998 *EMU and market conventions* ACT/ACT cases (regular period, short first stub, long first stub, the penultimate and final periods of a short final stub) under ISDA, ICMA and AFB, each asserted as an exact rational and as the 5-decimal figure printed in the paper. Also the 22-row 30/360, 30E/360 and 30E/360 ISDA day-count table that accompanies ISDA 2006 §4.16, the termination-date rule, the SIA February rules, ACT/365L, NL/365, AFB whole-year counting and a hand-derived Canadian example.
- **Property tests** (`tests/properties.rs`, proptest, 512 cases each, shrinking capped at 2,000 iterations or 20 s). ICMA gives exactly `n/f` over any `n` whole periods for random frequencies, anchors and EOM flags, and `days/(f·P)` on any single-period stub. ACT/360, ACT/365F, NL/365, ACT/ACT ISDA, ACT/ACT ICMA and 30E/360 are additive. `f(a,a) = 0`, `f(a,b) >= 0`, and `start > end` is an error for all twelve conventions. Canadian gives `1/f` on a full period, and `days/365` or `1/f − shortfall/365` on each side of the `365/f` threshold. ACT/ACT ISDA equals a day-by-day sum. AFB gives an integer on whole calendar years.
- **Non-additivity** (`tests/disagree.rs`). `find_additivity_counterexample` must find a counterexample for 30/360 Bond Basis, 30U/360 and 30E/360 ISDA (and AFB), and the triple it returns is re-verified. 30E/360 is the exception: its two day adjustments do not depend on each other, so it telescopes, and the test asserts that no counterexample exists.
- **Exhaustive** (`tests/exhaustive.rs`). Every pair in 2019-01-01..2021-12-31, including empty periods, goes through all twelve conventions under three schedules, 21.6 million evaluations. The test requires no panic, no error and no overflow, a non-empty trace, and a value inside the convention's bounds (for example `days/372 <= ICMA <= days/336`).
- **Enumerator vs brute force** (`tests/disagree.rs`). Seven convention pairs over a one-year window, with and without a span limit, on 1 and 7 threads, compared against an oracle that materializes every differing pair, sorts and groups them. Counts, group keys, minimal examples and total pairs must all match.

## Design notes

**Group by the symmetric difference of branch sets, not by the full trace.** Keying on (A's trace, B's trace) splits one real cause into many groups: Bond Basis vs 30E/360 would get separate groups for "D1 = 31" and "D1 ≠ 31" even though that rule is identical on both sides. Removing the shared branches collapses them. The cost is that branches must be modelled as shared identities: "D1 = 31 -> 30" is the same `Branch` value in all four 30/360 variants. It also means a convention's base formula needs a branch of its own ("partial piece: days / (f · reference days)"). Otherwise, when one convention's trace is a subset of the other's, one side of the group shows as "(none)".

**Closed-form roll dates instead of stepping.** Roll date `k` is `anchor + k·(12/f)` months, computed directly rather than by stepping from the previous roll. That gives two properties for free. Rolling backward from maturity and forward from an anchor produce the same grid, so ICMA needs no "direction" setting, only the anchor. And a 31st anchor does not decay to the 30th after passing through a 30-day month (2021-08-31 quarterly rolls back to 05-31, 02-28, 11-30, 08-31). The trade-off: a system that steps month by month *does* drift, and stubfrac will not reproduce its numbers. That drift is a disagreement between implementations rather than between conventions, so it is out of scope.

Two smaller choices. Every convention returns 0 for `start == end` before applying any rule, because 30E/360 ISDA with `start = end = termination = Feb 28` would otherwise give −2/360. The Canadian rule is applied per reference-period piece, so a long stub is a sum of the per-period rule. The source definition covers a single coupon period, so the multi-period behaviour is this library's extension.

## Limitations

- **No business-day calendars or date adjustment.** Dates are taken as given. Schedules are only generated as far as a day count needs; there is no coupon schedule builder with stub placement rules.
- **30U/360 applies the SIA February rules unconditionally.** Some systems apply them only to end-of-month securities, and that variant is not implemented.
- **ACT/365L and AFB follow one reading each.** ACT/365L uses the end date's year for non-annual coupons and "Feb 29 in (start, end]" for annual. AFB uses the Feb 28 -> Feb 29 back-counting rule, which is why a 366-day period can give 367/366. Implementations that differ on these points will disagree with stubfrac.
- **ACT/365L takes the frequency from `--freq` only.** It does not check that the period is actually one coupon long.
- **Enumeration is O(pairs).** 2000–2040 is instant for 30/360 variants. Conventions that loop over years (AFB, ISDA) or reference periods get slower on long spans, so use `--max-days` for them.
- **Test-vector provenance.** The published vectors were transcribed by hand from the ISDA documents, and each was also re-derived from the rules in a comment or in the rational literal. They are not machine-extracted from the PDFs.
- **Dates are proleptic Gregorian, years 0001–9999.**

## License

MIT. See [LICENSE](LICENSE).
