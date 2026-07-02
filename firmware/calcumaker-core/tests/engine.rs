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

/// RLC: (n+1)-bit rotate through carry — MSB → C, old C → LSB.
#[test]
fn rlc_rotates_through_carry() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "unsgn", "80", "rlc"] {
        c.input(t).unwrap();
    }
    // C was clear: 1000_0000 ⟳ → C=1, word 0000_0000
    assert_eq!(c.display(), "0");
    assert!(c.carry());
    c.input("rlc").unwrap();
    // C was set: 0000_0000 ⟳ pulls C into bit0 → 01, C=0
    assert_eq!(c.display(), "1");
    assert!(!c.carry());
}

#[test]
fn rrc_pulls_carry_into_msb() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "unsgn", "1", "rrc"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0"); // bit0 → C
    assert!(c.carry());
    c.input("rrc").unwrap();
    assert_eq!(c.display(), "80"); // C → MSB
    assert!(!c.carry());
}

/// A full n+1 rotations through carry restores word AND flag.
#[test]
fn rlc_full_cycle_is_identity() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "unsgn", "a5"] {
        c.input(t).unwrap();
    }
    for _ in 0..9 {
        c.input("rlc").unwrap();
    }
    assert_eq!(c.display(), "A5");
    assert!(!c.carry());
}

#[test]
fn rlcn_rotates_by_x() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    // 9-bit register: rotating by 9 = identity; by 1 matches rlc
    for t in ["8", "wsize", "unsgn", "80", "1", "rlcn"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0");
    assert!(c.carry());
}

#[test]
fn lj_left_justifies_with_count_in_x() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "unsgn", "5", "lj"] {
        c.input(t).unwrap();
    }
    // 0000_0101 → justified A0 (Y), 5 shifts (X)
    assert_eq!(c.display(), "5");
    assert_eq!(c.stack().len(), 2);
    c.input("drop").unwrap();
    assert_eq!(c.display(), "A0");
}

#[test]
fn lj_zero_and_full() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "unsgn", "0", "lj"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0"); // count
    c.input("drop").unwrap();
    assert_eq!(c.display(), "0"); // value
    for t in ["clear", "255", "lj"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0"); // already justified
}

#[test]
fn rlc_and_lj_need_word_size() {
    let mut c = Calc::new(64);
    c.input("5").unwrap();
    assert!(c.input("rlc").is_err());
    assert!(c.input("lj").is_err());
    assert_eq!(c.stack().len(), 1); // untouched
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

// ---- double-word ops (DBL× / DBL÷ / DBLR) ------------------------------------

#[test]
fn dblmul_splits_the_full_product() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "unsgn", "100", "100", "dbl*"] {
        c.input(t).unwrap();
    }
    // 10000 = 0x2710: Y = 0x27, X = 0x10
    c.set_radix(Radix::Hex);
    assert_eq!(c.display(), "10");
    assert_eq!(c.stack().len(), 2);
    c.input("drop").unwrap();
    assert_eq!(c.display(), "27");
}

/// Signed split reconstructs: enc(Y)·2ⁿ + enc(X) = the 2n-bit product pattern.
#[test]
fn dblmul_signed_product() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "1", "chs", "1", "dbl*"] {
        c.input(t).unwrap();
    }
    // -1 × 1 = -1 → pattern FFFF → high FF (-1), low FF (-1)
    assert_eq!(c.display(), "-1");
    c.set_radix(Radix::Hex);
    assert_eq!(c.display(), "FF");
    c.input("drop").unwrap();
    assert_eq!(c.display(), "FF");
}

#[test]
fn dbldiv_divides_the_double_dividend() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    // Z:Y = 27:10 = 10000, X = 0x64 = 100 → 100
    for t in ["8", "wsize", "unsgn", "27", "10", "64", "dbl/"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "64");
    assert_eq!(c.stack().len(), 1);
}

#[test]
fn dblr_gives_the_remainder() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    // Z:Y = 27:11 = 10001, X = 100 → r = 1
    for t in ["8", "wsize", "unsgn", "27", "11", "64", "dblr"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "1");
}

#[test]
fn dbldiv_signed() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    // -300 @16 bits = FED4 → Z = FE (-2), Y = D4; ÷ 7 → -42 (trunc), r = -6
    for t in ["8", "wsize", "fe", "d4", "7", "dbl/"] {
        c.input(t).unwrap();
    }
    c.set_radix(Radix::Dec);
    assert_eq!(c.display(), "-42");
}

#[test]
fn dbldiv_overflowing_quotient_errors_non_destructively() {
    let mut c = Calc::new(64);
    // Z:Y = 1:0 = 256, ÷ 1 → q = 256 > 8-bit unsigned max
    for t in ["8", "wsize", "unsgn", "1", "0", "1"] {
        c.input(t).unwrap();
    }
    assert!(c.input("dbl/").is_err());
    assert_eq!(c.stack().len(), 3); // untouched
    // …but the remainder of the same division fits
    c.input("dblr").unwrap();
    assert_eq!(c.display(), "0");
}

/// 1's complement is refused: the −0 fold makes the double word ambiguous
/// (found by stage validation — dbl* and dbl/ disagreed there).
#[test]
fn dbl_ops_refuse_ones_complement() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "1s", "1", "chs", "1"] {
        c.input(t).unwrap();
    }
    assert!(c.input("dbl*").is_err());
    assert_eq!(c.stack().len(), 2); // untouched
    c.input("2s").unwrap();
    c.input("dbl*").unwrap(); // fine in 2's complement
    assert_eq!(c.stack().len(), 2);
}

#[test]
fn dbl_ops_need_word_size() {
    let mut c = Calc::new(64);
    for t in ["2", "3"] {
        c.input(t).unwrap();
    }
    assert!(c.input("dbl*").is_err());
    assert_eq!(c.stack().len(), 2);
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
fn user_flags_set_clear_test() {
    let mut c = Calc::new(64);
    for t in ["1", "sf", "1", "ftest"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "1");
    assert!(c.user_flag(1));
    assert!(!c.user_flag(0));
    for t in ["drop", "1", "cf", "1", "ftest"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0");
}

/// Flags 3/4/5 alias leading-zeros / carry / overflow (16C).
#[test]
fn flag_aliases() {
    let mut c = Calc::new(64);
    for t in ["3", "sf"] {
        c.input(t).unwrap();
    }
    assert!(c.leading_zeros());
    for t in ["4", "sf", "4", "ftest"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "1");
    assert!(c.carry());
    for t in ["drop", "5", "sf"] {
        c.input(t).unwrap();
    }
    assert!(c.overflow());
}

#[test]
fn flag_index_out_of_range_errors() {
    let mut c = Calc::new(64);
    c.input("6").unwrap();
    assert!(c.input("sf").is_err());
    assert_eq!(c.display(), "6"); // untouched
}

#[test]
fn clreg_clears_the_register_file() {
    let mut c = Calc::new(64);
    for t in ["42", "sto5", "sto0", "clreg"] {
        c.input(t).unwrap();
    }
    assert!(c.input("rcl5").is_err());
    assert!(c.input("rcl0").is_err());
    assert_eq!(c.display(), "42"); // X untouched
}

#[test]
fn show_in_formats_x_in_another_radix() {
    let mut c = Calc::new(64);
    for t in ["8", "wsize", "lz", "15", "chs"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "-15");
    assert_eq!(c.show_in(Radix::Hex), "F1"); // word + lz apply
    assert_eq!(c.show_in(Radix::Bin), "11110001");
    assert_eq!(c.radix(), Radix::Dec); // mode untouched
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

// ---- leading zeros (16C flag 3) ----------------------------------------------

#[test]
fn lz_pads_to_word_width() {
    let mut c = Calc::new(64);
    // NB: word sizes are entered in Dec — in hex radix "16" would be 0x16!
    for t in ["8", "wsize", "lz", "15"] {
        c.input(t).unwrap();
    }
    c.set_radix(Radix::Hex);
    assert_eq!(c.display(), "0F");
    c.set_radix(Radix::Dec);
    for t in ["16", "wsize"] {
        c.input(t).unwrap();
    }
    c.set_radix(Radix::Hex);
    assert_eq!(c.display(), "000F");
    c.set_radix(Radix::Dec);
    for t in ["8", "wsize"] {
        c.input(t).unwrap();
    }
    c.set_radix(Radix::Bin);
    assert_eq!(c.display(), "00001111");
    c.set_radix(Radix::Oct);
    assert_eq!(c.display(), "017"); // ceil(8/3) = 3 digits
}

#[test]
fn lz_toggles_off_and_ignores_dec_and_unbounded() {
    let mut c = Calc::new(64);
    c.set_radix(Radix::Hex);
    for t in ["8", "wsize", "lz", "f"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0F");
    c.input("lz").unwrap(); // toggle off
    assert_eq!(c.display(), "F");
    c.input("lz").unwrap(); // on again
    c.set_radix(Radix::Dec);
    assert_eq!(c.display(), "15"); // decimal never pads
    c.set_radix(Radix::Hex);
    for t in ["0", "wsize"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "F"); // no word width to pad to
}

#[test]
fn lz_negative_pattern_already_full_width() {
    let mut c = Calc::new(64);
    for t in ["16", "wsize", "lz", "15", "chs"] {
        c.input(t).unwrap();
    }
    c.set_radix(Radix::Hex);
    assert_eq!(c.display(), "FFF1"); // pattern fills the word, no padding needed
}

// ---- angle modes (RAD default; DEG/GRAD reduce exactly + hit exact angles) ---

#[test]
fn rad_is_default_and_unchanged() {
    // atan(1) = π/4 in radians — the pre-angle-mode behavior
    let s = run(200, &["1", "atan"]);
    assert!(s.starts_with("0.785398163397448309"), "atan(1) rad = {s}");
}

#[test]
fn deg_quadrants_are_exact() {
    assert_eq!(run(128, &["deg", "90", "sin"]), "1");
    assert_eq!(run(128, &["deg", "180", "sin"]), "0"); // not a 2^-prec residue
    assert_eq!(run(128, &["deg", "270", "sin"]), "-1");
    assert_eq!(run(128, &["deg", "180", "cos"]), "-1");
    assert_eq!(run(128, &["deg", "180", "tan"]), "0");
    assert_eq!(run(128, &["deg", "90", "tan"]), "inf");
}

#[test]
fn deg_half_exact_angles() {
    assert_eq!(run(128, &["deg", "30", "sin"]), "0.5");
    assert_eq!(run(128, &["deg", "150", "sin"]), "0.5");
    assert_eq!(run(128, &["deg", "210", "sin"]), "-0.5");
    assert_eq!(run(128, &["deg", "60", "cos"]), "0.5");
    assert_eq!(run(128, &["deg", "120", "cos"]), "-0.5");
    assert_eq!(run(128, &["deg", "45", "tan"]), "1");
    assert_eq!(run(128, &["deg", "135", "tan"]), "-1");
}

#[test]
fn deg_reduces_huge_and_negative_angles_exactly() {
    assert_eq!(run(128, &["deg", "36000090", "sin"]), "1"); // ≡ 90°
    assert_eq!(run(128, &["deg", "-90", "sin"]), "-1"); // ≡ 270°
    assert_eq!(run(128, &["deg", "-30", "sin"]), "-0.5"); // ≡ 330°
}

#[test]
fn deg_general_angle_correctly_rounded() {
    // sin 60° = √3/2 — general path with guard bits
    let s = run(200, &["deg", "60", "sin"]);
    assert!(s.starts_with("0.86602540378443864676"), "sin 60° = {s}");
}

#[test]
fn grad_quadrants_and_tan() {
    assert_eq!(run(128, &["grad", "100", "sin"]), "1");
    assert_eq!(run(128, &["grad", "200", "cos"]), "-1");
    assert_eq!(run(128, &["grad", "50", "tan"]), "1");
}

#[test]
fn inverse_trig_exact_hits_in_deg() {
    assert_eq!(run(128, &["deg", "0.5", "asin"]), "30");
    assert_eq!(run(128, &["deg", "1", "atan"]), "45");
    assert_eq!(run(128, &["deg", "-1", "acos"]), "180");
    assert_eq!(run(128, &["deg", "0", "acos"]), "90");
    assert_eq!(run(128, &["grad", "1", "asin"]), "100");
}

#[test]
fn inverse_trig_general_deg() {
    // asin(0.6) = 36.86989764584402...°
    let s = run(200, &["deg", "0.6", "asin"]);
    assert!(s.starts_with("36.8698976458440212"), "asin(0.6)° = {s}");
}

#[test]
fn anglemode_cycles() {
    use calcumaker_core::AngleMode;
    let mut c = Calc::new(64);
    assert_eq!(c.angle_mode(), AngleMode::Rad);
    c.input("anglemode").unwrap();
    assert_eq!(c.angle_mode(), AngleMode::Deg);
    c.input("anglemode").unwrap();
    assert_eq!(c.angle_mode(), AngleMode::Grad);
    c.input("anglemode").unwrap();
    assert_eq!(c.angle_mode(), AngleMode::Rad);
}

#[test]
fn hyperbolics_ignore_angle_mode() {
    assert_eq!(run(128, &["deg", "0", "cosh"]), "1");
    let rad = run(200, &["1", "sinh"]);
    let deg = run(200, &["deg", "1", "sinh"]);
    assert_eq!(rad, deg);
}

// ---- SCI pack: statistics, combinatorics, RAN# --------------------------------

#[test]
fn stats_mean_and_sdev() {
    let mut c = Calc::new(128);
    for t in ["1", "s+", "drop", "2", "s+", "drop", "3", "s+"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "3"); // Σ+ leaves n in X
    c.input("mean").unwrap();
    assert_eq!(c.display(), "2"); // x̄
    c.input("drop").unwrap();
    c.input("drop").unwrap();
    c.input("sdev").unwrap();
    assert_eq!(c.display(), "1"); // s of {1,2,3}
}

/// Pairs (x from X, y from Y) fit y = 2x − 1 exactly.
#[test]
fn stats_linear_regression_and_estimate() {
    let mut c = Calc::new(128);
    for (y, x) in [("1", "1"), ("3", "2"), ("5", "3")] {
        for t in [y, x, "s+", "drop", "drop"] {
            c.input(t).unwrap();
        }
    }
    c.input("lr").unwrap();
    assert_eq!(c.display(), "-1"); // intercept in X
    c.input("drop").unwrap();
    assert_eq!(c.display(), "2"); // slope in Y
    for t in ["drop", "4", "yhat"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "7"); // ŷ(4) = 2·4 − 1
    c.input("drop").unwrap();
    c.input("corr").unwrap();
    assert_eq!(c.display(), "1"); // perfectly linear
}

#[test]
fn sigma_minus_removes_a_point() {
    let mut c = Calc::new(128);
    for t in ["1", "s+", "drop", "2", "s+", "drop", "9", "s+", "drop", "9", "s-"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "2"); // n back to 2
    c.input("mean").unwrap();
    assert_eq!(c.display(), "1.5");
}

#[test]
fn stats_need_data() {
    let mut c = Calc::new(128);
    assert!(c.input("mean").is_err());
    c.input("1").unwrap();
    c.input("s+").unwrap();
    assert!(c.input("sdev").is_err()); // needs 2+
    c.input("clstat").unwrap();
    assert!(c.input("mean").is_err()); // cleared
}

#[test]
fn ncr_npr_exact() {
    assert_eq!(run(64, &["10", "3", "ncr"]), "120");
    assert_eq!(run(64, &["10", "3", "npr"]), "720");
    // exact far beyond f64: C(100, 50)
    assert_eq!(
        run(64, &["100", "50", "ncr"]),
        "100891344545564193334812497256"
    );
    assert_eq!(run(64, &["3", "10", "ncr"]), "0"); // r > n
}

#[test]
fn ncr_guard_and_errors_preserve_stack() {
    let mut c = Calc::new(64);
    for t in ["2.5", "3"] {
        c.input(t).unwrap();
    }
    assert!(c.input("ncr").is_err()); // n must be an integer
    assert_eq!(c.stack().len(), 2);
}

#[test]
fn ran_is_deterministic_and_in_range() {
    let mut a = Calc::new(128);
    let mut b = Calc::new(128);
    a.input("ran").unwrap();
    b.input("ran").unwrap();
    assert_eq!(a.display(), b.display()); // same default seed
    a.input("floor").unwrap();
    assert_eq!(a.display(), "0"); // 0 ≤ ran < 1
    // re-seeding changes the stream
    let mut c = Calc::new(128);
    for t in ["42", "seed", "ran"] {
        c.input(t).unwrap();
    }
    assert_ne!(c.display(), b.display());
}

// ---- classic 4-level stack (HP X/Y/Z/T, T-replication, stack lift) -----------

/// The signature HP idiom: park a constant in T, mash the operator.
#[test]
fn classic4_constant_in_t_idiom() {
    let mut c = Calc::new(64);
    c.input("stack4").unwrap();
    for t in ["5", "enter", "enter", "enter", "2"] {
        c.input(t).unwrap();
    }
    // ENTER disabled lift, so the 2 overwrote X: [5,5,5,2]
    assert_eq!(c.stack().len(), 4);
    c.input("*").unwrap();
    assert_eq!(c.display(), "10"); // [5,5,5,10] — T replicated
    c.input("*").unwrap();
    assert_eq!(c.display(), "50");
    c.input("*").unwrap();
    assert_eq!(c.display(), "250");
    assert_eq!(c.stack().len(), 4);
}

#[test]
fn classic4_lift_discipline() {
    let mut c = Calc::new(64);
    c.input("stack4").unwrap();
    for t in ["2", "enter", "3"] {
        c.input(t).unwrap();
    }
    // 3 overwrote X after ENTER: [0,0,2,3]
    c.input("+").unwrap();
    assert_eq!(c.display(), "5");
    // after an operation lift is enabled: a number PUSHES (5 stays in Y)
    c.input("7").unwrap();
    c.input("+").unwrap();
    assert_eq!(c.display(), "12");
}

/// CLx zeroes X in place — Y/Z/T survive — and disables lift.
#[test]
fn classic4_clx_zeroes_x_keeps_y() {
    let mut c = Calc::new(64);
    c.input("stack4").unwrap();
    for t in ["7", "8", "drop"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "0");
    assert_eq!(c.stack().len(), 4);
    c.input("9").unwrap(); // lift disabled: overwrites the 0
    c.input("+").unwrap();
    assert_eq!(c.display(), "16"); // 7 + 9
}

#[test]
fn classic4_switch_keeps_top_four_and_back_lossless() {
    let mut c = Calc::new(64);
    for t in ["1", "2", "3", "4", "5", "6"] {
        c.input(t).unwrap();
    }
    c.input("stack4").unwrap();
    assert_eq!(c.stack().len(), 4);
    assert_eq!(c.display(), "6"); // top kept: [3,4,5,6]
    c.input("rolldn").unwrap();
    assert_eq!(c.display(), "5"); // rotation of exactly 4
    c.input("stackfree").unwrap();
    assert_eq!(c.stack().len(), 4); // lossless back
}

#[test]
fn classic4_pads_zeros_beneath() {
    let mut c = Calc::new(64);
    c.input("5").unwrap();
    c.input("stack4").unwrap();
    assert_eq!(c.stack().len(), 4); // [0,0,0,5]
    c.input("rollup").unwrap();
    assert_eq!(c.display(), "0"); // a padded zero rose to X
    for t in ["rolldn"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.display(), "5");
}

/// Consuming meta-ops replicate T too (wsize pops X).
#[test]
fn classic4_wsize_replicates() {
    let mut c = Calc::new(64);
    c.input("stack4").unwrap();
    for t in ["9", "enter", "enter", "enter", "8", "wsize"] {
        c.input(t).unwrap();
    }
    assert_eq!(c.word_bits(), Some(8));
    assert_eq!(c.stack().len(), 4);
    assert_eq!(c.display(), "9"); // stack dropped, T=9 replicated
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
