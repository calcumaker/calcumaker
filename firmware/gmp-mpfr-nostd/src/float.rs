//! Arbitrary-precision floating point (`mpfr`), round-to-nearest.

use alloc::format;
use alloc::string::{String, ToString};
use core::ffi::{c_char, c_int, c_long};
use core::mem::MaybeUninit;

use crate::ffi::{self, RNDN};
use crate::integer::Integer;

/// An MPFR float with a fixed precision (bits). Owns its `mpfr_t`; cleared on
/// drop. Operations round to nearest and produce a result at the left operand's
/// precision.
pub struct Float {
    pub(crate) raw: ffi::mpfr_struct,
}

type MpfrUnary = unsafe extern "C" fn(*mut ffi::mpfr_struct, *const ffi::mpfr_struct, c_int) -> c_int;

impl Float {
    /// New value of `prec` bits, set to NaN until assigned (we always assign).
    pub fn new(prec: u32) -> Self {
        let mut raw = MaybeUninit::<ffi::mpfr_struct>::uninit();
        unsafe {
            ffi::mpfr_init2(raw.as_mut_ptr(), prec.max(2) as ffi::mpfr_prec_t);
            Float { raw: raw.assume_init() }
        }
    }

    pub fn prec(&self) -> u32 {
        unsafe { ffi::mpfr_get_prec(&self.raw) as u32 }
    }

    pub fn from_i64(prec: u32, v: i64) -> Self {
        let mut f = Self::new(prec);
        unsafe { ffi::mpfr_set_si(&mut f.raw, v as c_long, RNDN) };
        f
    }

    pub fn from_integer(prec: u32, i: &Integer) -> Self {
        let mut f = Self::new(prec);
        unsafe { ffi::mpfr_set_z(&mut f.raw, i.as_raw(), RNDN) };
        f
    }

    /// Copy `src` rounded to `prec` bits.
    pub fn with_prec(prec: u32, src: &Float) -> Self {
        let mut f = Self::new(prec);
        unsafe { ffi::mpfr_set(&mut f.raw, &src.raw, RNDN) };
        f
    }

    /// Parse a base-10 string. `None` if MPFR rejects it.
    pub fn from_str(prec: u32, s: &str) -> Option<Self> {
        let c = crate::cbytes(s);
        let mut f = Self::new(prec);
        let ok = unsafe { ffi::mpfr_set_str(&mut f.raw, c.as_ptr() as *const c_char, 10, RNDN) };
        if ok == 0 {
            Some(f)
        } else {
            None
        }
    }

    pub fn pi(prec: u32) -> Self {
        let mut f = Self::new(prec);
        unsafe { ffi::mpfr_const_pi(&mut f.raw, RNDN) };
        f
    }

    pub fn is_zero(&self) -> bool {
        unsafe { ffi::mpfr_zero_p(&self.raw) != 0 }
    }

    /// Normalized `"d.ffffe<exp>"` form with `ndigits` significant digits — the
    /// shape a `%g`-style formatter can post-process.
    pub fn to_string_radix(&self, base: i32, ndigits: usize) -> String {
        let n = ndigits.max(2);
        let mut exp: ffi::mpfr_exp_t = 0;
        let p = unsafe {
            ffi::mpfr_get_str(core::ptr::null_mut(), &mut exp, base as c_int, n, &self.raw, RNDN)
        };
        if p.is_null() {
            return String::new();
        }
        // SAFETY: `p` is a NUL-terminated string MPFR allocated; we free it.
        let s = unsafe {
            let cstr = core::ffi::CStr::from_ptr(p);
            let owned = core::str::from_utf8(cstr.to_bytes()).unwrap_or("").to_string();
            ffi::mpfr_free_str(p);
            owned
        };
        // MPFR yields value = 0.<s> × base^exp. Re-center to one leading digit:
        // value = <d>.<rest> × base^(exp-1).
        let (sign, digits) = match s.strip_prefix('-') {
            Some(r) => ("-", r),
            None => ("", s.as_str()),
        };
        if digits.is_empty() {
            return format!("{sign}0");
        }
        let dexp = exp as i64 - 1;
        let (first, rest) = digits.split_at(1);
        if rest.is_empty() {
            format!("{sign}{first}e{dexp}")
        } else {
            format!("{sign}{first}.{rest}e{dexp}")
        }
    }

    fn unary(self, f: MpfrUnary) -> Float {
        let mut r = Float::new(self.prec());
        unsafe { f(&mut r.raw, &self.raw, RNDN) };
        r
    }

    pub fn sqrt(self) -> Float {
        self.unary(ffi::mpfr_sqrt)
    }
    pub fn abs(self) -> Float {
        self.unary(ffi::mpfr_abs)
    }
    pub fn sin(self) -> Float {
        self.unary(ffi::mpfr_sin)
    }
    pub fn cos(self) -> Float {
        self.unary(ffi::mpfr_cos)
    }
    pub fn tan(self) -> Float {
        self.unary(ffi::mpfr_tan)
    }
    pub fn asin(self) -> Float {
        self.unary(ffi::mpfr_asin)
    }
    pub fn acos(self) -> Float {
        self.unary(ffi::mpfr_acos)
    }
    pub fn atan(self) -> Float {
        self.unary(ffi::mpfr_atan)
    }
    pub fn sinh(self) -> Float {
        self.unary(ffi::mpfr_sinh)
    }
    pub fn cosh(self) -> Float {
        self.unary(ffi::mpfr_cosh)
    }
    pub fn tanh(self) -> Float {
        self.unary(ffi::mpfr_tanh)
    }
    pub fn ln(self) -> Float {
        self.unary(ffi::mpfr_log)
    }
    pub fn log10(self) -> Float {
        self.unary(ffi::mpfr_log10)
    }
    pub fn exp(self) -> Float {
        self.unary(ffi::mpfr_exp)
    }
    pub fn exp10(self) -> Float {
        self.unary(ffi::mpfr_exp10)
    }
    pub fn recip(self) -> Float {
        let mut r = Float::new(self.prec());
        unsafe { ffi::mpfr_ui_div(&mut r.raw, 1, &self.raw, RNDN) };
        r
    }
    /// `self ^ exp`, result at `self`'s precision.
    pub fn pow(self, exp: Float) -> Float {
        let mut r = Float::new(self.prec());
        unsafe { ffi::mpfr_pow(&mut r.raw, &self.raw, &exp.raw, RNDN) };
        r
    }
}

impl Clone for Float {
    fn clone(&self) -> Self {
        Float::with_prec(self.prec(), self)
    }
}

impl Drop for Float {
    fn drop(&mut self) {
        unsafe { ffi::mpfr_clear(&mut self.raw) };
    }
}

macro_rules! fbin {
    ($Trait:ident, $method:ident, $cfn:path) => {
        impl core::ops::$Trait for Float {
            type Output = Float;
            fn $method(self, rhs: Float) -> Float {
                let mut r = Float::new(self.prec());
                unsafe { $cfn(&mut r.raw, &self.raw, &rhs.raw, RNDN) };
                r
            }
        }
    };
}
fbin!(Add, add, ffi::mpfr_add);
fbin!(Sub, sub, ffi::mpfr_sub);
fbin!(Mul, mul, ffi::mpfr_mul);
fbin!(Div, div, ffi::mpfr_div);

impl core::ops::Neg for Float {
    type Output = Float;
    fn neg(self) -> Float {
        let mut r = Float::new(self.prec());
        unsafe { ffi::mpfr_neg(&mut r.raw, &self.raw, RNDN) };
        r
    }
}
