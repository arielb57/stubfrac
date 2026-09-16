use std::process::ExitCode;

use stubfrac::disagree::{disagree, find_additivity_counterexample, Window};
use stubfrac::{year_fraction, Context, Convention, Date, Error};

const USAGE: &str = "\
stubfrac: exact day-count year fractions and where conventions disagree

USAGE:
  stubfrac list
  stubfrac frac <convention> <start> <end> [deal terms]
  stubfrac disagree <convention-a> <convention-b> --from DATE --to DATE
                    [--max-days N] [--threads N] [deal terms]
  stubfrac additive <convention> --from DATE --to DATE [--max-days N] [deal terms]

DEAL TERMS (only read by the conventions that need them):
  --freq N             coupons per year: 1, 2, 3, 4, 6 or 12
  --anchor DATE        a coupon roll date, usually maturity
  --eom                roll on month ends when the anchor is a month end
  --termination DATE   termination date for 30E/360 ISDA
                       (disagree/additive default it to --to)

Dates are YYYY-MM-DD. Pairs examined by `disagree` are from <= start < end <= to.
";

struct Args {
    positional: Vec<String>,
    context: Context,
    from: Option<Date>,
    to: Option<Date>,
    max_days: Option<i64>,
    threads: Option<usize>,
}

fn parse_args(raw: &[String]) -> Result<Args, String> {
    let mut args = Args {
        positional: Vec::new(),
        context: Context::default(),
        from: None,
        to: None,
        max_days: None,
        threads: None,
    };
    let mut iter = raw.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        let date = |s: String| s.parse::<Date>().map_err(|e| e.to_string());
        let number = |name: &str, s: String| {
            s.parse::<u64>()
                .map_err(|_| format!("{name}: '{s}' is not a non-negative integer"))
        };
        match arg.as_str() {
            "--freq" => args.context.frequency = Some(number("--freq", value("--freq")?)? as u32),
            "--anchor" => args.context.anchor = Some(date(value("--anchor")?)?),
            "--termination" => args.context.termination = Some(date(value("--termination")?)?),
            "--eom" => args.context.end_of_month = true,
            "--from" => args.from = Some(date(value("--from")?)?),
            "--to" => args.to = Some(date(value("--to")?)?),
            "--max-days" => {
                args.max_days = Some(number("--max-days", value("--max-days")?)? as i64)
            }
            "--threads" => args.threads = Some(number("--threads", value("--threads")?)? as usize),
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            _ => args.positional.push(arg.clone()),
        }
    }
    Ok(args)
}

fn convention(s: &str) -> Result<Convention, String> {
    s.parse().map_err(|e: Error| e.to_string())
}

fn window(args: &Args) -> Result<Window, String> {
    let from = args.from.ok_or("--from is required")?;
    let to = args.to.ok_or("--to is required")?;
    Ok(Window {
        from,
        to,
        max_days: args.max_days,
    })
}

fn cmd_list() {
    println!("{:<16} {:<22} needs", "id", "name");
    for c in Convention::ALL {
        println!("{:<16} {:<22} {}", c.id(), c.name(), c.requirements());
    }
}

fn days(n: i64) -> String {
    if n == 1 {
        "1 day".to_string()
    } else {
        format!("{n} days")
    }
}

fn cmd_frac(args: &Args) -> Result<(), String> {
    let [_, conv, start, end] = args.positional.as_slice() else {
        return Err("frac takes <convention> <start> <end>".into());
    };
    let conv = convention(conv)?;
    let start: Date = start.parse().map_err(|e: Error| e.to_string())?;
    let end: Date = end.parse().map_err(|e: Error| e.to_string())?;
    let result = year_fraction(conv, start, end, &args.context).map_err(|e| e.to_string())?;
    println!("convention: {}", conv.name());
    println!(
        "period:     {start} -> {end} ({})",
        days(start.days_until(end))
    );
    println!(
        "fraction:   {} = {}",
        result.value,
        result.value.to_decimal(10)
    );
    println!("trace:");
    for branch in result.trace.branches() {
        println!("  - {branch}");
    }
    if matches!(conv, Convention::ActActIcma | Convention::Act365Canadian) {
        let schedule = args.context.schedule(conv).map_err(|e| e.to_string())?;
        println!("reference periods:");
        for piece in schedule.pieces(start, end) {
            println!(
                "  - {} -> {} ({}): accrues {} -> {} ({})",
                piece.period_start,
                piece.period_end,
                days(piece.period_days()),
                piece.from,
                piece.to,
                days(piece.days())
            );
        }
    }
    Ok(())
}

fn with_default_termination(args: &Args) -> Context {
    let mut context = args.context;
    if context.termination.is_none() {
        context.termination = args.to;
    }
    context
}

fn cmd_disagree(args: &Args) -> Result<(), String> {
    let [_, a, b] = args.positional.as_slice() else {
        return Err("disagree takes <convention-a> <convention-b>".into());
    };
    let (a, b) = (convention(a)?, convention(b)?);
    let window = window(args)?;
    let threads = args
        .threads
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));
    let context = with_default_termination(args);
    let report = disagree(a, b, &context, window, threads).map_err(|e| e.to_string())?;

    let span = window
        .max_days
        .map_or(String::new(), |m| format!(", periods up to {m} days"));
    println!(
        "{} vs {}, {} to {}{span}",
        a.name(),
        b.name(),
        window.from,
        window.to
    );
    println!(
        "{} of {} pairs differ, in {} groups",
        report.differing,
        report.pairs_checked,
        report.groups.len()
    );
    for (i, group) in report.groups.iter().enumerate() {
        let ex = group.example;
        println!();
        println!("group {} ({} pairs)", i + 1, group.count);
        println!("  {} only: {}", a.name(), group.only_a);
        println!("  {} only: {}", b.name(), group.only_b);
        println!(
            "  minimal example: {} -> {} ({}): {} = {}, {} = {}",
            ex.start,
            ex.end,
            days(ex.start.days_until(ex.end)),
            a.name(),
            ex.a,
            b.name(),
            ex.b
        );
    }
    Ok(())
}

fn cmd_additive(args: &Args) -> Result<(), String> {
    let [_, conv] = args.positional.as_slice() else {
        return Err("additive takes <convention>".into());
    };
    let conv = convention(conv)?;
    let window = window(args)?;
    let context = with_default_termination(args);
    match find_additivity_counterexample(conv, &context, window).map_err(|e| e.to_string())? {
        Some(cx) => println!(
            "{} is not additive: f({}, {}) = {} but f({}, {}) + f({}, {}) = {}",
            conv.name(),
            cx.a,
            cx.c,
            cx.whole,
            cx.a,
            cx.b,
            cx.b,
            cx.c,
            cx.sum_of_parts
        ),
        None => println!(
            "{} is additive on every triple in {} to {}{}",
            conv.name(),
            window.from,
            window.to,
            window
                .max_days
                .map_or(String::new(), |m| format!(" spanning at most {m} days"))
        ),
    }
    Ok(())
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() || raw.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return if raw.is_empty() {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        };
    }
    let result =
        parse_args(&raw).and_then(|args| match args.positional.first().map(String::as_str) {
            Some("list") => {
                cmd_list();
                Ok(())
            }
            Some("frac") => cmd_frac(&args),
            Some("disagree") => cmd_disagree(&args),
            Some("additive") => cmd_additive(&args),
            Some(other) => Err(format!("unknown command '{other}'\n\n{USAGE}")),
            None => Err(USAGE.to_string()),
        });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}
