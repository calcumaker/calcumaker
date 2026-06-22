//! RPN stack + evaluation (programmer's model, HP-16C lineage) over the
//! arbitrary-precision [`crate::numeric`] core.
//!
//! The classic visible stack is X/Y/Z/T, but because values are arbitrary
//! precision we back it with a growable `Vec` and present the top rows on the
//! display. Host-testable (keep HAL-free).

use alloc::vec::Vec;

use crate::keypad::{Key, Op};
use crate::numeric::{Number, Radix};

/// In-progress numeric entry (digits typed but not yet pushed).
#[derive(Default)]
struct Entry {
    buf: alloc::string::String,
    active: bool,
}

pub struct Stack {
    regs: Vec<Number>,
    entry: Entry,
    pub radix: Radix,
    /// Word size for the programmer modes (bits); `None` = unbounded (GMP).
    pub word_bits: Option<u16>,
}

impl Stack {
    pub fn new() -> Self {
        Self {
            regs: Vec::new(),
            entry: Entry::default(),
            radix: Radix::Dec,
            word_bits: None,
        }
    }

    /// The visible registers, top (X) first.
    pub fn top(&self, n: usize) -> impl Iterator<Item = &Number> {
        self.regs.iter().rev().take(n)
    }

    /// Handle one decoded keypress.
    pub fn handle(&mut self, key: Key) {
        match key {
            Key::Digit(_d) => {
                // TODO: append to entry.buf honoring self.radix.
                self.entry.active = true;
            }
            Key::Enter => self.commit_entry(),
            Key::Op(op) => self.apply_op(op),
            Key::Func(_f) => { /* TODO: MPFR transcendental on X */ }
        }
    }

    fn commit_entry(&mut self) {
        if self.entry.active {
            // TODO: parse entry.buf in self.radix -> Number, push.
            self.regs.push(Number::zero());
            self.entry = Entry::default();
        } else if let Some(x) = self.regs.last().cloned() {
            self.regs.push(x); // ENTER with no entry duplicates X
        }
    }

    fn apply_op(&mut self, _op: Op) {
        self.commit_entry();
        // TODO: pop Y, X; compute via numeric backend (honoring word_bits for
        // integer modes); push result.
    }
}
