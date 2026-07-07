//! Multi-row 7-segment display — the TM1640 bus half of the display.
//!
//! Driver: 3× TM1640 (one per row), each driving 16 common-cathode digits over a
//! 2-wire bus (shared CLK + per-row DIN1/2/3), across the board-to-board
//! interconnect to the display PCB. See ../../DESIGN.md → Display.
//!
//! The **content pipeline lives in `calcumaker_core`**: `App::seg_rows()`
//! produces the per-digit segment bytes (`seg7` bit layout = TM1640 SEG1..8 =
//! a..g + dp) that this driver pushes to the chips verbatim — the emulator
//! renders the same bytes. Skeleton — bit-bang/timing wired after MCU bring-up.

// Placeholder until the TM1640 bus is wired (post-USB bring-up).
#![allow(dead_code)]

/// Rows shown at once. Board is laid out for 3; set to 2 for the 2-row build
/// (top row unpopulated). Must match `calcumaker_core::seg7::DISPLAY_ROWS`.
pub const ROWS: usize = 3;

/// Digit cells per row (one TM1640 = 16 digits). Must match
/// `calcumaker_core::seg7::DIGITS_PER_ROW`.
pub const DIGITS_PER_ROW: usize = 16;

pub struct Display {
    // TODO(mcu): per-row TM1640 handles (shared CLK GPIO + DIN1/2/3 GPIOs).
}

impl Display {
    pub fn new() -> Self {
        Self {}
    }

    /// Push the segment bytes for every row (index 0 = top) to the TM1640s —
    /// the same bytes `calcumaker_core::App::seg_rows()` returns.
    pub fn render(&mut self, _rows: &[[u8; DIGITS_PER_ROW]; ROWS]) {
        // TODO(mcu): per row: START, address auto-increment command, 16 data
        // bytes on CLK + that row's DIN, brightness/display-on. (TM1640 has no
        // ACK — pure 2-wire push.)
    }
}
