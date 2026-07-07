//! Render a [`Value`] for the display, honoring the integer radix, word size /
//! sign mode (non-decimal radices show the raw bit pattern, 16C style), the
//! real display format (AUTO `%g` / FIX / SCI / ENG), and the working
//! precision.

use alloc::format;
use alloc::string::{String, ToString};

use gmp_mpfr_nostd::{Complex, Float};

use crate::calc::{encode_bits, AngleMode, Calc, FloatFmt, Radix};
use crate::value::Value;

/// Decimal significant digits worth showing for a given binary precision.
fn dec_digits(prec: u32) -> usize {
    // log10(2) ≈ 0.30103; keep at least a few.
    ((prec as f64) * 0.30103) as usize + 1
}

pub fn format(v: &Value, c: &Calc) -> String {
    format_radix(v, c, c.radix())
}

/// [`format`] with a radix override — the SHOW-base view (16C f-SHOW):
/// X momentarily in another base without switching modes.
pub(crate) fn format_radix(v: &Value, c: &Calc, radix: Radix) -> String {
    match v {
        Value::Int(i) => {
            let s = match (c.word_bits(), radix) {
                // Word mode: hex/oct/bin show the n-bit pattern (-15 @16b 2's
                // comp is FFF1); decimal shows the signed value. With leading
                // zeros on (16C flag 3), pad to the word width.
                (Some(n), r) if r != Radix::Dec => {
                    let mut s = encode_bits(i, c.sign_mode(), n).to_string_radix(r.base());
                    if c.leading_zeros() {
                        let width = match r {
                            Radix::Bin => n as usize,
                            Radix::Oct => (n as usize).div_ceil(3),
                            Radix::Hex => (n as usize).div_ceil(4),
                            Radix::Dec => unreachable!(),
                        };
                        while s.len() < width {
                            s.insert(0, '0');
                        }
                    }
                    s
                }
                (_, r) => i.to_string_radix(r.base()),
            };
            if radix == Radix::Hex {
                s.to_uppercase()
            } else {
                s
            }
        }
        Value::Real(f) => format_real(f, c.prec(), c.float_fmt()),
        Value::Complex(z) => format_complex(z, c),
    }
}

/// Format a complex number: `a+bi` (rectangular) or `r ∠ θ` (polar, θ in the
/// current angle unit) — HP-42S RECT/POLAR.
fn format_complex(z: &Complex, c: &Calc) -> String {
    let (prec, fmt) = (c.prec(), c.float_fmt());
    if c.polar() {
        let r = format_real(&z.abs(prec), prec, fmt);
        let theta = format_real(&angle_out(z.arg(prec), c.angle_mode(), prec), prec, fmt);
        format!("{r} \u{2220} {theta}")
    } else {
        let re = format_real(&z.real(prec), prec, fmt);
        let im = format_real(&z.imag(prec), prec, fmt);
        // `im` already carries its own sign; join without doubling it.
        if im.starts_with('-') {
            format!("{re}{im}i")
        } else {
            format!("{re}+{im}i")
        }
    }
}

/// Convert an angle in radians to the display angle unit (polar θ / complex).
fn angle_out(rad: Float, mode: AngleMode, prec: u32) -> Float {
    match mode {
        AngleMode::Rad => rad,
        AngleMode::Deg => rad * Float::from_i64(prec, 180) / Float::pi(prec),
        AngleMode::Grad => rad * Float::from_i64(prec, 200) / Float::pi(prec),
    }
}

/// FIX with an integer part wider than this falls back to SCI (16C-style
/// "doesn't fit" escape; the 7-seg row is 16 digits anyway).
const FIX_MAX_EXP: i64 = 40;

fn format_real(f: &Float, prec: u32, fmt: FloatFmt) -> String {
    if f.is_nan() {
        return "nan".to_string();
    }
    if f.is_inf() {
        return if f.is_sign_negative() { "-inf" } else { "inf" }.to_string();
    }
    if f.is_zero() {
        return zero_str(fmt);
    }
    match fmt {
        FloatFmt::Auto => reformat(&f.to_string_radix(10, dec_digits(prec))),
        FloatFmt::Fix(d) => fix(f, d as i64),
        FloatFmt::Sci(d) => sci_n(f, d as usize),
        FloatFmt::Eng(d) => eng(f, d as usize),
    }
}

fn zero_str(fmt: FloatFmt) -> String {
    match fmt {
        FloatFmt::Auto => "0".to_string(),
        FloatFmt::Fix(d) => fixed_zero(d as i64),
        FloatFmt::Sci(d) | FloatFmt::Eng(d) => {
            let mut s = pad_decimals("0", d as usize);
            s.push_str("e0");
            s
        }
    }
}

fn fixed_zero(d: i64) -> String {
    pad_decimals("0", d.max(0) as usize)
}

/// `int_digits` plus exactly `d` decimals (appends `.` + zero-padding).
fn pad_decimals(int: &str, d: usize) -> String {
    if d == 0 {
        return int.to_string();
    }
    format!("{int}.{}", "0".repeat(d))
}

/// Sign, significant digits (exactly `sig`, no point), and the normalized
/// exponent (value = 0.digits × 10^(exp+1), i.e. d.fff × 10^exp).
fn split_sig(f: &Float, sig: usize) -> (&'static str, String, i64) {
    if sig >= 2 {
        return split_raw(f, sig);
    }
    // MPFR won't produce a single digit — take two and round by hand
    // (half away from zero, HP-style).
    let (sign, digits, mut exp) = split_raw(f, 2);
    let b = digits.as_bytes();
    let mut d1 = b[0] - b'0';
    if b[1] >= b'5' {
        d1 += 1;
    }
    let digits = if d1 == 10 {
        exp += 1;
        "1".to_string()
    } else {
        char::from(b'0' + d1).to_string()
    };
    (sign, digits, exp)
}

fn split_raw(f: &Float, sig: usize) -> (&'static str, String, i64) {
    let s = f.to_string_radix(10, sig);
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s.as_str()),
    };
    let (mant, exp) = match rest.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i64>().unwrap_or(0)),
        None => (rest, 0),
    };
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    (sign, digits, exp)
}

/// FIX d — exactly `d` digits after the decimal point.
fn fix(f: &Float, d: i64) -> String {
    let (_, _, e0) = split_sig(f, 2); // exponent probe
    if e0 > FIX_MAX_EXP {
        return sci_n(f, d.max(0) as usize);
    }
    let sig = e0 + 1 + d;
    if sig <= 0 {
        // The value sits below the last displayed decimal. Probe with plenty
        // of digits (no double rounding) and decide: zero, or it rounds up
        // into the last decimal place.
        let (sign, digits, e1) = split_raw(f, 20);
        if e1 >= -d {
            return fix_from(sign, &digits, e1, d); // all-nines bump into view
        }
        if e1 == -(d + 1) && digits.as_bytes()[0] >= b'5' {
            if d == 0 {
                return format!("{sign}1");
            }
            return format!("{sign}0.{}1", "0".repeat((d - 1) as usize));
        }
        return fixed_zero(d);
    }
    let (sign, digits, e1) = split_sig(f, sig as usize);
    // The 2-digit exponent probe can overshoot for mantissas in [9.95, 10)
    // (999.6 probes as 1.0e3): the sig-digit split then lands at a LOWER
    // exponent with one digit too many, and slicing would truncate instead of
    // round (review finding: `0 fix 999.6` showed 999). Re-split at the digit
    // count the true exponent needs — a rounding bump there pads correctly.
    if e1 < e0 {
        let sig2 = e1 + 1 + d;
        if sig2 <= 0 {
            return fixed_zero(d);
        }
        let (sign, digits, e2) = split_sig(f, sig2 as usize);
        return fix_from(sign, &digits, e2, d);
    }
    fix_from(sign, &digits, e1, d)
}

/// Assemble a fixed-point string from `digits` (value = d.fff × 10^e1) with
/// exactly `d` decimals. `digits` always covers the decimals (the caller sized
/// it); the integer part is zero-padded if rounding shortened it.
fn fix_from(sign: &str, digits: &str, e1: i64, d: i64) -> String {
    let d = d.max(0) as usize;
    if e1 < 0 {
        let lead = (-e1 - 1) as usize;
        let mut dec: String = "0".repeat(lead);
        dec.push_str(digits);
        dec.truncate(d);
        while dec.len() < d {
            dec.push('0');
        }
        if d == 0 {
            return format!("{sign}0");
        }
        return format!("{sign}0.{dec}");
    }
    let int_w = (e1 + 1) as usize;
    let mut all: String = digits.to_string();
    while all.len() < int_w + d {
        all.push('0');
    }
    let (int, dec) = all.split_at(int_w);
    if d == 0 {
        return format!("{sign}{int}");
    }
    format!("{sign}{int}.{}", &dec[..d])
}

/// SCI d — d.ffffe±X with exactly `d` digits after the point.
fn sci_n(f: &Float, d: usize) -> String {
    let (sign, digits, e) = split_sig(f, d + 1);
    let (first, rest) = digits.split_at(1);
    let mut mant = first.to_string();
    if d > 0 {
        let mut dec = rest.to_string();
        dec.truncate(d);
        while dec.len() < d {
            dec.push('0');
        }
        mant = format!("{mant}.{dec}");
    }
    format!("{sign}{mant}e{e}")
}

/// ENG d — like SCI but the exponent is a multiple of 3 (d+1 significant
/// digits, 1–3 of them before the point).
fn eng(f: &Float, d: usize) -> String {
    let (sign, mut digits, e) = split_sig(f, d + 1);
    let shift = e.rem_euclid(3) as usize; // int part is shift+1 digits
    let e3 = e - shift as i64;
    while digits.len() < shift + 1 {
        digits.push('0');
    }
    let (int, dec) = digits.split_at(shift + 1);
    let body = if dec.is_empty() {
        int.to_string()
    } else {
        format!("{int}.{dec}")
    };
    format!("{sign}{body}e{e3}")
}

// ---- fit-to-window (the glass rounds, the register doesn't) ------------------

/// Display cells a string occupies on the 7-seg row (dots fold into the
/// preceding digit's dp, so they're free — matches `seg7::encode_row`).
fn cells_of(s: &str) -> usize {
    s.chars().filter(|&c| c != '.').count()
}

/// Like [`format`], but AUTO-mode reals are rounded (correctly, by MPFR) to
/// the largest digit count that fits `max_cells` — HP behaviour: the display
/// rounds to the window, the stored value keeps full precision. So a value a
/// hair under 382.1 shows as `382.1`, not `382.09999…` off the glass.
/// Integers and explicit FIX/SCI/ENG are shown as configured (the 7-seg
/// overflow marker handles what still doesn't fit).
pub(crate) fn format_fit(v: &Value, c: &Calc, max_cells: usize) -> String {
    let full = format(v, c);
    if cells_of(&full) <= max_cells {
        return full;
    }
    let Value::Real(f) = v else { return full };
    if !matches!(c.float_fmt(), FloatFmt::Auto) || f.is_nan() || f.is_inf() || f.is_zero() {
        return full;
    }
    let start = dec_digits(c.prec()).min(max_cells).max(1);
    let mut last = full;
    for sig in (1..=start).rev() {
        let raw = f.to_string_radix(10, sig);
        let plain = reformat(&raw);
        if cells_of(&plain) <= max_cells {
            return plain;
        }
        // Plain form is exponent-bound (e.g. 1.2345e17): force scientific at
        // this digit count instead of losing digits waiting for the flip.
        let (sign, digits, e) = parts(&raw);
        let scis = sci(sign, &digits, e);
        if cells_of(&scis) <= max_cells {
            return scis;
        }
        last = scis;
    }
    last
}

// ---- AUTO (%g-style) --------------------------------------------------------

/// Beyond this many leading/trailing zeros, switch to scientific notation.
const PLAIN_RANGE: i64 = 12;

/// Sign, significant digits (trailing zeros trimmed, no point), exponent of
/// the normalized `d.ffffe<exp>` form.
fn parts(s: &str) -> (&'static str, String, i64) {
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s),
    };
    let (mant, exp) = match rest.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i64>().unwrap_or(0)),
        None => (rest, 0),
    };
    let mut digits: String = mant.chars().filter(|c| *c != '.').collect();
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    (sign, digits, exp)
}

fn reformat(s: &str) -> String {
    let (sign, digits, exp) = parts(s);

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
