//! Value formatting: decimals/rounding for money & numbers, and date reshaping.
//!
//! Both are done with integer math / string scanning rather than pulling in a
//! decimal or datetime crate. That keeps the wasm bundle small and — more
//! importantly — keeps the output bit-for-bit identical between native and wasm,
//! since there's no floating-point formatting in the hot path.

use crate::schema::{NumberFormat, Rounding};

/// Format a raw value string as a number. If it doesn't parse, the raw string is
/// returned untouched (better a visible bad value than a silent blank).
pub fn format_number(raw: &str, fmt: &NumberFormat) -> String {
    let value: f64 = match raw.trim().replace(',', "").parse() {
        Ok(v) => v,
        Err(_) => return raw.to_string(),
    };
    // Clamp decimals hard: money never needs more than a handful, and 10^15 is
    // the largest power of ten that is still an exact f64 integer *and* keeps
    // 10^decimals well within u128 — so no `pow` overflow and no float rounding
    // surprise (defends against a hand-authored `decimals: 200`).
    let decimals = (fmt.decimals as u32).min(15);
    let negative = value.is_sign_negative() && value != 0.0;
    // Integer power of ten cast to f64 (exact, and identical native/wasm — no
    // `powi` intrinsic whose expansion could differ across targets).
    let divisor = 10u128.pow(decimals);
    let scale = divisor as f64;
    let scaled = value.abs() * scale;

    // Round on the non-negative magnitude, then re-apply the sign — symmetric and
    // easy to reason about for every method.
    let rounded: u128 = match fmt.rounding {
        Rounding::HalfUp => (scaled + 0.5).floor() as u128,
        Rounding::Down => scaled.floor() as u128,
        Rounding::Up => scaled.ceil() as u128,
        Rounding::HalfEven => {
            let floor = scaled.floor();
            let f = floor as u128;
            let diff = scaled - floor;
            // Ties round to even: up when diff > 0.5, or exactly 0.5 and f is odd.
            if diff > 0.5 || (diff == 0.5 && !f.is_multiple_of(2)) {
                f + 1
            } else {
                f
            }
        }
    };

    let divisor = 10u128.pow(decimals);
    let int_part = rounded / divisor;
    let frac_part = rounded % divisor;

    let int_str = if fmt.thousands {
        group_thousands(int_part)
    } else {
        int_part.to_string()
    };

    let mut out = String::new();
    if negative && rounded != 0 {
        out.push('-');
    }
    out.push_str(&int_str);
    if decimals > 0 {
        out.push('.');
        out.push_str(&format!("{:0width$}", frac_part, width = decimals as usize));
    }
    out
}

fn group_thousands(mut n: u128) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    while n > 0 {
        parts.push(format!("{:03}", n % 1000));
        n /= 1000;
    }
    parts.reverse();
    // Trim leading zeros on the most-significant group.
    let mut s = parts.join(",");
    while s.starts_with('0') && s.len() > 1 && s.as_bytes()[1] != b',' {
        s.remove(0);
    }
    s
}

/// Reshape a timestamp string against a pattern. Supported tokens:
/// `YYYY YY MM DD HH mm ss`. Everything else in the pattern is literal.
///
/// The input is parsed leniently: the numeric groups are read in ISO order
/// (year, month, day, hour, minute, second), which matches the `2030-01-01
/// 12:12:22` shape the spec's data uses.
pub fn format_date(raw: &str, pattern: &str) -> String {
    let nums: Vec<&str> = raw
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .collect();
    if nums.is_empty() {
        return raw.to_string();
    }
    let get = |i: usize| nums.get(i).copied().unwrap_or("0");
    let year = get(0);
    let month = pad2(get(1));
    let day = pad2(get(2));
    let hour = pad2(get(3));
    let min = pad2(get(4));
    let sec = pad2(get(5));
    let yy = if year.len() >= 2 { &year[year.len() - 2..] } else { year };

    // Longest tokens first so `YYYY` is consumed before `YY`.
    pattern
        .replace("YYYY", year)
        .replace("YY", yy)
        .replace("MM", &month)
        .replace("DD", &day)
        .replace("HH", &hour)
        .replace("mm", &min)
        .replace("ss", &sec)
}

fn pad2(s: &str) -> String {
    if s.len() == 1 {
        format!("0{s}")
    } else {
        s.to_string()
    }
}
