//! Minimal 7-segment font: ASCII → TM1640 SEG1..8 byte (bit0=a … bit6=g, bit7=dp).
//!
//! Mirrors `calcumaker_core::seg7` but is duplicated here so this module needn't
//! link the GMP/MPFR-bearing engine crate (no bignum on the display module).
//! TODO: split `core::seg7` into a shared `no_std` crate and depend on that so
//! there is ONE glyph source of truth.

use calcumaker_display_proto::MAX_COLS;

pub const DIGITS_PER_ROW: usize = 16;
pub const DP: u8 = 0x80;

/// Encode one character to a segment byte, or `None` if unrenderable.
pub fn encode(c: char) -> Option<u8> {
    Some(match c.to_ascii_uppercase() {
        '0' => 0x3F, '1' => 0x06, '2' => 0x5B, '3' => 0x4F, '4' => 0x66,
        '5' => 0x6D, '6' => 0x7D, '7' => 0x07, '8' => 0x7F, '9' => 0x6F,
        'A' => 0x77, 'B' => 0x7C, 'C' => 0x39, 'D' => 0x5E, 'E' => 0x79, 'F' => 0x71,
        'G' => 0x3D, 'H' => 0x76, 'I' => 0x06, 'J' => 0x1E, 'L' => 0x38, 'N' => 0x54,
        'O' => 0x5C, 'P' => 0x73, 'R' => 0x50, 'S' => 0x6D, 'T' => 0x78, 'U' => 0x3E,
        'Y' => 0x6E,
        ' ' => 0x00, '-' => 0x40, '_' => 0x08, '=' => 0x48, '.' => DP,
        _ => return None,
    })
}

/// Encode a row of ASCII (from a `DisplayFrame`) to 16 TM1640 cells, left-aligned
/// and space-padded. A '.' folds into the previous cell's decimal point.
pub fn encode_row(text: &[u8]) -> [u8; DIGITS_PER_ROW] {
    let mut cells = [0u8; DIGITS_PER_ROW];
    let mut i = 0usize;
    for &b in text.iter().take(MAX_COLS) {
        if i >= DIGITS_PER_ROW {
            break;
        }
        let ch = b as char;
        if ch == '.' && i > 0 {
            cells[i - 1] |= DP; // fold onto the previous glyph
            continue;
        }
        cells[i] = encode(ch).unwrap_or(0x40); // unrenderable -> '-'
        i += 1;
    }
    cells
}
