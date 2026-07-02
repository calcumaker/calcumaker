//! 7-segment encoding — text to the per-digit segment bytes the display shows.
//!
//! Bit layout matches the TM1640 SEG1..SEG8 outputs wired a..g + dp:
//! bit0 = a (top), bit1 = b, bit2 = c, bit3 = d (bottom), bit4 = e, bit5 = f,
//! bit6 = g (middle), bit7 = dp. A `.` in the text folds into the previous
//! digit's dp bit (standard 7-seg practice), so `1.5` occupies two digit cells.
//!
//! Both frontends consume these bytes: the firmware pushes them to the TM1640
//! chain verbatim, the emulator renders them as ASCII art — so the emulator
//! shows exactly what the hardware would.

/// Display rows the board is laid out for (top row optional on the 2-row build).
pub const DISPLAY_ROWS: usize = 3;

/// Digit cells per row (one TM1640 = 16 digits; fits a 64-bit hex word, or a
/// signed mantissa + exponent).
pub const DIGITS_PER_ROW: usize = 16;

/// Decimal-point bit, folded into the preceding digit cell.
pub const DP: u8 = 0x80;

/// Rightmost-cell marker when a value doesn't fit the row (renders `]`-like).
pub const OVERFLOW: u8 = 0x0F;

/// Segment pattern for one character; `None` if it has no 7-seg rendering.
/// Letters map case-insensitively to their canonical 7-seg shape (hex digits
/// display as `AbCdEF`).
pub const fn encode(c: char) -> Option<u8> {
    // bits: 0bdp_g_f_e_d_c_b_a
    Some(match c {
        '0' => 0x3F,
        '1' => 0x06,
        '2' => 0x5B,
        '3' => 0x4F,
        '4' => 0x66,
        '5' | 'S' | 's' => 0x6D,
        '6' => 0x7D,
        '7' => 0x07,
        '8' => 0x7F,
        '9' => 0x6F,
        'A' | 'a' => 0x77,
        'B' | 'b' => 0x7C,
        'C' => 0x39,
        'c' => 0x58,
        'D' | 'd' => 0x5E,
        'E' | 'e' => 0x79,
        'F' | 'f' => 0x71,
        'G' | 'g' => 0x3D,
        'H' => 0x76,
        'h' => 0x74,
        'L' | 'l' => 0x38,
        'n' | 'N' => 0x54,
        'o' | 'O' => 0x5C,
        'P' | 'p' => 0x73,
        'r' | 'R' => 0x50,
        't' | 'T' => 0x78,
        'U' => 0x3E,
        'u' => 0x1C,
        'y' | 'Y' => 0x6E,
        '-' => 0x40,
        '_' => 0x08,
        ' ' => 0x00,
        '>' => OVERFLOW,
        _ => return None,
    })
}

/// Encode one display line: right-aligned into `DIGITS_PER_ROW` cells, `.`
/// folded into the preceding cell's dp, unknown characters blanked. Text wider
/// than the row is truncated with [`OVERFLOW`] in the last cell (windowing /
/// scrolling is a later, app-level policy).
pub fn encode_row(text: &str) -> [u8; DIGITS_PER_ROW] {
    let mut cells = alloc::vec::Vec::with_capacity(text.len());
    for ch in text.chars() {
        if ch == '.' {
            match cells.last_mut() {
                // A digit can carry one dp; a second consecutive dot gets its
                // own cell.
                Some(last) if *last & DP == 0 => *last |= DP,
                _ => cells.push(DP),
            }
        } else {
            cells.push(encode(ch).unwrap_or(0x00));
        }
    }
    if cells.len() > DIGITS_PER_ROW {
        cells.truncate(DIGITS_PER_ROW - 1);
        cells.push(OVERFLOW);
    }
    let mut row = [0u8; DIGITS_PER_ROW];
    row[DIGITS_PER_ROW - cells.len()..].copy_from_slice(&cells);
    row
}
