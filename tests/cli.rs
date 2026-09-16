use std::process::Command;

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_stubfrac"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn frac_prints_exact_value_trace_and_reference_periods() {
    let (ok, out, _) = run(&[
        "frac",
        "actact-icma",
        "2002-08-15",
        "2003-07-15",
        "--freq",
        "2",
        "--anchor",
        "2003-07-15",
    ]);
    assert!(ok);
    assert!(out.contains("fraction:   337/368 = 0.9157608696"), "{out}");
    assert!(
        out.contains("stub spanning several reference periods"),
        "{out}"
    );
    assert!(
        out.contains(
            "2002-07-15 -> 2003-01-15 (184 days): accrues 2002-08-15 -> 2003-01-15 (153 days)"
        ),
        "{out}"
    );
}

#[test]
fn disagree_prints_groups_with_minimal_examples() {
    let (ok, out, _) = run(&[
        "disagree",
        "30-360",
        "30e-360",
        "--from",
        "2020-01-01",
        "--to",
        "2020-12-31",
        "--threads",
        "2",
    ]);
    assert!(ok);
    assert!(
        out.contains("1334 of 66795 pairs differ, in 1 groups"),
        "{out}"
    );
    assert!(
        out.contains("minimal example: 2020-01-29 -> 2020-01-31 (2 days)"),
        "{out}"
    );
}

#[test]
fn additive_reports_a_counterexample() {
    let (ok, out, _) = run(&[
        "additive",
        "30u-360",
        "--from",
        "2020-01-01",
        "--to",
        "2020-12-31",
        "--max-days",
        "40",
    ]);
    assert!(ok);
    assert!(out.contains("30U/360 (SIA) is not additive"), "{out}");
}

#[test]
fn errors_exit_non_zero_with_a_message() {
    let (ok, _, err) = run(&["frac", "actact-icma", "2020-01-01", "2020-02-01"]);
    assert!(!ok);
    assert!(err.contains("needs a coupon frequency"), "{err}");
    let (ok, _, err) = run(&["frac", "act360", "2020-02-30", "2020-03-01"]);
    assert!(!ok);
    assert!(err.contains("invalid date"), "{err}");
    let (ok, _, err) = run(&["frac", "act360", "2020-03-01", "2020-02-01"]);
    assert!(!ok);
    assert!(err.contains("is after end"), "{err}");
    let (ok, _, err) = run(&["bogus"]);
    assert!(!ok);
    assert!(err.contains("unknown command"), "{err}");
}

#[test]
fn list_names_every_convention() {
    let (ok, out, _) = run(&["list"]);
    assert!(ok);
    for id in [
        "act360",
        "act365l",
        "actact-icma",
        "actact-afb",
        "30e-360-isda",
        "act365-canadian",
    ] {
        assert!(out.contains(id), "{out}");
    }
}
