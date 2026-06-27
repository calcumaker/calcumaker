//! Build script for calcumaker-fw.
//!
//! 1. Puts `memory.x` on the linker search path (cortex-m-rt's `link.x`
//!    includes it). If you adopt `embassy-stm32`'s `memory-x` feature instead,
//!    drop the local `memory.x` and this block.
//! 2. (TODO) links the cross-built `libgmp` / `libmpfr` for the calculator
//!    engine on the target. See ../../DESIGN.md → "GMP/MPFR on the target" for
//!    how these static libs are produced (configure --host=arm-none-eabi
//!    --disable-assembly, built against picolibc, malloc routed to the firmware
//!    heap), e.g.:
//!        let libdir = env::var("GMP_MPFR_LIBDIR").unwrap();
//!        println!("cargo:rustc-link-search=native={libdir}");
//!        println!("cargo:rustc-link-lib=static=mpfr");
//!        println!("cargo:rustc-link-lib=static=gmp");

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // memory.x -> linker search path (cortex-m-rt's link.x includes it).
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out.join("memory.x"), include_bytes!("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
