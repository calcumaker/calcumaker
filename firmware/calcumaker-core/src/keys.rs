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
    Enter, Swap, RollDn, RollUp, LastX, Sto, Rcl,
    // base / word modes (programmer)
    Hex, Dec, Oct, Bin, WordSize, SignMode, Float,
    // bitwise / shift / rotate (programmer)
    And, Or, Xor, Not, Shl, Shr, Asr, Rotl, Rotr, Rlc, Rrc, Lj,
    BitSet, BitClr, BitTest, MaskL, MaskR, BitCount, Rmd,
    // scientific (MPFR)
    Sin, Cos, Tan, Asin, Acos, Atan, Sinh, Cosh, Tanh,
    Ln, Exp, Log10, Exp10, Sqrt, Sq, Pow, Recip, Pi, Fact, Pct, Round,
    // real display format (X = digit count; FmtAuto = %g-style)
    Fix, Sci, Eng, FmtAuto,
    // angle unit for circular trig (cycles RAD → DEG → GRAD)
    AngleMode,
    // leading-zeros display toggle (16C flag 3)
    Lz,
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
    [Asin,   Acos,   Atan, Exp,  Sq,       Nop,  Prec,   Pi,   LastX, Nop],
    [BitSet, BitClr, BitTest, MaskL, MaskR, BitCount, Lj, Nop, Nop, Nop],
    [Rotl,   Rotr,   Asr,  Rmd,  Rlc,      Rrc,  Nop,    Nop,  Nop,   Nop],
    [Float,  Float,  Float,Float,SignMode, RollUp, Nop,  Nop,  Nop,   Nop],
    [ShiftF, ShiftG, Nop,  Nop,  RollUp,   Nop,  Off,    Nop,  Eex,   Nop],
];

/// g (blue) layer — hyperbolic / secondary. (FIX/SCI/ENG/auto sit over the
/// radix keys — display format over display base; angle mode over WSIZE.)
pub const LAYER_G: [[Key; COLS]; ROWS] = [
    [Sinh,   Cosh,   Tanh, Log10,Exp10,    Nop,  Nop,    Nop,  Nop,   Nop],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
    [Lz,     Nop,    Nop,  Nop,  Nop,      Nop,  Fact,   Pct,  Round, Nop],
    [Fix,    Sci,    Eng,  FmtAuto, AngleMode, Nop, Nop, Nop,  Nop,   Nop],
    [ShiftF, ShiftG, Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
];

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
    /// Resolve a physical `(row, col)` press through the active layer, updating
    /// the pending shift. `None` for shift keys themselves and unassigned cells.
    pub fn resolve(&mut self, row: usize, col: usize) -> Option<Key> {
        let k = match *self {
            Shift::None => BASE[row][col],
            Shift::F => LAYER_F[row][col],
            Shift::G => LAYER_G[row][col],
        };
        match k {
            ShiftF => { *self = if *self == Shift::F { Shift::None } else { Shift::F }; None }
            ShiftG => { *self = if *self == Shift::G { Shift::None } else { Shift::G }; None }
            Nop => { *self = Shift::None; None }
            other => { *self = Shift::None; Some(other) }
        }
    }
}
