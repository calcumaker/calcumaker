//! Engine self-test — the **validation payload** shared by every build target.
//!
//! Each case is an RPN token sequence whose expected result was captured from
//! the **host** engine (`calcumaker-core` linked against system GMP/MPFR, the
//! same source that passes `cargo test`). `run_all` replays them on whatever
//! GMP/MPFR this image links (on the target: the cross-built archives in
//! `firmware/vendor/gmp-mpfr-arm`) and checks the output matches byte-for-byte.
//!
//! MPFR is deterministic at a fixed precision + rounding mode and the decimal
//! formatting is our own (`calcumaker_core::format`), so the float cases
//! (`sqrt2_flt`, `div_flt`, `pi`) MUST reproduce the host string exactly — any
//! divergence means the target math stack is miscompiled/mislinked, which is
//! exactly what this validates.
//!
//! Output is emitted through the crate's `log_info!` / `log_error!` shim: on the
//! Nucleo validation target (`--features nucleo`) it goes to RTT via defmt; in
//! the production link-smoke image it compiles to nothing. The check itself runs
//! either way, so the same code proves both "the stack links" and "it computes".

use alloc::string::String;
use calcumaker_core::{Calc, Radix};

/// One golden case: replay `tokens` on a fresh 256-bit `Calc` in `radix`, then
/// `display()` must equal `expect`.
struct Case {
    // Read only by the log macros, which compile out in the silent production
    // image — so it's "unused" there, but essential on the validation target.
    #[cfg_attr(not(feature = "nucleo"), allow(dead_code))]
    name: &'static str,
    radix: Radix,
    tokens: &'static [&'static str],
    expect: &'static str,
}

/// Golden values captured from the host engine (system GMP/MPFR, 256-bit prec).
/// The engine defaults to **integer mode**, so `20 4 /` = 5 and `2 sqrt` = 1
/// (floor); `float` switches to MPFR reals. Keep this list in sync with the host
/// if operations/formatting change — regenerate against `calcumaker-core`.
const CASES: &[Case] = &[
    // Integer arithmetic (GMP).
    Case { name: "add",       radix: Radix::Dec, tokens: &["2", "3", "+"],      expect: "5" },
    Case { name: "sub",       radix: Radix::Dec, tokens: &["10", "4", "-"],     expect: "6" },
    Case { name: "mul",       radix: Radix::Dec, tokens: &["6", "7", "*"],      expect: "42" },
    Case { name: "div",       radix: Radix::Dec, tokens: &["20", "4", "/"],     expect: "5" },
    // Big-integer stress — factorial(100), 158 digits (exercises GMP allocation
    // churn through the firmware malloc shim / TLSF heap).
    Case { name: "fact100",   radix: Radix::Dec, tokens: &["100", "!"],
        expect: "93326215443944152681699238856266700490715968264381621468592963895217599993229915608941463976156518286253697920827223758251185210916864000000000000000000000000" },
    // Bitwise (programmer core), hex radix.
    Case { name: "hex_and",   radix: Radix::Hex, tokens: &["ff", "0f", "and"],  expect: "F" },
    Case { name: "hex_or",    radix: Radix::Hex, tokens: &["f0", "0f", "or"],   expect: "FF" },
    Case { name: "hex_xor",   radix: Radix::Hex, tokens: &["ff", "0f", "xor"],  expect: "F0" },
    // Integer-mode sqrt = floor.
    Case { name: "sqrt2_int", radix: Radix::Dec, tokens: &["2", "sqrt"],        expect: "1" },
    // Float mode (MPFR): correctly-rounded transcendentals + exact fractions.
    Case { name: "sqrt2_flt", radix: Radix::Dec, tokens: &["float", "2", "sqrt"],
        expect: "1.4142135623730950488016887242096980785696718753769480731766797379907324784621" },
    Case { name: "div_flt",   radix: Radix::Dec, tokens: &["float", "1", "8", "/"], expect: "0.125" },
    Case { name: "pi",        radix: Radix::Dec, tokens: &["pi"],
        expect: "3.1415926535897932384626433832795028841971693993751058209749445923078164062862" },
    Case { name: "sin0",      radix: Radix::Dec, tokens: &["0", "sin"],         expect: "0" },
    Case { name: "exp0",      radix: Radix::Dec, tokens: &["0", "exp"],         expect: "1" },
    Case { name: "ln1",       radix: Radix::Dec, tokens: &["1", "ln"],          expect: "0" },
];

fn run(case: &Case) -> String {
    let mut c = Calc::new(256);
    c.set_radix(case.radix);
    for t in case.tokens {
        // Per-token errors are ignored; a case's correctness is judged solely by
        // the final display() vs. the golden string.
        let _ = c.input(t);
    }
    c.display()
}

/// Result tally so the caller (main) can signal overall pass/fail.
pub struct Report {
    pub passed: u32,
    pub failed: u32,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.failed == 0
    }
}

/// Run every golden case, logging each result, and return the tally.
pub fn run_all() -> Report {
    let mut passed = 0u32;
    let mut failed = 0u32;
    for case in CASES {
        let got = run(case);
        if got == case.expect {
            passed += 1;
            log_info!("PASS {=str}: {=str}", case.name, got.as_str());
        } else {
            failed += 1;
            log_error!(
                "FAIL {=str}: got \"{=str}\" want \"{=str}\"",
                case.name,
                got.as_str(),
                case.expect
            );
        }
    }
    if failed == 0 {
        log_info!("SELF-TEST PASS: {=u32}/{=u32}", passed, passed);
    } else {
        log_error!("SELF-TEST FAIL: {=u32} passed, {=u32} failed", passed, failed);
    }
    Report { passed, failed }
}
