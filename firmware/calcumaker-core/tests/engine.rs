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
