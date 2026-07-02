//! App-layer tests — key presses (matrix + logical) through to display rows
//! and segment bytes, on the real GMP/MPFR path.

use calcumaker_core::keys::{BASE, COLS, LAYER_F, ROWS};
use calcumaker_core::seg7::{self, DIGITS_PER_ROW};
use calcumaker_core::{App, Key};

/// Feed a sequence of logical keys.
fn press_all(app: &mut App, keys: &[Key]) {
    for &k in keys {
        app.press_key(k);
    }
}

/// Matrix position of a key in the base layer (tests drive real (row,col)).
fn pos(k: Key) -> (usize, usize) {
    for r in 0..ROWS {
        for c in 0..COLS {
            if BASE[r][c] == k {
                return (r, c);
            }
        }
    }
    panic!("{k:?} not in base layer");
}

fn x_row(app: &App) -> String {
    app.text_rows().last().unwrap().clone()
}

#[test]
fn digit_entry_shows_cursor_then_pushes() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(4), Key::Digit(2)]);
    assert_eq!(x_row(&app), "42_");
    app.press_key(Key::Enter);
    assert_eq!(x_row(&app), "42");
    assert_eq!(app.calc().stack().len(), 1);
}

#[test]
fn op_flushes_pending_entry() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(2), Key::Enter, Key::Digit(3), Key::Add]);
    assert_eq!(x_row(&app), "5");
}

#[test]
fn matrix_presses_resolve_layers() {
    let mut app = App::new(200);
    // 2 ENTER, then f-shift + Sqrt cell = Sq  → 4
    let (r2, c2) = pos(Key::Digit(2));
    let (re, ce) = pos(Key::Enter);
    let (rf, cf) = pos(Key::ShiftF);
    let (rs, cs) = pos(Key::Sqrt);
    app.press(r2, c2);
    app.press(re, ce);
    assert_eq!(app.shift(), None);
    app.press(rf, cf);
    assert_eq!(app.shift(), Some('f'));
    app.press(rs, cs); // LAYER_F at Sqrt's cell is Sq
    assert_eq!(LAYER_F[rs][cs], Key::Sq);
    assert_eq!(x_row(&app), "4");
    assert_eq!(app.shift(), None);
}

#[test]
fn dot_and_eex_build_reals() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(1), Key::Dot, Key::Digit(5), Key::Eex, Key::Digit(3), Key::Enter]);
    assert_eq!(x_row(&app), "1500");
}

#[test]
fn chs_flips_exponent_sign_during_entry() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(2), Key::Eex, Key::Digit(2), Key::Chs, Key::Enter]);
    assert_eq!(x_row(&app), "0.02");
}

#[test]
fn chs_without_entry_negates_x() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(7), Key::Enter, Key::Chs]);
    assert_eq!(x_row(&app), "-7");
}

#[test]
fn backspace_edits_and_cancels_entry() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(1), Key::Digit(2), Key::Back]);
    assert_eq!(x_row(&app), "1_");
    app.press_key(Key::Back);
    assert_eq!(x_row(&app), ""); // entry cancelled, stack empty
    assert!(app.calc().stack().is_empty());
}

#[test]
fn clrx_cancels_entry_then_drops_x() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(5), Key::Enter, Key::Digit(9), Key::ClrX]);
    assert_eq!(x_row(&app), "5"); // entry gone, X intact
    app.press_key(Key::ClrX);
    assert!(app.calc().stack().is_empty());
}

#[test]
fn bad_digit_for_radix_is_rejected() {
    let mut app = App::new(128);
    app.press_key(Key::Digit(10)); // 'A' in Dec mode
    assert!(app.message().is_some());
    assert_eq!(x_row(&app), "");
}

#[test]
fn hex_entry_and_bitwise() {
    let mut app = App::new(128);
    press_all(
        &mut app,
        &[Key::Hex, Key::Digit(15), Key::Digit(15), Key::Enter, Key::Digit(0), Key::Digit(15), Key::And],
    );
    assert_eq!(x_row(&app), "F");
}

#[test]
fn wsize_key_sets_word_from_x() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(8), Key::WordSize]);
    assert_eq!(app.calc().word_bits(), Some(8));
    press_all(&mut app, &[Key::Hex, Key::Digit(0), Key::Digit(15), Key::Not]);
    assert_eq!(x_row(&app), "F0");
}

#[test]
fn prec_key_sets_precision_from_x() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(5), Key::Digit(1), Key::Digit(2), Key::Prec]);
    assert_eq!(app.calc().prec(), 512);
}

#[test]
fn sto_rcl_via_pending_register_digit() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(4), Key::Digit(2), Key::Sto]);
    assert_eq!(app.pending_register(), Some("STO"));
    app.press_key(Key::Digit(5)); // register 5
    assert_eq!(x_row(&app), "42"); // STO keeps X
    press_all(&mut app, &[Key::ClrX, Key::Rcl, Key::Digit(5)]);
    assert_eq!(x_row(&app), "42");
}

#[test]
fn pending_register_cancelled_by_non_digit() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(7), Key::Sto, Key::Add]);
    assert_eq!(app.message(), Some("register select cancelled"));
    assert_eq!(app.pending_register(), None);
    assert_eq!(x_row(&app), "7"); // the Add was swallowed, X untouched
}

#[test]
fn three_rows_show_stack_top() {
    let mut app = App::new(128);
    for k in [Key::Digit(1), Key::Enter, Key::Digit(2), Key::Enter, Key::Digit(3), Key::Enter, Key::Digit(4)] {
        app.press_key(k);
    }
    let rows = app.text_rows();
    assert_eq!(rows[0], "2"); // Z
    assert_eq!(rows[1], "3"); // Y
    assert_eq!(rows[2], "4_"); // X = live entry
}

/// The glass rounds AUTO reals to the window (HP behaviour): a binary value a
/// hair under 382.1 shows as 382.1, not 382.09999… spilling off the row.
#[test]
fn glass_rounds_auto_reals_to_the_window() {
    let mut app = App::new(256);
    press_all(
        &mut app,
        &[Key::Digit(3), Key::Digit(8), Key::Digit(2), Key::Dot, Key::Digit(1), Key::Enter],
    );
    assert_eq!(x_row(&app), "382.1");
    // …while the register keeps full precision (the X:/SHOW view).
    assert!(app.x_full().len() > 20, "x_full = {}", app.x_full());
}

#[test]
fn glass_shows_16_digits_of_pi() {
    let mut app = App::new(256);
    app.press_key(Key::Pi);
    assert_eq!(x_row(&app), "3.141592653589793");
    assert!(app.x_full().len() > 70); // full precision below
}

/// Exponent-bound values go scientific on the glass with maximal digits.
#[test]
fn glass_forces_sci_when_plain_is_too_wide() {
    let mut app = App::new(256);
    press_all(
        &mut app,
        &[Key::Digit(1), Key::Dot, Key::Digit(2), Key::Digit(3), Key::Digit(4), Key::Digit(5), Key::Eex, Key::Digit(1), Key::Digit(7), Key::Enter],
    );
    assert_eq!(x_row(&app), "1.2345e17");
}

/// f-SHOW: transient view of X in another base via the status message.
#[test]
fn show_base_is_a_transient_message() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(2), Key::Digit(5), Key::Digit(5), Key::ShowHex]);
    assert_eq!(app.message(), Some("hex: FF"));
    assert_eq!(x_row(&app), "255"); // display radix unchanged
    app.press_key(Key::Enter);
    assert_eq!(app.message(), None); // gone on the next key
}

#[test]
fn clrreg_key_wipes_registers() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(7), Key::Sto, Key::Digit(2), Key::ClrReg, Key::Rcl, Key::Digit(2)]);
    assert_eq!(app.message(), Some("empty register"));
}

#[test]
fn seg_rows_encode_x() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(1), Key::Dot, Key::Digit(5), Key::Enter]);
    let rows = app.seg_rows();
    let bottom = rows[seg7::DISPLAY_ROWS - 1];
    // "1.5" folds the dot into the '1' cell: [... , '1'+dp, '5']
    assert_eq!(bottom[DIGITS_PER_ROW - 2], 0x06 | seg7::DP);
    assert_eq!(bottom[DIGITS_PER_ROW - 1], 0x6D);
}

#[test]
fn overflow_marks_last_cell() {
    let mut app = App::new(256);
    press_all(&mut app, &[Key::Digit(2), Key::Digit(0), Key::Fact, Key::Digit(3), Key::Mul]);
    // 20! * 3 = 7298712478720819200000 → 22 digits, > 16 cells
    let bottom = app.seg_rows()[seg7::DISPLAY_ROWS - 1];
    assert_eq!(bottom[DIGITS_PER_ROW - 1], seg7::OVERFLOW);
    assert_ne!(bottom[0], 0); // row is full
}
