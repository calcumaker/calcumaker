//! Cherry MX key-matrix: wide HP-16C-style layout, 5 rows x 10 cols = 50 keys,
//! with HP-Voyager-style f (gold) / g (blue) shifts (3 functions per key).
//!
//! Electrical: ROWr = GPIO out, COLc = GPIO in w/ internal pull-up, one diode
//! per key (anode at switch, cathode to COL) for n-key rollover. Scan drives one
//! row low and reads columns; in Stop, all rows low + EXTI on a column wakes on
//! any press. Power is a slide switch (not in the matrix). See ../../DESIGN.md.
//!
//! The matrix scan (GPIO + debounce) is wired after MCU bring-up; the keymap
//! below is the design source of truth (mirror it in DESIGN.md).

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
    And, Or, Xor, Not, Shl, Shr, Asr, Rotl, Rotr,
    BitSet, BitClr, BitTest, MaskL, MaskR, BitCount, Rmd,
    // scientific (MPFR)
    Sin, Cos, Tan, Asin, Acos, Atan, Sinh, Cosh, Tanh,
    Ln, Exp, Log10, Exp10, Sqrt, Sq, Pow, Recip, Pi, Fact, Pct, Round,
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
    [BitSet, BitClr, BitTest, MaskL, MaskR, BitCount, Nop, Nop, Nop, Nop],
    [Rotl,   Rotr,   Asr,  Rmd,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
    [Float,  Float,  Float,Float,SignMode, RollUp, Nop,  Nop,  Nop,   Nop],
    [ShiftF, ShiftG, Nop,  Nop,  RollUp,   Nop,  Off,    Nop,  Eex,   Nop],
];

/// g (blue) layer — hyperbolic / secondary.
pub const LAYER_G: [[Key; COLS]; ROWS] = [
    [Sinh,   Cosh,   Tanh, Log10,Exp10,    Nop,  Nop,    Nop,  Nop,   Nop],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Fact,   Pct,  Round, Nop],
    [Nop,    Nop,    Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
    [ShiftF, ShiftG, Nop,  Nop,  Nop,      Nop,  Nop,    Nop,  Nop,   Nop],
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mod { None, F, G }

pub struct Keypad {
    pending: Mod,
    // TODO(mcu): row/col GPIOs + debounce state.
}

impl Keypad {
    pub fn new() -> Self {
        Self { pending: Mod::None }
    }

    /// Resolve a physical (row, col) press through the active shift layer.
    fn resolve(&mut self, row: usize, col: usize) -> Option<Key> {
        let k = match self.pending {
            Mod::None => BASE[row][col],
            Mod::F => LAYER_F[row][col],
            Mod::G => LAYER_G[row][col],
        };
        match k {
            ShiftF => { self.pending = if self.pending == Mod::F { Mod::None } else { Mod::F }; None }
            ShiftG => { self.pending = if self.pending == Mod::G { Mod::None } else { Mod::G }; None }
            Nop => { self.pending = Mod::None; None }
            other => { self.pending = Mod::None; Some(other) }
        }
    }

    /// Scan the matrix once; return a debounced, shift-resolved key event.
    pub fn scan(&mut self) -> Option<Key> {
        // TODO(mcu): drive each ROW low, read COLs (internal pull-ups), debounce;
        // on a stable press call self.resolve(row, col).
        let _ = self.resolve as fn(&mut Self, usize, usize) -> Option<Key>;
        None
    }
}
