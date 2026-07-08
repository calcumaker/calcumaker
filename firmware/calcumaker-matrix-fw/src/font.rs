//! 96×24 RGB framebuffer + a tiny 5×7 font blitter for the dot-matrix.
//!
//! Geometry matches the `calcumaker-matrix` board: 96 wide × 24 tall = 2304 px,
//! laid out as 3 stack rows of 8 px. The array is split into 3 data chains of
//! 768 px (one per stack row) at flush time.

use calcumaker_display_proto::MAX_COLS;

pub const COLS: usize = 96;
pub const ROWS: usize = 24; // 3 stack rows × 8 px
pub const NPX: usize = COLS * ROWS; // 2304
pub const CHAINS: usize = 3;
pub const PER_CHAIN: usize = NPX / CHAINS; // 768

/// A pixel in the framebuffer's logical order (R, G, B); WS2812 GRB reorder
/// happens in the driver at flush.
pub type Rgb = [u8; 3];

pub struct Frame {
    pub px: [Rgb; NPX],
}

impl Frame {
    pub const fn new() -> Self {
        Self { px: [[0, 0, 0]; NPX] }
    }

    pub fn clear(&mut self) {
        self.px = [[0, 0, 0]; NPX];
    }

    #[inline]
    fn idx(x: usize, y: usize) -> usize {
        y * COLS + x // simple row-major; physical serpentine handled in the driver
    }

    pub fn set(&mut self, x: usize, y: usize, c: Rgb) {
        if x < COLS && y < ROWS {
            self.px[Self::idx(x, y)] = c;
        }
    }

    /// Blit an ASCII string at stack-row `row` (0..3, each 8 px tall), starting at
    /// column `x0`, in colour `c`. 5×7 glyphs on a 6 px pitch.
    pub fn text(&mut self, x0: usize, row: usize, s: &[u8], c: Rgb) {
        let mut x = x0;
        for &b in s.iter().take(MAX_COLS) {
            let g = glyph(b);
            for (col, bits) in g.iter().enumerate() {
                for ry in 0..7usize {
                    if bits & (1 << ry) != 0 {
                        self.set(x + col, row * 8 + ry, c);
                    }
                }
            }
            x += 6;
            if x >= COLS {
                break;
            }
        }
    }
}

/// One glyph = 5 columns, each a 7-bit column bitmap (bit0 = top row).
///
/// SCAFFOLD FONT: real digits/hex below; everything else falls back to a hollow
/// box so unrendered text is visible. TODO: a full 5×7 ASCII font (share with a
/// future `core::seg7`/glyph split).
fn glyph(b: u8) -> [u8; 5] {
    match b.to_ascii_uppercase() {
        b' ' => [0x00, 0x00, 0x00, 0x00, 0x00],
        b'0' => [0x3E, 0x51, 0x49, 0x45, 0x3E],
        b'1' => [0x00, 0x42, 0x7F, 0x40, 0x00],
        b'2' => [0x42, 0x61, 0x51, 0x49, 0x46],
        b'3' => [0x21, 0x41, 0x45, 0x4B, 0x31],
        b'4' => [0x18, 0x14, 0x12, 0x7F, 0x10],
        b'5' => [0x27, 0x45, 0x45, 0x45, 0x39],
        b'6' => [0x3C, 0x4A, 0x49, 0x49, 0x30],
        b'7' => [0x01, 0x71, 0x09, 0x05, 0x03],
        b'8' => [0x36, 0x49, 0x49, 0x49, 0x36],
        b'9' => [0x06, 0x49, 0x49, 0x29, 0x1E],
        b'A' => [0x7E, 0x11, 0x11, 0x11, 0x7E],
        b'B' => [0x7F, 0x49, 0x49, 0x49, 0x36],
        b'C' => [0x3E, 0x41, 0x41, 0x41, 0x22],
        b'D' => [0x7F, 0x41, 0x41, 0x22, 0x1C],
        b'E' => [0x7F, 0x49, 0x49, 0x49, 0x41],
        b'F' => [0x7F, 0x09, 0x09, 0x09, 0x01],
        b'-' => [0x08, 0x08, 0x08, 0x08, 0x08],
        b'.' => [0x00, 0x40, 0x40, 0x00, 0x00],
        _ => [0x7F, 0x41, 0x41, 0x41, 0x7F], // fallback: hollow box
    }
}
