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
    let s = run(200, &["2", "sqrt"]);
    assert!(
        s.starts_with("1.4142135623730950488016887242"),
        "sqrt(2) wrong: {s}"
    );
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
