//! Multi-row 7-segment display — the top of the RPN stack.
//!
//! Driver: 3× TM1640 (one per row), each driving 16 common-cathode digits over a
//! 2-wire bus (shared CLK + per-row DIN1/2/3), across the board-to-board
//! interconnect to the display PCB. See ../../DESIGN.md → Display.
//! Skeleton — the TM1640 bit-bang/timing is wired after the MCU bring-up.

/// Rows shown at once. Board is laid out for 3; set to 2 for the 2-row build
/// (top row unpopulated). See ../../DESIGN.md → Display.
pub const ROWS: usize = 3;

/// Digits per row (one TM1640 = 16 digits; fits a 64-bit hex word, or a signed
/// mantissa + exponent).
pub const DIGITS_PER_ROW: usize = 16;

pub struct Display {
    // TODO(mcu): per-row TM1640 handles (shared CLK GPIO + DIN1/2/3 GPIOs).
}

impl Display {
    pub fn new() -> Self {
        Self {}
    }

    /// Render the top stack rows (already formatted by the engine, top first).
    /// Arbitrary-precision values longer than `DIGITS_PER_ROW` are
    /// windowed/scrolled (policy TBD).
    pub fn render(&mut self, _rows: &[&str]) {
        // TODO(mcu): map each row's text to 7-seg patterns and push to the
        // TM1640 chain (CLK + per-row DIN).
    }
}
