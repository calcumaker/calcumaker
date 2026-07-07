//! Link the C math libraries.
//!
//! - **Host** (running `cargo test`, the REPL, etc.): dynamically link the
//!   system / Homebrew `libgmp` + `libmpfr`.
//! - **Target** (`*-none-eabi*`): link nothing here — the firmware crate's
//!   linker pulls in the GMP/MPFR cross-built for the MCU (see
//!   `calcumaker-fw/build.rs` and DESIGN.md → "GMP/MPFR on the target").

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "none" {
        return; // bare-metal: linked by the firmware against cross-built libs
    }

    // Homebrew keg-only prefixes (macOS), if present.
    for lib in ["mpc", "mpfr", "gmp"] {
        if let Ok(out) = Command::new("brew").args(["--prefix", lib]).output() {
            if out.status.success() {
                if let Ok(p) = String::from_utf8(out.stdout) {
                    let p = p.trim();
                    if !p.is_empty() {
                        println!("cargo:rustc-link-search=native={p}/lib");
                    }
                }
            }
        }
    }
    // Common fallbacks (harmless if absent).
    for d in ["/opt/homebrew/lib", "/usr/local/lib", "/usr/lib"] {
        println!("cargo:rustc-link-search=native={d}");
    }
    // mpc -> mpfr -> gmp (each depends on the next).
    println!("cargo:rustc-link-lib=dylib=mpc");
    println!("cargo:rustc-link-lib=dylib=mpfr");
    println!("cargo:rustc-link-lib=dylib=gmp");
}
