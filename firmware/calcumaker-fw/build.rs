//! Build script for calcumaker-fw.
//!
//! 1. Puts `memory.x` on the linker search path (cortex-m-rt's `link.x`
//!    includes it). If you adopt `embassy-stm32`'s `memory-x` feature instead,
//!    drop the local `memory.x` and this block.
//! 2. (feature `numeric-gmp`) links the cross-built `libgmp` / `libmpfr`.
//!    See ../../DESIGN.md → "GMP/MPFR on no_std" for how these static libs are
//!    produced (configure --host=arm-none-eabi --disable-assembly, built against
//!    picolibc/newlib-nano, malloc routed to the firmware heap).

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // ---- memory.x -> linker search path -------------------------------------
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out.join("memory.x"), include_bytes!("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");

    // ---- GMP / MPFR static libs (preferred numeric backend) -----------------
    if env::var("CARGO_FEATURE_NUMERIC_GMP").is_ok() {
        // TODO(numeric-gmp): point this at the cross-built libs directory and
        // link them. Example:
        //   let libdir = env::var("GMP_MPFR_LIBDIR")
        //       .expect("set GMP_MPFR_LIBDIR to the cross-built libgmp/libmpfr");
        //   println!("cargo:rustc-link-search=native={libdir}");
        //   println!("cargo:rustc-link-lib=static=mpfr");
        //   println!("cargo:rustc-link-lib=static=gmp");
        println!(
            "cargo:warning=feature numeric-gmp is selected but the GMP/MPFR \
             cross-built libs are not yet wired in build.rs (see DESIGN.md)."
        );
    }
}
