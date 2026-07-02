//! The RPN calculator engine: a value stack + operations over the
//! arbitrary-precision [`Value`] type. One numeric path (GMP + MPFR).
//!
//! Integers stay integers through `+ - * /` and the bitwise ops; the scientific
//! functions promote to MPFR reals. With a word size set the engine follows the
//! HP-16C programmer model: values are interpreted per the **sign mode**
//! (unsigned / 1's / 2's complement), results wrap into the word (setting the
//! **G** overflow flag), adds/subs/shifts/rotates report **carry** (C), and the
//! non-decimal radices display the raw bit pattern. Without a word size,
//! integers are unbounded GMP values and the flags stay untouched.
//!
//! Errors never consume operands: every operation validates its stack depth,
//! operand types, and domain **before** popping, so a failed op leaves the
//! stack (and LASTx) exactly as it was — HP behaviour.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use gmp_mpfr_nostd::{Float, Integer};

use crate::value::Value;

/// Integer display / entry radix (HP-16C programmer modes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Radix {
    Hex,
    Dec,
    Oct,
    Bin,
}

impl Radix {
    pub fn base(self) -> i32 {
        match self {
            Radix::Hex => 16,
            Radix::Dec => 10,
            Radix::Oct => 8,
            Radix::Bin => 2,
        }
    }
}

/// Integer sign interpretation under a word size (HP-16C UNSGN / 1's / 2's).
/// Only meaningful with `wsize` set; 2's complement is the default.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignMode {
    Unsigned,
    Ones,
    Twos,
}

/// Real-number display format (HP FIX/SCI/ENG; `Auto` is `%g`-style).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FloatFmt {
    Auto,
    Fix(u8),
    Sci(u8),
    Eng(u8),
}

/// Angle unit for the circular trig functions (hyperbolics are unaffected).
/// Radians is the default — the math-natural unit for an arbitrary-precision
/// engine; `deg`/`grad` scale via MPFR π with guard bits, reduce mod the full
/// circle exactly, and special-case the exactly-representable angles
/// (90°-multiples, sin 30° = ½, tan 45° = 1, …) so `180 sin` is 0, not a
/// 2^-prec residue.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AngleMode {
    Rad,
    Deg,
    Grad,
}

impl AngleMode {
    /// Degrees in a half turn (π radians) — the conversion factor.
    fn half_turn(self) -> i64 {
        match self {
            AngleMode::Rad => 0, // unused
            AngleMode::Deg => 180,
            AngleMode::Grad => 200,
        }
    }
}

/// Which circular function — needed for the exact-angle tables.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Circ {
    Sin,
    Cos,
    Tan,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InvCirc {
    Asin,
    Acos,
    Atan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalcError {
    /// Token was neither a number (in the active radix) nor a known command.
    Parse(String),
    /// Stack underflow.
    Empty,
    /// Wrong operand type / domain for the operation.
    TypeError(&'static str),
    /// Division by zero.
    DivZero,
}

/// STO/RCL register file size (one register per hex digit key, 0–F).
pub const REGISTERS: usize = 16;

/// Word sizes beyond this are almost certainly a slip; refuse them.
const MAX_WORD_BITS: u32 = 16384;
const MAX_FMT_DIGITS: u32 = 32;

/// Exact-power results are capped at ~1 Mbit (≈300k decimal digits) so a slip
/// like `2 1e9 pow` errors instead of exhausting memory. Generous for real use.
const MAX_POW_BITS: u64 = 1 << 20;

pub struct Calc {
    stack: Vec<Value>,
    prec: u32,
    radix: Radix,
    word_bits: Option<u32>,
    sign_mode: SignMode,
    float_fmt: FloatFmt,
    angle_mode: AngleMode,
    leading_zeros: bool,
    carry: bool,
    overflow: bool,
    last_x: Option<Value>,
    regs: Vec<Option<Value>>,
}

// ---- word-size helpers (shared with the formatter) --------------------------

fn one() -> Integer {
    Integer::from_i64(1)
}

fn pow2(n: u32) -> Integer {
    one() << n
}

/// Euclidean modulus (result in `[0, m)`), built from GMP's truncating `%`.
fn euclid_mod(v: Integer, m: &Integer) -> Integer {
    let r = v % m.clone();
    if r.is_negative() {
        r + m.clone()
    } else {
        r
    }
}

/// Canonical signed value → its n-bit pattern in `[0, 2^n)`.
pub(crate) fn encode_bits(v: &Integer, mode: SignMode, n: u32) -> Integer {
    if !v.is_negative() {
        return v.clone();
    }
    match mode {
        SignMode::Unsigned => v.clone(), // canonical unsigned is never negative
        SignMode::Twos => v.clone() + pow2(n),
        // 1's complement: -x is the bitwise complement; -0 folds onto +0.
        SignMode::Ones => v.clone() + pow2(n) - one(),
    }
}

/// n-bit pattern in `[0, 2^n)` → the canonical signed value for the mode.
fn decode_bits(bits: Integer, mode: SignMode, n: u32) -> Integer {
    let half = pow2(n - 1);
    match mode {
        SignMode::Unsigned => bits,
        SignMode::Twos => {
            if bits >= half {
                bits - pow2(n)
            } else {
                bits
            }
        }
        SignMode::Ones => {
            // The all-ones pattern (-0) decodes to 0 via 2^n-1 - (2^n-1).
            if bits >= half {
                bits - (pow2(n) - one())
            } else {
                bits
            }
        }
    }
}

/// Wrap an exact signed result into the mode's canonical range; the flag
/// reports whether anything was lost (the G / out-of-range condition).
fn wrap(v: Integer, mode: SignMode, n: u32) -> (Integer, bool) {
    let w = match mode {
        SignMode::Unsigned => euclid_mod(v.clone(), &pow2(n)),
        SignMode::Twos => decode_bits(euclid_mod(v.clone(), &pow2(n)), SignMode::Twos, n),
        SignMode::Ones => {
            // 1's-complement arithmetic is mod 2^n - 1 (end-around carry).
            let m = pow2(n) - one();
            let t = euclid_mod(v.clone(), &m);
            if t >= pow2(n - 1) {
                t - m
            } else {
                t
            }
        }
    };
    let ovf = w != v;
    (w, ovf)
}

/// Kinds of single-value shift/rotate (`X` by one, or `Y` by `X`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ShiftKind {
    Left,     // SL: logical left
    Right,    // SR: logical right (zero fill)
    ArithR,   // ASR: right, sign fill
    RotLeft,  // RL
    RotRight, // RR
}

impl Calc {
    /// New calculator with `prec` bits of MPFR working precision (e.g. 256 ≈ 77
    /// decimal digits). Decimal radix, unbounded integers, 2's complement.
    pub fn new(prec: u32) -> Self {
        Self {
            stack: Vec::new(),
            prec: prec.max(2),
            radix: Radix::Dec,
            word_bits: None,
            sign_mode: SignMode::Twos,
            float_fmt: FloatFmt::Auto,
            angle_mode: AngleMode::Rad,
            leading_zeros: false,
            carry: false,
            overflow: false,
            last_x: None,
            regs: vec![None; REGISTERS],
        }
    }

    // ---- state ------------------------------------------------------------
    pub fn prec(&self) -> u32 {
        self.prec
    }
    pub fn set_prec(&mut self, prec: u32) {
        self.prec = prec.max(2);
    }
    pub fn radix(&self) -> Radix {
        self.radix
    }
    pub fn set_radix(&mut self, r: Radix) {
        self.radix = r;
    }
    /// Word size in bits for the programmer modes; `None` = unbounded (GMP).
    /// Stack integers are reinterpreted bit-pattern-preserving (HP behaviour).
    pub fn set_word_bits(&mut self, bits: Option<u32>) {
        let old = (self.word_bits, self.sign_mode);
        self.word_bits = bits;
        self.renormalize(old);
    }
    pub fn word_bits(&self) -> Option<u32> {
        self.word_bits
    }
    /// Sign interpretation under the word size. Changing it reinterprets the
    /// bit patterns on the stack (HP behaviour), not the signed values.
    pub fn set_sign_mode(&mut self, m: SignMode) {
        let old = (self.word_bits, self.sign_mode);
        self.sign_mode = m;
        self.renormalize(old);
    }
    pub fn sign_mode(&self) -> SignMode {
        self.sign_mode
    }
    pub fn float_fmt(&self) -> FloatFmt {
        self.float_fmt
    }
    pub fn set_float_fmt(&mut self, f: FloatFmt) {
        self.float_fmt = f;
    }
    pub fn angle_mode(&self) -> AngleMode {
        self.angle_mode
    }
    pub fn set_angle_mode(&mut self, m: AngleMode) {
        self.angle_mode = m;
    }
    /// 16C flag 3: pad hex/oct/bin display with leading zeros to the word width.
    pub fn leading_zeros(&self) -> bool {
        self.leading_zeros
    }
    pub fn set_leading_zeros(&mut self, on: bool) {
        self.leading_zeros = on;
    }
    /// Carry flag (C) — set by word-mode add/sub/shift/rotate.
    pub fn carry(&self) -> bool {
        self.carry
    }
    /// Out-of-range flag (G) — set when a word-mode result wrapped.
    pub fn overflow(&self) -> bool {
        self.overflow
    }
    pub fn stack(&self) -> &[Value] {
        &self.stack
    }
    /// STO/RCL register file (read-only view; `sto<i>`/`rcl<i>` mutate).
    pub fn registers(&self) -> &[Option<Value>] {
        &self.regs
    }

    /// Format the top of stack (X) for the display; empty string if the stack is
    /// empty.
    pub fn display(&self) -> String {
        match self.stack.last() {
            Some(v) => crate::format::format(v, self),
            None => String::new(),
        }
    }

    /// Reinterpret stack integers after a word-size / sign-mode change,
    /// preserving bit patterns like the 16C (registers are left alone).
    fn renormalize(&mut self, (old_bits, old_mode): (Option<u32>, SignMode)) {
        let new = (self.word_bits, self.sign_mode);
        if new == (old_bits, old_mode) {
            return;
        }
        for v in &mut self.stack {
            if let Value::Int(i) = v {
                let bits = match old_bits {
                    Some(n) => encode_bits(i, old_mode, n),
                    None => i.clone(), // unbounded → take the value as-is
                };
                *i = match self.word_bits {
                    Some(n) => {
                        let pattern = if bits.is_negative() {
                            // negative unbounded value entering a word: wrap
                            euclid_mod(bits, &pow2(n))
                        } else {
                            euclid_mod(bits, &pow2(n))
                        };
                        decode_bits(pattern, self.sign_mode, n)
                    }
                    None => {
                        if old_bits.is_some() {
                            // leaving word mode: keep the signed value
                            decode_bits_back(bits, old_mode, old_bits.unwrap())
                        } else {
                            bits
                        }
                    }
                };
            }
        }
    }

    // ---- input ------------------------------------------------------------
    /// Feed one token: a number (pushed) or a command (applied). Commands are
    /// matched first, so `and`/`dec`/... are operators even in hex mode.
    pub fn input(&mut self, tok: &str) -> Result<(), CalcError> {
        let t = tok.trim();
        if t.is_empty() {
            return Ok(());
        }
        let lower = t.to_ascii_lowercase();
        if let Some(i) = reg_index(&lower, "sto") {
            return self.sto(i);
        }
        if let Some(i) = reg_index(&lower, "rcl") {
            return self.rcl(i);
        }
        if is_command(&lower) {
            return self.command(&lower);
        }
        if let Some(v) = self.try_parse_number(t) {
            self.stack.push(v);
            return Ok(());
        }
        Err(CalcError::Parse(t.to_string()))
    }

    fn try_parse_number(&self, t: &str) -> Option<Value> {
        if self.radix == Radix::Dec && (t.contains('.') || t.contains('e') || t.contains('E')) {
            return Float::from_str(self.prec, t).map(Value::Real);
        }
        let v = Integer::from_str_radix(t, self.radix.base())?;
        Some(Value::Int(self.canon_entry(v)))
    }

    /// Canonicalize an entered integer under the word size: non-decimal entry
    /// is a raw bit pattern (16C style), decimal entry a signed value; both
    /// wrap silently (flags are op-only).
    fn canon_entry(&self, v: Integer) -> Integer {
        let Some(n) = self.word_bits else { return v };
        if self.radix != Radix::Dec && !v.is_negative() {
            decode_bits(euclid_mod(v, &pow2(n)), self.sign_mode, n)
        } else {
            wrap(v, self.sign_mode, n).0
        }
    }

    // ---- dispatch ---------------------------------------------------------
    fn command(&mut self, cmd: &str) -> Result<(), CalcError> {
        match cmd {
            "+" => self.arith('+'),
            "-" => self.arith('-'),
            "*" => self.arith('*'),
            "/" => self.arith('/'),
            "chs" => self.chs(),
            "swap" => self.swap(),
            "drop" => self.pop_unchecked().map(|_| ()),
            "dup" => self.dup(),
            "sqrt" => self.sqrt_op(),
            "sin" => self.circ(Circ::Sin),
            "cos" => self.circ(Circ::Cos),
            "tan" => self.circ(Circ::Tan),
            "ln" => self.unary_real(|x| x.ln()),
            "exp" => self.unary_real(|x| x.exp()),
            "inv" => self.unary_real(|x| x.recip()),
            "sq" => self.sq(),
            "asin" => self.inv_circ(InvCirc::Asin),
            "acos" => self.inv_circ(InvCirc::Acos),
            "atan" => self.inv_circ(InvCirc::Atan),
            "sinh" => self.unary_real(|x| x.sinh()),
            "cosh" => self.unary_real(|x| x.cosh()),
            "tanh" => self.unary_real(|x| x.tanh()),
            "log" => self.unary_real(|x| x.log10()),
            "exp10" => self.exp10_op(),
            "abs" => self.abs_op(),
            "pow" => self.pow_op(),
            "mod" => self.mod_op(),
            "pct" => self.pct(),
            "e" => {
                self.push_e();
                Ok(())
            }
            "pi" => {
                self.stack.push(Value::Real(Float::pi(self.prec)));
                Ok(())
            }
            "lastx" => self.lastx(),
            "enter" => self.dup(),
            "over" => self.over(),
            "rolldn" | "roll" => self.roll_down(),
            "rollup" => self.roll_up(),
            "and" => self.bitwise('&'),
            "or" => self.bitwise('|'),
            "xor" => self.bitwise('^'),
            "not" => self.not_op(),
            "sl" => self.shift_rot(ShiftKind::Left, false),
            "sr" => self.shift_rot(ShiftKind::Right, false),
            "asr" => self.shift_rot(ShiftKind::ArithR, false),
            "rl" => self.shift_rot(ShiftKind::RotLeft, false),
            "rr" => self.shift_rot(ShiftKind::RotRight, false),
            "rlc" => self.rot_carry(true, false),
            "rrc" => self.rot_carry(false, false),
            "shl" | "sln" => self.shift_rot(ShiftKind::Left, true),
            "shr" | "srn" => self.shift_rot(ShiftKind::Right, true),
            "asrn" => self.shift_rot(ShiftKind::ArithR, true),
            "rln" => self.shift_rot(ShiftKind::RotLeft, true),
            "rrn" => self.shift_rot(ShiftKind::RotRight, true),
            "rlcn" => self.rot_carry(true, true),
            "rrcn" => self.rot_carry(false, true),
            "lj" => self.left_justify(),
            "bset" => self.bit_op(BitOp::Set),
            "bclr" => self.bit_op(BitOp::Clear),
            "btest" => self.bit_op(BitOp::Test),
            "maskl" => self.mask_op(true),
            "maskr" => self.mask_op(false),
            "popcnt" => self.popcnt(),
            "fact" | "!" => self.fact(),
            "float" => self.to_float(),
            "round" => self.real_to_int(|f| f.round_to_int()),
            "trunc" => self.real_to_int(|f| f.trunc_to_int()),
            "floor" => self.real_to_int(|f| f.floor_to_int()),
            "ceil" => self.real_to_int(|f| f.ceil_to_int()),
            "frac" => self.frac(),
            "hex" => self.set_radix_ok(Radix::Hex),
            "dec" => self.set_radix_ok(Radix::Dec),
            "oct" => self.set_radix_ok(Radix::Oct),
            "bin" => self.set_radix_ok(Radix::Bin),
            "wsize" => self.wsize(),
            "prec" => self.prec_cmd(),
            "unsgn" => {
                self.set_sign_mode(SignMode::Unsigned);
                Ok(())
            }
            "1s" => {
                self.set_sign_mode(SignMode::Ones);
                Ok(())
            }
            "2s" => {
                self.set_sign_mode(SignMode::Twos);
                Ok(())
            }
            "signmode" => {
                self.set_sign_mode(match self.sign_mode {
                    SignMode::Twos => SignMode::Ones,
                    SignMode::Ones => SignMode::Unsigned,
                    SignMode::Unsigned => SignMode::Twos,
                });
                Ok(())
            }
            "rad" => {
                self.angle_mode = AngleMode::Rad;
                Ok(())
            }
            "deg" => {
                self.angle_mode = AngleMode::Deg;
                Ok(())
            }
            "grad" => {
                self.angle_mode = AngleMode::Grad;
                Ok(())
            }
            "lz" => {
                self.leading_zeros = !self.leading_zeros;
                Ok(())
            }
            "anglemode" => {
                self.angle_mode = match self.angle_mode {
                    AngleMode::Rad => AngleMode::Deg,
                    AngleMode::Deg => AngleMode::Grad,
                    AngleMode::Grad => AngleMode::Rad,
                };
                Ok(())
            }
            "fix" => self.fmt_cmd(FloatFmt::Fix(0)),
            "sci" => self.fmt_cmd(FloatFmt::Sci(0)),
            "eng" => self.fmt_cmd(FloatFmt::Eng(0)),
            "std" => {
                self.float_fmt = FloatFmt::Auto;
                Ok(())
            }
            "clear" => {
                self.stack.clear();
                self.carry = false;
                self.overflow = false;
                self.last_x = None;
                Ok(())
            }
            _ => Err(CalcError::Parse(cmd.to_string())),
        }
    }

    // ---- validation helpers (before any pop — errors must not consume) -----
    fn need(&self, n: usize) -> Result<(), CalcError> {
        if self.stack.len() < n {
            Err(CalcError::Empty)
        } else {
            Ok(())
        }
    }

    /// Operand at `depth` (0 = X) as an integer, or a TypeError.
    fn peek_int(&self, depth: usize, what: &'static str) -> Result<&Integer, CalcError> {
        match &self.stack[self.stack.len() - 1 - depth] {
            Value::Int(i) => Ok(i),
            Value::Real(_) => Err(CalcError::TypeError(what)),
        }
    }

    /// X as a small non-negative count, validated in place.
    fn peek_u32(&self, what: &'static str) -> Result<u32, CalcError> {
        self.peek_int(0, what)?.to_u32().ok_or(CalcError::TypeError(what))
    }

    /// Pop X, recording it as LASTx. Call only after validation succeeded.
    fn pop_x(&mut self) -> Value {
        let v = self.stack.pop().expect("validated");
        self.last_x = Some(v.clone());
        v
    }

    /// Pop without touching LASTx (drop, and inner operands).
    fn pop_unchecked(&mut self) -> Result<Value, CalcError> {
        self.stack.pop().ok_or(CalcError::Empty)
    }

    fn set_radix_ok(&mut self, r: Radix) -> Result<(), CalcError> {
        self.radix = r;
        Ok(())
    }

    /// Wrap a word-mode result and record the G flag; identity when unbounded.
    fn canon_flagged(&mut self, v: Integer) -> Integer {
        match self.word_bits {
            Some(n) => {
                let (w, ovf) = wrap(v, self.sign_mode, n);
                self.overflow = ovf;
                w
            }
            None => v,
        }
    }

    /// Wrap without flag side effects (conversions, masks).
    fn canon_silent(&self, v: Integer) -> Integer {
        match self.word_bits {
            Some(n) => wrap(v, self.sign_mode, n).0,
            None => v,
        }
    }

    // ---- operations ---------------------------------------------------------
    fn arith(&mut self, op: char) -> Result<(), CalcError> {
        self.need(2)?;
        let len = self.stack.len();
        let both_int = matches!(&self.stack[len - 1], Value::Int(_))
            && matches!(&self.stack[len - 2], Value::Int(_));
        if both_int {
            if op == '/' {
                if let Value::Int(d) = &self.stack[len - 1] {
                    if d.is_zero() {
                        return Err(CalcError::DivZero);
                    }
                }
            }
            let Value::Int(b) = self.pop_x() else { unreachable!() };
            let Value::Int(a) = self.stack.pop().expect("validated") else { unreachable!() };

            // Carry: computed in the bit domain before the operands move.
            if let Some(n) = self.word_bits {
                self.carry = match op {
                    '+' => {
                        let m = match self.sign_mode {
                            SignMode::Ones => pow2(n) - one(),
                            _ => pow2(n),
                        };
                        encode_bits(&a, self.sign_mode, n) + encode_bits(&b, self.sign_mode, n) >= m
                    }
                    '-' => encode_bits(&a, self.sign_mode, n) < encode_bits(&b, self.sign_mode, n),
                    _ => false,
                };
            }

            let exact = match op {
                '+' => a + b,
                '-' => a - b,
                '*' => a * b,
                '/' => a / b,
                _ => unreachable!(),
            };
            let v = self.canon_flagged(exact);
            self.stack.push(Value::Int(v));
        } else {
            let b = self.pop_x().to_real(self.prec);
            let a = self.stack.pop().expect("validated").to_real(self.prec);
            let r = match op {
                '+' => a + b,
                '-' => a - b,
                '*' => a * b,
                '/' => a / b,
                _ => unreachable!(),
            };
            self.stack.push(Value::Real(r));
        }
        Ok(())
    }

    fn bitwise(&mut self, op: char) -> Result<(), CalcError> {
        self.need(2)?;
        self.peek_int(0, "bitwise needs integers")?;
        self.peek_int(1, "bitwise needs integers")?;
        let Value::Int(b) = self.pop_x() else { unreachable!() };
        let Value::Int(a) = self.stack.pop().expect("validated") else { unreachable!() };
        let v = match self.word_bits {
            Some(n) => {
                let (pa, pb) = (encode_bits(&a, self.sign_mode, n), encode_bits(&b, self.sign_mode, n));
                let bits = match op {
                    '&' => pa & pb,
                    '|' => pa | pb,
                    '^' => pa ^ pb,
                    _ => unreachable!(),
                };
                decode_bits(bits, self.sign_mode, n)
            }
            // Unbounded: GMP's infinite two's-complement semantics.
            None => match op {
                '&' => a & b,
                '|' => a | b,
                '^' => a ^ b,
                _ => unreachable!(),
            },
        };
        self.stack.push(Value::Int(v));
        Ok(())
    }

    fn not_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        self.peek_int(0, "not needs an integer")?;
        let Value::Int(x) = self.pop_x() else { unreachable!() };
        let v = match self.word_bits {
            Some(n) => {
                let bits = encode_bits(&x, self.sign_mode, n) ^ (pow2(n) - one());
                decode_bits(bits, self.sign_mode, n)
            }
            None => !x,
        };
        self.stack.push(Value::Int(v));
        Ok(())
    }

    /// SL/SR/ASR/RL/RR (X by one bit) and their count forms (Y by X bits).
    fn shift_rot(&mut self, kind: ShiftKind, count_from_x: bool) -> Result<(), CalcError> {
        let (k, val_depth) = if count_from_x {
            self.need(2)?;
            (self.peek_u32("shift/rotate count out of range")?, 1)
        } else {
            self.need(1)?;
            (1, 0)
        };
        self.peek_int(val_depth, "shift/rotate needs an integer")?;

        let rotate = matches!(kind, ShiftKind::RotLeft | ShiftKind::RotRight);
        if self.word_bits.is_none() && rotate {
            return Err(CalcError::TypeError("rotate needs a word size (wsize)"));
        }

        // Validated — commit the pops.
        let x = if count_from_x {
            let c = self.pop_x();
            let Value::Int(v) = self.stack.pop().expect("validated") else { unreachable!() };
            let _ = c;
            v
        } else {
            let Value::Int(v) = self.pop_x() else { unreachable!() };
            v
        };

        let v = match self.word_bits {
            Some(n) => {
                let bits = encode_bits(&x, self.sign_mode, n);
                let mask = pow2(n) - one();
                let bit = |b: &Integer, i: u32| -> bool {
                    if i >= n {
                        return false;
                    }
                    !((b.clone() >> i) & one()).is_zero()
                };
                let (out, carry) = match kind {
                    ShiftKind::Left => {
                        let c = k >= 1 && k <= n && bit(&bits, n - k);
                        ((bits << k.min(n + 1)) & mask, c)
                    }
                    ShiftKind::Right => {
                        let c = k >= 1 && bit(&bits, k - 1);
                        (bits >> k.min(n + 1), c)
                    }
                    ShiftKind::ArithR => {
                        let c = k >= 1 && bit(&bits, k - 1);
                        let fill = if bit(&bits, n - 1) {
                            // sign-fill: the top min(k,n) bits become ones
                            let kk = k.min(n);
                            (pow2(kk) - one()) << (n - kk)
                        } else {
                            Integer::new()
                        };
                        ((bits >> k.min(n + 1)) | fill, c)
                    }
                    ShiftKind::RotLeft => {
                        let k = k % n;
                        let c = k >= 1 && bit(&bits, n - k);
                        (((bits.clone() << k) | (bits >> (n - k).min(n))) & mask, c)
                    }
                    ShiftKind::RotRight => {
                        let k = k % n;
                        let c = k >= 1 && bit(&bits, k - 1);
                        (((bits.clone() >> k) | (bits << (n - k.min(n)))) & mask, c)
                    }
                };
                self.carry = carry;
                self.overflow = false;
                decode_bits(out, self.sign_mode, n)
            }
            None => match kind {
                ShiftKind::Left => x << k,
                ShiftKind::Right => x >> k, // truncating (toward zero)
                ShiftKind::ArithR => x.shr_floor(k), // sign-extending
                _ => unreachable!("rotate rejected above"),
            },
        };
        self.stack.push(Value::Int(v));
        Ok(())
    }

    /// RLC/RRC — rotate through carry: an (n+1)-bit rotation of the word plus
    /// the C flag (bit n). By one bit, or by X bits (`count_from_x`). Word
    /// mode only.
    fn rot_carry(&mut self, left: bool, count_from_x: bool) -> Result<(), CalcError> {
        let Some(n) = self.word_bits else {
            return Err(CalcError::TypeError("rotate needs a word size (wsize)"));
        };
        let (k, val_depth) = if count_from_x {
            self.need(2)?;
            (self.peek_u32("rotate count out of range")?, 1)
        } else {
            self.need(1)?;
            (1, 0)
        };
        self.peek_int(val_depth, "rotate needs an integer")?;

        // Committed — pop.
        let x = if count_from_x {
            let _ = self.pop_x();
            let Value::Int(v) = self.stack.pop().expect("validated") else { unreachable!() };
            v
        } else {
            let Value::Int(v) = self.pop_x() else { unreachable!() };
            v
        };

        // Build the (n+1)-bit register: carry in bit n, the word below.
        let w = n + 1;
        let k = k % w;
        let mut t = encode_bits(&x, self.sign_mode, n);
        if self.carry {
            t = t | (one() << n);
        }
        let full = pow2(w) - one();
        let t = if left {
            ((t.clone() << k) | (t >> (w - k).min(w))) & full
        } else {
            ((t.clone() >> k) | (t << (w - k.min(w)))) & full
        };
        self.carry = !((t.clone() >> n) & one()).is_zero();
        self.overflow = false;
        let bits = t & (pow2(n) - one());
        self.stack.push(Value::Int(decode_bits(bits, self.sign_mode, n)));
        Ok(())
    }

    /// LJ — left-justify X in the word: Y gets the justified value, X the
    /// shift count (HP-16C). Word mode only.
    fn left_justify(&mut self) -> Result<(), CalcError> {
        let Some(n) = self.word_bits else {
            return Err(CalcError::TypeError("lj needs a word size (wsize)"));
        };
        self.need(1)?;
        self.peek_int(0, "lj needs an integer")?;
        let Value::Int(x) = self.pop_x() else { unreachable!() };
        let bits = encode_bits(&x, self.sign_mode, n);
        let shifts = if bits.is_zero() { 0 } else { n as usize - bits.bit_len() };
        let justified = decode_bits(bits << shifts as u32, self.sign_mode, n);
        self.stack.push(Value::Int(justified));
        self.stack.push(Value::Int(Integer::from_i64(shifts as i64)));
        Ok(())
    }

    /// Mathematical bit `i` of a signed value (infinite two's complement),
    /// via Euclidean mod — correct for negatives where `>>` (tdiv) is not.
    fn math_bit(v: &Integer, i: u32) -> bool {
        euclid_mod(v.clone(), &pow2(i + 1)) >= pow2(i)
    }

    fn bit_index(&self, what: &'static str) -> Result<u32, CalcError> {
        let i = self.peek_u32(what)?;
        if let Some(n) = self.word_bits {
            if i >= n {
                return Err(CalcError::TypeError("bit index exceeds the word size"));
            }
        }
        Ok(i)
    }

    fn bit_op(&mut self, op: BitOp) -> Result<(), CalcError> {
        self.need(2)?;
        let i = self.bit_index("bit index out of range")?;
        self.peek_int(1, "bit ops need integers")?;
        let _ = self.pop_x(); // the index
        match op {
            BitOp::Test => {
                // Y stays put; the 0/1 answer lands in X above it.
                let Value::Int(y) = self.stack.last().expect("validated") else { unreachable!() };
                let t = Self::math_bit(y, i);
                self.stack.push(Value::Int(Integer::from_i64(t as i64)));
            }
            BitOp::Set | BitOp::Clear => {
                let Value::Int(y) = self.stack.pop().expect("validated") else { unreachable!() };
                let exact = match op {
                    BitOp::Set => y | (one() << i),
                    BitOp::Clear => y & !(one() << i),
                    BitOp::Test => unreachable!(),
                };
                let v = self.canon_silent(exact);
                self.stack.push(Value::Int(v));
            }
        }
        Ok(())
    }

    /// MASKL (n leading ones — needs a word size) / MASKR (n trailing ones).
    fn mask_op(&mut self, left: bool) -> Result<(), CalcError> {
        self.need(1)?;
        let k = self.peek_u32("mask width out of range")?;
        let v = match (self.word_bits, left) {
            (Some(n), _) => {
                if k > n {
                    return Err(CalcError::TypeError("mask width exceeds the word size"));
                }
                let ones = pow2(k) - one();
                let bits = if left { ones << (n - k) } else { ones };
                decode_bits(bits, self.sign_mode, n)
            }
            (None, false) => pow2(k) - one(),
            (None, true) => return Err(CalcError::TypeError("maskl needs a word size (wsize)")),
        };
        let _ = self.pop_x();
        self.stack.push(Value::Int(v));
        Ok(())
    }

    /// #B — count of one-bits in X's word pattern.
    fn popcnt(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let x = self.peek_int(0, "popcount needs an integer")?;
        let count = match self.word_bits {
            Some(n) => encode_bits(x, self.sign_mode, n).popcount().expect("pattern is non-negative"),
            None => x
                .popcount()
                .ok_or(CalcError::TypeError("popcount of a negative needs a word size"))?,
        };
        let _ = self.pop_x();
        self.stack.push(Value::Int(Integer::from_i64(count as i64)));
        Ok(())
    }

    fn fact(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        self.peek_int(0, "factorial needs an integer")?;
        let n = self.peek_u32("factorial needs a small non-negative integer")?;
        let _ = self.pop_x();
        let v = self.canon_flagged(Integer::factorial(n));
        self.stack.push(Value::Int(v));
        Ok(())
    }

    fn chs(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let v = match self.pop_x() {
            Value::Int(x) => Value::Int(self.canon_flagged(-x)),
            Value::Real(f) => Value::Real(-f),
        };
        self.stack.push(v);
        Ok(())
    }

    fn swap(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let len = self.stack.len();
        self.stack.swap(len - 1, len - 2);
        Ok(())
    }

    fn dup(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let top = self.stack.last().expect("validated").clone();
        self.stack.push(top);
        Ok(())
    }

    fn unary_real(&mut self, f: impl FnOnce(Float) -> Float) -> Result<(), CalcError> {
        self.need(1)?;
        let x = self.pop_x().to_real(self.prec);
        self.stack.push(Value::Real(f(x)));
        Ok(())
    }

    // ---- circular trig (angle-mode aware) ------------------------------------
    fn circ(&mut self, kind: Circ) -> Result<(), CalcError> {
        self.need(1)?;
        let x = self.pop_x().to_real(self.prec);
        let v = self.circ_value(kind, x);
        self.stack.push(Value::Real(v));
        Ok(())
    }

    fn circ_value(&self, kind: Circ, x: Float) -> Float {
        if self.angle_mode == AngleMode::Rad {
            return match kind {
                Circ::Sin => x.sin(),
                Circ::Cos => x.cos(),
                Circ::Tan => x.tan(),
            };
        }
        let half = self.angle_mode.half_turn();
        // Reduce mod a full turn — exact for exactly-representable angles, so
        // the table below fires for 36000090° as well as 90°.
        let turn = Float::from_i64(self.prec, 2 * half);
        let mut r = x.fmod(&turn);
        if r.is_sign_negative() && !r.is_zero() {
            r = r + turn;
        }
        if let Some(v) = self.circ_table(kind, &r, half) {
            return v;
        }
        // General angle: convert with guard bits so the π multiply/divide
        // doesn't eat working precision, then round back.
        let p = self.prec + 32;
        let theta = Float::with_prec(p, &r) * Float::pi(p) / Float::from_i64(p, half);
        let v = match kind {
            Circ::Sin => theta.sin(),
            Circ::Cos => theta.cos(),
            Circ::Tan => theta.tan(),
        };
        Float::with_prec(self.prec, &v)
    }

    /// Exactly-representable results at exactly-hit angles, `r ∈ [0, 2·half)`:
    /// quadrant boundaries, the ½-exact sin/cos angles (degrees), tan = ±1.
    fn circ_table(&self, kind: Circ, r: &Float, half: i64) -> Option<Float> {
        let q = half / 2; // a quarter turn: 90° / 100g
        let at = |v: i64| r.equals(&Float::from_i64(self.prec, v));
        let int = |v: i64| Some(Float::from_i64(self.prec, v));
        let half_of = |sign: i64| Some(Float::from_i64(self.prec, sign) / Float::from_i64(self.prec, 2));
        // tan at a quadrant boundary: ±∞ (1/+0); HP errors here, we show inf.
        let inf = || Some(Float::from_i64(self.prec, 0).recip());
        for k in 0..4i64 {
            if at(k * q) {
                return match kind {
                    Circ::Sin => int([0, 1, 0, -1][k as usize]),
                    Circ::Cos => int([1, 0, -1, 0][k as usize]),
                    Circ::Tan => {
                        if k % 2 == 0 {
                            int(0)
                        } else {
                            inf()
                        }
                    }
                };
            }
        }
        if half == 180 {
            // sin/cos = ±½ (degrees only; the grad equivalents aren't integral)
            match kind {
                Circ::Sin => {
                    if at(30) || at(150) {
                        return half_of(1);
                    }
                    if at(210) || at(330) {
                        return half_of(-1);
                    }
                }
                Circ::Cos => {
                    if at(60) || at(300) {
                        return half_of(1);
                    }
                    if at(120) || at(240) {
                        return half_of(-1);
                    }
                }
                Circ::Tan => {}
            }
        }
        if kind == Circ::Tan {
            let e = half / 4; // 45° / 50g
            if at(e) || at(e + 2 * q) {
                return int(1);
            }
            if at(e + q) || at(e + 3 * q) {
                return int(-1);
            }
        }
        None
    }

    fn inv_circ(&mut self, kind: InvCirc) -> Result<(), CalcError> {
        self.need(1)?;
        let x = self.pop_x().to_real(self.prec);
        let v = self.inv_circ_value(kind, x);
        self.stack.push(Value::Real(v));
        Ok(())
    }

    fn inv_circ_value(&self, kind: InvCirc, x: Float) -> Float {
        if self.angle_mode == AngleMode::Rad {
            return match kind {
                InvCirc::Asin => x.asin(),
                InvCirc::Acos => x.acos(),
                InvCirc::Atan => x.atan(),
            };
        }
        let half = self.angle_mode.half_turn();
        if let Some(v) = self.inv_circ_table(kind, &x, half) {
            return v;
        }
        let p = self.prec + 32;
        let xp = Float::with_prec(p, &x);
        let r = match kind {
            InvCirc::Asin => xp.asin(),
            InvCirc::Acos => xp.acos(),
            InvCirc::Atan => xp.atan(),
        };
        let v = r * Float::from_i64(p, half) / Float::pi(p);
        Float::with_prec(self.prec, &v)
    }

    /// Exact inverse-trig hits: asin ±1/±½/0, acos ±1/±½/0, atan ±1/0.
    fn inv_circ_table(&self, kind: InvCirc, x: &Float, half: i64) -> Option<Float> {
        let q = half / 2;
        let eq = |v: i64| x.equals(&Float::from_i64(self.prec, v));
        let int = |v: i64| Some(Float::from_i64(self.prec, v));
        let one_half = Float::from_i64(self.prec, 1) / Float::from_i64(self.prec, 2);
        let eq_half = |sign: i64| {
            let t = if sign < 0 { -one_half.clone() } else { one_half.clone() };
            x.equals(&t)
        };
        match kind {
            InvCirc::Asin => {
                if eq(0) {
                    return int(0);
                }
                if eq(1) {
                    return int(q);
                }
                if eq(-1) {
                    return int(-q);
                }
                if half == 180 && eq_half(1) {
                    return int(30);
                }
                if half == 180 && eq_half(-1) {
                    return int(-30);
                }
            }
            InvCirc::Acos => {
                if eq(1) {
                    return int(0);
                }
                if eq(-1) {
                    return int(half);
                }
                if eq(0) {
                    return int(q);
                }
                if half == 180 && eq_half(1) {
                    return int(60);
                }
                if half == 180 && eq_half(-1) {
                    return int(120);
                }
            }
            InvCirc::Atan => {
                if eq(0) {
                    return int(0);
                }
                if eq(1) {
                    return int(half / 4);
                }
                if eq(-1) {
                    return int(-half / 4);
                }
            }
        }
        None
    }

    /// |X| — preserves the integer/real kind.
    fn abs_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let v = match self.pop_x() {
            Value::Int(x) => Value::Int(self.canon_flagged(x.abs())),
            Value::Real(f) => Value::Real(f.abs()),
        };
        self.stack.push(v);
        Ok(())
    }

    /// Y ^ X. **Integer base and non-negative integer exponent stay exact**
    /// (GMP `pow` — no rounding); anything else is MPFR real. A negative
    /// integer exponent promotes (the result is fractional). Computed from
    /// peeks so an oversized power errors without consuming operands.
    fn pow_op(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let len = self.stack.len();
        let exact = match (&self.stack[len - 2], &self.stack[len - 1]) {
            (Value::Int(base), Value::Int(e)) if !e.is_negative() => {
                let one = Integer::from_i64(1);
                if base.is_zero() || *base == one || *base == -one.clone() {
                    // 0 / ±1 bases are exact for ANY exponent size.
                    Some(Ok(if base.is_zero() {
                        if e.is_zero() {
                            one // 0^0 = 1 (mpz_pow_ui convention)
                        } else {
                            Integer::new()
                        }
                    } else if base.is_negative() && Self::math_bit(e, 0) {
                        -one
                    } else {
                        one
                    }))
                } else {
                    match e.to_u32() {
                        Some(e) if base.bit_len() as u64 * e as u64 <= MAX_POW_BITS => {
                            Some(Ok(base.pow_exact(e)))
                        }
                        _ => Some(Err(CalcError::TypeError("power result too large"))),
                    }
                }
            }
            _ => None,
        };
        match exact {
            Some(Ok(v)) => {
                let _ = self.pop_x();
                let _ = self.stack.pop();
                let v = self.canon_flagged(v);
                self.stack.push(Value::Int(v));
                Ok(())
            }
            Some(Err(e)) => Err(e),
            None => {
                let x = self.pop_x().to_real(self.prec);
                let y = self.stack.pop().expect("validated").to_real(self.prec);
                self.stack.push(Value::Real(y.pow(x)));
                Ok(())
            }
        }
    }

    /// √X — 16C integer model: an integer gets the exact **integer** square
    /// root (⌊√x⌋) with the carry flag set when inexact; use `float` (or enter
    /// with a decimal point) for the real root. Reals stay MPFR.
    fn sqrt_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        if !matches!(self.stack.last(), Some(Value::Int(_))) {
            return self.unary_real(|x| x.sqrt());
        }
        if matches!(self.stack.last(), Some(Value::Int(x)) if x.is_negative()) {
            return Err(CalcError::TypeError("sqrt of a negative integer (float it for nan)"));
        }
        let Value::Int(x) = self.pop_x() else { unreachable!() };
        self.carry = !x.is_perfect_square(); // 16C: C = the root was inexact
        self.stack.push(Value::Int(x.isqrt()));
        Ok(())
    }

    /// x² — exact for integers, real otherwise.
    fn sq(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let v = match self.pop_x() {
            Value::Int(x) => {
                let v = self.canon_flagged(x.clone() * x);
                Value::Int(v)
            }
            Value::Real(f) => {
                let g = f.clone();
                Value::Real(f * g)
            }
        };
        self.stack.push(v);
        Ok(())
    }

    /// 10^X — exact for non-negative integer X, real otherwise.
    fn exp10_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        if let Value::Int(n) = &self.stack[self.stack.len() - 1] {
            if !n.is_negative() {
                let n = n.to_u32().ok_or(CalcError::TypeError("power result too large"))?;
                if n as u64 * 4 > MAX_POW_BITS {
                    return Err(CalcError::TypeError("power result too large"));
                }
                let _ = self.pop_x();
                let v = self.canon_flagged(Integer::from_i64(10).pow_exact(n));
                self.stack.push(Value::Int(v));
                return Ok(());
            }
        }
        self.unary_real(|x| x.exp10())
    }

    /// Y mod X (integers only; truncating remainder).
    fn mod_op(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let d = self.peek_int(0, "mod needs integers")?;
        if d.is_zero() {
            return Err(CalcError::DivZero);
        }
        self.peek_int(1, "mod needs integers")?;
        let Value::Int(b) = self.pop_x() else { unreachable!() };
        let Value::Int(a) = self.stack.pop().expect("validated") else { unreachable!() };
        self.stack.push(Value::Int(self.canon_silent(a % b)));
        Ok(())
    }

    /// % — X becomes X percent of Y; Y is preserved (HP behaviour).
    fn pct(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let x = self.pop_x().to_real(self.prec);
        let y = self.stack.last().expect("validated").to_real(self.prec);
        let hundred = Float::from_i64(self.prec, 100);
        self.stack.push(Value::Real(y * x / hundred));
        Ok(())
    }

    /// FLOAT — convert an integer X to a real (no-op on reals).
    fn to_float(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        if matches!(self.stack.last(), Some(Value::Int(_))) {
            let Value::Int(x) = self.pop_x() else { unreachable!() };
            let f = Float::from_integer(self.prec, &x);
            self.stack.push(Value::Real(f));
        }
        Ok(())
    }

    /// round/trunc/floor/ceil — convert a real X to an integer (no-op on ints).
    fn real_to_int(&mut self, conv: impl FnOnce(&Float) -> Integer) -> Result<(), CalcError> {
        self.need(1)?;
        match self.stack.last().expect("validated") {
            Value::Int(_) => Ok(()),
            Value::Real(f) => {
                if f.is_nan() || f.is_inf() {
                    return Err(CalcError::TypeError("cannot convert nan/inf to an integer"));
                }
                let Value::Real(f) = self.pop_x() else { unreachable!() };
                let v = self.canon_silent(conv(&f));
                self.stack.push(Value::Int(v));
                Ok(())
            }
        }
    }

    /// FRAC — fractional part; 0 for integers (kind-preserving).
    fn frac(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let v = match self.pop_x() {
            Value::Int(_) => Value::Int(Integer::new()),
            Value::Real(f) => Value::Real(f.frac()),
        };
        self.stack.push(v);
        Ok(())
    }

    /// WSIZE (HP-16C): pop X as the word size in bits; 0 = unbounded (GMP).
    fn wsize(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let n = self.peek_u32("word size out of range")?;
        if n > MAX_WORD_BITS {
            return Err(CalcError::TypeError("word size out of range"));
        }
        let _ = self.pop_x();
        self.set_word_bits(if n == 0 { None } else { Some(n) });
        Ok(())
    }

    /// Pop X as the MPFR working precision in bits.
    fn prec_cmd(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let n = self.peek_u32("precision out of range")?;
        let _ = self.pop_x();
        self.set_prec(n);
        Ok(())
    }

    /// FIX/SCI/ENG n — pop X as the digit count.
    fn fmt_cmd(&mut self, shape: FloatFmt) -> Result<(), CalcError> {
        self.need(1)?;
        let d = self.peek_u32("format digits out of range")?;
        if d > MAX_FMT_DIGITS {
            return Err(CalcError::TypeError("format digits out of range"));
        }
        let _ = self.pop_x();
        self.float_fmt = match shape {
            FloatFmt::Fix(_) => FloatFmt::Fix(d as u8),
            FloatFmt::Sci(_) => FloatFmt::Sci(d as u8),
            FloatFmt::Eng(_) => FloatFmt::Eng(d as u8),
            FloatFmt::Auto => FloatFmt::Auto,
        };
        Ok(())
    }

    /// STO i — copy X into register i (X stays, LASTx untouched).
    fn sto(&mut self, i: usize) -> Result<(), CalcError> {
        self.need(1)?;
        self.regs[i] = Some(self.stack.last().expect("validated").clone());
        Ok(())
    }

    /// RCL i — push register i.
    fn rcl(&mut self, i: usize) -> Result<(), CalcError> {
        match &self.regs[i] {
            Some(v) => {
                self.stack.push(v.clone());
                Ok(())
            }
            None => Err(CalcError::TypeError("empty register")),
        }
    }

    /// Push e = exp(1) at the working precision.
    fn push_e(&mut self) {
        let one = Float::from_i64(self.prec, 1);
        self.stack.push(Value::Real(one.exp()));
    }

    /// Recall LASTx (the X consumed by the previous operation).
    fn lastx(&mut self) -> Result<(), CalcError> {
        match &self.last_x {
            Some(v) => {
                self.stack.push(v.clone());
                Ok(())
            }
            None => Err(CalcError::Empty),
        }
    }

    /// Copy Y above X (… Y X → … Y X Y).
    fn over(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let n = self.stack.len();
        self.stack.push(self.stack[n - 2].clone());
        Ok(())
    }

    /// Roll the stack down: X drops to the bottom.
    fn roll_down(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let x = self.stack.pop().expect("validated");
        self.stack.insert(0, x);
        Ok(())
    }

    /// Roll the stack up: the bottom element rises to X.
    fn roll_up(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let b = self.stack.remove(0);
        self.stack.push(b);
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BitOp {
    Set,
    Clear,
    Test,
}

/// Leaving word mode: a pattern-domain value back to signed (helper for
/// [`Calc::renormalize`]).
fn decode_bits_back(bits: Integer, mode: SignMode, n: u32) -> Integer {
    if bits.is_negative() {
        // encode of a canonical value is never negative; belt and braces
        bits
    } else {
        decode_bits(bits, mode, n)
    }
}

/// `sto<h>` / `rcl<h>` → register index.
fn reg_index(t: &str, prefix: &str) -> Option<usize> {
    let rest = t.strip_prefix(prefix)?;
    if rest.len() != 1 {
        return None;
    }
    rest.chars().next()?.to_digit(16).map(|d| d as usize)
}

fn is_command(t: &str) -> bool {
    matches!(
        t,
        "+" | "-" | "*" | "/" | "chs" | "swap" | "drop" | "dup" | "sqrt" | "sin"
            | "cos" | "tan" | "asin" | "acos" | "atan" | "sinh" | "cosh" | "tanh"
            | "ln" | "log" | "exp" | "exp10" | "inv" | "sq" | "abs" | "pow"
            | "mod" | "pct" | "e" | "pi" | "and" | "or" | "xor" | "not"
            | "sl" | "sr" | "asr" | "rl" | "rr" | "rlc" | "rrc"
            | "shl" | "shr" | "sln" | "srn" | "asrn" | "rln" | "rrn"
            | "rlcn" | "rrcn" | "lj"
            | "bset" | "bclr" | "btest" | "maskl" | "maskr" | "popcnt"
            | "fact" | "!" | "float" | "round" | "trunc" | "floor" | "ceil" | "frac"
            | "hex" | "dec" | "oct" | "bin" | "wsize" | "prec"
            | "unsgn" | "1s" | "2s" | "signmode" | "rad" | "deg" | "grad" | "anglemode" | "lz"
            | "fix" | "sci" | "eng" | "std" | "clear"
            | "lastx" | "enter" | "over" | "rolldn" | "roll" | "rollup"
    )
}
