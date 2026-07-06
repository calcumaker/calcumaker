//! Keyboard-board event intake — the hardware half of key presses.
//!
//! The 50-key Cherry MX matrix, debounce, Stop/EXTI wake source, and annunciator
//! drive live on the keyboard board's STM32G0. The U575 receives debounced
//! `(row, col)` events over the mezzanine link and wakes on `KB_IRQ`. Power is a
//! slide switch (not in the matrix). See ../../DESIGN.md.
//!
//! The **keymap and f/g shift layers live in `calcumaker_core::keys`** (the
//! design source of truth, shared with the emulator); this module only turns
//! link frames into `(row, col)` presses, which `calcumaker_core::App::press`
//! resolves and applies. The transport is wired after MCU bring-up.

/// Matrix dimensions — must match `calcumaker_core::keys::{ROWS, COLS}`.
/// (Consumed by the keyboard-link adapter once the protocol is wired.)
#[allow(dead_code)]
pub const ROWS: usize = 5;
#[allow(dead_code)]
pub const COLS: usize = 10;

pub struct Keypad {
    // TODO(mcu): I2C/UART handle + event/debounce protocol state.
}

impl Keypad {
    pub fn new() -> Self {
        Self {}
    }

    /// Poll the keyboard link once; return a debounced press as a raw matrix position
    /// (for `calcumaker_core::App::press`).
    pub fn scan(&mut self) -> Option<(usize, usize)> {
        // TODO(mcu): read pending event(s) from the keyboard-board G0 over I2C
        // or UART; report the stable (row, col).
        None
    }
}
