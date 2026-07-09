//! The Calcumaker 16 keymap — wide HP-16C-style layout, 5 rows × 10 cols =
//! 50 keys, with HP-Voyager-style f (gold) / g (blue) shifts (3 functions per
//! key). **Design source of truth** for the key legends (mirror in DESIGN.md).
//!
//! This is the device-independent half of the keypad: logical keys, the three
//! shift layers, and the shift-resolution state machine. Frontends deliver raw
//! `(row, col)` presses — the firmware from the Cherry MX matrix scan
//! (`calcumaker-fw/src/keypad.rs`), the emulator from the host keyboard
//! (`calcumaker-emu`) — and [`crate::App`] resolves them through the active
//! layer here.

pub const ROWS: usize = 5;
pub const COLS: usize = 10;

/// A decoded action. `Digit(0..=15)` covers 0-9 and A-F (hex entry).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    // entry
    Digit(u8), Dot, Chs, Eex, Back, ClrX,
    // arithmetic
    Add, Sub, Mul, Div,
    // stack / memory
    Enter, Swap, RollDn, RollUp, LastX, Sto, Rcl, ClrReg,
    // flags (X = index 0-5; 3/4/5 alias lz/carry/overflow)
    Sf, Cf, Ftest,
    // SHOW: transient display of X in another base (App-level, 16C f-SHOW)
    ShowHex, ShowDec, ShowOct, ShowBin,
    // STATUS: momentary all-modes view on the glass (App-level, 16C STATUS)
    Status,
    // SETUP: interactive settings menu on the glass (App-level; runtime
    // configuration of the display/interaction tunables)
    Setup,
    // base / word modes (programmer)
    Hex, Dec, Oct, Bin, WordSize, SignMode, Float,
    // bitwise / shift / rotate (programmer)
    And, Or, Xor, Not, Shl, Shr, Asr, Rotl, Rotr, Rlc, Rrc, Lj,
    BitSet, BitClr, BitTest, MaskL, MaskR, BitCount, Rmd,
    DblMul, DblDiv, DblRem,
    // scientific (MPFR)
    Sin, Cos, Tan, Asin, Acos, Atan, Sinh, Cosh, Tanh,
    Ln, Exp, Log10, Exp10, Sqrt, Sq, Pow, Recip, Pi, Fact, Pct, Round,
    // complex (HP-42S / 15C): COMPLEX merges/splits re+im; CplxDisp toggles
    // RECT/POLAR; then the 15C's part ops — conjugate, argument, real/imag
    // extraction, and Re<>Im swap.
    Complex, CplxDisp, Conj, Arg, Re, Im, ReIm,
    // 15C coordinate conversion (two reals): →P rectangular→polar, →R the inverse
    ToPolar, ToRect,
    // 15C matrices: MatNew (DIM + open fill), MatSet (store next cell), then the
    // operations — determinant, transpose, inverse, and A⁻¹B solve.
    MatNew, MatSet, Det, Transpose, Minv, Matsolve,
    // real display format (X = digit count; FmtAuto = %g-style)
    Fix, Sci, Eng, FmtAuto,
    // angle unit for circular trig (cycles RAD → DEG → GRAD)
    AngleMode,
    // leading-zeros display toggle (16C flag 3)
    Lz,
    // display window scroll for values wider than the row (16C < / >)
    WinL, WinR,
    // statistics / combinatorics / random (SCI personality; engine superset)
    SigmaPlus, SigmaMinus, Mean, Sdev, Lr, Yhat, Corr, ClStat, Ncr, Npr, Ran, Seed,
    // finance (FIN personality): TVM keys store on pending entry, solve
    // otherwise (12C); cash flows, dates, depreciation, percent family
    TvmN, TvmI, TvmPv, TvmPmt, TvmFv, X12Mul, X12Div, BegKey, EndKey, ClFin,
    Cf0, Cfj, NjKey, Npv, Irr, ClCf,
    PctChg, PctT, Wmean,
    Ddays, DateAdd, Dow,
    DepSl, DepSoyd, DepDb,
    // arbitrary-precision control (the headline feature)
    Prec,
    // system / modifiers (ShiftF/ShiftG are handled, not emitted)
    ShiftF, ShiftG, Off, Nop,
}

use Key::*;
const fn d(n: u8) -> Key { Digit(n) }

/// Base (unshifted) layer — the printed key faces.
pub const BASE: [[Key; COLS]; ROWS] = [
    [Sin,    Cos,    Tan,  Ln,   Sqrt,     Pow,  Recip,  Eex,  Back, ClrX],
    [d(10),  d(11),  d(12),d(13),d(14),    d(15),d(7),   d(8), d(9),  Div],
    [And,    Or,     Xor,  Not,  Shl,      Shr,  d(4),   d(5), d(6),  Mul],
    [Hex,    Dec,    Oct,  Bin,  WordSize, Swap, d(1),   d(2), d(3),  Sub],
    [ShiftF, ShiftG, Sto,  Rcl,  RollDn,   Enter,d(0),   Dot,  Chs,   Add],
];

/// f (gold) layer — inverse / advanced / set. (Nop = unassigned, refine later.)
pub const LAYER_F: [[Key; COLS]; ROWS] = [
    [Asin,   Acos,   Atan, Exp,  Sq,       Nop,  Prec,   Pi,   LastX, Status],
    [BitSet, BitClr, BitTest, MaskL, MaskR, BitCount, Lj, Sf,  Cf,    Ftest],
    [Rotl,   Rotr,   Asr,  Rmd,  Rlc,      Rrc,  DblMul, DblDiv, DblRem, Nop],
    [ShowHex,ShowDec,ShowOct,ShowBin,SignMode, Float, Nop, Nop, Nop,  Nop],
    [ShiftF, ShiftG, ClrReg, Nop, RollUp,  Nop,  Off,    Nop,  Eex,   Nop],
];

/// g (blue) layer — hyperbolic / secondary. (FIX/SCI/ENG/auto sit over the
/// radix keys — display format over display base; angle mode over WSIZE.)
pub const LAYER_G: [[Key; COLS]; ROWS] = [
    [Sinh,   Cosh,   Tanh, Log10,Exp10,    Nop,  Nop,    Nop,  Nop,   Setup],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
    [Lz,     Nop,    Nop,  Nop,  WinL,     WinR, Fact,   Pct,  Round, Nop],
    [Fix,    Sci,    Eng,  FmtAuto, AngleMode, Nop, Nop, Nop,  Nop,   Nop],
    [ShiftF, ShiftG, Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
];

/// A **personality**: a named set of the three shift layers (see
/// `DESIGN-MODES.md`). The engine is personality-agnostic — a personality
/// only selects which superset functions the keys reach. `HP16C` is the
/// default (and today the only) entry; future: `SCI`, `FIN`.
pub struct Keymap {
    pub name: &'static str,
    pub base: [[Key; COLS]; ROWS],
    pub f: [[Key; COLS]; ROWS],
    pub g: [[Key; COLS]; ROWS],
    /// Display-mode defaults applied when this personality is selected
    /// (angle/format/radix conventions — data is never touched).
    pub apply_defaults: fn(&mut crate::calc::Calc),
}

fn defaults_16c(c: &mut crate::calc::Calc) {
    c.set_angle_mode(crate::calc::AngleMode::Rad);
    c.set_float_fmt(crate::calc::FloatFmt::Auto);
    c.set_real_entry(false); // exact-integer entry: the programmer identity
    c.set_cpxres(false); // period-correct: the 16C has no complex — √-1 = Error 0
}

fn defaults_sci(c: &mut crate::calc::Calc) {
    // 15C conventions: degrees, FIX 4, decimal — and it's a FLOAT machine.
    c.set_angle_mode(crate::calc::AngleMode::Deg);
    c.set_float_fmt(crate::calc::FloatFmt::Fix(4));
    c.set_radix(crate::calc::Radix::Dec);
    c.set_real_entry(true);
    c.set_cpxres(true); // scientific: real ops go complex (√-1 = i)
}

/// The default personality — the tables above.
pub static HP16C: Keymap =
    Keymap { name: "16C", base: BASE, f: LAYER_F, g: LAYER_G, apply_defaults: defaults_16c };

// ---- SCI personality (HP-15C-flavored scientific) ---------------------------
// Digits, ENTER, shifts, arithmetic, STATUS (f-CLx) and SETUP (g-CLx) keep the
// SAME physical positions as 16C — muscle memory carries over. The programmer
// row (bitops/radix) gives way to inverse trig / logs / statistics on primary
// faces; f = hyperbolics + regression; g = combinatorics + random.

/// SCI base layer.
pub const SCI_BASE: [[Key; COLS]; ROWS] = [
    [Sin,    Cos,    Tan,  Ln,   Sqrt,     Pow,  Recip,  Eex,  Back, ClrX],
    [Asin,   Acos,   Atan, Log10,Exp,      Exp10,d(7),   d(8), d(9),  Div],
    [SigmaPlus, SigmaMinus, Mean, Sdev,    Fact, Pct,  d(4),   d(5), d(6),  Mul],
    [Fix,    Sci,    Eng,  FmtAuto, AngleMode, Swap, d(1), d(2), d(3), Sub],
    [ShiftF, ShiftG, Sto,  Rcl,  RollDn,   Enter,d(0),   Dot,  Chs,   Add],
];

/// SCI f (gold) layer — hyperbolics, regression, precision.
pub const SCI_LAYER_F: [[Key; COLS]; ROWS] = [
    // f+P (col 5) = RECT/POLAR toggle, f+I (col 6) = COMPLEX (i = imaginary)
    [Sinh,   Cosh,   Tanh, Prec, Sq,       CplxDisp, Complex, Pi, LastX, Status],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
    [Lr,     Yhat,   Corr, ClStat, Nop,    Nop,  Nop,    Nop,  Nop,   Nop],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Float,Nop,    Nop,  Nop,   Nop],
    [ShiftF, ShiftG, ClrReg, Nop, RollUp,  Nop,  Off,    Nop,  Eex,   Nop],
];

/// SCI g (blue) layer — combinatorics, random, display windows, SETUP.
pub const SCI_LAYER_G: [[Key; COLS]; ROWS] = [
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Setup],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
    [Ncr,    Npr,    Ran,  Seed, WinL,     WinR, Nop,    Nop,  Round, Nop],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
    [ShiftF, ShiftG, Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
];

/// The scientific personality (15C-flavored; decimal machine — defaults set
/// DEG + FIX 4 + Dec on switch, see `App::set_keymap`).
pub static SCI: Keymap = Keymap {
    name: "SCI",
    base: SCI_BASE,
    f: SCI_LAYER_F,
    g: SCI_LAYER_G,
    apply_defaults: defaults_sci,
};

// The 15C's f-layer = the scientific gold layer, but its empty row 1 (over
// ASIN..EXP10) becomes the **matrix row**: f+ASIN=MDIM (dimension + open fill),
// f+ACOS=MSTO (store next cell), f+ATAN=DET, f+LOG=transpose, f+e^x=1/M
// (inverse), f+10^x=M/ (A⁻¹B solve). The rest matches SCI (hyperbolics, COMPLEX,
// R<>P, PREC, PI, LSTx, STATUS, FLOAT).
pub const C15_LAYER_F: [[Key; COLS]; ROWS] = [
    [Sinh,   Cosh,   Tanh,  Prec,      Sq,    CplxDisp, Complex, Pi,   LastX, Status],
    [MatNew, MatSet, Det,   Transpose, Minv,  Matsolve, Nop,     Nop,  Nop,   Nop],
    [Lr,     Yhat,   Corr,  ClStat,    Nop,   Nop,      Nop,     Nop,  Nop,   Nop],
    [Nop,    Nop,    Nop,   Nop,       Nop,   Float,    Nop,     Nop,  Nop,   Nop],
    [ShiftF, ShiftG, ClrReg, Nop,      RollUp, Nop,     Off,     Nop,  Eex,   Nop],
];

// The 15C wears the scientific base + its own f-layer (with f+I COMPLEX, f+P
// R<>P, and the matrix row),
// but its **g-layer is the complex faceplate**: the part ops the 15C is known for
// live on the top row over SIN/COS/TAN/LN/√ — g+SIN=Re, g+COS=Im, g+TAN=Re<>Im,
// g+LN=CONJ, g+√=ARG — plus the coordinate conversions g+y^x=→P, g+1/x=→R. Stats
// stay on g-row 2, SETUP on g-CLx. This sets the 15C apart from the plain SCI.
pub const C15_LAYER_G: [[Key; COLS]; ROWS] = [
    [Re,     Im,     ReIm, Conj, Arg,      ToPolar, ToRect, Nop, Nop,  Setup],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
    [Ncr,    Npr,    Ran,  Seed, WinL,     WinR, Nop,    Nop,  Round, Nop],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
    [ShiftF, ShiftG, Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
];

/// HP-15C "Advanced Scientific" — the classic complex-capable RPN, and the one
/// that actually had complex (via flag 8). Scientific base + f-layer, plus its
/// own **complex g-layer** ([`C15_LAYER_G`]); complex results ON by default.
/// Matrices / SOLVE / ∫ are future work.
pub static C15: Keymap = Keymap {
    name: "15C",
    base: SCI_BASE,
    f: C15_LAYER_F,
    g: C15_LAYER_G,
    apply_defaults: defaults_15c,
};

// ---- FIN personality (HP-12C-flavored financial) -----------------------------
// Same invariants as SCI: digits, ENTER, shifts, arithmetic, STATUS (f-CLx)
// and SETUP (g-CLx) at the 16C positions. The 12C's famous TVM row lands on
// the hex-digit row; cash flows below it; f = 12×/12÷ + dates + depreciation;
// g = BEG/END + CLCF. Sci row 0 stays — a desk calculator keeps its math.

pub const FIN_BASE: [[Key; COLS]; ROWS] = [
    [Sin,    Cos,    Tan,  Ln,   Sqrt,     Pow,  Recip,  Eex,  Back, ClrX],
    [TvmN,   TvmI,   TvmPv,TvmPmt,TvmFv,   Pct,  d(7),   d(8), d(9),  Div],
    [Cf0,    Cfj,    NjKey,Npv,  Irr,      PctChg, d(4), d(5), d(6),  Mul],
    [Fix,    Sci,    Eng,  FmtAuto, PctT,  Swap, d(1),   d(2), d(3),  Sub],
    [ShiftF, ShiftG, Sto,  Rcl,  RollDn,   Enter,d(0),   Dot,  Chs,   Add],
];

pub const FIN_LAYER_F: [[Key; COLS]; ROWS] = [
    [Sinh,   Cosh,   Tanh, Prec, Sq,       Nop,  Nop,    Pi,   LastX, Status],
    [X12Mul, X12Div, Nop,  Nop,  Nop,      Nop,  DepSl,  DepSoyd, DepDb, Nop],
    [Ddays,  DateAdd,Dow,  Wmean, Nop,     Nop,  Nop,    Nop,  Nop,   Nop],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Float,Nop,    Nop,  Nop,   Nop],
    [ShiftF, ShiftG, ClrReg, ClFin, RollUp, Nop, Off,    Nop,  Eex,   Nop],
];

pub const FIN_LAYER_G: [[Key; COLS]; ROWS] = [
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Setup],
    [Nop,    Nop,    Nop,  BegKey, EndKey, Nop,  Nop,    Nop,  Nop,   Nop],
    [ClCf,   Nop,    Nop,  Nop,  WinL,     WinR, Fact,   Nop,  Round, Nop],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
    [ShiftF, ShiftG, Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
];

fn defaults_15c(c: &mut crate::calc::Calc) {
    // HP-15C "Advanced Scientific": a FLOAT machine with complex. Radians (the
    // mathematician's default), FIX 4, decimal, complex results on.
    c.set_angle_mode(crate::calc::AngleMode::Rad);
    c.set_float_fmt(crate::calc::FloatFmt::Fix(4));
    c.set_radix(crate::calc::Radix::Dec);
    c.set_real_entry(true);
    c.set_cpxres(true);
}

fn defaults_fin(c: &mut crate::calc::Calc) {
    // 12C conventions: FIX 2, decimal, float machine (angle left alone).
    c.set_float_fmt(crate::calc::FloatFmt::Fix(2));
    c.set_radix(crate::calc::Radix::Dec);
    c.set_real_entry(true);
    c.set_cpxres(true);
}

/// The financial personality (12C-flavored; bonds deferred — see
/// DESIGN-MODES.md §4.3).
pub static FIN: Keymap = Keymap {
    name: "FIN",
    base: FIN_BASE,
    f: FIN_LAYER_F,
    g: FIN_LAYER_G,
    apply_defaults: defaults_fin,
};

/// Installed personalities, in `PErS`-menu cycle order.
pub static PERSONALITIES: &[&Keymap] = &[&HP16C, &C15, &SCI, &FIN];

/// Pending shift modifier (f = gold, g = blue). Press toggles; any resolved key
/// clears it (HP-Voyager behaviour).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Shift {
    #[default]
    None,
    F,
    G,
}

impl Shift {
    /// Resolve a physical `(row, col)` press through the active layer of the
    /// given keymap, updating the pending shift. `None` for shift keys
    /// themselves and unassigned cells.
    pub fn resolve(&mut self, km: &Keymap, row: usize, col: usize) -> Option<Key> {
        let k = match *self {
            Shift::None => km.base[row][col],
            Shift::F => km.f[row][col],
            Shift::G => km.g[row][col],
        };
        match k {
            ShiftF => { *self = if *self == Shift::F { Shift::None } else { Shift::F }; None }
            ShiftG => { *self = if *self == Shift::G { Shift::None } else { Shift::G }; None }
            Nop => { *self = Shift::None; None }
            other => { *self = Shift::None; Some(other) }
        }
    }
}
