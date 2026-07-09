//! Golden test: the committed ASCII keymap diagrams in `doc/` must match the
//! keys.rs tables exactly — the diagrams can never lie about the layout.

use calcumaker_core::{keydoc, keys};

#[test]
fn keymap_docs_are_fresh() {
    let committed: [(&keys::Keymap, &str); 4] = [
        (&keys::HP16C, include_str!("../../../doc/keymap-16c.txt")),
        (&keys::C15, include_str!("../../../doc/keymap-15c.txt")),
        (&keys::SCI, include_str!("../../../doc/keymap-sci.txt")),
        (&keys::FIN, include_str!("../../../doc/keymap-fin.txt")),
    ];
    // Also guards against a personality being added without its diagram.
    assert_eq!(
        committed.len(),
        keys::PERSONALITIES.len(),
        "add the new personality's diagram"
    );
    for (km, file) in committed {
        assert_eq!(
            keydoc::render(km),
            file,
            "doc/keymap-{}.txt is stale — regenerate: cargo run --example keymaps",
            km.name.to_lowercase()
        );
    }
}

/// Every label used in the diagrams stays ASCII (they're meant to be readable
/// anywhere, including keycap-legend planning docs).
#[test]
fn keymap_docs_are_ascii() {
    for km in keys::PERSONALITIES {
        assert!(
            keydoc::render(km).is_ascii(),
            "{} diagram contains non-ASCII",
            km.name
        );
    }
}

/// The 2U ENTER: one switch in the lower cell, no switch in the cell above, in
/// every personality and every layer. Guards the PCB's 9-key row variant too —
/// if this drifts, the keyboard schematic and the firmware scan disagree.
#[test]
fn enter_is_2u_in_every_personality() {
    use calcumaker_core::keys::{cell_has_switch, ENTER_SPAN_CELL, ENTER_SWITCH_CELL};
    use calcumaker_core::Key;

    let (sr, sc) = ENTER_SWITCH_CELL;
    let (ar, ac) = ENTER_SPAN_CELL;
    assert!(
        cell_has_switch(sr, sc),
        "ENTER's switch cell must have a switch"
    );
    assert!(
        !cell_has_switch(ar, ac),
        "the spanned cell must have no switch"
    );

    for km in keys::PERSONALITIES {
        assert!(
            matches!(km.base[sr][sc], Key::Enter),
            "{}: ENTER left its cell",
            km.name
        );
        for (name, layer) in [("base", &km.base), ("f", &km.f), ("g", &km.g)] {
            assert!(
                matches!(layer[ar][ac], Key::Absent),
                "{}: {name} layer must be Absent at the spanned cell",
                km.name
            );
            // Absent must appear nowhere else — it means "no switch".
            for (r, row) in layer.iter().enumerate() {
                for (c, k) in row.iter().enumerate() {
                    if matches!(k, Key::Absent) {
                        assert_eq!((r, c), (ar, ac), "{}: stray Absent in {name}", km.name);
                    }
                }
            }
        }
    }
}
