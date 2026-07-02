//! Engine tests — exercise the real GMP/MPFR path on the host.

use calcumaker_core::{Calc, Radix};

fn run(prec: u32, toks: &[&str]) -> String {
    let mut c = Calc::new(prec);
    for t in toks {
        c.input(t).unwrap();
    }
    c.display()
}

#[test]
fn int_add() {
    assert_eq!(run(128, &["2", "3", "+"]), "5");
}

#[test]
fn int_division_truncates() {
    assert_eq!(run(128, &["7", "2", "/"]), "3"); // integer (programmer) division
}

#[test]
fn factorial_20_exact() {
    assert_eq!(run(64, &["20", "fact"]), "2432902008176640000");
}

#[test]
fn factorial_100_is_a_bignum() {
    let s = run(64, &["100", "fact"]);
    assert!(s.len() > 150, "100! should be ~158 digits, got {}", s.len());
    assert!(s.ends_with("000000000000000000000000"), "100! ends in 24 zeros: {s}");
}

#[test]
fn hex_and() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["ff", "0f", "and"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "F"); // command 'and' wins over a hex parse
}

#[test]
fn hex_or() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["f0", "0f", "or"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "FF");
}

#[test]
fn not_in_8bit_word() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    c.set_word_bits(Some(8));
    for t in ["0f", "not"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "F0");
}

#[test]
fn sqrt2_mpfr() {
    // real sqrt needs a real X (16C model: integer X gets the integer root)
    let s = run(200, &["2.0", "sqrt"]);
    assert!(
        s.starts_with("1.4142135623730950488016887242"),
        "sqrt(2) wrong: {s}"
    );
}

// ---- integer square root (16C: ⌊√x⌋, carry = inexact) -----------------------

#[test]
fn isqrt_floor_with_carry_when_inexact() {
    let mut c = Calc::new(64);
    for t in ["17", "sqrt"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "4");
    assert!(c.carry(), "17 is not a perfect square");
    assert!(matches!(c.stack()[0], calcumaker_core::Value::Int(_)));
}

#[test]
fn isqrt_exact_clears_carry() {
    let mut c = Calc::new(64);
    for t in ["1024", "sq", "sqrt"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "1024");
    assert!(!c.carry());
}

#[test]
fn isqrt_huge_is_exact() {
    // √(10^40) = 10^20, digit-exact far beyond f64
    let mut c = Calc::new(64);
    for t in ["40", "exp10", "sqrt"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "100000000000000000000");
    assert!(!c.carry());
}

#[test]
fn isqrt_zero() {
    let mut c = Calc::new(64);
    for t in ["0", "sqrt"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0");
    assert!(!c.carry());
}

#[test]
fn sqrt_negative_int_errors_and_preserves_stack() {
    let mut c = Calc::new(64);
    c.input("-9").unwrap();
    assert!(c.input("sqrt").is_err());
    assert_eq!(c.display(), "-9"); // untouched
}

#[test]
fn float_then_sqrt_gives_the_real_root() {
    let s = run(200, &["2", "float", "sqrt"]);
    assert!(s.starts_with("1.41421356237309504880"), "got {s}");
}

#[test]
fn exp1_is_e() {
    let s = run(200, &["1", "exp"]);
    assert!(s.starts_with("2.718281828459045"), "e wrong: {s}");
}

#[test]
fn cos0_is_one() {
    assert_eq!(run(128, &["0", "cos"]), "1");
}

#[test]
fn precision_scales() {
    let mut c = Calc::new(512); // ~154 decimal digits
    c.input("pi").unwrap();
    let s = c.display();
    assert!(s.starts_with("3.14159265358979"), "pi = {s}");
    assert!(s.len() > 140, "expected ~154 digits, got {}", s.len());
}

#[test]
fn mixed_int_real_promotes() {
    assert_eq!(run(64, &["1", "2.0", "/"]), "0.5");
}

#[test]
fn pow_2_10_is_1024() {
    assert_eq!(run(128, &["2", "10", "pow"]), "1024");
}

// ---- exact integer results (the arbitrary-precision contract) ---------------

/// int^int is exact GMP, not a rounded real: every digit of 2^100.
#[test]
fn pow_int_stays_exact() {
    let mut c = Calc::new(64); // tiny real precision — must not matter
    for t in ["2", "100", "pow"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "1267650600228229401496703205376");
    assert!(matches!(c.stack()[0], calcumaker_core::Value::Int(_)));
}

#[test]
fn pow_big_base_exact() {
    // (10^12 - 1)^3, exact
    assert_eq!(
        run(64, &["999999999999", "3", "pow"]),
        "999999999997000000000002999999999999"
    );
}

#[test]
fn pow_negative_exponent_promotes_to_real() {
    assert_eq!(run(128, &["2", "2", "chs", "pow"]), "0.25");
}

#[test]
fn pow_real_operand_is_real() {
    let mut c = Calc::new(128);
    for t in ["2.0", "10", "pow"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "1024");
    assert!(matches!(c.stack()[0], calcumaker_core::Value::Real(_)));
}

#[test]
fn pow_result_too_large_errors_and_preserves_stack() {
    let mut c = Calc::new(64);
    for t in ["2", "10000000"] {
        c.input(t).unwrap();
    }
    assert!(c.input("pow").is_err());
    assert_eq!(c.stack().len(), 2); // operands intact
    assert_eq!(c.display(), "10000000");
}

/// 0/±1 bases are exact for any exponent size (no guard needed).
#[test]
fn pow_unit_base_huge_exponent() {
    assert_eq!(run(64, &["1", "99999999999999999999", "pow"]), "1");
    assert_eq!(run(64, &["-1", "99999999999999999999", "pow"]), "-1"); // odd
    assert_eq!(run(64, &["0", "99999999999999999999", "pow"]), "0");
    assert_eq!(run(64, &["0", "0", "pow"]), "1");
}

#[test]
fn pow_word_mode_wraps_with_overflow_flag() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "2", "10", "pow"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0"); // 1024 mod 256
    assert!(c.overflow());
}

#[test]
fn sq_int_exact() {
    assert_eq!(
        run(64, &["12345678901234567890", "sq"]),
        "152415787532388367501905199875019052100"
    );
}

#[test]
fn exp10_int_exact() {
    assert_eq!(run(64, &["30", "exp10"]), "1000000000000000000000000000000");
    let mut c = Calc::new(64);
    for t in ["30", "exp10"] {
        c.input(t).unwrap();
    }
    assert!(matches!(c.stack()[0], calcumaker_core::Value::Int(_)));
}

/// Negative exponent promotes to real (10^-3 is not binary-exact, so assert
/// through FIX which display-rounds).
#[test]
fn exp10_negative_promotes_to_real() {
    let mut c = Calc::new(128);
    for t in ["6", "fix", "3", "chs", "exp10"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0.001000");
    assert!(matches!(c.stack()[0], calcumaker_core::Value::Real(_)));
}

#[test]
fn atan1_is_quarter_pi() {
    let s = run(200, &["1", "atan"]);
    assert!(s.starts_with("0.785398163397448309"), "atan(1) = {s}");
}

#[test]
fn log10_1000_is_3() {
    assert!(run(200, &["1000", "log"]).starts_with("3"));
}

#[test]
fn cosh0_is_one() {
    assert_eq!(run(64, &["0", "cosh"]), "1");
}

#[test]
fn e_constant() {
    assert!(run(128, &["e"]).starts_with("2.718281828459045"));
}

#[test]
fn abs_int() {
    assert_eq!(run(64, &["-5", "abs"]), "5");
}

#[test]
fn mod_17_5_is_2() {
    assert_eq!(run(64, &["17", "5", "mod"]), "2");
}

#[test]
fn lastx_recalls_consumed_x() {
    let mut c = Calc::new(64);
    for t in ["2", "3", "+", "lastx"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "3"); // the X (=3) consumed by +
    assert_eq!(c.stack().len(), 2); // [5, 3]
}

#[test]
fn enter_dups_x() {
    let mut c = Calc::new(64);
    for t in ["7", "enter"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "7");
    assert_eq!(c.stack().len(), 2);
}

#[test]
fn over_copies_y_above_x() {
    let mut c = Calc::new(64);
    for t in ["2", "3", "over"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "2"); // [2, 3, 2]
    assert_eq!(c.stack().len(), 3);
}

// ---- word size, sign modes, flags ------------------------------------------

/// -15 at 16-bit 2's complement displays as the bit pattern FFF1 in hex,
/// and as the signed value in decimal.
#[test]
fn twos_complement_hex_display() {
    let mut c = Calc::new(64);
    for t in ["16", "wsize", "15", "chs"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "-15");
    c.set_radix(Radix::Hex);
    assert_eq!(c.display(), "FFF1");
}

/// Entering a high bit pattern in hex decodes per the sign mode.
#[test]
fn pattern_entry_decodes_by_sign_mode() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "ff", "dec"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "-1"); // 2's comp default
    for t in ["hex", "unsgn", "dec"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "255"); // same bits, unsigned view
}

#[test]
fn ones_complement_negation_is_bit_complement() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "1s", "5", "chs"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "-5");
    c.set_radix(Radix::Hex);
    assert_eq!(c.display(), "FA"); // ~00000101 = 11111010
}

#[test]
fn add_wraps_and_sets_flags() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "unsgn", "200", "100", "+"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "44"); // 300 mod 256
    assert!(c.carry(), "carry out of bit 8");
    assert!(c.overflow(), "result wrapped");
}

/// Decimal entry wraps into the word silently; the *add* then overflows the
/// signed range without a carry (100+100 = 200 > 127 @8b 2's comp).
#[test]
fn signed_overflow_without_carry() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "100", "100", "+"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "-56");
    assert!(!c.carry());
    assert!(c.overflow());
}

#[test]
fn add_in_range_clears_flags() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "3", "4", "+"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "7");
    assert!(!c.carry());
    assert!(!c.overflow());
}

#[test]
fn sub_borrow_sets_carry() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "3", "5", "-"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "-2");
    assert!(c.carry(), "borrow");
}

#[test]
fn wsize_change_preserves_bit_pattern() {
    let mut c = Calc::new(64);
    for t in ["16", "wsize", "15", "chs"] {
        c.input(t).unwrap();
    }
    // FFF1 @16b → narrow to 8 bits keeps the low byte F1 = -15 (2's comp)
    for t in ["8", "wsize"] {
        c.input(t).unwrap();
    }
    c.set_radix(Radix::Hex);
    assert_eq!(c.display(), "F1");
}

// ---- shifts, rotates, bit ops ------------------------------------------------

#[test]
fn sl_shifts_x_by_one_and_carries_msb() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "81", "sl"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "2");
    assert!(c.carry(), "MSB shifted out");
}

#[test]
fn asr_fills_with_sign() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "f0", "asr"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "F8"); // sign bit replicated
}

#[test]
fn rotate_left_wraps_msb_to_lsb() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "81", "rl"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "3"); // 1000_0001 → 0000_0011
    assert!(c.carry());
}

#[test]
fn rotate_needs_word_size() {
    let mut c = Calc::new(64);
    for t in ["5"] {
        c.input(t).unwrap();
    }
    assert!(c.input("rl").is_err());
    assert_eq!(c.stack().len(), 1); // operand preserved on error
}

#[test]
fn rln_rotates_y_by_x() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "12", "4", "rln"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "21"); // nibble swap
}

#[test]
fn bset_bclr_btest() {
    let mut c = Calc::new(64);
    for t in ["0", "3", "bset"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "8");
    for t in ["3", "btest"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "1"); // bit is set; value stays in Y
    assert_eq!(c.stack().len(), 2);
    for t in ["drop", "3", "bclr"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0");
}

#[test]
fn maskl_maskr() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "unsgn", "4", "maskl"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "F0");
    for t in ["drop", "4", "maskr"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "F");
}

#[test]
fn popcnt_counts_pattern_bits() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "ff", "popcnt"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "8");
}

// ---- conversions, %, registers ------------------------------------------------

#[test]
fn float_and_round_convert_kinds() {
    let mut c = Calc::new(128);
    for t in ["7", "float"] {
        c.input(t).unwrap();
    }
    for t in ["2", "/"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "3.5"); // real division after FLOAT
    c.input("round").unwrap();
    assert_eq!(c.display(), "4");
}

#[test]
fn floor_ceil_trunc_frac() {
    let mut c = Calc::new(128);
    c.input("-2.5").unwrap();
    c.input("floor").unwrap();
    assert_eq!(c.display(), "-3");
    c.input("drop").unwrap();
    c.input("-2.5").unwrap();
    c.input("ceil").unwrap();
    assert_eq!(c.display(), "-2");
    c.input("drop").unwrap();
    c.input("-2.5").unwrap();
    c.input("trunc").unwrap();
    assert_eq!(c.display(), "-2");
    c.input("drop").unwrap();
    c.input("2.75").unwrap();
    c.input("frac").unwrap();
    assert_eq!(c.display(), "0.75");
}

#[test]
fn pct_preserves_y() {
    let mut c = Calc::new(128);
    for t in ["200", "15", "pct"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "30");
    assert_eq!(c.stack().len(), 2); // Y = 200 still there
}

#[test]
fn sto_rcl_registers() {
    let mut c = Calc::new(64);
    for t in ["42", "sto5", "drop", "rcl5"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "42");
    assert!(c.input("rcl6").is_err()); // empty register
}

#[test]
fn clear_empties_stack_and_flags() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "200", "100", "+", "clear"] {
        c.input(t).unwrap();
    }
    assert!(c.stack().is_empty());
    assert!(!c.carry() && !c.overflow());
}

// ---- FIX / SCI / ENG -----------------------------------------------------------

#[test]
fn fix_mode() {
    let mut c = Calc::new(200);
    for t in ["4", "fix", "pi"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "3.1416");

    let mut c2 = Calc::new(200);
    for t in ["2", "fix", "0.0049"] {
        c2.input(t).unwrap();
    }
    assert_eq!(c2.display(), "0.00"); // below the last decimal
    for t in ["drop", "0.006"] {
        c2.input(t).unwrap();
    }
    assert_eq!(c2.display(), "0.01"); // rounds up into view
}

#[test]
fn sci_mode() {
    let mut c = Calc::new(200);
    for t in ["3", "sci", "1500", "float"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "1.500e3");
}

#[test]
fn eng_mode() {
    let mut c = Calc::new(200);
    for t in ["2", "eng", "0.0472"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "47.2e-3");
}

#[test]
fn std_returns_to_auto() {
    let mut c = Calc::new(200);
    for t in ["4", "fix", "0.5", "std"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0.5");
}

#[test]
fn real_div_zero_is_inf() {
    let mut c = Calc::new(64);
    for t in ["1.0", "0", "/"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "inf");
}

#[test]
fn sqrt_of_negative_real_is_nan() {
    assert_eq!(run(64, &["-1.0", "sqrt"]), "nan");
}

// ---- errors never consume operands ---------------------------------------------

#[test]
fn type_error_preserves_stack() {
    let mut c = Calc::new(64);
    for t in ["1.5", "2"] {
        c.input(t).unwrap();
    }
    assert!(c.input("and").is_err()); // Y is a real
    assert_eq!(c.stack().len(), 2);
    assert_eq!(c.display(), "2");
}

#[test]
fn lastx_untouched_by_failed_op() {
    let mut c = Calc::new(64);
    for t in ["2", "3", "+"] {
        c.input(t).unwrap();
    }
    let _ = c.input("and").err(); // fails: one operand only... actually 5 is X
    let mut c = Calc::new(64);
    for t in ["2", "3", "+", "0", "/"] {
        c.input(t).unwrap_or(());
    }
    c.input("lastx").unwrap();
    assert_eq!(c.display(), "3"); // still the + operand, not the failed /
}

#[test]
fn div_zero_preserves_stack() {
    let mut c = Calc::new(64);
    for t in ["5", "0"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.input("/"), Err(calcumaker_core::CalcError::DivZero));
    assert_eq!(c.stack().len(), 2); // 5 and 0 still there
    assert_eq!(c.display(), "0");
}

#[test]
fn wsize_command_sets_word_from_x() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "0f", "not"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "F0");
    assert_eq!(c.word_bits(), Some(8));
    for t in ["0", "wsize"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.word_bits(), None); // 0 = unbounded
}

#[test]
fn prec_command_sets_precision_from_x() {
    let mut c = Calc::new(64);
    for t in ["512", "prec"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.prec(), 512);
    assert!(c.stack().is_empty());
}

#[test]
fn exp10_of_3_is_1000() {
    assert_eq!(run(128, &["3", "exp10"]), "1000");
}

#[test]
fn rolldn_moves_x_to_bottom() {
    let mut c = Calc::new(64);
    for t in ["1", "2", "3", "rolldn"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "2"); // [3, 1, 2] -> X = 2
}
