//! calcumaker display-module wire protocol — "display intent" over SPI.
//!
//! The calculator's main MCU (STM32U575, `calcumaker-fw`) is the SPI **master**;
//! each display board carries its own MCU (STM32G031 on the 7-seg module,
//! RP2040 on the RGB-matrix module) that is the SPI **slave**. Rather than push
//! raw driver bytes, the master writes a small, display-agnostic *intent* frame
//! — text rows + annunciator/mode flags + aux-OLED content — and each module
//! renders it natively (7-seg glyphs vs. an RGB pixel framebuffer). One frame,
//! any display; a new display is a new module with no MCU-board change.
//!
//! This crate is pure `no_std` with **no dependencies** and is shared by the
//! sender and both module firmwares, so the frame is defined exactly once.
//!
//! Wire format (fixed length [`WIRE_LEN`], little-endian, so a slave can DMA a
//! constant-size transfer):
//! ```text
//!   0      MAGIC0 (0xCA)
//!   1      MAGIC1 (0x1C)
//!   2      VERSION
//!   3      seq            (frame counter; wraps)
//!   4..5   flags          (u16 LE — see F_*)
//!   6      row_len[0..MAX_ROWS]        (MAX_ROWS bytes)
//!   ..     rows[MAX_ROWS][MAX_COLS]    ASCII, space-padded
//!   ..     aux[AUX_LINES][AUX_COLS]    ASCII (OLED), space-padded
//!   last   XOR checksum of all preceding bytes
//! ```
#![no_std]
#![forbid(unsafe_code)]

/// Stack rows carried in a frame (matches `core::seg7::DISPLAY_ROWS`).
pub const MAX_ROWS: usize = 3;
/// Max glyph columns per row. 7-seg uses 16; the matrix packs ~16 at 6px/char in
/// 96px, and can scroll wider. 24 leaves headroom without bloating the frame.
pub const MAX_COLS: usize = 24;
/// Aux OLED geometry (128x32 @ 6x8 font), matching `core::App::aux_lines()`.
pub const AUX_LINES: usize = 4;
pub const AUX_COLS: usize = 21;

pub const MAGIC0: u8 = 0xCA;
pub const MAGIC1: u8 = 0x1C;
pub const VERSION: u8 = 1;

// --- annunciator / mode flags (bit positions in `DisplayFrame::flags`) -------
pub const F_SHIFT_F: u16 = 1 << 0; // 'f' (gold) shift pending
pub const F_SHIFT_G: u16 = 1 << 1; // 'g' (blue) shift pending
pub const F_CARRY: u16 = 1 << 2; // C — carry/borrow
pub const F_OVERFLOW: u16 = 1 << 3; // G — out-of-range / shifted-out bit
pub const F_LOWBAT: u16 = 1 << 4; // low battery
pub const F_LEADZERO: u16 = 1 << 5; // leading-zero display (16C flag 3)
pub const F_INT: u16 = 1 << 6; // integer number-mode
pub const F_REAL: u16 = 1 << 7; // real number-mode
pub const F_ERROR: u16 = 1 << 8; // an error is being shown

const HDR: usize = 6; // magic0, magic1, version, seq, flags(2)
const ROWLEN: usize = MAX_ROWS;
const TEXT: usize = MAX_ROWS * MAX_COLS;
const AUX: usize = AUX_LINES * AUX_COLS;
/// Total on-wire frame length (header + row lengths + text + aux + checksum).
pub const WIRE_LEN: usize = HDR + ROWLEN + TEXT + AUX + 1;

/// One display frame — "what to show", display-agnostic.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DisplayFrame {
    /// ASCII text per row, space-padded to `MAX_COLS`. Index 0 = top row.
    pub rows: [[u8; MAX_COLS]; MAX_ROWS],
    /// Significant length of each row (<= MAX_COLS).
    pub row_len: [u8; MAX_ROWS],
    /// Annunciator / mode bits (see `F_*`).
    pub flags: u16,
    /// Aux OLED content (space-padded); ignored by modules without an OLED.
    pub aux: [[u8; AUX_COLS]; AUX_LINES],
    /// Frame sequence counter (lets a module detect drops / no-change).
    pub seq: u8,
}

impl Default for DisplayFrame {
    fn default() -> Self {
        Self {
            rows: [[b' '; MAX_COLS]; MAX_ROWS],
            row_len: [0; MAX_ROWS],
            flags: 0,
            aux: [[b' '; AUX_COLS]; AUX_LINES],
            seq: 0,
        }
    }
}

impl DisplayFrame {
    pub const fn new() -> Self {
        Self {
            rows: [[b' '; MAX_COLS]; MAX_ROWS],
            row_len: [0; MAX_ROWS],
            flags: 0,
            aux: [[b' '; AUX_COLS]; AUX_LINES],
            seq: 0,
        }
    }

    /// Set row `r` from an ASCII string (truncated to `MAX_COLS`, space-padded).
    pub fn set_row(&mut self, r: usize, text: &str) {
        if r >= MAX_ROWS {
            return;
        }
        let mut n = 0;
        self.rows[r] = [b' '; MAX_COLS];
        for (i, b) in text.bytes().take(MAX_COLS).enumerate() {
            self.rows[r][i] = b;
            n = i + 1;
        }
        self.row_len[r] = n as u8;
    }

    /// The significant text of row `r` (no trailing pad) as bytes.
    pub fn row(&self, r: usize) -> &[u8] {
        if r >= MAX_ROWS {
            return &[];
        }
        let n = (self.row_len[r] as usize).min(MAX_COLS);
        &self.rows[r][..n]
    }

    pub fn has(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    /// Serialize to the fixed-size wire buffer. Returns [`WIRE_LEN`].
    pub fn encode(&self, buf: &mut [u8; WIRE_LEN]) {
        buf[0] = MAGIC0;
        buf[1] = MAGIC1;
        buf[2] = VERSION;
        buf[3] = self.seq;
        buf[4] = (self.flags & 0xFF) as u8;
        buf[5] = (self.flags >> 8) as u8;
        let mut p = HDR;
        for r in 0..MAX_ROWS {
            buf[p] = self.row_len[r];
            p += 1;
        }
        for r in 0..MAX_ROWS {
            for c in 0..MAX_COLS {
                buf[p] = self.rows[r][c];
                p += 1;
            }
        }
        for l in 0..AUX_LINES {
            for c in 0..AUX_COLS {
                buf[p] = self.aux[l][c];
                p += 1;
            }
        }
        let mut ck = 0u8;
        for b in &buf[..WIRE_LEN - 1] {
            ck ^= *b;
        }
        buf[WIRE_LEN - 1] = ck;
    }

    /// Parse a wire buffer. Returns `None` on bad magic/version/checksum.
    pub fn decode(buf: &[u8; WIRE_LEN]) -> Option<Self> {
        if buf[0] != MAGIC0 || buf[1] != MAGIC1 || buf[2] != VERSION {
            return None;
        }
        let mut ck = 0u8;
        for b in &buf[..WIRE_LEN - 1] {
            ck ^= *b;
        }
        if ck != buf[WIRE_LEN - 1] {
            return None;
        }
        let mut f = DisplayFrame::new();
        f.seq = buf[3];
        f.flags = (buf[4] as u16) | ((buf[5] as u16) << 8);
        let mut p = HDR;
        for r in 0..MAX_ROWS {
            f.row_len[r] = buf[p];
            p += 1;
        }
        for r in 0..MAX_ROWS {
            for c in 0..MAX_COLS {
                f.rows[r][c] = buf[p];
                p += 1;
            }
        }
        for l in 0..AUX_LINES {
            for c in 0..AUX_COLS {
                f.aux[l][c] = buf[p];
                p += 1;
            }
        }
        Some(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut f = DisplayFrame::new();
        f.set_row(0, "3.14159265");
        f.set_row(1, "DEADBEEF h");
        f.set_row(2, "");
        f.flags = F_SHIFT_F | F_CARRY | F_INT;
        f.seq = 42;
        let mut buf = [0u8; WIRE_LEN];
        f.encode(&mut buf);
        let g = DisplayFrame::decode(&buf).expect("decode");
        assert_eq!(f, g);
        assert_eq!(g.row(0), b"3.14159265");
        assert_eq!(g.row(1), b"DEADBEEF h");
        assert_eq!(g.row(2), b"");
        assert!(g.has(F_SHIFT_F) && g.has(F_CARRY) && g.has(F_INT));
        assert!(!g.has(F_SHIFT_G));
        assert_eq!(g.seq, 42);
    }

    #[test]
    fn rejects_corruption() {
        let f = DisplayFrame::new();
        let mut buf = [0u8; WIRE_LEN];
        f.encode(&mut buf);
        buf[10] ^= 0xFF; // flip a payload byte -> checksum fails
        assert!(DisplayFrame::decode(&buf).is_none());
        f.encode(&mut buf);
        buf[0] = 0x00; // bad magic
        assert!(DisplayFrame::decode(&buf).is_none());
    }

    #[test]
    fn truncates_and_pads() {
        let mut f = DisplayFrame::new();
        let long = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"; // > MAX_COLS
        f.set_row(0, long);
        assert_eq!(f.row(0).len(), MAX_COLS);
        assert_eq!(f.row(0), &long.as_bytes()[..MAX_COLS]);
    }
}
