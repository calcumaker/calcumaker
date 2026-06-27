//! FFI smoke tests — link the real system GMP/MPFR on the host.

use gmp_mpfr_nostd::{Float, Integer};

#[test]
fn int_arith_and_clone() {
    let a = Integer::from_str_radix("123456789012345678901234567890", 10).unwrap();
    let b = Integer::from_i64(10);
    let c = a.clone() * b;
    assert_eq!(c.to_string_radix(10), "1234567890123456789012345678900");
    assert_eq!(a.to_string_radix(10), "123456789012345678901234567890"); // clone intact
}

#[test]
fn int_div_truncates_and_bitwise() {
    let q = Integer::from_i64(7) / Integer::from_i64(2);
    assert_eq!(q.to_string_radix(10), "3");
    let and = Integer::from_str_radix("ff", 16).unwrap() & Integer::from_str_radix("0f", 16).unwrap();
    assert_eq!(and.to_string_radix(16), "f");
    let shifted = Integer::from_i64(1) << 64u32;
    assert_eq!(shifted.to_string_radix(10), "18446744073709551616");
}

#[test]
fn factorial_and_to_u32() {
    assert_eq!(Integer::factorial(20).to_string_radix(10), "2432902008176640000");
    assert_eq!(Integer::from_i64(42).to_u32(), Some(42));
    assert_eq!(Integer::from_i64(-1).to_u32(), None);
}

#[test]
fn float_sqrt2() {
    let s = Float::from_i64(200, 2).sqrt().to_string_radix(10, 40);
    assert!(s.starts_with("1.414213562373095048801688724209"), "{s}");
}

#[test]
fn float_pi_and_ops() {
    let pi = Float::pi(160);
    assert!(pi.to_string_radix(10, 20).starts_with("3.1415926535897932"));
    // (1/2) as a float
    let half = Float::from_i64(64, 1) / Float::from_i64(64, 2);
    assert!(half.to_string_radix(10, 5).starts_with("5"), "0.5 normalized form"); // 5.0e-1
}

#[test]
fn float_from_integer_and_str() {
    let big = Integer::factorial(30);
    let f = Float::from_integer(256, &big);
    assert!(f.to_string_radix(10, 12).starts_with("2.65252859812"), "30! as float");
    let x = Float::from_str(64, "0.25").unwrap();
    assert!(x.to_string_radix(10, 4).starts_with("2.5"), "0.25 -> 2.5e-1");
}
