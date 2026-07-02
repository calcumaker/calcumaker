//! Golden test: the committed ASCII keymap diagrams in `doc/` must match the
//! keys.rs tables exactly — the diagrams can never lie about the layout.

use calcumaker_core::{keydoc, keys};

#[test]
fn keymap_docs_are_fresh() {
    let committed: [(&keys::Keymap, &str); 3] = [
        (&keys::HP16C, include_str!("../../../doc/keymap-16c.txt")),
        (&keys::SCI, include_str!("../../../doc/keymap-sci.txt")),
        (&keys::FIN, include_str!("../../../doc/keymap-fin.txt")),
    ];
    // Also guards against a personality being added without its diagram.
    assert_eq!(committed.len(), keys::PERSONALITIES.len(), "add the new personality's diagram");
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
        assert!(keydoc::render(km).is_ascii(), "{} diagram contains non-ASCII", km.name);
    }
}
