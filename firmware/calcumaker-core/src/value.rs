//! The calculator value: an arbitrary-precision integer (programmer modes) or an
//! arbitrary-precision real (scientific modes). Backed by GMP + MPFR via
//! `gmp-mpfr-nostd`.

use gmp_mpfr_nostd::{Complex, Float, Integer};

use crate::matrix::Matrix;

#[derive(Clone)]
pub enum Value {
    /// Arbitrary-precision integer (GMP `mpz`).
    Int(Integer),
    /// Arbitrary-precision real (MPFR `mpfr`).
    Real(Float),
    /// Arbitrary-precision complex (MPC) — one stack object (HP-42S model).
    Complex(Complex),
    /// A dense MPFR matrix on the stack (HP-15C matrices, modernized).
    Matrix(Matrix),
}

impl Value {
    /// View/convert this value as an MPFR float at the given precision (bits).
    /// A complex collapses to its real part (real-only contexts; complex-aware
    /// ops use [`Value::to_complex`]).
    pub fn to_real(&self, prec: u32) -> Float {
        match self {
            Value::Int(i) => Float::from_integer(prec, i),
            Value::Real(f) => Float::with_prec(prec, f),
            Value::Complex(z) => z.real(prec),
            // A matrix is not a scalar — callers guard first; NaN as a fallback.
            Value::Matrix(_) => Float::new(prec),
        }
    }

    /// View/convert this value as an MPC complex (real values gain a zero
    /// imaginary part).
    pub fn to_complex(&self, prec: u32) -> Complex {
        match self {
            Value::Complex(z) => Complex::with_prec(prec, z),
            _ => {
                let zero = Float::from_i64(prec, 0);
                Complex::from_reals(prec, &self.to_real(prec), &zero)
            }
        }
    }

    /// Round the value to a new working precision; integers are exact.
    pub(crate) fn set_prec(&mut self, prec: u32) {
        *self = match self {
            Value::Int(_) => return,
            Value::Real(f) => Value::Real(Float::with_prec(prec, f)),
            Value::Complex(z) => Value::Complex(Complex::with_prec(prec, z)),
            Value::Matrix(m) => Value::Matrix(m.with_prec(prec)),
        };
    }

    pub fn is_int(&self) -> bool {
        matches!(self, Value::Int(_))
    }

    pub fn is_complex(&self) -> bool {
        matches!(self, Value::Complex(_))
    }

    pub fn is_matrix(&self) -> bool {
        matches!(self, Value::Matrix(_))
    }
}

impl From<Matrix> for Value {
    fn from(m: Matrix) -> Self {
        Value::Matrix(m)
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

impl From<Complex> for Value {
    fn from(z: Complex) -> Self {
        Value::Complex(z)
    }
}
