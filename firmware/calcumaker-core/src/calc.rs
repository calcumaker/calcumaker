//! The RPN calculator engine: a value stack + operations over the
//! arbitrary-precision [`Value`] type. One numeric path (GMP + MPFR via rug).
//!
//! Integers stay integers through `+ - * /` and the bitwise ops (HP-16C
//! programmer model, masked to the word size); the scientific functions promote
//! to MPFR reals. Input is token-at-a-time so it drives both the REPL and tests.

use core::cmp::Ordering;

use rug::float::Constant;
use rug::{Float, Integer};

use crate::value::Value;

/// Integer display / entry radix (HP-16C programmer modes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Radix {
    Hex,
    Dec,
    Oct,
    Bin,
}

impl Radix {
    pub fn base(self) -> i32 {
        match self {
            Radix::Hex => 16,
            Radix::Dec => 10,
            Radix::Oct => 8,
            Radix::Bin => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalcError {
    /// Token was neither a number (in the active radix) nor a known command.
    Parse(String),
    /// Stack underflow.
    Empty,
    /// Wrong operand type for the operation.
    TypeError(&'static str),
    /// Division by zero.
    DivZero,
}

pub struct Calc {
    stack: Vec<Value>,
    prec: u32,
    radix: Radix,
    word_bits: Option<u32>,
}

impl Calc {
    /// New calculator with `prec` bits of MPFR working precision (e.g. 256 ≈ 77
    /// decimal digits). Decimal radix, unbounded integers.
    pub fn new(prec: u32) -> Self {
        Self { stack: Vec::new(), prec: prec.max(2), radix: Radix::Dec, word_bits: None }
    }

    // ---- state ------------------------------------------------------------
    pub fn prec(&self) -> u32 {
        self.prec
    }
    pub fn set_prec(&mut self, prec: u32) {
        self.prec = prec.max(2);
    }
    pub fn radix(&self) -> Radix {
        self.radix
    }
    pub fn set_radix(&mut self, r: Radix) {
        self.radix = r;
    }
    /// Word size in bits for the programmer modes; `None` = unbounded (GMP).
    pub fn set_word_bits(&mut self, bits: Option<u32>) {
        self.word_bits = bits;
    }
    pub fn stack(&self) -> &[Value] {
        &self.stack
    }

    /// Format the top of stack (X) for the display; empty string if the stack is
    /// empty.
    pub fn display(&self) -> String {
        match self.stack.last() {
            Some(v) => crate::format::format(v, self.radix, self.prec),
            None => String::new(),
        }
    }

    // ---- input ------------------------------------------------------------
    /// Feed one token: a number (pushed) or a command (applied). Commands are
    /// matched first, so `and`/`dec`/... are operators even in hex mode.
    pub fn input(&mut self, tok: &str) -> Result<(), CalcError> {
        let t = tok.trim();
        if t.is_empty() {
            return Ok(());
        }
        let lower = t.to_ascii_lowercase();
        if is_command(&lower) {
            return self.command(&lower);
        }
        if let Some(v) = self.try_parse_number(t) {
            self.stack.push(v);
            return Ok(());
        }
        Err(CalcError::Parse(t.to_string()))
    }

    fn try_parse_number(&self, t: &str) -> Option<Value> {
        if self.radix == Radix::Dec && (t.contains('.') || t.contains('e') || t.contains('E')) {
            return Float::parse(t).ok().map(|p| Value::Real(Float::with_val(self.prec, p)));
        }
        Integer::from_str_radix(t, self.radix.base()).ok().map(Value::Int)
    }

    // ---- dispatch ---------------------------------------------------------
    fn command(&mut self, cmd: &str) -> Result<(), CalcError> {
        match cmd {
            "+" => self.arith('+'),
            "-" => self.arith('-'),
            "*" => self.arith('*'),
            "/" => self.arith('/'),
            "chs" => self.chs(),
            "swap" => self.swap(),
            "drop" => self.pop().map(|_| ()),
            "dup" => self.dup(),
            "sqrt" => self.unary_real(|x| x.sqrt()),
            "sin" => self.unary_real(|x| x.sin()),
            "cos" => self.unary_real(|x| x.cos()),
            "tan" => self.unary_real(|x| x.tan()),
            "ln" => self.unary_real(|x| x.ln()),
            "exp" => self.unary_real(|x| x.exp()),
            "inv" => self.unary_real(|x| x.recip()),
            "sq" => self.unary_real(|x| {
                let y = x.clone();
                x * y
            }),
            "pi" => {
                self.stack.push(Value::Real(Float::with_val(self.prec, Constant::Pi)));
                Ok(())
            }
            "and" => self.bitwise('&'),
            "or" => self.bitwise('|'),
            "xor" => self.bitwise('^'),
            "not" => self.not_op(),
            "shl" => self.shift(true),
            "shr" => self.shift(false),
            "fact" | "!" => self.fact(),
            "hex" => self.set_radix_ok(Radix::Hex),
            "dec" => self.set_radix_ok(Radix::Dec),
            "oct" => self.set_radix_ok(Radix::Oct),
            "bin" => self.set_radix_ok(Radix::Bin),
            _ => Err(CalcError::Parse(cmd.to_string())),
        }
    }

    // ---- helpers ----------------------------------------------------------
    fn pop(&mut self) -> Result<Value, CalcError> {
        self.stack.pop().ok_or(CalcError::Empty)
    }

    fn set_radix_ok(&mut self, r: Radix) -> Result<(), CalcError> {
        self.radix = r;
        Ok(())
    }

    fn mask(&self, r: Integer) -> Integer {
        match self.word_bits {
            Some(n) => {
                let m = (Integer::from(1) << n) - Integer::from(1);
                r & m
            }
            None => r,
        }
    }

    fn arith(&mut self, op: char) -> Result<(), CalcError> {
        let b = self.pop()?;
        let a = self.pop()?;
        let v = match (a, b) {
            (Value::Int(x), Value::Int(y)) => {
                let r = match op {
                    '+' => x + y,
                    '-' => x - y,
                    '*' => x * y,
                    '/' => {
                        if y.cmp0() == Ordering::Equal {
                            return Err(CalcError::DivZero);
                        }
                        x / y
                    }
                    _ => unreachable!(),
                };
                Value::Int(self.mask(r))
            }
            (a, b) => {
                let x = a.to_real(self.prec);
                let y = b.to_real(self.prec);
                let r = match op {
                    '+' => x + y,
                    '-' => x - y,
                    '*' => x * y,
                    '/' => x / y,
                    _ => unreachable!(),
                };
                Value::Real(r)
            }
        };
        self.stack.push(v);
        Ok(())
    }

    fn bitwise(&mut self, op: char) -> Result<(), CalcError> {
        let b = self.pop()?;
        let a = self.pop()?;
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => {
                let r = match op {
                    '&' => x & y,
                    '|' => x | y,
                    '^' => x ^ y,
                    _ => unreachable!(),
                };
                self.stack.push(Value::Int(self.mask(r)));
                Ok(())
            }
            _ => Err(CalcError::TypeError("bitwise needs integers")),
        }
    }

    fn not_op(&mut self) -> Result<(), CalcError> {
        match self.pop()? {
            Value::Int(x) => {
                let r = self.mask(!x);
                self.stack.push(Value::Int(r));
                Ok(())
            }
            _ => Err(CalcError::TypeError("not needs an integer")),
        }
    }

    fn shift(&mut self, left: bool) -> Result<(), CalcError> {
        let cnt = self.pop()?;
        let val = self.pop()?;
        let n = match cnt {
            Value::Int(c) => c.to_u32().ok_or(CalcError::TypeError("shift count out of range"))?,
            _ => return Err(CalcError::TypeError("shift count must be an integer")),
        };
        let x = match val {
            Value::Int(x) => x,
            _ => return Err(CalcError::TypeError("shift value must be an integer")),
        };
        let r = if left { x << n } else { x >> n };
        self.stack.push(Value::Int(self.mask(r)));
        Ok(())
    }

    fn fact(&mut self) -> Result<(), CalcError> {
        match self.pop()? {
            Value::Int(x) => {
                let n = x.to_u32().ok_or(CalcError::TypeError("factorial needs a small non-negative integer"))?;
                self.stack.push(Value::Int(Integer::from(Integer::factorial(n))));
                Ok(())
            }
            _ => Err(CalcError::TypeError("factorial needs an integer")),
        }
    }

    fn chs(&mut self) -> Result<(), CalcError> {
        let v = match self.pop()? {
            Value::Int(x) => Value::Int(-x),
            Value::Real(f) => Value::Real(-f),
        };
        self.stack.push(v);
        Ok(())
    }

    fn swap(&mut self) -> Result<(), CalcError> {
        let b = self.pop()?;
        let a = self.pop()?;
        self.stack.push(b);
        self.stack.push(a);
        Ok(())
    }

    fn dup(&mut self) -> Result<(), CalcError> {
        let top = self.stack.last().ok_or(CalcError::Empty)?.clone();
        self.stack.push(top);
        Ok(())
    }

    fn unary_real(&mut self, f: impl FnOnce(Float) -> Float) -> Result<(), CalcError> {
        let x = self.pop()?.to_real(self.prec);
        self.stack.push(Value::Real(f(x)));
        Ok(())
    }
}

fn is_command(t: &str) -> bool {
    matches!(
        t,
        "+" | "-" | "*" | "/" | "chs" | "swap" | "drop" | "dup" | "sqrt" | "sin"
            | "cos" | "tan" | "ln" | "exp" | "inv" | "sq" | "pi" | "and" | "or"
            | "xor" | "not" | "shl" | "shr" | "fact" | "!" | "hex" | "dec"
            | "oct" | "bin"
    )
}
