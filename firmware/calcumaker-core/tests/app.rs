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

/// Hex entries that spell command names are NUMBERS, not commands: the entry
/// buffer goes through the number-only door (review finding: 'E' pushed
/// Euler's constant, 'DEC' switched radix, 0xCF was unenterable).
#[test]
fn hex_entry_is_never_stolen_by_commands() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Hex, Key::Digit(14), Key::Enter]); // E
    assert_eq!(x_row(&app), "E h");
    press_all(&mut app, &[Key::Digit(12), Key::Digit(15), Key::Enter]); // CF
    assert_eq!(x_row(&app), "CF h");
    press_all(
        &mut app,
        &[Key::Digit(13), Key::Digit(14), Key::Digit(12), Key::Enter], // DEC
    );
    assert_eq!(x_row(&app), "DEC h");
    assert_eq!(app.calc().radix(), calcumaker_core::Radix::Hex); // no mode switch
    assert_eq!(app.calc().stack().len(), 3);
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

/// SETUP items 5/6: the stack model and the personality selector.
#[test]
fn setup_stack_and_pers_items() {
    let mut app = App::new(128);
    // deep stack so the truncation warning fires
    for d in 1..=5 {
        press_all(&mut app, &[Key::Digit(d), Key::Enter]);
    }
    app.press_key(Key::Setup);
    for _ in 0..4 {
        app.press_key(Key::RollDn);
    }
    assert_eq!(app.text_rows()[1].trim_end(), "5 StAC");
    assert_eq!(app.text_rows()[2].trim_end(), "FrEE");
    app.press_key(Key::Enter);
    assert_eq!(app.text_rows()[2].trim_end(), "HP4");
    assert_eq!(app.message(), Some("top 4 kept"));
    assert_eq!(app.calc().stack().len(), 4);

    app.press_key(Key::RollDn);
    assert_eq!(app.text_rows()[1].trim_end(), "6 PErS");
    assert_eq!(app.text_rows()[2].trim_end(), "16C");
    app.press_key(Key::Enter); // cycles to SCI…
    assert_eq!(app.keymap().name, "SCI");
    app.press_key(Key::Enter); // …then FIN…
    assert_eq!(app.keymap().name, "FIN");
    app.press_key(Key::Enter); // …and back
    assert_eq!(app.keymap().name, "16C");
    app.press_key(Key::ClrX);
    assert_eq!(x_row(&app), "5"); // classic stack view, X intact
}

/// Every non-Nop key in every personality's three layers must dispatch —
/// "not implemented" on a printed face is a keymap regression.
#[test]
fn all_personality_keys_dispatch() {
    for km in calcumaker_core::keys::PERSONALITIES {
        for layer in [&km.base, &km.f, &km.g] {
            for row in layer.iter() {
                for &k in row.iter() {
                    let mut app = App::new(64);
                    app.set_keymap(km);
                    // a stack of two small integers satisfies most preconditions
                    for kk in [Key::Digit(2), Key::Enter, Key::Digit(3), Key::Enter] {
                        app.press_key(kk);
                    }
                    app.press_key(k);
                    assert_ne!(
                        app.message(),
                        Some("not implemented"),
                        "{}: key {k:?} is unmapped",
                        km.name
                    );
                }
            }
        }
    }
}

/// PErS actually switches now: SCI applies its defaults (DEG, FIX 4, Dec)
/// and its keymap; cycling again returns to 16C (RAD, AUTO).
#[test]
fn pers_cycles_to_sci_and_back() {
    use calcumaker_core::{AngleMode, FloatFmt};
    let mut app = App::new(128);
    app.press_key(Key::Setup);
    for _ in 0..5 {
        app.press_key(Key::RollDn); // item 6: PErS
    }
    assert_eq!(app.text_rows()[2].trim_end(), "16C");
    app.press_key(Key::Enter);
    assert_eq!(app.keymap().name, "SCI");
    assert_eq!(app.text_rows()[2].trim_end(), "SCI");
    assert_eq!(app.calc().angle_mode(), AngleMode::Deg);
    assert_eq!(app.calc().float_fmt(), FloatFmt::Fix(4));
    app.press_key(Key::Enter); // FIN
    app.press_key(Key::Enter); // back to 16C
    assert_eq!(app.keymap().name, "16C");
    assert_eq!(app.calc().angle_mode(), AngleMode::Rad);
    app.press_key(Key::ClrX); // exit menu
    // SCI again via API: the matrix now resolves SCI faces
    app.set_keymap(&calcumaker_core::keys::SCI);
    app.press(0, 0); // Sin in both — but at (2,0) SCI has Σ+
    let _ = app.calc();
}

/// SCI physical positions: digits unchanged; (2,0) is Σ+; g-(2,0) is nCr.
#[test]
fn sci_keymap_positions() {
    let mut app = App::new(128);
    app.set_keymap(&calcumaker_core::keys::SCI);
    // digits at 16C positions
    app.press(4, 6); // 0 key
    app.press(3, 6); // 1 key
    assert_eq!(x_row(&app), "01_");
    app.press_key(Key::ClrX);
    // Σ+ on the old AND key
    app.press(3, 6); // 1
    app.press(2, 0); // Σ+
    assert_eq!(x_row(&app), "1"); // n = 1 (FIX shows ints plain)
    // g-shifted nCr on the same key
    press_all(&mut app, &[Key::ClrX, Key::Digit(5), Key::Enter, Key::Digit(2)]);
    app.press(4, 1); // g shift
    app.press(2, 0); // nCr
    assert_eq!(x_row(&app), "10"); // C(5,2)
}

/// SCI/FIN are float machines: 3 ÷ 2 is 1.5 there, never 1 (the SETUP EntrY
/// tunable controls it everywhere).
#[test]
fn sci_division_is_real() {
    let mut app = App::new(128);
    app.set_keymap(&calcumaker_core::keys::SCI);
    press_all(&mut app, &[Key::Digit(3), Key::Enter, Key::Digit(2), Key::Div]);
    assert_eq!(x_row(&app), "1.5000"); // FIX 4 default
    // EntrY item exists in SETUP (index 7)
    app.set_keymap(&calcumaker_core::keys::HP16C);
    app.press_key(Key::Setup);
    for _ in 0..6 {
        app.press_key(Key::RollDn);
    }
    assert_eq!(app.text_rows()[1].trim_end(), "7 tYPE");
    assert_eq!(app.text_rows()[2].trim_end(), "FLE");
    app.press_key(Key::Enter);
    assert_eq!(app.text_rows()[2].trim_end(), "Int");
    app.press_key(Key::Enter);
    assert_eq!(app.text_rows()[2].trim_end(), "rEAL");
    app.press_key(Key::ClrX);
}

/// FIN TVM keys: a keyed number stores, a bare press solves (12C).
#[test]
fn fin_tvm_keys_store_then_solve() {
    let mut app = App::new(256);
    app.set_keymap(&calcumaker_core::keys::FIN); // applies FIX 2
    // 360 n | 0.5 i | 100000 PV | 0 FV | PMT (bare = solve)
    press_all(&mut app, &[Key::Digit(3), Key::Digit(6), Key::Digit(0), Key::TvmN]);
    press_all(&mut app, &[Key::Digit(0), Key::Dot, Key::Digit(5), Key::TvmI]);
    for d in [1, 0, 0, 0, 0, 0] {
        app.press_key(Key::Digit(d));
    }
    app.press_key(Key::TvmPv);
    press_all(&mut app, &[Key::Digit(0), Key::TvmFv]);
    app.press_key(Key::TvmPmt); // no entry pending → solve
    assert_eq!(x_row(&app), "-599.55");
}

/// FIN matrix positions: the TVM row sits on the old hex-digit row.
#[test]
fn fin_keymap_positions() {
    let mut app = App::new(256);
    app.set_keymap(&calcumaker_core::keys::FIN);
    assert_eq!(app.calc().float_fmt(), calcumaker_core::FloatFmt::Fix(2));
    press_all(&mut app, &[Key::Digit(1), Key::Digit(2)]);
    app.press(1, 0); // TvmN cell (was hex 'A')
    app.press_key(Key::Enter); // nothing pending: engine enter dups
    let _ = app.calc();
    // g-BEG on the PMT cell; f-12× on the n cell
    app.press(4, 1); // g
    app.press(1, 3); // BegKey
    app.press(4, 0); // f
    app.press(1, 0); // X12Mul — needs X; harmless error is fine
    assert_ne!(app.message(), Some("not implemented"));
}

/// PErS cycles all three: 16C → SCI → FIN → 16C.
#[test]
fn pers_cycles_three_ways() {
    let mut app = App::new(128);
    app.press_key(Key::Setup);
    for _ in 0..5 {
        app.press_key(Key::RollDn);
    }
    app.press_key(Key::Enter);
    assert_eq!(app.keymap().name, "SCI");
    app.press_key(Key::Enter);
    assert_eq!(app.keymap().name, "FIN");
    assert_eq!(app.calc().float_fmt(), calcumaker_core::FloatFmt::Fix(2));
    app.press_key(Key::Enter);
    assert_eq!(app.keymap().name, "16C");
}

/// In HP4 mode, keyed-number + ENTER duplicates (the real HP model):
/// `3 ENTER +` doubles.
#[test]
fn classic4_app_enter_duplicates() {
    let mut app = App::new(128);
    app.calc_mut().set_stack_model(calcumaker_core::StackModel::Classic4);
    press_all(&mut app, &[Key::Digit(3), Key::Enter, Key::Add]);
    assert_eq!(x_row(&app), "6");
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

/// Errors show HP-style `Error N` on the glass (transient), with the full
/// text on the status line — both cleared by the next key.
#[test]
fn glass_shows_error_code() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(5), Key::Enter, Key::Digit(0), Key::Div]);
    assert_eq!(x_row(&app), "Error 0");
    assert_eq!(app.message(), Some("divide by zero"));
    // renderable on the 7-seg
    for ch in "Error 0".chars() {
        assert!(ch == ' ' || calcumaker_core::seg7::encode(ch).is_some());
    }
    // next key restores the stack view (operands were never consumed)
    app.press_key(Key::Digit(3));
    assert_eq!(x_row(&app), "3_");
    app.press_key(Key::ClrX);
    assert_eq!(x_row(&app), "0"); // the 0 divisor still there
}

/// The aux OLED content (4x21, one code path for firmware + emulator):
/// flags header + message/full-precision X; SETUP > OLEd toggles the header.
#[test]
fn aux_oled_lines() {
    let mut app = App::new(128);
    press_all(&mut app, &[Key::Digit(5), Key::Enter, Key::Digit(0), Key::Div]);
    let l = app.aux_lines();
    assert_eq!(l[0], "16C DEC RAD FLEX");
    assert_eq!(l[1], "P128");
    assert_eq!(l[2], "divide by zero");
    assert!(l.iter().all(|s| s.chars().count() <= 21));
    // idle: the body is the full-precision X (windowing helper)
    app.press_key(Key::ClrX);
    press_all(&mut app, &[Key::Digit(7), Key::Enter]);
    assert_eq!(app.aux_lines()[2], "7");
    // SETUP > OLEd (item 8) toggles the flags header off -> all body
    app.press_key(Key::Setup);
    for _ in 0..7 {
        app.press_key(Key::RollDn);
    }
    assert_eq!(app.text_rows()[1].trim_end(), "8 OLEd");
    assert_eq!(app.text_rows()[2].trim_end(), "FLAG");
    app.press_key(Key::Enter);
    assert_eq!(app.text_rows()[2].trim_end(), "oFF");
    app.press_key(Key::ClrX);
    assert!(!app.aux_shows_flags());
    assert_eq!(app.aux_lines()[0], "7"); // body starts at line 0 now
    // long errors wrap across the full width
    app.calc_mut().set_num_mode(calcumaker_core::NumMode::Flex);
    press_all(&mut app, &[Key::Digit(2), Key::Dot, Key::Digit(5), Key::Enter]);
    app.press_key(Key::Sto);
    app.press_key(Key::Add); // "register select cancelled"
    let l = app.aux_lines();
    assert_eq!(l[0], "register select cance");
    assert_eq!(l[1], "lled");
}

#[test]
fn complex_spans_two_rows() {
    use calcumaker_core::App;
    let mut a = App::new(256);
    for t in ["3", "4", "complex"] { a.calc_mut().input(t).unwrap(); } // X = 3+4i
    let r = a.text_rows();
    assert_eq!(r[1].trim(), "3"); // real part
    assert_eq!(r[2].trim(), "4 i"); // imaginary part with indicator
    // polar: magnitude / angle
    a.calc_mut().input("polar").unwrap();
    a.calc_mut().input("deg").unwrap();
    let p = a.text_rows();
    assert_eq!(p[1].trim(), "5"); // |3+4i| = 5
    assert!(p[2].trim().starts_with("53.13")); // arg in degrees
}
