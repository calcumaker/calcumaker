//! 7-segment encoder tests — the byte patterns the TM1640s (and the emulator)
//! consume.

use calcumaker_core::seg7::{encode, encode_row, DIGITS_PER_ROW, DP, OVERFLOW};

#[test]
fn digit_patterns() {
    assert_eq!(encode('0'), Some(0x3F));
    assert_eq!(encode('1'), Some(0x06));
    assert_eq!(encode('8'), Some(0x7F));
    assert_eq!(encode('F'), Some(0x71));
    assert_eq!(encode('f'), Some(0x71)); // case-insensitive hex
    assert_eq!(encode('-'), Some(0x40));
    assert_eq!(encode(' '), Some(0x00));
    assert_eq!(encode('%'), None);
}

#[test]
fn row_right_aligns() {
    let row = encode_row("42");
    assert_eq!(&row[..DIGITS_PER_ROW - 2], &[0u8; DIGITS_PER_ROW - 2]);
    assert_eq!(row[DIGITS_PER_ROW - 2], 0x66); // 4
    assert_eq!(row[DIGITS_PER_ROW - 1], 0x5B); // 2
}

#[test]
fn dot_folds_into_previous_cell() {
    let row = encode_row("1.5");
    assert_eq!(row[DIGITS_PER_ROW - 2], 0x06 | DP); // 1.
    assert_eq!(row[DIGITS_PER_ROW - 1], 0x6D); // 5
}

#[test]
fn leading_dot_gets_own_cell() {
    let row = encode_row(".5");
    assert_eq!(row[DIGITS_PER_ROW - 2], DP);
    assert_eq!(row[DIGITS_PER_ROW - 1], 0x6D);
}

#[test]
fn exact_fit_is_not_truncated() {
    let row = encode_row("1234567812345678");
    assert_ne!(row[DIGITS_PER_ROW - 1], OVERFLOW);
    assert_eq!(row[0], 0x06); // leading 1
}

#[test]
fn too_long_gets_overflow_marker() {
    let row = encode_row("12345678123456789");
    assert_eq!(row[DIGITS_PER_ROW - 1], OVERFLOW);
    assert_eq!(row[0], 0x06);
}

#[test]
fn dp_makes_text_fit_that_chars_would_not() {
    // 17 chars but the dot folds → 16 cells, exact fit.
    let row = encode_row("1.234567812345678");
    assert_ne!(row[DIGITS_PER_ROW - 1], OVERFLOW);
    assert_eq!(row[0], 0x06 | DP);
}
