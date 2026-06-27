//! Calcumaker 16 calculator engine.
//!
//! The RPN stack + arbitrary-precision math core, as a **plain library** you can
//! unit-test and run against on the host. There is a **single** numeric path:
//! real **GNU MP** (integers) + **MPFR** (floats, correctly-rounded
//! transcendentals) via the [`rug`] crate, which vendors and builds the C
//! libraries itself — no system packages, no feature-gated fallback.
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

mod calc;
mod format;
mod value;

pub use calc::{Calc, CalcError, Radix};
pub use value::Value;

/// Format any [`Value`] for display in the given radix / precision (the same
/// formatting [`Calc::display`] uses for the top of stack).
pub use format::format as display_value;
