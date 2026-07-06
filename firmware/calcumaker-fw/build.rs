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

    // Cross-built GMP + MPFR for the calculator engine. Point GMP_MPFR_LIBDIR at
    // the install prefix produced by `firmware/scripts/build-gmp-mpfr-arm.sh`
    // (default: firmware/vendor/gmp-mpfr-arm). mpfr before gmp (mpfr -> gmp).
    println!("cargo:rerun-if-env-changed=GMP_MPFR_LIBDIR");
    println!("cargo:rerun-if-env-changed=ARM_NONE_EABI_GCC");
    if let Ok(prefix) = env::var("GMP_MPFR_LIBDIR") {
        // The cross-built GMP + MPFR reference a little newlib — number parsing
        // (ctype/strtol/localeconv) and error paths (assert/exception/fwrite/
        // raise) — plus libgcc soft-float double routines. Link newlib-nano +
        // libm + libnosys + libgcc for the SAME cortex-m33 hard-float multilib
        // the archives were built for, resolving the dirs from arm-none-eabi-gcc.
        // Everything goes in ONE `--start-group` so the circular deps
        // (gmp<->mpfr<->libc<->libgcc) resolve; `--gc-sections` drops the unused
        // GMP/MPFR functions (the archives were built -ffunction/-fdata-sections).
        let gcc = env::var("ARM_NONE_EABI_GCC").unwrap_or_else(|_| "arm-none-eabi-gcc".into());
        let arch = ["-mcpu=cortex-m33", "-mthumb", "-mfloat-abi=hard", "-mfpu=fpv5-sp-d16"];
        let dir_of = |args: &[&str]| -> Option<String> {
            let out = std::process::Command::new(&gcc).args(arch).args(args).output().ok()?;
            let p = PathBuf::from(String::from_utf8(out.stdout).ok()?.trim());
            if p.is_absolute() {
                p.parent().map(|d| d.display().to_string())
            } else {
                None
            }
        };
        let newlib = dir_of(&["-print-file-name=libc_nano.a"]);
        let libgcc = dir_of(&["-print-libgcc-file-name"]);

        println!("cargo:rustc-link-arg=-L{prefix}/lib");
        if let Some(d) = &newlib {
            println!("cargo:rustc-link-arg=-L{d}");
        }
        if let Some(d) = &libgcc {
            println!("cargo:rustc-link-arg=-L{d}");
        }
        println!("cargo:rustc-link-arg=--gc-sections");
        println!("cargo:rustc-link-arg=--start-group");
        for l in ["mpfr", "gmp", "c_nano", "m", "nosys", "gcc"] {
            println!("cargo:rustc-link-arg=-l{l}");
        }
        println!("cargo:rustc-link-arg=--end-group");
        if newlib.is_none() {
            println!(
                "cargo:warning=arm-none-eabi-gcc not found for the newlib multilib; \
                 set ARM_NONE_EABI_GCC or add the toolchain bin to PATH"
            );
        }
    }
}
