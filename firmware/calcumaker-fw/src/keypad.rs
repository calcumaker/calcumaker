//! Cherry MX key-matrix scanning (wide, HP-16C-style technical layout).
//!
//! Skeleton — the GPIO row/column pins, debounce, and the full key map are
//! pinned after MCU selection + front-panel layout (see ../../DESIGN.md). Each
//! key has a series diode for n-key rollover.

/// A decoded, debounced key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Digit/nibble for entry: 0..=15 (0-9, A-F) honoring the active radix.
    Digit(u8),
    /// Push / duplicate (RPN ENTER).
    Enter,
    /// Arithmetic operator.
    Op(Op),
    /// Function key (scientific / programmer).
    Func(Func),
    // TODO: full key set — radix (HEX/DEC/OCT/BIN), word-size select, bitwise
    // (AND/OR/XOR/NOT/shifts/rotates), stack ops (SWAP/ROLL/DROP), precision.
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

/// Function keys — the programmer (GMP) and scientific (MPFR) operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Func {
    // bitwise / programmer
    And,
    Or,
    Xor,
    Not,
    Shl,
    Shr,
    // scientific / MPFR transcendentals
    Sin,
    Cos,
    Tan,
    Exp,
    Ln,
    Sqrt,
}

pub struct Keypad {
    // TODO(mcu): row/column GPIOs, debounce state.
}

impl Keypad {
    pub fn new() -> Self {
        Self {}
    }

    /// Scan the matrix once; return a debounced key event if one is ready.
    pub fn scan(&mut self) -> Option<Key> {
        // TODO(mcu): drive rows, read columns, debounce, decode via the key map.
        None
    }
}
