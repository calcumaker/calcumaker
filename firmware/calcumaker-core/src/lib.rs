//! Calcumaker 16 calculator engine.
//!
//! The RPN stack + arbitrary-precision math core, as a **plain `no_std` library**
//! you can unit-test and run against on the host. There is a **single** numeric
//! path: real **GNU MP** (integers) + **MPFR** (floats, correctly-rounded
//! transcendentals) via our own no_std bindings (`gmp-mpfr-nostd`) — no `std`,
//! no feature-gated fallback. The same crate builds for the bare-metal target.
//!
//! ```
//! use calcumaker_core::{Calc, Radix};
//! let mut c = Calc::new(256);          // 256-bit working precision
//! for t in ["2", "3", "+"] { c.input(t).unwrap(); }
//! assert_eq!(c.display(), "5");
//! c.set_radix(Radix::Hex);
//! for t in ["ff", "0f", "and"] { c.input(t).unwrap(); }
//! assert_eq!(c.display(), "F");
//! ```
#![cfg_attr(not(test), no_std)]

extern crate alloc;

mod app;
mod calc;
mod format;
pub mod keydoc;
pub mod keys;
pub mod seg7;
mod value;

pub use app::App;
pub use calc::{AngleMode, Calc, CalcError, FloatFmt, Radix, SignMode, StackModel};
pub use keys::Key;
pub use value::Value;

/// Format any [`Value`] for display under the calculator's current modes
/// (radix, word size / sign mode, FIX/SCI/ENG) — the same formatting
/// [`Calc::display`] uses for the top of stack.
pub use format::format as display_value;
