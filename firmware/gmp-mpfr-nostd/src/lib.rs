//! Minimal **`no_std`** FFI bindings to **GNU MP** (`mpz` → [`Integer`]) and
//! **MPFR** (`mpfr` → [`Float`]) — the subset a calculator needs. Like `rug`,
//! but for a `no_std` world: one crate that links the **system** GMP/MPFR on the
//! host (for `cargo test` / dev) and the **cross-built** libraries on the target,
//! with no `std` and no build-from-source.
//!
//! ```
//! use gmp_mpfr_nostd::{Integer, Float};
//! let s = (Integer::from_str_radix("2", 10).unwrap()
//!          + Integer::from_str_radix("3", 10).unwrap()).to_string_radix(10);
//! assert_eq!(s, "5");
//! let two = Float::from_i64(200, 2);
//! assert!(two.sqrt().to_string_radix(10, 30).starts_with("1.41421356237309"));
//! ```
#![cfg_attr(not(test), no_std)]

extern crate alloc;

mod complex;
mod ffi;
mod float;
mod integer;

pub use complex::Complex;
pub use float::Float;
pub use integer::Integer;

use alloc::vec::Vec;

/// A NUL-terminated copy of `s` for the C string APIs. (Inputs here are numeric /
/// format strings with no interior NUL.)
pub(crate) fn cbytes(s: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(s.len() + 1);
    v.extend_from_slice(s.as_bytes());
    v.push(0);
    v
}
