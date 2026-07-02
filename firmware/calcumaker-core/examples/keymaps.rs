//! Regenerate the ASCII keymap diagrams in `doc/` from the keys.rs tables:
//!
//! ```sh
//! cargo run --example keymaps            # writes doc/keymap-<name>.txt
//! cargo run --example keymaps -- <dir>   # custom output directory
//! ```
//!
//! The `keymap_docs_are_fresh` test fails when the committed files drift.

use std::fs;
use std::path::PathBuf;

fn main() {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../doc")));
    fs::create_dir_all(&out).expect("create output dir");
    for km in calcumaker_core::keys::PERSONALITIES {
        let path = out.join(format!("keymap-{}.txt", km.name.to_lowercase()));
        fs::write(&path, calcumaker_core::keydoc::render(km)).expect("write keymap file");
        println!("wrote {}", path.display());
    }
}
