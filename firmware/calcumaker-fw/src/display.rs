//! Multi-row 7-segment display — the live RPN stack (T / Z / Y / X).
//!
//! Skeleton — the driver (MAX7219 over SPI, or HT16K33 over I²C) and the digit
//! geometry are pinned after part selection (see ../../DESIGN.md → Display).

use crate::rpn::Stack;

/// Number of stack rows shown simultaneously (T, Z, Y, X).
pub const ROWS: usize = 4;

/// Digits per row (mantissa + sign + exponent field). TBD with the panel.
pub const DIGITS_PER_ROW: usize = 16;

pub struct Display {
    // TODO(mcu): driver handle (SPI/I2C peripheral + chip-selects).
}

impl Display {
    pub fn new() -> Self {
        Self {}
    }

    /// Render the live RPN stack across the rows. Arbitrary-precision values
    /// that exceed `DIGITS_PER_ROW` are windowed/scrolled (policy TBD).
    pub fn render(&mut self, stack: &Stack) {
        for (_row, value) in stack.top(ROWS).enumerate() {
            let _text = value.format(stack.radix);
            // TODO(mcu): map `_text` to 7-seg patterns and push to the driver.
        }
    }
}
