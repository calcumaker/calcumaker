//! Arbitrary-precision integer (`mpz`).

use alloc::string::{String, ToString};
use alloc::vec;
use core::ffi::{c_char, c_int, c_long, c_ulong};
use core::mem::MaybeUninit;

use crate::ffi;

/// A GMP arbitrary-precision integer. Owns its `mpz_t`; cleared on drop.
pub struct Integer {
    pub(crate) raw: ffi::mpz_struct,
}

impl Integer {
    /// Zero.
    pub fn new() -> Self {
        let mut raw = MaybeUninit::<ffi::mpz_struct>::uninit();
        // SAFETY: __gmpz_init fully initializes the struct in place.
        unsafe {
            ffi::__gmpz_init(raw.as_mut_ptr());
            Integer { raw: raw.assume_init() }
        }
    }

    /// From a machine integer. (On 32-bit targets `c_long` is 32-bit, so very
    /// large `v` would truncate — entry uses [`from_str_radix`] for big values.)
    pub fn from_i64(v: i64) -> Self {
        let mut x = Self::new();
        unsafe { ffi::__gmpz_set_si(&mut x.raw, v as c_long) };
        x
    }

    /// Parse in the given radix (2..=62). `None` on a malformed string.
    pub fn from_str_radix(s: &str, base: i32) -> Option<Self> {
        let c = crate::cbytes(s);
        let mut x = Self::new();
        let ok = unsafe { ffi::__gmpz_set_str(&mut x.raw, c.as_ptr() as *const c_char, base as c_int) };
        if ok == 0 {
            Some(x)
        } else {
            None
        }
    }

    /// Digits in the given radix (lowercase for hex; sign-prefixed if negative).
    pub fn to_string_radix(&self, base: i32) -> String {
        let absbase = base.unsigned_abs().max(2) as c_int;
        let size = unsafe { ffi::__gmpz_sizeinbase(&self.raw, absbase) };
        let mut buf = vec![0u8; size + 2]; // digits + sign + NUL
        unsafe { ffi::__gmpz_get_str(buf.as_mut_ptr() as *mut c_char, base as c_int, &self.raw) };
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        core::str::from_utf8(&buf[..end]).unwrap_or("").to_string()
    }

    pub fn is_zero(&self) -> bool {
        unsafe { ffi::__gmpz_cmp_si(&self.raw, 0) == 0 }
    }

    /// Value as `u32` if it is non-negative and fits.
    pub fn to_u32(&self) -> Option<u32> {
        if unsafe { ffi::__gmpz_fits_ulong_p(&self.raw) } == 0 {
            return None;
        }
        let u = unsafe { ffi::__gmpz_get_ui(&self.raw) };
        if u <= u32::MAX as c_ulong {
            Some(u as u32)
        } else {
            None
        }
    }

    /// `n!`
    pub fn factorial(n: u32) -> Self {
        let mut x = Self::new();
        unsafe { ffi::__gmpz_fac_ui(&mut x.raw, n as c_ulong) };
        x
    }

    pub(crate) fn as_raw(&self) -> *const ffi::mpz_struct {
        &self.raw
    }
}

impl Default for Integer {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Integer {
    fn clone(&self) -> Self {
        let mut raw = MaybeUninit::<ffi::mpz_struct>::uninit();
        unsafe {
            ffi::__gmpz_init_set(raw.as_mut_ptr(), &self.raw);
            Integer { raw: raw.assume_init() }
        }
    }
}

impl Drop for Integer {
    fn drop(&mut self) {
        unsafe { ffi::__gmpz_clear(&mut self.raw) };
    }
}

macro_rules! binop {
    ($Trait:ident, $method:ident, $cfn:path) => {
        impl core::ops::$Trait for Integer {
            type Output = Integer;
            fn $method(self, rhs: Integer) -> Integer {
                let mut r = Integer::new();
                unsafe { $cfn(&mut r.raw, &self.raw, &rhs.raw) };
                r
            }
        }
    };
}
binop!(Add, add, ffi::__gmpz_add);
binop!(Sub, sub, ffi::__gmpz_sub);
binop!(Mul, mul, ffi::__gmpz_mul);
binop!(Div, div, ffi::__gmpz_tdiv_q); // truncating quotient
binop!(BitAnd, bitand, ffi::__gmpz_and);
binop!(BitOr, bitor, ffi::__gmpz_ior);
binop!(BitXor, bitxor, ffi::__gmpz_xor);

impl core::ops::Not for Integer {
    type Output = Integer;
    fn not(self) -> Integer {
        let mut r = Integer::new();
        unsafe { ffi::__gmpz_com(&mut r.raw, &self.raw) };
        r
    }
}

impl core::ops::Neg for Integer {
    type Output = Integer;
    fn neg(self) -> Integer {
        let mut r = Integer::new();
        unsafe { ffi::__gmpz_neg(&mut r.raw, &self.raw) };
        r
    }
}

impl core::ops::Shl<u32> for Integer {
    type Output = Integer;
    fn shl(self, n: u32) -> Integer {
        let mut r = Integer::new();
        unsafe { ffi::__gmpz_mul_2exp(&mut r.raw, &self.raw, n as ffi::mp_bitcnt_t) };
        r
    }
}

impl core::ops::Shr<u32> for Integer {
    type Output = Integer;
    fn shr(self, n: u32) -> Integer {
        let mut r = Integer::new();
        unsafe { ffi::__gmpz_tdiv_q_2exp(&mut r.raw, &self.raw, n as ffi::mp_bitcnt_t) };
        r
    }
}
