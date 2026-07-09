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

use gmp_mpfr_nostd::{Complex, Float, Integer};

use crate::matrix::Matrix;
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

/// Stack discipline. `Unbounded` (default) is our modern model: the stack
/// grows without limit and entry always pushes. `Classic4` is the faithful
/// HP four-level stack: fixed X/Y/Z/T, **T replicates** into Z on every
/// consuming operation (the "constant in T" idiom), **stack lift** discipline
/// (entry after ENTER/CLx overwrites X instead of pushing), and CLx zeroes X
/// in place. Switching to `Classic4` keeps the top four values (zero-padded
/// beneath); switching back is lossless.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StackModel {
    Unbounded,
    Classic4,
}

/// The number-type mode — the 16C's mode-is-the-type model, as a setting.
/// `Flex` (default): exact integers with SAFE division — an exact quotient
/// stays an exact integer, an inexact one promotes to a real, never silent
/// truncation. `Int`: proper 16C integer mode — division truncates and sets
/// Carry on an inexact quotient (unbounded included). `Real`: the
/// float-machine model (SCI/FIN default) — decimal entry parses as reals,
/// division is real.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NumMode {
    Flex,
    Int,
    Real,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum StatFn {
    Mean,
    Sdev,
    Lr,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dep {
    Sl,
    Soyd,
    Db,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalcError {
    /// Token was neither a number (in the active radix) nor a known command.
    Parse(String),
    /// Stack underflow.
    Empty,
    /// Division by zero.
    DivZero,
    /// Any other operation error: an HP-style glass code (`Error N`, see
    /// [`CalcError::code`]) plus the full text for host frontends / the aux
    /// display.
    Op(u8, &'static str),
}

impl CalcError {
    /// HP-16C-style error class for the 7-seg glass (`Error N`):
    /// 0 math domain · 1 register/flag · 2 bits/shift/word · 3 mode ranges ·
    /// 4 result too large · 5 no solution · 6 stack/entry/usage · 7 dates ·
    /// 8 statistics · 9 reserved (crash recovery).
    pub fn code(&self) -> u8 {
        match self {
            CalcError::DivZero => 0,
            CalcError::Parse(_) | CalcError::Empty => 6,
            CalcError::Op(c, _) => *c,
        }
    }

    /// Full error text — host frontends and the aux (OLED) display.
    pub fn text(&self) -> &str {
        match self {
            CalcError::Parse(_) => "parse error",
            CalcError::Empty => "stack empty",
            CalcError::DivZero => "divide by zero",
            CalcError::Op(_, m) => m,
        }
    }
}

// Class constructors — every operation error picks its glass code at the site.
const fn e_domain(m: &'static str) -> CalcError { CalcError::Op(0, m) }
const fn e_reg(m: &'static str) -> CalcError { CalcError::Op(1, m) }
const fn e_bits(m: &'static str) -> CalcError { CalcError::Op(2, m) }
const fn e_mode(m: &'static str) -> CalcError { CalcError::Op(3, m) }
const fn e_big(m: &'static str) -> CalcError { CalcError::Op(4, m) }
const fn e_nosol(m: &'static str) -> CalcError { CalcError::Op(5, m) }
const fn e_use(m: &'static str) -> CalcError { CalcError::Op(6, m) }
const fn e_date(m: &'static str) -> CalcError { CalcError::Op(7, m) }
const fn e_stats(m: &'static str) -> CalcError { CalcError::Op(8, m) }

/// STO/RCL register file size (one register per hex digit key, 0–F).
pub const REGISTERS: usize = 16;

/// Word sizes beyond this are almost certainly a slip; refuse them.
const MAX_WORD_BITS: u32 = 16384;
const MAX_FMT_DIGITS: u32 = 32;

/// Working precision is capped like the word size — an absurd `prec` is a
/// slip that would exhaust the device heap on the next real operation.
const MAX_PREC_BITS: u32 = 16384;

/// Exact-power results are capped at ~1 Mbit (≈300k decimal digits) so a slip
/// like `2 1e9 pow` errors instead of exhausting memory. Generous for real use.
const MAX_POW_BITS: u64 = 1 << 20;

/// Statistics accumulation registers (HP-15C style): Σ+ pairs `(x, y)` from
/// X/Y feed n, Σx, Σx², Σy, Σy², Σxy at the working precision.
struct Stats {
    n: u64,
    sx: Float,
    sxx: Float,
    sy: Float,
    syy: Float,
    sxy: Float,
}

impl Stats {
    fn new(prec: u32) -> Self {
        let z = || Float::from_i64(prec, 0);
        Stats { n: 0, sx: z(), sxx: z(), sy: z(), syy: z(), sxy: z() }
    }
}

/// TVM registers (HP-12C model): `i` is the periodic rate in **percent**;
/// `begin` selects annuity-due (BEG) vs ordinary (END) payments. The sign
/// convention is cash-flow: money paid out is negative.
struct Tvm {
    n: Float,
    i: Float,
    pv: Float,
    pmt: Float,
    fv: Float,
    begin: bool,
}

impl Tvm {
    fn new(prec: u32) -> Self {
        let z = || Float::from_i64(prec, 0);
        Tvm { n: z(), i: z(), pv: z(), pmt: z(), fv: z(), begin: false }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TvmReg {
    N,
    I,
    Pv,
    Pmt,
    Fv,
}

pub struct Calc {
    stack: Vec<Value>,
    stack_model: StackModel,
    /// Classic4 stack-lift flag: entry pushes when set, overwrites X when
    /// clear (cleared by ENTER / CLx / CLEAR, set by everything else).
    lift: bool,
    prec: u32,
    radix: Radix,
    word_bits: Option<u32>,
    sign_mode: SignMode,
    float_fmt: FloatFmt,
    angle_mode: AngleMode,
    leading_zeros: bool,
    radix_suffix: bool,
    /// Number-type mode (see [`NumMode`]).
    num_mode: NumMode,
    /// Complex display/interpret mode: `false` = rectangular (`a+bi`), `true` =
    /// polar (`r∠θ`, θ in `angle_mode`). HP-42S RECT/POLAR.
    polar: bool,
    /// HP-42S CPXRES/REALRES: when set (default), a real op with a complex
    /// result (e.g. `√-4`) returns the complex value; when clear it errors.
    cpxres: bool,
    user_flags: [bool; 3],
    carry: bool,
    overflow: bool,
    last_x: Option<Value>,
    regs: Vec<Option<Value>>,
    stats: Option<Stats>,
    tvm: Option<Tvm>,
    /// Grouped cash flows for NPV/IRR: (amount, repeat count); index 0 is
    /// the time-0 flow (CF₀).
    cfs: Vec<(Float, u32)>,
    /// xorshift64 PRNG state for `ran` (deterministic; `seed` re-seeds —
    /// firmware seeds from hardware entropy at boot). Never zero.
    rng: u64,
    /// The user function for SOLVE: an RPN token list with `x` as the variable
    /// (set via `fn:tok,tok,…`). Evaluated on a scratch engine at each step.
    func: Option<Vec<String>>,
    /// Row-major fill index into the matrix on top of the stack while entering
    /// one from the keyboard (`mnew` opens it, `mset` advances; `None` when not
    /// filling). Lets the 50-key matrix build a matrix without a `[…]` literal.
    mat_cursor: Option<usize>,
}

// ---- word-size helpers (shared with the formatter) --------------------------

fn one() -> Integer {
    Integer::from_i64(1)
}

fn zero_value() -> Value {
    Value::Int(Integer::new())
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
            stack_model: StackModel::Unbounded,
            lift: true,
            prec: prec.max(2),
            radix: Radix::Dec,
            word_bits: None,
            sign_mode: SignMode::Twos,
            float_fmt: FloatFmt::Auto,
            angle_mode: AngleMode::Rad,
            leading_zeros: false,
            radix_suffix: true,
            num_mode: NumMode::Flex,
            polar: false,
            cpxres: true,
            user_flags: [false; 3],
            carry: false,
            overflow: false,
            last_x: None,
            regs: vec![None; REGISTERS],
            stats: None,
            tvm: None,
            cfs: Vec::new(),
            rng: 0x9E37_79B9_7F4A_7C15,
            func: None,
            mat_cursor: None,
        }
    }

    // ---- state ------------------------------------------------------------
    pub fn prec(&self) -> u32 {
        self.prec
    }
    /// Set the working precision (clamped to 2..=16384 bits). Live Σ / TVM /
    /// cash-flow registers are re-rounded so later accumulation happens at
    /// the NEW precision (Float ops take the left operand's precision — the
    /// old behavior silently froze them at creation-time precision).
    pub fn set_prec(&mut self, prec: u32) {
        let p = prec.clamp(2, MAX_PREC_BITS);
        self.prec = p;
        let re = |f: &mut Float| *f = Float::with_prec(p, f);
        if let Some(st) = &mut self.stats {
            re(&mut st.sx);
            re(&mut st.sxx);
            re(&mut st.sy);
            re(&mut st.syy);
            re(&mut st.sxy);
        }
        if let Some(t) = &mut self.tvm {
            re(&mut t.n);
            re(&mut t.i);
            re(&mut t.pv);
            re(&mut t.pmt);
            re(&mut t.fv);
        }
        for (a, _) in &mut self.cfs {
            re(a);
        }
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
    /// Complex display mode: `true` = polar (`r∠θ`), `false` = rectangular.
    pub fn polar(&self) -> bool {
        self.polar
    }
    /// HP-42S CPXRES (`true`, default) vs REALRES (`false`).
    pub fn cpxres(&self) -> bool {
        self.cpxres
    }
    /// Enable complex results (CPXRES) or restrict to reals (REALRES). Set by the
    /// personality defaults + the SETUP menu; off on the 16C (period-correct).
    pub fn set_cpxres(&mut self, on: bool) {
        self.cpxres = on;
    }
    /// 16C flag 3: pad hex/oct/bin display with leading zeros to the word width.
    pub fn leading_zeros(&self) -> bool {
        self.leading_zeros
    }
    pub fn set_leading_zeros(&mut self, on: bool) {
        self.leading_zeros = on;
    }
    /// User flag 0–2 (16C flags; 3/4/5 are lz/carry/overflow — see `sf`/`cf`).
    pub fn user_flag(&self, i: usize) -> bool {
        self.user_flags.get(i).copied().unwrap_or(false)
    }
    /// Display tunable: 16C-style base letter (`h o b`) after a non-decimal
    /// integer X on the glass. On by default; `suffix` toggles.
    pub fn radix_suffix(&self) -> bool {
        self.radix_suffix
    }
    pub fn set_radix_suffix(&mut self, on: bool) {
        self.radix_suffix = on;
    }
    pub fn num_mode(&self) -> NumMode {
        self.num_mode
    }
    pub fn set_num_mode(&mut self, m: NumMode) {
        self.num_mode = m;
    }
    /// Float-machine entry active (compat shim over [`NumMode`]).
    pub fn real_entry(&self) -> bool {
        self.num_mode == NumMode::Real
    }
    pub fn set_real_entry(&mut self, on: bool) {
        self.num_mode = if on { NumMode::Real } else { NumMode::Flex };
    }
    pub fn stack_model(&self) -> StackModel {
        self.stack_model
    }
    /// Switch the stack discipline. → Classic4 keeps the **top four** values
    /// (padding with zeros beneath); → Unbounded is lossless.
    pub fn set_stack_model(&mut self, m: StackModel) {
        if m == self.stack_model {
            return;
        }
        self.stack_model = m;
        if m == StackModel::Classic4 {
            while self.stack.len() > 4 {
                self.stack.remove(0);
            }
            while self.stack.len() < 4 {
                self.stack.insert(0, zero_value());
            }
            self.lift = true;
        }
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
    /// empty. This is the **full working-precision** rendering (respects
    /// FIX/SCI/ENG but, in `Auto`, shows all `prec`-worth of digits). For what
    /// the physical display actually shows — clamped + rounded to the digit
    /// window — use [`Calc::show_fit`].
    pub fn display(&self) -> String {
        match self.stack.last() {
            Some(v) => crate::format::format(v, self),
            None => String::new(),
        }
    }

    /// The X readout **as the hardware display renders it**: the top of stack
    /// fitted to a `cells`-wide digit window (e.g. `seg7::DIGITS_PER_ROW`),
    /// applying the same precision limit + rounding the 7-seg glass uses (this
    /// is what `App::text_rows` shows). Empty string if the stack is empty.
    pub fn show_fit(&self, cells: usize) -> String {
        match self.stack.last() {
            Some(v) => crate::format::format_fit(v, self, cells),
            None => String::new(),
        }
    }

    /// X formatted in another radix without switching modes — the 16C f-SHOW
    /// view (word size / sign mode / lz still apply).
    pub fn show_in(&self, r: Radix) -> String {
        match self.stack.last() {
            Some(v) => crate::format::format_radix(v, self, r),
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
        // SOLVE function definition: fn:tok,tok,… — an RPN expression in `x`.
        if let Some(spec) = t.strip_prefix("fn:") {
            self.func = Some(spec.split(',').map(|s| s.trim().to_string()).collect());
            return Ok(());
        }
        // Matrix literal: [a,b;c,d] (rows by ';', elements by ',').
        if t.starts_with('[') {
            return match self.try_parse_matrix(t) {
                Some(m) => {
                    self.push_entry(Value::Matrix(m));
                    Ok(())
                }
                None => Err(CalcError::Parse(t.to_string())),
            };
        }
        let lower = t.to_ascii_lowercase();
        if let Some(i) = reg_index(&lower, "sto") {
            self.sto(i)?;
            if self.stack_model == StackModel::Classic4 {
                self.lift = true; // HP: STO re-enables stack lift
            }
            return Ok(());
        }
        if let Some(i) = reg_index(&lower, "rcl") {
            self.rcl(i)?;
            self.post_op_classic4("rcl");
            return Ok(());
        }
        // Commands match first (so `and`/`dec` are operators even in hex);
        // the Parse fallback arm means "not a command — try it as a number".
        match self.command(&lower) {
            Err(CalcError::Parse(_)) => {}
            r => {
                r?;
                self.post_op_classic4(&lower);
                return Ok(());
            }
        }
        self.push_number(t)
    }

    /// Parse a `[a,b;c,d]` matrix literal (rows by `;`, elements by `,`); each
    /// element is an MPFR real. `None` on any malformed element or ragged rows.
    fn try_parse_matrix(&self, t: &str) -> Option<Matrix> {
        let inner = t.strip_prefix('[')?.strip_suffix(']')?.trim();
        if inner.is_empty() {
            return None;
        }
        let mut rows: Vec<Vec<Float>> = Vec::new();
        for row in inner.split(';') {
            let mut r: Vec<Float> = Vec::new();
            for elem in row.split(',') {
                r.push(Float::from_str(self.prec, elem.trim())?);
            }
            rows.push(r);
        }
        Matrix::from_rows(self.prec, &rows)
    }

    /// Push a NUMBER, bypassing command matching entirely — the door the
    /// keypad's digit-entry buffer uses, so hex entries that spell command
    /// names (`E`, `DEC`, `CF`…) are never stolen by the command table.
    pub fn push_number(&mut self, t: &str) -> Result<(), CalcError> {
        match self.try_parse_number(t.trim()) {
            Some(v) => {
                self.push_entry(v);
                Ok(())
            }
            None => Err(CalcError::Parse(t.trim().to_string())),
        }
    }

    /// Push an entered number, honoring the Classic4 stack-lift discipline.
    fn push_entry(&mut self, v: Value) {
        if self.stack_model == StackModel::Classic4 {
            if self.lift {
                self.stack.push(v);
                self.replicate4();
            } else {
                let n = self.stack.len();
                self.stack[n - 1] = v; // ENTER/CLx disabled lift: overwrite X
            }
            self.lift = true;
        } else {
            self.stack.push(v);
        }
    }

    /// After a successful command in Classic4: restore the fixed four levels
    /// (dropping T on growth, **replicating T** on consumption — the HP
    /// behavior that keeps a constant in T) and update the lift flag. CLx
    /// (`drop`) and CLEAR get their HP shapes instead.
    fn post_op_classic4(&mut self, cmd: &str) {
        if self.stack_model != StackModel::Classic4 {
            return;
        }
        match cmd {
            "clear" => {
                for _ in 0..4 {
                    self.stack.push(zero_value());
                }
            }
            "drop" => self.stack.push(zero_value()), // CLx: X := 0, Y/Z/T kept
            _ => self.replicate4(),
        }
        self.lift = !matches!(cmd, "enter" | "drop" | "clear" | "s+" | "s-");
    }

    /// Trim/refill to exactly four levels: excess falls off the top (T lost —
    /// a lift), shortfall re-fills from the bottom (T replicates).
    fn replicate4(&mut self) {
        while self.stack.len() > 4 {
            self.stack.remove(0);
        }
        while self.stack.len() < 4 {
            let b = self.stack.first().cloned().unwrap_or_else(zero_value);
            self.stack.insert(0, b);
        }
    }

    fn try_parse_number(&self, t: &str) -> Option<Value> {
        if self.radix == Radix::Dec
            && (self.num_mode == NumMode::Real
                || t.contains('.')
                || t.contains('e')
                || t.contains('E'))
        {
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
            // Complex (HP-42S): merge/split, display mode, and CPXRES/REALRES.
            "complex" => self.complex_op(),
            "rect" => {
                self.polar = false;
                Ok(())
            }
            "polar" => {
                self.polar = true;
                Ok(())
            }
            "cplxdisp" => {
                self.polar = !self.polar; // toggle rect <-> polar (for a key)
                Ok(())
            }
            "conj" => self.conj_op(),
            "arg" => self.arg_op(),
            "re" => self.re_op(),
            "im" => self.im_op(),
            "reim" => self.reim_op(),
            "topolar" => self.to_polar_op(),
            "torect" => self.to_rect_op(),
            "det" => self.det_op(),
            "transpose" => self.transpose_op(),
            "minv" => self.minv_op(),
            "matsolve" => self.matsolve_op(),
            "mnew" => self.mnew_op(),
            "mset" => self.mset_op(),
            "solve" => self.solve_op(),
            "integ" => self.integ_op(),
            "cpxres" => {
                self.cpxres = true;
                Ok(())
            }
            "realres" => {
                self.cpxres = false;
                Ok(())
            }
            "swap" => self.swap(),
            "drop" => self.pop_unchecked().map(|_| ()),
            "dup" => self.dup(),
            "sqrt" => self.sqrt_op(),
            "sin" => self.circ(Circ::Sin),
            "cos" => self.circ(Circ::Cos),
            "tan" => self.circ(Circ::Tan),
            "ln" => self.cunary(Float::is_negative, |z| z.ln(), |x| x.ln()),
            "exp" => self.cunary(|_| false, |z| z.exp(), |x| x.exp()),
            "inv" => self.cunary(|_| false, |z| z.recip(), |x| x.recip()),
            "sq" => self.sq(),
            "asin" => self.inv_circ(InvCirc::Asin),
            "acos" => self.inv_circ(InvCirc::Acos),
            "atan" => self.inv_circ(InvCirc::Atan),
            "sinh" => self.cunary(|_| false, |z| z.sinh(), |x| x.sinh()),
            "cosh" => self.cunary(|_| false, |z| z.cosh(), |x| x.cosh()),
            "tanh" => self.cunary(|_| false, |z| z.tanh(), |x| x.tanh()),
            "log" => self.cunary(Float::is_negative, |z| z.log10(), |x| x.log10()),
            "exp10" => self.exp10_op(),
            "abs" => self.abs_op(),
            "pow" => self.pow_op(),
            "mod" => self.mod_op(),
            "idiv" => self.idiv(),
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
            "dbl*" => self.dbl_mul(),
            "dbl/" => self.dbl_div(true),
            "dblr" => self.dbl_div(false),
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
            "suffix" => {
                self.radix_suffix = !self.radix_suffix;
                Ok(())
            }
            "realmode" | "floatentry" => {
                self.num_mode = NumMode::Real;
                Ok(())
            }
            "intmode" => {
                self.num_mode = NumMode::Int;
                Ok(())
            }
            "flexmode" | "intentry" => {
                self.num_mode = NumMode::Flex;
                Ok(())
            }
            "stack4" => {
                self.set_stack_model(StackModel::Classic4);
                Ok(())
            }
            "stackfree" => {
                self.set_stack_model(StackModel::Unbounded);
                Ok(())
            }
            "s+" => self.sigma(true),
            "s-" => self.sigma(false),
            "mean" => self.stat_pair(StatFn::Mean),
            "sdev" => self.stat_pair(StatFn::Sdev),
            "lr" => self.stat_pair(StatFn::Lr),
            "yhat" => self.yhat(),
            "corr" => self.corr(),
            "clstat" => {
                self.stats = None;
                Ok(())
            }
            "ncr" => self.comb_perm(false),
            "npr" => self.comb_perm(true),
            ">n" => self.tvm_store(TvmReg::N),
            ">i" => self.tvm_store(TvmReg::I),
            ">pv" => self.tvm_store(TvmReg::Pv),
            ">pmt" => self.tvm_store(TvmReg::Pmt),
            ">fv" => self.tvm_store(TvmReg::Fv),
            "n?" => self.tvm_solve(TvmReg::N),
            "i?" => self.tvm_solve(TvmReg::I),
            "pv?" => self.tvm_solve(TvmReg::Pv),
            "pmt?" => self.tvm_solve(TvmReg::Pmt),
            "fv?" => self.tvm_solve(TvmReg::Fv),
            "rcln" => self.tvm_recall(TvmReg::N),
            "rcli" => self.tvm_recall(TvmReg::I),
            "rclpv" => self.tvm_recall(TvmReg::Pv),
            "rclpmt" => self.tvm_recall(TvmReg::Pmt),
            "rclfv" => self.tvm_recall(TvmReg::Fv),
            "beg" => {
                self.tvm_mut().begin = true;
                Ok(())
            }
            "end" => {
                self.tvm_mut().begin = false;
                Ok(())
            }
            "clfin" => {
                self.tvm = None;
                Ok(())
            }
            "12/" => self.tvm_by12(false),
            "12*" => self.tvm_by12(true),
            "pctchg" => self.pct_of(true),
            "pctt" => self.pct_of(false),
            "wmean" => self.weighted_mean(),
            "cf0" => self.cash_flow(true),
            "cfj" => self.cash_flow(false),
            "nj" => self.cash_count(),
            "clcf" => {
                self.cfs.clear();
                Ok(())
            }
            "npv" => self.npv_cmd(),
            "irr" => self.irr_cmd(),
            "ddays" => self.ddays(),
            "dateadd" => self.date_add(),
            "dow" => self.day_of_week(),
            "depsl" => self.depreciation(Dep::Sl),
            "depsoyd" => self.depreciation(Dep::Soyd),
            "depdb" => self.depreciation(Dep::Db),
            "ran" => {
                let f = self.next_ran();
                self.stack.push(Value::Real(f));
                Ok(())
            }
            "seed" => self.seed_cmd(),
            "sf" => self.set_flag(true),
            "cf" => self.set_flag(false),
            "ftest" => self.flag_test(),
            "clreg" => {
                self.regs = vec![None; REGISTERS];
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

    /// Error (leaving the stack intact) if X is a matrix — for the scalar-only
    /// ops (abs, sq, frac, transcendentals, …) that would otherwise treat it as
    /// a bogus real.
    fn no_matrix(&self, what: &'static str) -> Result<(), CalcError> {
        if self.stack.last().map(Value::is_matrix).unwrap_or(false) {
            return Err(e_domain(what));
        }
        Ok(())
    }

    /// Operand at `depth` (0 = X) as an integer, or a TypeError.
    fn peek_int(&self, depth: usize, what: &'static str) -> Result<&Integer, CalcError> {
        match &self.stack[self.stack.len() - 1 - depth] {
            Value::Int(i) => Ok(i),
            Value::Real(_) | Value::Complex(_) | Value::Matrix(_) => Err(e_domain(what)),
        }
    }

    /// Operand at `depth` as an exact Integer — accepts genuine integers AND
    /// integral reals (a float-machine `5` is still a valid count/index).
    fn peek_integral(&self, depth: usize, what: &'static str) -> Result<Integer, CalcError> {
        match &self.stack[self.stack.len() - 1 - depth] {
            Value::Int(i) => Ok(i.clone()),
            Value::Real(f) => {
                if !f.is_nan() && !f.is_inf() && f.clone().frac().is_zero() {
                    Ok(f.round_to_int())
                } else {
                    Err(e_domain(what))
                }
            }
            Value::Complex(_) | Value::Matrix(_) => Err(e_domain(what)),
        }
    }

    /// X as a small non-negative count, validated in place (integral reals
    /// accepted — see [`Self::peek_integral`]).
    fn peek_u32(&self, what: &'static str) -> Result<u32, CalcError> {
        self.peek_integral(0, what)?.to_u32().ok_or(e_domain(what))
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
        if self.num_mode == NumMode::Real {
            self.num_mode = NumMode::Flex; // 16C: a base key exits FLOAT mode
        }
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

    /// Push an Integer result canonicalized into the current word mode —
    /// EVERY integer producer must go through this (or canon_flagged) so the
    /// word-mode canonical-range invariant holds (review finding: rcl/lastx/
    /// ddays/dow/sigma bypassed it, panicking lj/popcnt downstream).
    fn push_int_canon(&mut self, v: Integer) {
        let v = self.canon_silent(v);
        self.stack.push(Value::Int(v));
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
                    '/' => !(a.clone() % b.clone()).is_zero(), // 16C: C = inexact quotient
                    _ => false,
                };
            }

            // The number MODE decides unbounded division (16C: mode is the
            // type). Real → real quotient. Flex (default) → SAFE: an inexact
            // quotient promotes to a real, never silent truncation. Int →
            // proper 16C integer mode: truncate + Carry on an inexact
            // quotient. Word-size mode truncates regardless (its own 16C
            // semantics, annunciators lit).
            if op == '/' && self.word_bits.is_none() {
                match self.num_mode {
                    NumMode::Real => {
                        let q = Float::from_integer(self.prec, &a)
                            / Float::from_integer(self.prec, &b);
                        self.stack.push(Value::Real(q));
                        return Ok(());
                    }
                    NumMode::Int => {
                        self.carry = !(a.clone() % b.clone()).is_zero();
                    }
                    NumMode::Flex => {
                        if !(a.clone() % b.clone()).is_zero() {
                            let q = Float::from_integer(self.prec, &a)
                                / Float::from_integer(self.prec, &b);
                            self.stack.push(Value::Real(q));
                            return Ok(());
                        }
                    }
                }
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
        } else if self.stack[len - 1].is_matrix() || self.stack[len - 2].is_matrix() {
            return self.matrix_arith(op);
        } else if self.stack[len - 1].is_complex() || self.stack[len - 2].is_complex() {
            // Complex arithmetic (HP-42S: the result stays a single complex
            // object even when the imaginary part is zero).
            let b = self.pop_x().to_complex(self.prec);
            let a = self.stack.pop().expect("validated").to_complex(self.prec);
            let r = match op {
                '+' => a.add(&b),
                '-' => a.sub(&b),
                '*' => a.mul(&b),
                '/' => a.div(&b),
                _ => unreachable!(),
            };
            self.stack.push(Value::Complex(r));
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

    /// Matrix arithmetic (HP-15C, modernized onto the stack). Supports
    /// matrix±matrix (equal shape), matrix×matrix (conformable), scalar×matrix /
    /// matrix×scalar, and matrix÷scalar. Matrix÷matrix and scalar÷matrix are
    /// ambiguous — use `matsolve` / `minv`. Operands are restored on error.
    fn matrix_arith(&mut self, op: char) -> Result<(), CalcError> {
        let b = self.pop_x();
        let a = self.stack.pop().expect("validated");
        let r: Result<Value, CalcError> = match (&a, &b, op) {
            (Value::Matrix(m), Value::Matrix(n), '+') => m
                .add(n)
                .map(Value::Matrix)
                .ok_or(e_domain("matrix + needs equal shapes")),
            (Value::Matrix(m), Value::Matrix(n), '-') => m
                .sub(n)
                .map(Value::Matrix)
                .ok_or(e_domain("matrix - needs equal shapes")),
            (Value::Matrix(m), Value::Matrix(n), '*') => m
                .mul(n)
                .map(Value::Matrix)
                .ok_or(e_domain("matrix * shapes not conformable")),
            // matrix × scalar / scalar × matrix
            (Value::Matrix(m), _, '*') => Ok(Value::Matrix(m.scalar_mul(&b.to_real(self.prec)))),
            (_, Value::Matrix(n), '*') => Ok(Value::Matrix(n.scalar_mul(&a.to_real(self.prec)))),
            // matrix ÷ scalar
            (Value::Matrix(m), _, '/') => {
                let s = b.to_real(self.prec);
                if s.is_zero() {
                    Err(CalcError::DivZero)
                } else {
                    Ok(Value::Matrix(m.scalar_mul(&s.recip())))
                }
            }
            _ => Err(e_domain("matrix op unsupported (try matsolve / minv)")),
        };
        match r {
            Ok(v) => {
                self.stack.push(v);
                Ok(())
            }
            Err(e) => {
                self.stack.push(a);
                self.stack.push(b);
                Err(e)
            }
        }
    }

    /// Determinant of a square matrix X → a real.
    fn det_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let Some(Value::Matrix(m)) = self.stack.last() else {
            return Err(e_domain("det needs a matrix"));
        };
        let d = m.determinant().ok_or(e_domain("det needs a square matrix"))?;
        let _ = self.pop_x();
        self.stack.push(Value::Real(d));
        Ok(())
    }

    /// Transpose of matrix X.
    fn transpose_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let Some(Value::Matrix(m)) = self.stack.last() else {
            return Err(e_domain("transpose needs a matrix"));
        };
        let t = m.transpose();
        let _ = self.pop_x();
        self.stack.push(Value::Matrix(t));
        Ok(())
    }

    /// Inverse of a square, non-singular matrix X.
    fn minv_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let Some(Value::Matrix(m)) = self.stack.last() else {
            return Err(e_domain("minv needs a matrix"));
        };
        let inv = m.inverse().ok_or(e_domain("matrix is singular or not square"))?;
        let _ = self.pop_x();
        self.stack.push(Value::Matrix(inv));
        Ok(())
    }

    /// Solve A·Z = B for Z, with Y = A (coefficients) and X = B (right-hand
    /// side); both replaced by Z.
    fn matsolve_op(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let len = self.stack.len();
        let (Value::Matrix(a), Value::Matrix(b)) =
            (&self.stack[len - 2], &self.stack[len - 1])
        else {
            return Err(e_domain("matsolve needs two matrices (Y=A, X=B)"));
        };
        let z = a
            .solve(b)
            .ok_or(e_domain("matsolve: A singular or shapes mismatch"))?;
        let _ = self.pop_x();
        let _ = self.stack.pop();
        self.stack.push(Value::Matrix(z));
        Ok(())
    }

    /// Open keyboard matrix entry: Y = rows, X = cols → a zero matrix on the
    /// stack with the fill cursor at (0,0). Fill it with `mset`.
    fn mnew_op(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let cols = self.peek_integral(0, "matrix cols")?;
        let rows = self.peek_integral(1, "matrix rows")?;
        let (rows, cols) = match (rows.to_u32(), cols.to_u32()) {
            (Some(r), Some(c)) if r >= 1 && c >= 1 && r <= 16 && c <= 16 => (r as usize, c as usize),
            _ => return Err(e_domain("matrix dims must be 1..=16")),
        };
        let _ = self.pop_x();
        let _ = self.stack.pop();
        self.stack
            .push(Value::Matrix(Matrix::zeros(rows, cols, self.prec)));
        self.mat_cursor = Some(0);
        Ok(())
    }

    /// Store the entered value X into the next cell (row-major) of the matrix
    /// being filled (below it), advancing the cursor; closes entry at the last
    /// cell. Requires an open `mnew` and a matrix under the value.
    fn mset_op(&mut self) -> Result<(), CalcError> {
        let cursor = self
            .mat_cursor
            .ok_or(e_domain("not filling a matrix — use mnew first"))?;
        self.need(2)?;
        if !self.stack[self.stack.len() - 2].is_matrix() {
            return Err(e_domain("mset: the matrix must be under the value"));
        }
        let val = self.pop_x().to_real(self.prec);
        let len = self.stack.len();
        let Value::Matrix(m) = &mut self.stack[len - 1] else {
            unreachable!("checked above")
        };
        let cols = m.cols();
        let total = m.rows() * cols;
        m.set(cursor / cols, cursor % cols, val);
        self.mat_cursor = if cursor + 1 >= total { None } else { Some(cursor + 1) };
        Ok(())
    }

    /// A fresh scratch engine for evaluating a `fn:` expression (real/float
    /// world, inheriting the angle + complex modes). Built once per solve/integ
    /// and reused across every f(x) — spinning one up per call is far too slow.
    fn make_scratch(&self) -> Calc {
        let mut s = Calc::new(self.prec);
        s.angle_mode = self.angle_mode;
        s.cpxres = self.cpxres;
        s.num_mode = NumMode::Real;
        s
    }

    /// Evaluate the stored function at `x` on the `scratch` engine (cleared
    /// first) — so the full command set is available inside `f(x)`. Returns X as
    /// a real, or `None` if the expression errors or yields a non-real.
    fn eval_func(&self, scratch: &mut Calc, tokens: &[String], x: &Float) -> Option<Float> {
        scratch.stack.clear();
        for tok in tokens {
            if tok.eq_ignore_ascii_case("x") {
                scratch.push_entry(Value::Real(Float::with_prec(self.prec, x)));
            } else {
                scratch.input(tok).ok()?;
            }
        }
        let v = scratch.stack.last()?;
        if v.is_complex() || v.is_matrix() {
            return None;
        }
        Some(v.to_real(self.prec))
    }

    /// Secant root-finder for the stored function, from two initial guesses.
    /// Converges when a step rounds away at the working precision.
    fn find_root(
        &self,
        scratch: &mut Calc,
        tokens: &[String],
        mut a: Float,
        mut b: Float,
    ) -> Option<Float> {
        let mut fa = self.eval_func(scratch, tokens, &a)?;
        let mut fb = self.eval_func(scratch, tokens, &b)?;
        for _ in 0..200 {
            if fb.is_zero() {
                return Some(b);
            }
            let denom = fb.clone() - fa.clone();
            if denom.is_zero() {
                break; // flat secant — give up, return best
            }
            let step = fb.clone() * (b.clone() - a.clone()) / denom;
            a = b.clone();
            fa = fb.clone();
            b = b.clone() - step.clone();
            fb = self.eval_func(scratch, tokens, &b)?;
            if step.is_zero() {
                break; // converged at working precision
            }
        }
        Some(b)
    }

    /// HP-15C SOLVE: find a root of the stored `fn:` function between the two
    /// guesses in Y and X. Guesses restored if it can't be evaluated.
    fn solve_op(&mut self) -> Result<(), CalcError> {
        let tokens = self
            .func
            .clone()
            .ok_or(e_domain("no function — set one with fn:tok,tok,…"))?;
        self.need(2)?;
        let b = self.pop_x().to_real(self.prec);
        let a = self.stack.pop().expect("validated").to_real(self.prec);
        let mut scratch = self.make_scratch();
        match self.find_root(&mut scratch, &tokens, a.clone(), b.clone()) {
            Some(root) => {
                self.stack.push(Value::Real(root));
                Ok(())
            }
            None => {
                self.stack.push(Value::Real(a));
                self.stack.push(Value::Real(b));
                Err(e_domain("solve: could not evaluate f(x)"))
            }
        }
    }

    /// One tanh-sinh node contribution at parameter `t`: the transformed
    /// abscissa `x = mid + half·tanh((π/2)·sinh t)` mapped back into `[a,b]`,
    /// times the node weight `(π/2)·cosh t / cosh²((π/2)·sinh t)`, times `f(x)`.
    /// `wp` is the (guard-padded) working precision.
    fn ts_node(
        &self,
        scratch: &mut Calc,
        toks: &[String],
        t: &Float,
        pi_half: &Float,
        mid: &Float,
        half: &Float,
    ) -> Option<Float> {
        let s = pi_half.clone() * t.clone().sinh(); // (π/2)·sinh t
        let u = s.clone().tanh(); // abscissa in (−1,1)
        let ch = s.cosh();
        let w = pi_half.clone() * t.clone().cosh() / (ch.clone() * ch);
        let x = mid.clone() + half.clone() * u;
        Some(w * self.eval_func(scratch, toks, &x)?)
    }

    /// HP-15C ∫ by **tanh-sinh (double-exponential) quadrature** — the DE change
    /// of variable makes the integrand decay double-exponentially, so the
    /// trapezoidal rule converges to (near) full precision for smooth integrands
    /// in a handful of node-halving levels. Computed with guard bits.
    fn tanh_sinh(&self, scratch: &mut Calc, toks: &[String], a: &Float, b: &Float) -> Option<Float> {
        let wp = self.prec + 32; // guard bits
        let two = Float::from_i64(wp, 2);
        let pi_half = Float::pi(wp) / two.clone();
        let aw = Float::with_prec(wp, a);
        let bw = Float::with_prec(wp, b);
        let mid = (aw.clone() + bw.clone()) / two.clone();
        let half = (bw - aw) / two.clone();
        let dec = (self.prec as usize) * 3 / 10; // ~ decimal digits
        let eps = Float::from_str(wp, &alloc::format!("1e-{}", dec.saturating_sub(2).max(10)))?;
        let node_eps = Float::from_str(wp, &alloc::format!("1e-{}", dec + 6))?;
        let negligible = |v: &Float| (v.clone().abs() - node_eps.clone()).is_sign_negative();

        // Running Σ over all node contributions g(jh); start at t=0.
        let mut total = self.ts_node(scratch, toks, &Float::from_i64(wp, 0), &pi_half, &mid, &half)?;
        let mut h = Float::from_i64(wp, 1); // h₀ = 1
        let node = |slf: &Self, scr: &mut Calc, j: i64, h: &Float| {
            let t = Float::from_i64(wp, j) * h.clone();
            slf.ts_node(scr, toks, &t, &pi_half, &mid, &half)
        };
        // Level 0: symmetric integer-multiple nodes until they vanish.
        let mut k = 1;
        loop {
            let (cp, cm) = (node(self, scratch, k, &h)?, node(self, scratch, -k, &h)?);
            total = total + cp.clone() + cm.clone();
            if (negligible(&cp) && negligible(&cm)) || k > 2000 {
                break;
            }
            k += 1;
        }
        let mut est = half.clone() * h.clone() * total.clone();
        // Halve h; add the new odd-multiple nodes; stop when the estimate settles.
        for _ in 0..10 {
            h = h.clone() / two.clone();
            let mut j = 1;
            loop {
                let (cp, cm) = (node(self, scratch, j, &h)?, node(self, scratch, -j, &h)?);
                total = total + cp.clone() + cm.clone();
                if (negligible(&cp) && negligible(&cm)) || j > 8000 {
                    break;
                }
                j += 2;
            }
            let next = half.clone() * h.clone() * total.clone();
            if ((next.clone() - est.clone()).abs() - eps.clone()).is_sign_negative() {
                return Some(Float::with_prec(self.prec, &next));
            }
            est = next;
        }
        Some(Float::with_prec(self.prec, &est))
    }

    /// HP-15C ∫: definite integral of the stored `fn:` function over `[Y, X]`,
    /// by tanh-sinh quadrature. Bounds restored on error.
    fn integ_op(&mut self) -> Result<(), CalcError> {
        let toks = self
            .func
            .clone()
            .ok_or(e_domain("no function — set one with fn:tok,tok,…"))?;
        self.need(2)?;
        let p = self.prec;
        let b = self.pop_x().to_real(p);
        let a = self.stack.pop().expect("validated").to_real(p);
        let mut scratch = self.make_scratch();
        match self.tanh_sinh(&mut scratch, &toks, &a, &b) {
            Some(v) => {
                self.stack.push(Value::Real(v));
                Ok(())
            }
            None => {
                self.stack.push(Value::Real(a));
                self.stack.push(Value::Real(b));
                Err(e_domain("integ: could not evaluate f(x)"))
            }
        }
    }

    /// HP-42S COMPLEX: merge Y (real) + X (imaginary) into one complex; applied
    /// to a complex, split it back into Y = real part, X = imaginary part.
    fn complex_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        if self.stack.last().map(Value::is_complex).unwrap_or(false) {
            let Value::Complex(z) = self.pop_x() else { unreachable!() };
            self.stack.push(Value::Real(z.real(self.prec)));
            self.stack.push(Value::Real(z.imag(self.prec)));
        } else {
            self.need(2)?;
            let im = self.pop_x().to_real(self.prec);
            let re = self.stack.pop().expect("validated").to_real(self.prec);
            self.stack
                .push(Value::Complex(Complex::from_reals(self.prec, &re, &im)));
        }
        Ok(())
    }

    /// Radians → the current angle unit (for `arg` and complex θ readouts).
    fn angle_from_rad(&self, rad: Float) -> Float {
        if self.angle_mode == AngleMode::Rad {
            return rad;
        }
        let half = self.angle_mode.half_turn();
        rad * Float::from_i64(self.prec, half) / Float::pi(self.prec)
    }

    /// Complex conjugate (a+bi → a−bi); a real/integer is unchanged.
    fn conj_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let v = match self.pop_x() {
            Value::Complex(z) => Value::Complex(z.conj()),
            other => other,
        };
        self.stack.push(v);
        Ok(())
    }

    /// Argument / phase of X (atan2(im, re)) in the current angle unit. A real is
    /// 0 (≥0) or ±π (a half turn, <0).
    fn arg_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let rad = self.pop_x().to_complex(self.prec).arg(self.prec);
        self.stack.push(Value::Real(self.angle_from_rad(rad)));
        Ok(())
    }

    /// Real part of X (a complex → its real part; a real/integer → itself).
    fn re_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let v = match self.pop_x() {
            Value::Complex(z) => Value::Real(z.real(self.prec)),
            other => other,
        };
        self.stack.push(v);
        Ok(())
    }

    /// Imaginary part of X as a real (a real/integer → 0).
    fn im_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let v = match self.pop_x() {
            Value::Complex(z) => Value::Real(z.imag(self.prec)),
            _ => Value::Real(Float::from_i64(self.prec, 0)),
        };
        self.stack.push(v);
        Ok(())
    }

    /// HP-15C Re≷Im — swap the real and imaginary parts of X. A real `a` becomes
    /// the pure imaginary `a·i` (activates complex, like the 15C).
    fn reim_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let z = self.pop_x().to_complex(self.prec);
        let (re, im) = (z.real(self.prec), z.imag(self.prec));
        self.stack
            .push(Value::Complex(Complex::from_reals(self.prec, &im, &re)));
        Ok(())
    }

    /// The current angle unit → radians (inverse of [`Self::angle_from_rad`]).
    fn angle_to_rad(&self, a: Float) -> Float {
        if self.angle_mode == AngleMode::Rad {
            return a;
        }
        let half = self.angle_mode.half_turn();
        a * Float::pi(self.prec) / Float::from_i64(self.prec, half)
    }

    /// HP →P: rectangular (X = x, Y = y) → polar (X = r, Y = θ in the angle
    /// unit). Two *reals* — the classic coordinate conversion, distinct from a
    /// complex value; r = |x+yi|, θ = arg, computed via the complex helpers.
    fn to_polar_op(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let x = self.pop_x().to_real(self.prec);
        let y = self.stack.pop().expect("validated").to_real(self.prec);
        let z = Complex::from_reals(self.prec, &x, &y);
        let theta = self.angle_from_rad(z.arg(self.prec));
        self.stack.push(Value::Real(theta)); // Y = θ
        self.stack.push(Value::Real(z.abs(self.prec))); // X = r
        Ok(())
    }

    /// HP →R: polar (X = r, Y = θ) → rectangular (X = x, Y = y).
    fn to_rect_op(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let r = self.pop_x().to_real(self.prec);
        let theta = self.stack.pop().expect("validated").to_real(self.prec);
        let rad = self.angle_to_rad(theta);
        let x = r.clone() * rad.clone().cos();
        let y = r * rad.sin();
        self.stack.push(Value::Real(y)); // Y = y
        self.stack.push(Value::Real(x)); // X = x
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
            return Err(e_bits("rotate needs a word size (wsize)"));
        }
        if rotate {
            if let Some(n) = self.word_bits {
                if k > n {
                    return Err(e_bits("rotate count exceeds the word size"));
                }
            }
        }
        // Unbounded shifts/masks/bit-indexes allocate the result: same
        // result-size guard as pow (a slip like `1 4e9 shl` must not OOM).
        if self.word_bits.is_none() && k as u64 > MAX_POW_BITS {
            return Err(e_bits("shift count too large"));
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
            return Err(e_bits("rotate needs a word size (wsize)"));
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
            return Err(e_bits("lj needs a word size (wsize)"));
        };
        self.need(1)?;
        self.peek_int(0, "lj needs an integer")?;
        let Value::Int(x) = self.pop_x() else { unreachable!() };
        let bits = encode_bits(&x, self.sign_mode, n) & (pow2(n) - one());
        let shifts = if bits.is_zero() { 0 } else { n as usize - bits.bit_len() };
        let justified = decode_bits(bits << shifts as u32, self.sign_mode, n);
        self.stack.push(Value::Int(justified));
        self.stack.push(Value::Int(Integer::from_i64(shifts as i64)));
        Ok(())
    }

    /// DBL× — the full 2n-bit product of Y × X, split into words:
    /// high word → Y, low word → X (both decoded per the sign mode, so
    /// enc(Y)·2ⁿ + enc(X) reconstructs the product pattern). Never overflows
    /// by construction. Word mode only; **not in 1's complement** — the −0
    /// fold makes an all-ones half ambiguous, so the double word can't round-
    /// trip through canonical values (the 16C stores raw bits; we don't).
    fn dbl_mul(&mut self) -> Result<(), CalcError> {
        let Some(n) = self.word_bits else {
            return Err(e_bits("dbl* needs a word size (wsize)"));
        };
        if self.sign_mode == SignMode::Ones {
            return Err(e_mode("dbl ops need 2's complement or unsigned mode"));
        }
        self.need(2)?;
        self.peek_int(0, "dbl* needs integers")?;
        self.peek_int(1, "dbl* needs integers")?;
        let Value::Int(b) = self.pop_x() else { unreachable!() };
        let Value::Int(a) = self.stack.pop().expect("validated") else { unreachable!() };
        let pattern = euclid_mod(a * b, &pow2(2 * n));
        let high = pattern.clone() >> n;
        let low = pattern & (pow2(n) - one());
        self.stack.push(Value::Int(decode_bits(high, self.sign_mode, n)));
        self.stack.push(Value::Int(decode_bits(low, self.sign_mode, n)));
        self.carry = false;
        self.overflow = false;
        Ok(())
    }

    /// DBL÷ / DBLR — the double-word dividend Z (high) : Y (low) divided by X;
    /// quotient (or remainder) → X. A quotient that doesn't fit the word
    /// errors without consuming anything (16C raises Error 0). Word mode only.
    fn dbl_div(&mut self, want_quotient: bool) -> Result<(), CalcError> {
        let Some(n) = self.word_bits else {
            return Err(e_bits("dbl/ needs a word size (wsize)"));
        };
        if self.sign_mode == SignMode::Ones {
            return Err(e_mode("dbl ops need 2's complement or unsigned mode"));
        }
        self.need(3)?;
        let d = self.peek_int(0, "dbl/ needs integers")?;
        if d.is_zero() {
            return Err(CalcError::DivZero);
        }
        self.peek_int(1, "dbl/ needs integers")?;
        self.peek_int(2, "dbl/ needs integers")?;

        // Assemble the dividend from peeks — the fit check must not consume.
        let len = self.stack.len();
        let (Value::Int(x), Value::Int(y_low), Value::Int(z_high)) =
            (&self.stack[len - 1], &self.stack[len - 2], &self.stack[len - 3])
        else {
            unreachable!()
        };
        let pattern = (encode_bits(z_high, self.sign_mode, n) << n)
            | encode_bits(y_low, self.sign_mode, n);
        let dividend = decode_bits(pattern, self.sign_mode, 2 * n);
        let q = dividend.clone() / x.clone();
        let r = dividend % x.clone();
        let result = if want_quotient {
            let (w, ovf) = wrap(q.clone(), self.sign_mode, n);
            if ovf {
                let _ = w;
                return Err(e_bits("double quotient exceeds the word size"));
            }
            q
        } else {
            r // |r| < |x| always fits the word
        };

        let _ = self.pop_x();
        let _ = self.stack.pop();
        let _ = self.stack.pop();
        self.carry = false;
        self.overflow = false;
        self.stack.push(Value::Int(result));
        Ok(())
    }

    /// Mathematical bit `i` of a signed value (infinite two's complement),
    /// via Euclidean mod — correct for negatives where `>>` (tdiv) is not.
    fn math_bit(v: &Integer, i: u32) -> bool {
        euclid_mod(v.clone(), &pow2(i + 1)) >= pow2(i)
    }

    fn bit_index(&self, what: &'static str) -> Result<u32, CalcError> {
        let i = self.peek_u32(what)?;
        match self.word_bits {
            Some(n) if i >= n => {
                return Err(e_bits("bit index exceeds the word size"))
            }
            None if i as u64 > MAX_POW_BITS => {
                return Err(e_bits("bit index too large"))
            }
            _ => {}
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
                self.push_int_canon(Integer::from_i64(t as i64));
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
                    return Err(e_bits("mask width exceeds the word size"));
                }
                let ones = pow2(k) - one();
                let bits = if left { ones << (n - k) } else { ones };
                decode_bits(bits, self.sign_mode, n)
            }
            (None, false) => {
                if k as u64 > MAX_POW_BITS {
                    return Err(e_bits("mask width too large"));
                }
                pow2(k) - one()
            }
            (None, true) => return Err(e_bits("maskl needs a word size (wsize)")),
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
            Some(n) => (encode_bits(x, self.sign_mode, n) & (pow2(n) - one()))
                .popcount()
                .expect("masked pattern is non-negative"),
            None => x
                .popcount()
                .ok_or(e_bits("popcount of a negative needs a word size"))?,
        };
        let _ = self.pop_x();
        self.push_int_canon(Integer::from_i64(count as i64));
        Ok(())
    }

    fn fact(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let n = self.peek_u32("factorial needs a small non-negative integer")?;
        // n! has ~n*log2(n) bits — same result-size guard as pow
        let bits = n as u64 * (64 - (n as u64 | 1).leading_zeros() as u64);
        if bits > MAX_POW_BITS {
            return Err(e_big("result too large"));
        }
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
            Value::Complex(z) => Value::Complex(z.neg()),
            // CHS on a matrix negates every element.
            Value::Matrix(m) => Value::Matrix(m.scalar_mul(&Float::from_i64(self.prec, -1))),
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

    /// Complex-aware unary function (HP-42S): a **complex** X → `cx`; a **real**
    /// X for which `goes_complex` holds, under **CPXRES**, is promoted to complex
    /// → `cx`; otherwise the real function `re`. Integers take the real path
    /// (`goes_complex` is only consulted for reals) — matching "float it first"
    /// for e.g. `√` of a negative integer.
    fn cunary(
        &mut self,
        goes_complex: impl FnOnce(&Float) -> bool,
        cx: impl FnOnce(&Complex) -> Complex,
        re: impl FnOnce(Float) -> Float,
    ) -> Result<(), CalcError> {
        self.need(1)?;
        enum Path {
            Cx,
            Promote,
            Re,
        }
        // A real OR integer X (outside word/programmer mode, where complex has no
        // meaning) whose real function goes complex — promote under CPXRES. This
        // is what makes `√-1 = i` for the default (integer) -1, not just floats.
        let path = match self.stack.last().expect("validated") {
            Value::Complex(_) => Path::Cx,
            v if self.cpxres
                && self.word_bits.is_none()
                && !v.is_complex()
                && goes_complex(&v.to_real(self.prec)) =>
            {
                Path::Promote
            }
            _ => Path::Re,
        };
        match path {
            Path::Cx => {
                let Value::Complex(z) = self.pop_x() else { unreachable!() };
                self.stack.push(Value::Complex(cx(&z)));
            }
            Path::Promote => {
                let z = self.pop_x().to_complex(self.prec);
                self.stack.push(Value::Complex(cx(&z)));
            }
            Path::Re => {
                let x = self.pop_x().to_real(self.prec);
                self.stack.push(Value::Real(re(x)));
            }
        }
        Ok(())
    }

    // ---- circular trig (angle-mode aware) ------------------------------------
    /// Multiply a complex by a real scalar.
    fn cplx_scale(&self, z: &Complex, f: Float) -> Complex {
        let zero = Float::from_i64(self.prec, 0);
        z.mul(&Complex::from_reals(self.prec, &f, &zero))
    }
    /// A complex "angle" in the current unit → radians (complex trig input).
    fn cplx_angle_in(&self, z: &Complex) -> Complex {
        let p = self.prec;
        match self.angle_mode {
            AngleMode::Rad => z.clone(),
            AngleMode::Deg => self.cplx_scale(z, Float::pi(p) / Float::from_i64(p, 180)),
            AngleMode::Grad => self.cplx_scale(z, Float::pi(p) / Float::from_i64(p, 200)),
        }
    }
    /// A complex angle result in radians → the current unit (complex inv trig).
    fn cplx_angle_out(&self, z: &Complex) -> Complex {
        let p = self.prec;
        match self.angle_mode {
            AngleMode::Rad => z.clone(),
            AngleMode::Deg => self.cplx_scale(z, Float::from_i64(p, 180) / Float::pi(p)),
            AngleMode::Grad => self.cplx_scale(z, Float::from_i64(p, 200) / Float::pi(p)),
        }
    }

    fn circ(&mut self, kind: Circ) -> Result<(), CalcError> {
        self.need(1)?;
        // Complex argument → complex trig; the argument is interpreted in the
        // current angle unit (converted to radians for MPC), like real trig.
        if self.stack.last().map(Value::is_complex).unwrap_or(false) {
            let Value::Complex(z) = self.pop_x() else { unreachable!() };
            let z = self.cplx_angle_in(&z);
            let r = match kind {
                Circ::Sin => z.sin(),
                Circ::Cos => z.cos(),
                Circ::Tan => z.tan(),
            };
            self.stack.push(Value::Complex(r));
            return Ok(());
        }
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
        // Complex argument → complex inverse trig; the radian result is returned
        // in the current angle unit, like real inverse trig.
        if self.stack.last().map(Value::is_complex).unwrap_or(false) {
            let Value::Complex(z) = self.pop_x() else { unreachable!() };
            let r = match kind {
                InvCirc::Asin => z.asin(),
                InvCirc::Acos => z.acos(),
                InvCirc::Atan => z.atan(),
            };
            self.stack.push(Value::Complex(self.cplx_angle_out(&r)));
            return Ok(());
        }
        // CPXRES: asin/acos of a real or integer with |x| > 1 → complex
        // (outside word/programmer mode).
        let promote = self.cpxres
            && self.word_bits.is_none()
            && matches!(kind, InvCirc::Asin | InvCirc::Acos)
            && match self.stack.last() {
                Some(Value::Real(f)) => f.clone().abs().cmp_si(1) > 0,
                Some(Value::Int(i)) => Float::from_integer(self.prec, i).abs().cmp_si(1) > 0,
                _ => false,
            };
        if promote {
            let z = self.pop_x().to_complex(self.prec);
            let r = match kind {
                InvCirc::Asin => z.asin(),
                InvCirc::Acos => z.acos(),
                InvCirc::Atan => unreachable!(),
            };
            self.stack.push(Value::Complex(self.cplx_angle_out(&r)));
            return Ok(());
        }
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
        self.no_matrix("abs undefined for a matrix (use det/norm)")?;
        let v = match self.pop_x() {
            Value::Int(x) => Value::Int(self.canon_flagged(x.abs())),
            Value::Real(f) => Value::Real(f.abs()),
            // |z| — the magnitude, a real (HP-42S).
            Value::Complex(z) => Value::Real(z.abs(self.prec)),
            Value::Matrix(_) => unreachable!("guarded"),
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
        if self.stack[len - 1].is_matrix() || self.stack[len - 2].is_matrix() {
            return Err(e_domain("y^x undefined for a matrix"));
        }
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
                        _ => Some(Err(e_big("power result too large"))),
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
                let (yv, xv) = (&self.stack[len - 2], &self.stack[len - 1]);
                let neg_base = match yv {
                    Value::Int(i) => i.is_negative(),
                    Value::Real(f) => f.is_negative(),
                    Value::Complex(_) | Value::Matrix(_) => false,
                };
                let frac_exp = matches!(xv, Value::Real(f) if !f.clone().frac().is_zero());
                // Complex when either operand is complex, or (CPXRES) a negative
                // base raised to a non-integer exponent, e.g. (-8)^(1/3).
                let complex = yv.is_complex()
                    || xv.is_complex()
                    || (self.cpxres && neg_base && frac_exp);
                if complex {
                    let x = self.pop_x().to_complex(self.prec);
                    let y = self.stack.pop().expect("validated").to_complex(self.prec);
                    self.stack.push(Value::Complex(y.pow(&x)));
                } else {
                    let x = self.pop_x().to_real(self.prec);
                    let y = self.stack.pop().expect("validated").to_real(self.prec);
                    self.stack.push(Value::Real(y.pow(x)));
                }
                Ok(())
            }
        }
    }

    /// √X — 16C integer model: an integer gets the exact **integer** square
    /// root (⌊√x⌋) with the carry flag set when inexact; use `float` (or enter
    /// with a decimal point) for the real root. Reals stay MPFR.
    fn sqrt_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let neg_int = matches!(self.stack.last(), Some(Value::Int(x)) if x.is_negative());
        // Real/Complex, or a negative integer that CPXRES will promote (√-1 = i):
        // let cunary handle the complex path. A non-negative integer keeps the
        // exact isqrt below.
        if !matches!(self.stack.last(), Some(Value::Int(_)))
            || (neg_int && self.cpxres && self.word_bits.is_none())
        {
            return self.cunary(Float::is_negative, |z| z.sqrt(), |x| x.sqrt());
        }
        if neg_int {
            return Err(e_domain("sqrt of a negative integer (float it for nan)"));
        }
        let Value::Int(x) = self.pop_x() else { unreachable!() };
        self.carry = !x.is_perfect_square(); // 16C: C = the root was inexact
        self.stack.push(Value::Int(x.isqrt()));
        Ok(())
    }

    /// x² — exact for integers, real otherwise.
    fn sq(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        // A square matrix squares to M·M.
        if let Some(Value::Matrix(m)) = self.stack.last() {
            let p = m.mul(m).ok_or(e_domain("x^2 needs a square matrix"))?;
            let _ = self.pop_x();
            self.stack.push(Value::Matrix(p));
            return Ok(());
        }
        let v = match self.pop_x() {
            Value::Int(x) => {
                let v = self.canon_flagged(x.clone() * x);
                Value::Int(v)
            }
            Value::Real(f) => {
                let g = f.clone();
                Value::Real(f * g)
            }
            Value::Complex(z) => Value::Complex(z.mul(&z)),
            Value::Matrix(_) => unreachable!("handled above"),
        };
        self.stack.push(v);
        Ok(())
    }

    /// 10^X — exact for non-negative integer X, real otherwise.
    fn exp10_op(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        if let Value::Int(n) = &self.stack[self.stack.len() - 1] {
            if !n.is_negative() {
                let n = n.to_u32().ok_or(e_big("power result too large"))?;
                if n as u64 * 4 > MAX_POW_BITS {
                    return Err(e_big("power result too large"));
                }
                let _ = self.pop_x();
                let v = self.canon_flagged(Integer::from_i64(10).pow_exact(n));
                self.stack.push(Value::Int(v));
                return Ok(());
            }
        }
        self.cunary(
            |_| false,
            |z| Complex::from_i64(z.prec(), 10, 0).pow(z),
            |x| x.exp10(),
        )
    }

    /// Y idiv X — EXPLICIT truncating integer division (the old silent `/`
    /// behavior, now opt-in outside word mode).
    fn idiv(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let d = self.peek_int(0, "idiv needs integers")?;
        if d.is_zero() {
            return Err(CalcError::DivZero);
        }
        self.peek_int(1, "idiv needs integers")?;
        let Value::Int(b) = self.pop_x() else { unreachable!() };
        let Value::Int(a) = self.stack.pop().expect("validated") else { unreachable!() };
        let v = self.canon_flagged(a / b);
        self.stack.push(Value::Int(v));
        Ok(())
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

    /// FLOAT — enter Real mode (the 16C's FLOAT-mode switch), converting an
    /// integer X on the way in (also 16C). Radix keys return to Int mode.
    fn to_float(&mut self) -> Result<(), CalcError> {
        self.num_mode = NumMode::Real;
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
            Value::Complex(_) => Err(e_domain("cannot convert a complex to an integer")),
            Value::Matrix(_) => Err(e_domain("cannot convert a matrix to an integer")),
            Value::Real(f) => {
                if f.is_nan() || f.is_inf() {
                    return Err(e_domain("cannot convert nan/inf to an integer"));
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
        if self.stack.last().map(Value::is_complex).unwrap_or(false) {
            return Err(e_domain("frac undefined for a complex"));
        }
        self.no_matrix("frac undefined for a matrix")?;
        let v = match self.pop_x() {
            Value::Int(_) => Value::Int(Integer::new()),
            Value::Real(f) => Value::Real(f.frac()),
            Value::Complex(_) | Value::Matrix(_) => unreachable!("guarded above"),
        };
        self.stack.push(v);
        Ok(())
    }

    /// WSIZE (HP-16C): pop X as the word size in bits; 0 = unbounded (GMP).
    fn wsize(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let n = self.peek_u32("word size out of range")?;
        if n > MAX_WORD_BITS {
            return Err(e_mode("word size out of range"));
        }
        let _ = self.pop_x();
        self.set_word_bits(if n == 0 { None } else { Some(n) });
        Ok(())
    }

    /// Pop X as the MPFR working precision in bits.
    fn prec_cmd(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let n = self.peek_u32("precision out of range")?;
        if !(2..=MAX_PREC_BITS).contains(&n) {
            return Err(e_mode("precision out of range"));
        }
        let _ = self.pop_x();
        self.set_prec(n);
        Ok(())
    }

    /// FIX/SCI/ENG n — pop X as the digit count.
    fn fmt_cmd(&mut self, shape: FloatFmt) -> Result<(), CalcError> {
        self.need(1)?;
        let d = self.peek_u32("format digits out of range")?;
        if d > MAX_FMT_DIGITS {
            return Err(e_mode("format digits out of range"));
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

    /// SF/CF i — set/clear flag i (pop X as the index). 0–2 = user flags;
    /// 3 = leading zeros, 4 = carry, 5 = out-of-range (16C aliases).
    fn set_flag(&mut self, on: bool) -> Result<(), CalcError> {
        self.need(1)?;
        let i = self.peek_u32("flag index must be 0-5")?;
        if i > 5 {
            return Err(e_reg("flag index must be 0-5"));
        }
        let _ = self.pop_x();
        match i {
            0..=2 => self.user_flags[i as usize] = on,
            3 => self.leading_zeros = on,
            4 => self.carry = on,
            _ => self.overflow = on,
        }
        Ok(())
    }

    /// F? i — pop the flag index, push 0/1.
    fn flag_test(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let i = self.peek_u32("flag index must be 0-5")?;
        if i > 5 {
            return Err(e_reg("flag index must be 0-5"));
        }
        let _ = self.pop_x();
        let v = match i {
            0..=2 => self.user_flags[i as usize],
            3 => self.leading_zeros,
            4 => self.carry,
            _ => self.overflow,
        };
        self.push_int_canon(Integer::from_i64(v as i64));
        Ok(())
    }

    // ---- statistics (Σ registers, 15C style) ---------------------------------
    /// Σ+ / Σ− — accumulate (x = X, y = Y; y is 0 with a one-deep stack) into
    /// the Σ registers; X is replaced by n (15C behavior), the consumed x
    /// goes to LASTx, the stack depth is unchanged.
    fn sigma(&mut self, add: bool) -> Result<(), CalcError> {
        self.need(1)?;
        if !add && self.stats.as_ref().map_or(true, |s| s.n == 0) {
            return Err(e_stats("no data accumulated"));
        }
        let prec = self.prec;
        let x = self.stack.last().expect("validated").clone().to_real(prec);
        let y = if self.stack.len() >= 2 {
            self.stack[self.stack.len() - 2].clone().to_real(prec)
        } else {
            Float::from_i64(prec, 0)
        };
        let st = self.stats.get_or_insert_with(|| Stats::new(prec));
        if add {
            st.n += 1;
            st.sx = st.sx.clone() + x.clone();
            st.sxx = st.sxx.clone() + x.clone() * x.clone();
            st.sy = st.sy.clone() + y.clone();
            st.syy = st.syy.clone() + y.clone() * y.clone();
            st.sxy = st.sxy.clone() + x * y;
        } else {
            st.n -= 1;
            st.sx = st.sx.clone() - x.clone();
            st.sxx = st.sxx.clone() - x.clone() * x.clone();
            st.sy = st.sy.clone() - y.clone();
            st.syy = st.syy.clone() - y.clone() * y.clone();
            st.sxy = st.sxy.clone() - x * y;
        }
        self.last_x = Some(self.stack.last().expect("validated").clone());
        let n = st.n as i64;
        let nc = self.canon_silent(Integer::from_i64(n));
        let l = self.stack.len();
        self.stack[l - 1] = Value::Int(nc);
        Ok(())
    }

    fn stats_ref(&self, min_n: u64) -> Result<&Stats, CalcError> {
        match &self.stats {
            Some(s) if s.n >= min_n => Ok(s),
            _ => Err(e_stats("need more data points (s+)")),
        }
    }

    /// Linear-regression coefficients `(slope, intercept)` for y = a·x + b.
    /// Degenerate data (all x equal → zero variance) errors like the 15C
    /// rather than yielding a silent NaN.
    fn lr_coeffs(&self) -> Result<(Float, Float), CalcError> {
        let prec = self.prec;
        let s = self.stats_ref(2)?;
        let n = Float::from_i64(prec, s.n as i64);
        let den = n.clone() * s.sxx.clone() - s.sx.clone() * s.sx.clone();
        if den.is_zero() {
            return Err(e_stats("regression needs varying x data"));
        }
        let slope = (n.clone() * s.sxy.clone() - s.sx.clone() * s.sy.clone()) / den;
        let intercept = (s.sy.clone() - slope.clone() * s.sx.clone()) / n;
        Ok((slope, intercept))
    }

    /// mean / sdev / L.R. — pushes the pair: X gets x̄ / sₓ / the intercept,
    /// Y gets ȳ / s_y / the slope (15C-style pairing).
    fn stat_pair(&mut self, f: StatFn) -> Result<(), CalcError> {
        let prec = self.prec;
        let (xv, yv) = match f {
            StatFn::Mean => {
                let s = self.stats_ref(1)?;
                let n = Float::from_i64(prec, s.n as i64);
                (s.sx.clone() / n.clone(), s.sy.clone() / n)
            }
            StatFn::Sdev => {
                let s = self.stats_ref(2)?;
                let n = Float::from_i64(prec, s.n as i64);
                let n1 = Float::from_i64(prec, s.n as i64 - 1);
                let vx = (s.sxx.clone() - s.sx.clone() * s.sx.clone() / n.clone()) / n1.clone();
                let vy = (s.syy.clone() - s.sy.clone() * s.sy.clone() / n) / n1;
                (vx.sqrt(), vy.sqrt())
            }
            StatFn::Lr => {
                let (slope, intercept) = self.lr_coeffs()?;
                (intercept, slope)
            }
        };
        self.stack.push(Value::Real(yv));
        self.stack.push(Value::Real(xv));
        Ok(())
    }

    /// ŷ — pop x, push the regression estimate a·x + b.
    fn yhat(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let (slope, intercept) = self.lr_coeffs()?;
        let x = self.pop_x().to_real(self.prec);
        self.stack.push(Value::Real(intercept + slope * x));
        Ok(())
    }

    /// r — the correlation coefficient.
    fn corr(&mut self) -> Result<(), CalcError> {
        let prec = self.prec;
        let r = {
            let s = self.stats_ref(2)?;
            let n = Float::from_i64(prec, s.n as i64);
            let cx = n.clone() * s.sxx.clone() - s.sx.clone() * s.sx.clone();
            let cy = n.clone() * s.syy.clone() - s.sy.clone() * s.sy.clone();
            if cx.is_zero() || cy.is_zero() {
                return Err(e_stats("correlation needs varying data"));
            }
            let cov = n * s.sxy.clone() - s.sx.clone() * s.sy.clone();
            cov / (cx * cy).sqrt()
        };
        self.stack.push(Value::Real(r));
        Ok(())
    }

    // ---- combinatorics + PRNG ------------------------------------------------
    /// nCr / nPr — Y = n, X = r; **exact** GMP (mpz binomial; nPr = C(n,r)·r!).
    fn comb_perm(&mut self, perm: bool) -> Result<(), CalcError> {
        self.need(2)?;
        let r = self.peek_u32("ncr/npr need small non-negative integers")?;
        let n = self.peek_integral(1, "ncr/npr need integers")?;
        let n = &n;
        if n.is_negative() {
            return Err(e_domain("ncr/npr need non-negative n"));
        }
        let v = if Integer::from_i64(r as i64) > n.clone() {
            Integer::new() // r > n → 0 (and skip a pointless huge r!)
        } else {
            if n.bit_len() as u64 * r as u64 > MAX_POW_BITS {
                return Err(e_big("result too large"));
            }
            let b = n.binomial(r);
            if perm {
                b * Integer::factorial(r)
            } else {
                b
            }
        };
        let _ = self.pop_x();
        let _ = self.stack.pop();
        let v = self.canon_flagged(v);
        self.stack.push(Value::Int(v));
        Ok(())
    }

    /// RAN# — uniform in [0, 1) at the working precision (xorshift64,
    /// deterministic until `seed`ed; the firmware seeds from hardware).
    fn next_ran(&mut self) -> Float {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        let hi = Integer::from_i64((x >> 32) as i64) << 32;
        let lo = Integer::from_i64((x & 0xFFFF_FFFF) as i64);
        let k = hi | lo;
        let num = Float::from_integer(self.prec, &k);
        let den = Float::from_integer(self.prec, &(Integer::from_i64(1) << 64));
        num / den
    }

    /// Pop X as the PRNG seed (splitmix64-expanded so nearby seeds diverge).
    fn seed_cmd(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let s = self.peek_u32("seed must be a small non-negative integer")?;
        let _ = self.pop_x();
        let mut z = (s as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        self.rng = if z == 0 { 1 } else { z };
        Ok(())
    }

    // ---- TVM (HP-12C time-value-of-money) ------------------------------------
    // Equation (END mode; k = 1+p in BEG): with p = i/100 per period,
    //   pv·(1+p)^n + pmt·k·((1+p)^n − 1)/p + fv = 0
    // and the linear p = 0 limit: pv + pmt·n + fv = 0.

    fn tvm_mut(&mut self) -> &mut Tvm {
        let prec = self.prec;
        self.tvm.get_or_insert_with(|| Tvm::new(prec))
    }

    /// `>reg` — copy X into a TVM register (X stays, like STO).
    fn tvm_store(&mut self, r: TvmReg) -> Result<(), CalcError> {
        self.need(1)?;
        let prec = self.prec;
        let v = self.stack.last().expect("validated").clone().to_real(prec);
        let t = self.tvm_mut();
        match r {
            TvmReg::N => t.n = v,
            TvmReg::I => t.i = v,
            TvmReg::Pv => t.pv = v,
            TvmReg::Pmt => t.pmt = v,
            TvmReg::Fv => t.fv = v,
        }
        Ok(())
    }

    /// `rclreg` — push a TVM register.
    fn tvm_recall(&mut self, r: TvmReg) -> Result<(), CalcError> {
        let t = self.tvm_mut();
        let v = match r {
            TvmReg::N => t.n.clone(),
            TvmReg::I => t.i.clone(),
            TvmReg::Pv => t.pv.clone(),
            TvmReg::Pmt => t.pmt.clone(),
            TvmReg::Fv => t.fv.clone(),
        };
        self.stack.push(Value::Real(v));
        Ok(())
    }

    /// g-12÷ / g-12× — X converted (annual↔monthly) and stored into i / n.
    fn tvm_by12(&mut self, mul: bool) -> Result<(), CalcError> {
        self.need(1)?;
        let prec = self.prec;
        let x = self.pop_x().to_real(prec);
        let twelve = Float::from_i64(prec, 12);
        let v = if mul { x * twelve } else { x / twelve };
        let t = self.tvm_mut();
        if mul {
            t.n = v.clone();
        } else {
            t.i = v.clone();
        }
        self.stack.push(Value::Real(v));
        Ok(())
    }

    /// The TVM balance f(p) at periodic rate p (fraction, not percent),
    /// and the compounding factor c = (1+p)^n.
    fn tvm_balance(t: &Tvm, prec: u32, p: &Float) -> Float {
        let one = Float::from_i64(prec, 1);
        let c = (one.clone() + p.clone()).pow(t.n.clone());
        let k = if t.begin { one + p.clone() } else { Float::from_i64(prec, 1) };
        let ann = t.pmt.clone() * k * (c.clone() - Float::from_i64(prec, 1)) / p.clone();
        t.pv.clone() * c + ann + t.fv.clone()
    }

    /// Solve one TVM register from the other four; the result is pushed AND
    /// stored back into the register (12C behavior). Errors are
    /// non-destructive (registers and stack untouched).
    fn tvm_solve(&mut self, r: TvmReg) -> Result<(), CalcError> {
        let prec = self.prec;
        if self.tvm.is_none() {
            self.tvm = Some(Tvm::new(prec));
        }
        let t = self.tvm.as_ref().expect("just ensured");
        let one = Float::from_i64(prec, 1);
        let hundred = Float::from_i64(prec, 100);
        let p = t.i.clone() / hundred.clone();
        let p_zero = p.is_zero();
        let k = if t.begin { one.clone() + p.clone() } else { Float::from_i64(prec, 1) };

        let v = match r {
            TvmReg::Fv => {
                if p_zero {
                    -(t.pv.clone() + t.pmt.clone() * t.n.clone())
                } else {
                    let c = (one.clone() + p.clone()).pow(t.n.clone());
                    -(t.pv.clone() * c.clone()
                        + t.pmt.clone() * k * (c - one) / p)
                }
            }
            TvmReg::Pv => {
                if p_zero {
                    -(t.fv.clone() + t.pmt.clone() * t.n.clone())
                } else {
                    let c = (one.clone() + p.clone()).pow(t.n.clone());
                    -(t.fv.clone() + t.pmt.clone() * k * (c.clone() - one) / p) / c
                }
            }
            TvmReg::Pmt => {
                if p_zero {
                    if t.n.is_zero() {
                        return Err(e_nosol("pmt needs n != 0"));
                    }
                    -(t.pv.clone() + t.fv.clone()) / t.n.clone()
                } else {
                    let c = (one.clone() + p.clone()).pow(t.n.clone());
                    let ann = k * (c.clone() - one) / p;
                    if ann.is_zero() {
                        return Err(e_nosol("pmt is undetermined (n = 0)"));
                    }
                    -(t.fv.clone() + t.pv.clone() * c) / ann
                }
            }
            TvmReg::N => {
                if p_zero {
                    if t.pmt.is_zero() {
                        return Err(e_nosol("n is undetermined (i = 0, pmt = 0)"));
                    }
                    -(t.pv.clone() + t.fv.clone()) / t.pmt.clone()
                } else {
                    // c·(pv + pmt·k/p) = pmt·k/p − fv  →  n = ln c / ln(1+p)
                    let a = t.pmt.clone() * k / p.clone();
                    let den = t.pv.clone() + a.clone();
                    if den.is_zero() {
                        return Err(e_nosol("n has no solution for these values"));
                    }
                    let c = (a - t.fv.clone()) / den;
                    if c.is_nan() || c.is_sign_negative() || c.is_zero() {
                        return Err(e_nosol("n has no solution for these values"));
                    }
                    c.ln() / (one + p).ln()
                }
            }
            TvmReg::I => self.tvm_solve_i()? * hundred,
        };
        if v.is_nan() || v.is_inf() {
            return Err(e_nosol("no TVM solution for these values"));
        }
        let t = self.tvm_mut();
        match r {
            TvmReg::N => t.n = v.clone(),
            TvmReg::I => t.i = v.clone(),
            TvmReg::Pv => t.pv = v.clone(),
            TvmReg::Pmt => t.pmt = v.clone(),
            TvmReg::Fv => t.fv = v.clone(),
        }
        self.stack.push(Value::Real(v));
        Ok(())
    }

    /// Solve the periodic TVM rate p (fraction) via [`bisect_rate`].
    fn tvm_solve_i(&self) -> Result<Float, CalcError> {
        let prec = self.prec;
        let t = self.tvm.as_ref().expect("caller ensured");
        bisect_rate(prec, |p| Self::tvm_balance(t, prec, p))
            .ok_or(e_nosol("no i solution found"))
    }

    // ---- cash flows: NPV / IRR (12C) -------------------------------------------
    /// CF₀ (`initial` — resets the list) / CFⱼ (appends) — X stays displayed.
    fn cash_flow(&mut self, initial: bool) -> Result<(), CalcError> {
        self.need(1)?;
        let prec = self.prec;
        let v = self.stack.last().expect("validated").clone().to_real(prec);
        if initial {
            self.cfs.clear();
        } else if self.cfs.is_empty() {
            return Err(e_use("enter cf0 first"));
        }
        self.cfs.push((v, 1));
        Ok(())
    }

    /// Nⱼ — repeat count for the most recent flow (1–9999).
    fn cash_count(&mut self) -> Result<(), CalcError> {
        if self.cfs.is_empty() {
            return Err(e_use("no cash flow to count (cfj first)"));
        }
        self.need(1)?;
        let n = self.peek_u32("nj must be 1-9999")?;
        if n == 0 || n > 9999 {
            return Err(e_use("nj must be 1-9999"));
        }
        let _ = self.pop_x();
        self.cfs.last_mut().expect("checked").1 = n;
        Ok(())
    }

    /// NPV of the flow list at rate p (fraction): grouped geometric sums —
    /// amt·vᵗ·(1−vⁿ)/(1−v) per group (plain sum when p = 0).
    fn npv_at(&self, p: &Float) -> Float {
        let prec = self.prec;
        let one = Float::from_i64(prec, 1);
        let v = one.clone() / (one.clone() + p.clone());
        let mut total = Float::from_i64(prec, 0);
        let mut vt = one.clone(); // v^t at the current group's first period
        for (amt, cnt) in &self.cfs {
            let cnt_f = Float::from_i64(prec, *cnt as i64);
            let group = if p.is_zero() {
                amt.clone() * cnt_f.clone()
            } else {
                amt.clone() * vt.clone() * (one.clone() - v.clone().pow(cnt_f.clone()))
                    / (one.clone() - v.clone())
            };
            total = total + group;
            vt = vt * v.clone().pow(cnt_f); // advance past this group
        }
        total
    }

    /// NPV — discounts the flow list at the TVM `i` register; pushes.
    fn npv_cmd(&mut self) -> Result<(), CalcError> {
        if self.cfs.is_empty() {
            return Err(e_use("no cash flows (cf0/cfj)"));
        }
        let prec = self.prec;
        let p = match &self.tvm {
            Some(t) => t.i.clone() / Float::from_i64(prec, 100),
            None => Float::from_i64(prec, 0),
        };
        let v = self.npv_at(&p);
        self.stack.push(Value::Real(v));
        Ok(())
    }

    /// IRR — the rate that zeroes NPV; pushed AND stored into `i` (12C).
    fn irr_cmd(&mut self) -> Result<(), CalcError> {
        if self.cfs.len() < 2 {
            return Err(e_use("irr needs cf0 and at least one cfj"));
        }
        let prec = self.prec;
        let p = bisect_rate(prec, |p| self.npv_at(p))
            .ok_or(e_nosol("no irr solution found"))?;
        let i = p * Float::from_i64(prec, 100);
        self.tvm_mut().i = i.clone();
        self.stack.push(Value::Real(i));
        Ok(())
    }

    // ---- dates (12C M.DY encoding: 3.152026 = March 15 2026) --------------------
    /// Decode an M.DYYYY date float; errors on invalid dates (Gregorian,
    /// years 1583–9999).
    fn decode_date(&self, f: &Float) -> Result<(i64, i64, i64), CalcError> {
        const BAD: CalcError = e_date("invalid date (use M.DYYYY)");
        if f.is_nan() || f.is_inf() || f.is_sign_negative() {
            return Err(BAD);
        }
        let prec = self.prec;
        let v = (f.clone() * Float::from_i64(prec, 1_000_000)).round_to_int();
        let v = v.to_u32().ok_or(BAD)? as i64;
        let m = v / 1_000_000;
        let d = (v % 1_000_000) / 10_000;
        let y = v % 10_000;
        if !(1..=12).contains(&m) || !(1583..=9999).contains(&y) || d < 1 || d > days_in_month(y, m) {
            return Err(BAD);
        }
        Ok((y, m, d))
    }

    fn encode_date(&self, y: i64, m: i64, d: i64) -> Float {
        let prec = self.prec;
        Float::from_i64(prec, m * 1_000_000 + d * 10_000 + y) / Float::from_i64(prec, 1_000_000)
    }

    /// ΔDYS — days from the date in Y to the date in X: actual → X,
    /// 30/360 (US) → Y.
    fn ddays(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let prec = self.prec;
        let d2 = self.decode_date(&self.stack[self.stack.len() - 1].clone().to_real(prec))?;
        let d1 = self.decode_date(&self.stack[self.stack.len() - 2].clone().to_real(prec))?;
        let _ = self.pop_x();
        let _ = self.stack.pop();
        let actual = days_from_civil(d2.0, d2.1, d2.2) - days_from_civil(d1.0, d1.1, d1.2);
        // 30/360 US: day-31 adjustments
        let mut dd1 = d1.2;
        let mut dd2 = d2.2;
        if dd1 == 31 {
            dd1 = 30;
        }
        if dd2 == 31 && dd1 >= 30 {
            dd2 = 30;
        }
        let d360 = 360 * (d2.0 - d1.0) + 30 * (d2.1 - d1.1) + (dd2 - dd1);
        self.push_int_canon(Integer::from_i64(d360));
        self.push_int_canon(Integer::from_i64(actual));
        Ok(())
    }

    /// DATE — the date in Y advanced by X days (may be negative).
    fn date_add(&mut self) -> Result<(), CalcError> {
        self.need(2)?;
        let prec = self.prec;
        let days = {
            let i = self.peek_integral(0, "day count must be a whole number")?;
            let mag =
                i.clone().abs().to_u32().ok_or(e_date("day count out of range"))? as i64;
            if i.is_negative() {
                -mag
            } else {
                mag
            }
        };
        let d = self.decode_date(&self.stack[self.stack.len() - 2].clone().to_real(prec))?;
        let z = days_from_civil(d.0, d.1, d.2) + days;
        let (y, m, dd) = civil_from_days(z);
        if !(1583..=9999).contains(&y) {
            return Err(e_date("date out of range"));
        }
        let _ = self.pop_x();
        let _ = self.stack.pop();
        let f = self.encode_date(y, m, dd);
        self.stack.push(Value::Real(f));
        Ok(())
    }

    /// Day of week for the date in X: 1 = Monday … 7 = Sunday (ISO).
    fn day_of_week(&mut self) -> Result<(), CalcError> {
        self.need(1)?;
        let prec = self.prec;
        let d = self.decode_date(&self.stack[self.stack.len() - 1].clone().to_real(prec))?;
        let _ = self.pop_x();
        let z = days_from_civil(d.0, d.1, d.2);
        let dow = (z + 3).rem_euclid(7) + 1;
        self.push_int_canon(Integer::from_i64(dow));
        Ok(())
    }

    // ---- depreciation (12C: cost = PV, salvage = FV, life = n) ------------------
    /// SL / SOYD / DB — pop the year j from X; push the remaining depreciable
    /// value (→ Y) then year-j depreciation (→ X), 12C style. DB reads the
    /// declining-balance factor (percent, e.g. 200) from `i` and floors at
    /// the salvage value. Integer whole years only (odd-period conventions
    /// deferred with the TVM odd-period note).
    fn depreciation(&mut self, kind: Dep) -> Result<(), CalcError> {
        self.need(1)?;
        let j = self.peek_u32("year must be a small positive integer")?;
        if j == 0 || j > 10_000 {
            return Err(e_use("year must be a small positive integer"));
        }
        let prec = self.prec;
        let t = self.tvm.as_ref().ok_or(e_use("set n (life), pv (cost), fv (salvage) first"))?;
        let life = t.n.clone();
        if life.is_zero() || life.is_sign_negative() {
            return Err(e_use("life (n) must be positive"));
        }
        // beyond the life: fully depreciated
        let beyond = (life.clone() - Float::from_i64(prec, j as i64)).is_sign_negative();
        let base = t.pv.clone() - t.fv.clone(); // depreciable amount
        let zero = || Float::from_i64(prec, 0);
        let (dep, remaining) = if beyond && kind != Dep::Db {
            (zero(), zero())
        } else {
            match kind {
                Dep::Sl => {
                    let dep = base.clone() / life.clone();
                    let rem = base - dep.clone() * Float::from_i64(prec, j as i64);
                    let rem = if rem.is_sign_negative() { zero() } else { rem };
                    (dep, rem)
                }
                Dep::Soyd => {
                    let one = Float::from_i64(prec, 1);
                    let two = Float::from_i64(prec, 2);
                    let soyd = life.clone() * (life.clone() + one.clone()) / two.clone();
                    let jf = Float::from_i64(prec, j as i64);
                    let dep = base.clone() * (life.clone() - jf.clone() + one) / soyd.clone();
                    let lj = life - jf;
                    let rem = base * lj.clone() * (lj + Float::from_i64(prec, 1)) / two / soyd;
                    (dep, rem)
                }
                Dep::Db => {
                    // rate per year = (i/100)/life; iterate, flooring at salvage
                    let rate = t.i.clone() / Float::from_i64(prec, 100) / life;
                    let mut book = t.pv.clone();
                    let mut dep = zero();
                    for _ in 0..j {
                        dep = book.clone() * rate.clone();
                        let floor_room = book.clone() - dep.clone() - t.fv.clone();
                        if floor_room.is_sign_negative() {
                            dep = book.clone() - t.fv.clone();
                            if dep.is_sign_negative() {
                                dep = zero();
                            }
                        }
                        book = book - dep.clone();
                    }
                    (dep, book - t.fv.clone())
                }
            }
        };
        let _ = self.pop_x();
        self.stack.push(Value::Real(remaining));
        self.stack.push(Value::Real(dep));
        Ok(())
    }

    // ---- percent family (12C) -------------------------------------------------
    /// Δ% (`true`) or %T (`false`) — X becomes the result, Y is preserved.
    fn pct_of(&mut self, change: bool) -> Result<(), CalcError> {
        self.need(2)?;
        if matches!(&self.stack[self.stack.len() - 2], Value::Real(f) if f.is_zero()) {
            return Err(CalcError::DivZero);
        }
        if matches!(&self.stack[self.stack.len() - 2], Value::Int(i) if i.is_zero()) {
            return Err(CalcError::DivZero);
        }
        let prec = self.prec;
        let x = self.pop_x().to_real(prec);
        let y = self.stack.last().expect("validated").clone().to_real(prec);
        let hundred = Float::from_i64(prec, 100);
        let v = if change {
            (x - y.clone()) / y * hundred
        } else {
            x / y * hundred
        };
        self.stack.push(Value::Real(v));
        Ok(())
    }

    /// x̄w — weighted mean Σxy/Σy over the Σ registers (x = values, y = weights).
    fn weighted_mean(&mut self) -> Result<(), CalcError> {
        let v = {
            let s = self.stats_ref(1)?;
            if s.sy.is_zero() {
                return Err(e_stats("weighted mean needs nonzero weights"));
            }
            s.sxy.clone() / s.sy.clone()
        };
        self.stack.push(Value::Real(v));
        Ok(())
    }

    /// STO i — copy X into register i (X stays, LASTx untouched).
    fn sto(&mut self, i: usize) -> Result<(), CalcError> {
        self.need(1)?;
        self.regs[i] = Some(self.stack.last().expect("validated").clone());
        Ok(())
    }

    /// RCL i — push register i (integers wrap into the current word mode:
    /// registers keep full values across mode changes, the stack does not).
    fn rcl(&mut self, i: usize) -> Result<(), CalcError> {
        match &self.regs[i] {
            Some(Value::Int(x)) => {
                let x = x.clone();
                self.push_int_canon(x);
                Ok(())
            }
            Some(v) => {
                self.stack.push(v.clone());
                Ok(())
            }
            None => Err(e_reg("empty register")),
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
            Some(Value::Int(x)) => {
                let x = x.clone();
                self.push_int_canon(x);
                Ok(())
            }
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

/// Bracketed bisection for a rate p > −1: scan a fixed candidate grid for a
/// sign change, then halve to working precision. The first bracket wins —
/// like the 12C, multi-root problems (sign-changing cash flows) are the
/// user's responsibility. `None` when no bracket exists.
fn bisect_rate(prec: u32, f: impl Fn(&Float) -> Float) -> Option<Float> {
    const GRID: [&str; 14] = [
        "-0.999999", "-0.9", "-0.5", "-0.1", "-0.01", "-0.0001", "0.0000001",
        "0.001", "0.01", "0.05", "0.2", "1", "10", "1000",
    ];
    let mut lo: Option<(Float, bool)> = None;
    let mut bracket = None;
    for s in GRID {
        let p = Float::from_str(prec, s).expect("grid literal");
        let y = f(&p);
        if y.is_nan() || y.is_inf() {
            lo = None;
            continue;
        }
        if y.is_zero() {
            return Some(p);
        }
        let neg = y.is_sign_negative();
        if let Some((pl, nl)) = lo.take() {
            if nl != neg {
                bracket = Some((pl, p.clone()));
                break;
            }
        }
        lo = Some((p, neg));
    }
    let (mut a, mut b) = bracket?;
    let fa_neg = f(&a).is_sign_negative();
    let two = Float::from_i64(prec, 2);
    for _ in 0..(prec + 48) {
        let m = (a.clone() + b.clone()) / two.clone();
        let ym = f(&m);
        if ym.is_zero() {
            return Some(m);
        }
        if ym.is_sign_negative() == fa_neg {
            a = m;
        } else {
            b = m;
        }
    }
    Some(a)
}

// ---- civil-calendar helpers (Gregorian; Hinnant's algorithms) ----------------

/// Days since 1970-01-01 for a civil date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Civil date from days since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = (mp + 2) % 12 + 1;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
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

