//! Arbitrary-precision numeric core.
//!
//! Two interchangeable backends, selected by Cargo feature. The RPN engine only
//! touches [`Number`] and its operations, so the backend is swappable without
//! changing `rpn.rs`:
//!
//!   * `numeric-gmp`  — GNU MP (integers) + MPFR (reals) via FFI   [preferred]
//!   * `numeric-pure` — pure-Rust (dashu / astro-float)            [fallback]
//!
//! See ../../DESIGN.md → "Numeric core" and "GMP/MPFR on no_std".

use alloc::string::String;

/// A calculator value: an arbitrary-precision integer (programmer modes:
/// HEX/DEC/OCT/BIN, bitwise) or an arbitrary-precision real (scientific modes).
#[derive(Clone)]
pub enum Number {
    Int(Int),
    Real(Real),
}

/// Integer display radix for the programmer modes (HP-16C lineage).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Radix {
    Hex,
    Dec,
    Oct,
    Bin,
}

impl Number {
    pub fn zero() -> Self {
        Number::Int(Int::from_i64(0))
    }

    /// Format for the 7-segment display in the given radix / precision.
    pub fn format(&self, _radix: Radix) -> String {
        // TODO: render via the active backend; window long values (DESIGN.md).
        String::new()
    }
}

/// One-time backend init (e.g. `mp_set_memory_functions` → global heap for GMP).
pub fn init() {
    #[cfg(feature = "numeric-gmp")]
    gmp::init();
}

// ===========================================================================
// Backend: pure-Rust (fallback)
// ===========================================================================
#[cfg(feature = "numeric-pure")]
mod pure {
    // TODO: map onto dashu (and astro-float for transcendentals):
    //   pub type Int  = dashu::integer::IBig;          // arbitrary-precision int
    //   pub type Real = dashu::float::DBig;            // arbitrary-precision float
    // For now, placeholder newtypes keep the surface visible.
    #[derive(Clone)]
    pub struct Int(/* dashu::integer::IBig */);
    #[derive(Clone)]
    pub struct Real(/* dashu::float::DBig / astro_float::BigFloat */);

    impl Int {
        pub fn from_i64(_v: i64) -> Self {
            Int()
        }
    }
}
#[cfg(feature = "numeric-pure")]
pub use pure::{Int, Real};

// ===========================================================================
// Backend: GMP + MPFR via FFI (preferred)
// ===========================================================================
#[cfg(feature = "numeric-gmp")]
mod gmp {
    //! FFI to a cross-built libgmp / libmpfr (static). The GMP allocator is
    //! routed to the firmware's global heap so all bignum allocation lands in
    //! the `embedded-alloc` region. See build.rs + DESIGN.md.

    pub fn init() {
        // TODO: extern "C" { fn __gmp_set_memory_functions(...); }
        //       wire malloc/realloc/free to the global allocator.
    }

    #[derive(Clone)]
    pub struct Int(/* mpz_t handle */);
    #[derive(Clone)]
    pub struct Real(/* mpfr_t handle */);

    impl Int {
        pub fn from_i64(_v: i64) -> Self {
            Int()
        }
    }
}
#[cfg(feature = "numeric-gmp")]
pub use gmp::{Int, Real};
