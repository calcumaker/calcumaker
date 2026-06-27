//! Raw FFI to GNU MP (`mpz`) and MPFR (`mpfr`).
//!
//! Struct layouts are the documented, stable ABIs:
//!   __mpz_struct  = { int _mp_alloc; int _mp_size; mp_limb_t *_mp_d; }
//!   __mpfr_struct = { mpfr_prec_t _mpfr_prec; mpfr_sign_t _mpfr_sign;
//!                     mpfr_exp_t _mpfr_exp; mp_limb_t *_mpfr_d; }
//! We only ever pass pointers to these structs and never dereference the limb
//! pointer, so `_mp_d` / `_mpfr_d` are typed as opaque `*mut c_void`.
//!
//! GMP's public `mpz_*` names are macros for the real `__gmpz_*` symbols, so we
//! bind those. MPFR exports `mpfr_*` directly.
#![allow(non_camel_case_types)]

use core::ffi::{c_char, c_int, c_long, c_ulong, c_void};

pub type size_t = usize;
pub type mp_bitcnt_t = c_ulong;
pub type mpfr_prec_t = c_long;
pub type mpfr_exp_t = c_long;

/// MPFR_RNDN — round to nearest (ties to even). We round-to-nearest throughout.
pub const RNDN: c_int = 0;

#[repr(C)]
pub struct mpz_struct {
    pub _mp_alloc: c_int,
    pub _mp_size: c_int,
    pub _mp_d: *mut c_void,
}

#[repr(C)]
pub struct mpfr_struct {
    pub _mpfr_prec: mpfr_prec_t,
    pub _mpfr_sign: c_int,
    pub _mpfr_exp: mpfr_exp_t,
    pub _mpfr_d: *mut c_void,
}

extern "C" {
    // ---- GNU MP: mpz ------------------------------------------------------
    pub fn __gmpz_init(x: *mut mpz_struct);
    pub fn __gmpz_init_set(r: *mut mpz_struct, s: *const mpz_struct);
    pub fn __gmpz_clear(x: *mut mpz_struct);
    pub fn __gmpz_set_si(r: *mut mpz_struct, v: c_long);
    pub fn __gmpz_set_str(r: *mut mpz_struct, s: *const c_char, base: c_int) -> c_int;
    pub fn __gmpz_get_str(s: *mut c_char, base: c_int, op: *const mpz_struct) -> *mut c_char;
    pub fn __gmpz_sizeinbase(op: *const mpz_struct, base: c_int) -> size_t;
    pub fn __gmpz_add(r: *mut mpz_struct, a: *const mpz_struct, b: *const mpz_struct);
    pub fn __gmpz_sub(r: *mut mpz_struct, a: *const mpz_struct, b: *const mpz_struct);
    pub fn __gmpz_mul(r: *mut mpz_struct, a: *const mpz_struct, b: *const mpz_struct);
    pub fn __gmpz_tdiv_q(q: *mut mpz_struct, n: *const mpz_struct, d: *const mpz_struct);
    pub fn __gmpz_tdiv_r(r: *mut mpz_struct, n: *const mpz_struct, d: *const mpz_struct);
    pub fn __gmpz_abs(r: *mut mpz_struct, a: *const mpz_struct);
    pub fn __gmpz_and(r: *mut mpz_struct, a: *const mpz_struct, b: *const mpz_struct);
    pub fn __gmpz_ior(r: *mut mpz_struct, a: *const mpz_struct, b: *const mpz_struct);
    pub fn __gmpz_xor(r: *mut mpz_struct, a: *const mpz_struct, b: *const mpz_struct);
    pub fn __gmpz_com(r: *mut mpz_struct, a: *const mpz_struct);
    pub fn __gmpz_mul_2exp(r: *mut mpz_struct, a: *const mpz_struct, n: mp_bitcnt_t);
    pub fn __gmpz_tdiv_q_2exp(r: *mut mpz_struct, a: *const mpz_struct, n: mp_bitcnt_t);
    pub fn __gmpz_neg(r: *mut mpz_struct, a: *const mpz_struct);
    pub fn __gmpz_cmp_si(a: *const mpz_struct, b: c_long) -> c_int;
    pub fn __gmpz_fac_ui(r: *mut mpz_struct, n: c_ulong);
    pub fn __gmpz_fits_ulong_p(a: *const mpz_struct) -> c_int;
    pub fn __gmpz_get_ui(a: *const mpz_struct) -> c_ulong;

    // ---- MPFR -------------------------------------------------------------
    pub fn mpfr_init2(x: *mut mpfr_struct, prec: mpfr_prec_t);
    pub fn mpfr_clear(x: *mut mpfr_struct);
    pub fn mpfr_get_prec(x: *const mpfr_struct) -> mpfr_prec_t;
    pub fn mpfr_set(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_set_si(r: *mut mpfr_struct, v: c_long, rnd: c_int) -> c_int;
    pub fn mpfr_set_z(r: *mut mpfr_struct, a: *const mpz_struct, rnd: c_int) -> c_int;
    pub fn mpfr_set_str(r: *mut mpfr_struct, s: *const c_char, base: c_int, rnd: c_int) -> c_int;
    pub fn mpfr_get_str(
        s: *mut c_char,
        exp: *mut mpfr_exp_t,
        base: c_int,
        n: size_t,
        op: *const mpfr_struct,
        rnd: c_int,
    ) -> *mut c_char;
    pub fn mpfr_free_str(s: *mut c_char);
    pub fn mpfr_zero_p(a: *const mpfr_struct) -> c_int;
    pub fn mpfr_add(r: *mut mpfr_struct, a: *const mpfr_struct, b: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_sub(r: *mut mpfr_struct, a: *const mpfr_struct, b: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_mul(r: *mut mpfr_struct, a: *const mpfr_struct, b: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_div(r: *mut mpfr_struct, a: *const mpfr_struct, b: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_ui_div(r: *mut mpfr_struct, u: c_ulong, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_neg(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_sqrt(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_abs(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_sin(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_cos(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_tan(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_asin(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_acos(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_atan(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_sinh(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_cosh(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_tanh(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_log(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_log10(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_exp(r: *mut mpfr_struct, a: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_pow(r: *mut mpfr_struct, a: *const mpfr_struct, b: *const mpfr_struct, rnd: c_int) -> c_int;
    pub fn mpfr_const_pi(r: *mut mpfr_struct, rnd: c_int) -> c_int;
}
