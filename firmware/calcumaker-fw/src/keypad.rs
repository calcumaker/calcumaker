//! Cherry MX key-matrix scan — the hardware half of the keypad.
//!
//! Electrical: ROWr = GPIO out, COLc = GPIO in w/ internal pull-up, one diode
//! per key (anode at switch, cathode to COL) for n-key rollover. Scan drives one
//! row low and reads columns; in Stop, all rows low + EXTI on a column wakes on
//! any press. Power is a slide switch (not in the matrix). See ../../DESIGN.md.
//!
//! The **keymap and f/g shift layers live in `calcumaker_core::keys`** (the
//! design source of truth, shared with the emulator); this module only turns
//! electrons into `(row, col)` presses, which `calcumaker_core::App::press`
//! resolves and applies. Wired after MCU bring-up.

/// Matrix dimensions — must match `calcumaker_core::keys::{ROWS, COLS}`.
/// (Consumed by the matrix scan once the GPIOs are wired.)
#[allow(dead_code)]
pub const ROWS: usize = 5;
#[allow(dead_code)]
pub const COLS: usize = 10;

pub struct Keypad {
    // TODO(mcu): row/col GPIOs + debounce state.
}

impl Keypad {
    pub fn new() -> Self {
        Self {}
    }

    /// Scan the matrix once; return a debounced press as a raw matrix position
    /// (for `calcumaker_core::App::press`).
    pub fn scan(&mut self) -> Option<(usize, usize)> {
        // TODO(mcu): drive each ROW low, read COLs (internal pull-ups),
        // debounce; report the stable (row, col).
        None
    }
}
