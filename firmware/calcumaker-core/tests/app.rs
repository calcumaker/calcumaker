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
    assert_eq!(x_row(&app), "F h"); // 16C radix letter on the glass
}

#[test]
fn wsize_key_sets_word_from_x() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(8), Key::WordSize]);
    assert_eq!(app.calc().word_bits(), Some(8));
    press_all(&mut app, &[Key::Hex, Key::Digit(0), Key::Digit(15), Key::Not]);
    assert_eq!(x_row(&app), "F0 h");
}

/// 16C radix letter: non-decimal integer X carries its base on the glass —
/// the hardware has no radix lamps. Decimal is unmarked (deviation from the
/// 16C, documented); reals, entry, and row-filling values are unmarked too.
#[test]
fn radix_suffix_letter() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(5), Key::Enter]);
    assert_eq!(x_row(&app), "5"); // decimal: bare
    app.press_key(Key::Oct);
    assert_eq!(x_row(&app), "5 o");
    app.press_key(Key::Bin);
    assert_eq!(x_row(&app), "101 b");
    app.press_key(Key::Hex);
    assert_eq!(x_row(&app), "5 h");
    // during entry: no suffix
    app.press_key(Key::Digit(10));
    assert_eq!(x_row(&app), "A_");
    app.press_key(Key::Enter);
    assert_eq!(x_row(&app), "A h");
}

#[test]
fn radix_suffix_skipped_when_it_cannot_fit() {
    let mut app = App::new(128);
    // 15-digit binary value + suffix (2 cells) would exceed the row: bare
    press_all(&mut app, &[Key::Digit(2), Key::Digit(0), Key::WordSize, Key::Bin, Key::Digit(1)]);
    for _ in 0..14 {
        app.press_key(Key::Digit(0));
    }
    app.press_key(Key::Enter);
    assert_eq!(x_row(&app).len(), 15); // 15 digits, no " b"
    assert!(!x_row(&app).ends_with('b'));
}

/// The suffix is a display tunable (`suffix` toggles; on by default).
#[test]
fn radix_suffix_is_tunable() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Hex, Key::Digit(15), Key::Enter]);
    assert_eq!(x_row(&app), "F h");
    app.calc_mut().set_radix_suffix(false);
    assert_eq!(x_row(&app), "F");
    app.calc_mut().set_radix_suffix(true);
    assert_eq!(x_row(&app), "F h");
}

#[test]
fn radix_suffix_not_on_reals() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(1), Key::Dot, Key::Digit(5), Key::Enter]);
    assert_eq!(x_row(&app), "1.5");
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

/// 16C window keys: scroll a value wider than the row through 16-cell chunks.
#[test]
fn window_keys_scroll_long_values() {
    let mut app = App::new(256);
    // 10^70: a '1' + 70 zeros = 71 cells → 5 windows
    press_all(&mut app, &[Key::Digit(7), Key::Digit(0), Key::Exp10]);
    assert_eq!(app.window(), (0, 5));
    // default view: overflow marker in the last cell
    assert_eq!(app.seg_rows()[seg7::DISPLAY_ROWS - 1][DIGITS_PER_ROW - 1], seg7::OVERFLOW);

    app.press_key(Key::WinR);
    assert_eq!(app.window(), (1, 5));
    // window 1 starts at cell 15 — exactly where the marker cut off
    let row = app.seg_rows()[seg7::DISPLAY_ROWS - 1];
    assert!(row.iter().all(|&c| c == 0x3F), "window 1 is all zeros: {row:?}");

    for _ in 0..10 {
        app.press_key(Key::WinR); // clamps at the last window
    }
    assert_eq!(app.window(), (4, 5));
    let row = app.seg_rows()[seg7::DISPLAY_ROWS - 1];
    // 71 cells: window 4 shows cells 63..71 = 8 cells, then blanks
    assert_eq!(row[7], 0x3F);
    assert_eq!(row[8], 0x00);

    app.press_key(Key::WinL);
    assert_eq!(app.window(), (3, 5));

    // any non-window key resets the view
    app.press_key(Key::Enter);
    assert_eq!(app.window().0, 0);
}

/// Reassembling window 0 (its 15 content cells) plus every scrolled window
/// must reproduce the full value — no digit may fall between windows.
#[test]
fn every_cell_is_reachable_across_windows() {
    let mut app = App::new(256);
    press_all(&mut app, &[Key::Digit(2), Key::Digit(2), Key::Fact]); // 22! = 22 digits
    let full = seg7::encode_cells(&app.text_rows()[seg7::DISPLAY_ROWS - 1]);
    let (_, total) = app.window();
    assert!(total > 1);
    let mut seen: Vec<u8> = Vec::new();
    seen.extend_from_slice(&app.seg_rows()[seg7::DISPLAY_ROWS - 1][..DIGITS_PER_ROW - 1]);
    for _ in 1..total {
        app.press_key(Key::WinR);
        seen.extend_from_slice(&app.seg_rows()[seg7::DISPLAY_ROWS - 1]);
    }
    seen.truncate(full.len());
    assert_eq!(seen, full);
}

/// f-STATUS: the glass shows the mode summary until the next key (16C).
#[test]
fn status_view_shows_modes_then_restores() {
    let mut app = App::new(256);
    press_all(
        &mut app,
        &[Key::Hex, Key::Digit(8), Key::WordSize, Key::Digit(4), Key::Fix, Key::Digit(1), Key::Sf],
    );
    app.press_key(Key::Status);
    let rows = app.text_rows();
    assert_eq!(rows[0].trim_end(), "bASE 16 2S rAd");
    assert_eq!(rows[1].trim_end(), "P256 b8");
    assert_eq!(rows[2].trim_end(), "FI 4 000010"); // flags 543210: F1 set
    // every character must be 7-seg renderable
    for r in &rows {
        for ch in r.chars() {
            assert!(
                ch == ' ' || calcumaker_core::seg7::encode(ch).is_some(),
                "unrenderable char {ch:?} in {r:?}"
            );
        }
    }
    // next key restores the stack view
    app.press_key(Key::Digit(5));
    assert_eq!(x_row(&app), "5_");
}

#[test]
fn status_reflects_carry_and_word_flags() {
    let mut app = App::new(128);
    // 8-bit word, inexact isqrt sets carry (flag 4)
    press_all(&mut app, &[Key::Digit(8), Key::WordSize, Key::Digit(1), Key::Digit(7), Key::Sqrt]);
    app.press_key(Key::Status);
    let rows = app.text_rows();
    assert_eq!(rows[2].trim_end(), "AUtO 010000");
}

/// g-SETUP: interactive settings menu — navigate, change, exit; the tunables
/// it flips are live on the glass afterwards.
#[test]
fn setup_menu_toggles_the_suffix() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Hex, Key::Digit(15), Key::Enter]);
    assert_eq!(x_row(&app), "F h");
    app.press_key(Key::Setup);
    let rows = app.text_rows();
    assert_eq!(rows[0].trim_end(), "SEtUP");
    assert_eq!(rows[1].trim_end(), "1 SUFF");
    assert_eq!(rows[2].trim_end(), "on");
    app.press_key(Key::Enter); // toggle
    assert_eq!(app.text_rows()[2].trim_end(), "oFF");
    app.press_key(Key::ClrX); // exit — ClrX must NOT drop X here
    assert_eq!(x_row(&app), "F"); // suffix off, X intact
    assert_eq!(app.calc().stack().len(), 1);
}

#[test]
fn setup_menu_navigates_and_cycles_angle() {
    let mut app = App::new(128);
    app.press_key(Key::Setup);
    app.press_key(Key::RollDn); // 2 LEAd 0
    app.press_key(Key::RollDn); // 3 AnGLE
    assert_eq!(app.text_rows()[1].trim_end(), "3 AnGLE");
    assert_eq!(app.text_rows()[2].trim_end(), "rAd");
    app.press_key(Key::Enter);
    assert_eq!(app.text_rows()[2].trim_end(), "dEG");
    assert_eq!(app.calc().angle_mode(), calcumaker_core::AngleMode::Deg);
    app.press_key(Key::RollUp); // back to 2
    assert_eq!(app.text_rows()[1].trim_end(), "2 LEAd 0");
    app.press_key(Key::Setup); // toggle key exits too
    assert_eq!(app.text_rows()[0].trim_end(), "");
    // every menu character is 7-seg renderable
    app.press_key(Key::Setup);
    for _ in 0..4 {
        for r in app.text_rows() {
            for ch in r.chars() {
                assert!(
                    ch == ' ' || calcumaker_core::seg7::encode(ch).is_some(),
                    "unrenderable {ch:?} in {r:?}"
                );
            }
        }
        app.press_key(Key::RollDn);
    }
}

#[test]
fn setup_swallows_other_keys() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(7), Key::Enter, Key::Setup, Key::Add]);
    assert!(app.message().is_some()); // hint, not an engine op
    assert_eq!(app.calc().stack().len(), 1); // 7 untouched
    app.press_key(Key::Back); // exits
    assert_eq!(x_row(&app), "7");
}

#[test]
fn window_is_single_for_short_values() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(4), Key::Digit(2), Key::Enter]);
    assert_eq!(app.window(), (0, 1));
    app.press_key(Key::WinR); // nothing to scroll
    assert_eq!(app.window(), (0, 1));
    assert_eq!(x_row(&app), "42");
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
