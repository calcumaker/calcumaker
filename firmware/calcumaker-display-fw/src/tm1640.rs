//! TM1640 bit-bang driver — 3 chips sharing one CLK, one DIN each.
//!
//! The TM1640 is a common-cathode LED driver: 16 GRID (digit) × 8 SEG. It has NO
//! address/chip-select and NO DOUT, so it can't cascade — each chip needs its own
//! DIN (shared SCLK). This module owns the shared CLK + the 3 per-row DIN lines
//! (driven at 5 V through the on-board 74HCT125, see the DispPower sheet) and
//! writes a whole 16-digit row at a time.
//!
//! Protocol (verify timing on hardware): a "data command" byte 0x40 (write, auto
//! address-increment), then an "address+data" burst 0xC0 (start at grid 0) + 16
//! data bytes. Bytes are clocked **LSB first**; each burst is framed by a
//! start (DIN 1→0 while CLK high) and stop (DIN 0→1 while CLK high) condition.

use embassy_stm32::gpio::Output;

/// Half-bit-cell delay in CPU cycles (~0.5 µs @ 64 MHz). TM1640 f_max ≈ 1 MHz.
const HALF: u32 = 32;

fn dly() {
    cortex_m::asm::delay(HALF);
}

pub struct Display {
    clk: Output<'static>,
    din: [Output<'static>; 3],
}

impl Display {
    pub fn new(
        clk: Output<'static>,
        din1: Output<'static>,
        din2: Output<'static>,
        din3: Output<'static>,
    ) -> Self {
        Self { clk, din: [din1, din2, din3] }
    }

    fn start(&mut self, row: usize) {
        self.clk.set_high();
        self.din[row].set_high();
        dly();
        self.din[row].set_low(); // DIN 1->0 while CLK high = start
        dly();
        self.clk.set_low();
    }

    fn stop(&mut self, row: usize) {
        self.clk.set_low();
        self.din[row].set_low();
        dly();
        self.clk.set_high();
        dly();
        self.din[row].set_high(); // DIN 0->1 while CLK high = stop
        dly();
    }

    fn write_byte(&mut self, row: usize, mut b: u8) {
        for _ in 0..8 {
            self.clk.set_low();
            if b & 0x01 != 0 {
                self.din[row].set_high();
            } else {
                self.din[row].set_low();
            }
            b >>= 1; // LSB first
            dly();
            self.clk.set_high(); // latch on rising edge
            dly();
        }
        self.clk.set_low();
    }

    /// Write all 16 digit cells of one row (segment bytes, bit0=a … bit7=dp).
    pub fn write_row(&mut self, row: usize, cells: &[u8; 16]) {
        if row >= self.din.len() {
            return;
        }
        // 1) data command: write data, auto address-increment
        self.start(row);
        self.write_byte(row, 0x40);
        self.stop(row);
        // 2) address command (grid 0) + 16 data bytes
        self.start(row);
        self.write_byte(row, 0xC0);
        for &c in cells.iter() {
            self.write_byte(row, c);
        }
        self.stop(row);
    }

    /// Set global brightness (0..=7) / on. TODO: emit the display-control command
    /// 0x88|level once per (re)init.
    pub fn set_brightness(&mut self, _row: usize, _level: u8) {
        // 0x88 | (level & 7); framed by start/stop. Left as a bring-up TODO.
    }
}
