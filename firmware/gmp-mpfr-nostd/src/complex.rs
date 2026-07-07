//! Arbitrary-precision complex numbers (`mpc`), round-to-nearest — like
//! `rug::Complex`, but `no_std`. A complex is two MPFR parts (real, imaginary)
//! at a fixed precision; owns its `mpc_t`, cleared on drop.

use core::ffi::c_int;
use core::mem::MaybeUninit;

use crate::ffi::{self, MPC_RNDNN, RNDN};
use crate::float::Float;

type McUnary = unsafe extern "C" fn(*mut ffi::mpc_struct, *const ffi::mpc_struct, c_int) -> c_int;
type McBinary =
    unsafe extern "C" fn(*mut ffi::mpc_struct, *const ffi::mpc_struct, *const ffi::mpc_struct, c_int) -> c_int;
type McToReal = unsafe extern "C" fn(*mut ffi::mpfr_struct, *const ffi::mpc_struct, c_int) -> c_int;

pub struct Complex {
    raw: ffi::mpc_struct,
}

impl Complex {
    /// New complex of `prec` bits (both parts), uninitialised value.
    pub fn new(prec: u32) -> Self {
        let mut raw = MaybeUninit::<ffi::mpc_struct>::uninit();
        unsafe {
            ffi::mpc_init2(raw.as_mut_ptr(), prec.max(2) as ffi::mpfr_prec_t);
            Complex { raw: raw.assume_init() }
        }
    }

    /// Build `re + im·i` from two integers at `prec` bits.
    pub fn from_i64(prec: u32, re: i64, im: i64) -> Self {
        Self::from_reals(prec, &Float::from_i64(prec, re), &Float::from_i64(prec, im))
    }

    /// Build `re + im·i` at `prec` bits.
    pub fn from_reals(prec: u32, re: &Float, im: &Float) -> Self {
        let mut z = Self::new(prec);
        unsafe { ffi::mpc_set_fr_fr(&mut z.raw, &re.raw, &im.raw, MPC_RNDNN) };
        z
    }

    /// Working precision (bits) of the real part (both parts share it).
    pub fn prec(&self) -> u32 {
        unsafe { ffi::mpfr_get_prec(&self.raw.re) as u32 }
    }

    fn part(&self, prec: u32, f: McToReal) -> Float {
        let mut out = Float::new(prec);
        unsafe { f(&mut out.raw, &self.raw, RNDN) };
        out
    }
    /// Real part.
    pub fn real(&self, prec: u32) -> Float {
        self.part(prec, ffi::mpc_real)
    }
    /// Imaginary part.
    pub fn imag(&self, prec: u32) -> Float {
        self.part(prec, ffi::mpc_imag)
    }
    /// Magnitude |z|.
    pub fn abs(&self, prec: u32) -> Float {
        self.part(prec, ffi::mpc_abs)
    }
    /// Argument arg(z) in radians.
    pub fn arg(&self, prec: u32) -> Float {
        self.part(prec, ffi::mpc_arg)
    }

    fn unary(&self, f: McUnary) -> Complex {
        let mut r = Complex::new(self.prec());
        unsafe { f(&mut r.raw, &self.raw, MPC_RNDNN) };
        r
    }
    fn binary(&self, o: &Complex, f: McBinary) -> Complex {
        let mut r = Complex::new(self.prec());
        unsafe { f(&mut r.raw, &self.raw, &o.raw, MPC_RNDNN) };
        r
    }

    pub fn add(&self, o: &Complex) -> Complex {
        self.binary(o, ffi::mpc_add)
    }
    pub fn sub(&self, o: &Complex) -> Complex {
        self.binary(o, ffi::mpc_sub)
    }
    pub fn mul(&self, o: &Complex) -> Complex {
        self.binary(o, ffi::mpc_mul)
    }
    pub fn div(&self, o: &Complex) -> Complex {
        self.binary(o, ffi::mpc_div)
    }
    pub fn pow(&self, o: &Complex) -> Complex {
        self.binary(o, ffi::mpc_pow)
    }
    pub fn neg(&self) -> Complex {
        self.unary(ffi::mpc_neg)
    }
    pub fn conj(&self) -> Complex {
        self.unary(ffi::mpc_conj)
    }
    pub fn sqrt(&self) -> Complex {
        self.unary(ffi::mpc_sqrt)
    }
    pub fn exp(&self) -> Complex {
        self.unary(ffi::mpc_exp)
    }
    pub fn ln(&self) -> Complex {
        self.unary(ffi::mpc_log)
    }
    pub fn sin(&self) -> Complex {
        self.unary(ffi::mpc_sin)
    }
    pub fn cos(&self) -> Complex {
        self.unary(ffi::mpc_cos)
    }
    pub fn tan(&self) -> Complex {
        self.unary(ffi::mpc_tan)
    }
}

impl Clone for Complex {
    fn clone(&self) -> Self {
        let mut r = Complex::new(self.prec());
        unsafe { ffi::mpc_set(&mut r.raw, &self.raw, MPC_RNDNN) };
        r
    }
}

impl Drop for Complex {
    fn drop(&mut self) {
        unsafe { ffi::mpc_clear(&mut self.raw) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sqrt_neg_four_is_2i() {
        let p = 256;
        let z = Complex::from_reals(p, &Float::from_i64(p, -4), &Float::from_i64(p, 0));
        let r = z.sqrt();
        assert!(r.real(p).is_zero());
        assert!(r.imag(p).to_string_radix(10, 10).starts_with('2'));
    }
    #[test]
    fn i_times_i_is_minus_one() {
        let p = 256;
        let i = Complex::from_reals(p, &Float::from_i64(p, 0), &Float::from_i64(p, 1));
        let r = i.mul(&i);
        assert!(r.real(p).to_string_radix(10, 10).starts_with("-1"));
        assert!(r.imag(p).is_zero());
    }
}
