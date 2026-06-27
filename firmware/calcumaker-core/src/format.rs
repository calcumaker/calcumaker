//! Render a [`Value`] for the display, honoring the integer radix and the
//! working precision.

use alloc::format;
use alloc::string::{String, ToString};

use gmp_mpfr_nostd::Float;

use crate::calc::Radix;
use crate::value::Value;

/// Decimal significant digits worth showing for a given binary precision.
fn dec_digits(prec: u32) -> usize {
    // log10(2) ≈ 0.30103; keep at least a few.
    ((prec as f64) * 0.30103) as usize + 1
}

pub fn format(v: &Value, radix: Radix, prec: u32) -> String {
    match v {
        Value::Int(i) => {
            let s = i.to_string_radix(radix.base());
            if radix == Radix::Hex {
                s.to_uppercase()
            } else {
                s
            }
        }
        Value::Real(f) => format_real(f, prec),
    }
}

/// Beyond this many leading/trailing zeros, switch to scientific notation.
const PLAIN_RANGE: i64 = 12;

fn format_real(f: &Float, prec: u32) -> String {
    if f.is_zero() {
        return "0".to_string();
    }
    // MPFR emits a normalized "d.ffff e±EXP" form; reformat it %g-style.
    let s = f.to_string_radix(10, dec_digits(prec));
    reformat(&s)
}

fn reformat(s: &str) -> String {
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s),
    };
    let (mant, exp) = match rest.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i64>().unwrap_or(0)),
        None => (rest, 0),
    };

    // Significant digits with no point; drop trailing zeros (keep one).
    let mut digits: String = mant.chars().filter(|c| *c != '.').collect();
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }

    // Decimal point sits after `point` digits (mant is normalized to d.ffff).
    let point = exp + 1;
    let body = if point <= 0 {
        if -point > PLAIN_RANGE {
            return sci(sign, &digits, exp);
        }
        format!("0.{}{}", "0".repeat((-point) as usize), digits)
    } else if point as usize >= digits.len() {
        let trail = point as usize - digits.len();
        if trail as i64 > PLAIN_RANGE {
            return sci(sign, &digits, exp);
        }
        format!("{}{}", digits, "0".repeat(trail))
    } else {
        let (int, frac) = digits.split_at(point as usize);
        format!("{int}.{frac}")
    };
    format!("{sign}{body}")
}

fn sci(sign: &str, digits: &str, exp: i64) -> String {
    let (first, rest) = digits.split_at(1);
    if rest.is_empty() {
        format!("{sign}{first}e{exp}")
    } else {
        format!("{sign}{first}.{rest}e{exp}")
    }
}
