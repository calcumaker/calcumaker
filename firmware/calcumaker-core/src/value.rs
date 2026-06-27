//! The calculator value: an arbitrary-precision integer (programmer modes) or an
//! arbitrary-precision real (scientific modes). Backed by GMP + MPFR via
//! `gmp-mpfr-nostd`.

use gmp_mpfr_nostd::{Float, Integer};

#[derive(Clone)]
pub enum Value {
    /// Arbitrary-precision integer (GMP `mpz`).
    Int(Integer),
    /// Arbitrary-precision real (MPFR `mpfr`).
    Real(Float),
}

impl Value {
    /// View/convert this value as an MPFR float at the given precision (bits).
    pub fn to_real(&self, prec: u32) -> Float {
        match self {
            Value::Int(i) => Float::from_integer(prec, i),
            Value::Real(f) => Float::with_prec(prec, f),
        }
    }

    pub fn is_int(&self) -> bool {
        matches!(self, Value::Int(_))
    }
}

impl From<Integer> for Value {
    fn from(i: Integer) -> Self {
        Value::Int(i)
    }
}

impl From<Float> for Value {
    fn from(f: Float) -> Self {
        Value::Real(f)
    }
}
