//! The calculator *application* — everything between a key press and the
//! segment bytes on the display, independent of hardware.
//!
//! Frontends feed raw `(row, col)` presses (firmware: Cherry MX matrix scan;
//! emulator: host keyboard) and render [`App::seg_rows`] (firmware: TM1640
//! chain; emulator: ASCII art). [`App`] owns the f/g shift resolution
//! ([`crate::keys`]), HP-style digit-by-digit entry editing, the [`Calc`]
//! engine, and row formatting — one code path for the device and the emulator.
//!
//! Entry model (simplified HP): digit/`.`/EEX keys edit a live entry buffer
//! shown in the X row with a `_` cursor; ENTER pushes it (or duplicates X when
//! not entering); any operation key first pushes the pending entry, then
//! applies. CHS during entry flips the exponent sign if after EEX, else the
//! mantissa sign. Backspace edits the entry only; CLx cancels the entry or
//! drops X.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::calc::{Calc, CalcError, Radix};
use crate::keys::{Key, Shift};
use crate::seg7::{self, DIGITS_PER_ROW, DISPLAY_ROWS};

pub struct App {
    calc: Calc,
    shift: Shift,
    entry: Option<String>,
    msg: Option<&'static str>,
}

impl App {
    /// New calculator app at `prec` bits of MPFR working precision.
    pub fn new(prec: u32) -> Self {
        Self {
            calc: Calc::new(prec),
            shift: Shift::None,
            entry: None,
            msg: None,
        }
    }

    // ---- state for annunciators --------------------------------------------
    pub fn calc(&self) -> &Calc {
        &self.calc
    }
    /// Pending shift for the annunciator: `Some('f')` / `Some('g')`.
    pub fn shift(&self) -> Option<char> {
        match self.shift {
            Shift::None => None,
            Shift::F => Some('f'),
            Shift::G => Some('g'),
        }
    }
    /// Status blip from the last press (error, unassigned key, …).
    pub fn message(&self) -> Option<&'static str> {
        self.msg
    }

    // ---- input --------------------------------------------------------------
    /// A physical key press at matrix position `(row, col)`, resolved through
    /// the active shift layer.
    pub fn press(&mut self, row: usize, col: usize) {
        if let Some(k) = self.shift.resolve(row, col) {
            self.press_key(k);
        }
    }

    /// A resolved logical key (shift layers already applied) — the `press`
    /// backend, public for tests and scripted input.
    pub fn press_key(&mut self, k: Key) {
        self.msg = None;
        match k {
            Key::Digit(n) => self.digit(n),
            Key::Dot => self.dot(),
            Key::Eex => self.eex(),
            Key::Chs => self.chs(),
            Key::Back => self.backspace(),
            Key::ClrX => self.clear_x(),
            Key::Enter => self.enter(),
            Key::Off => self.msg = Some("off"),
            Key::Nop | Key::ShiftF | Key::ShiftG => {}
            other => match token_for(other) {
                Some(tok) => {
                    self.flush();
                    self.run(tok);
                }
                None => self.msg = Some("not implemented"),
            },
        }
    }

    // ---- entry editing ------------------------------------------------------
    fn digit(&mut self, n: u8) {
        if i32::from(n) >= self.calc.radix().base() {
            self.msg = Some("bad digit for radix");
            return;
        }
        let c = char::from_digit(u32::from(n), 16).unwrap().to_ascii_uppercase();
        self.entry.get_or_insert_with(String::new).push(c);
    }

    fn dot(&mut self) {
        if self.calc.radix() != Radix::Dec {
            self.msg = Some("reals are decimal only");
            return;
        }
        match &mut self.entry {
            None => self.entry = Some("0.".to_string()),
            Some(b) if b.contains('.') || b.contains('e') => self.msg = Some("misplaced ."),
            Some(b) => b.push('.'),
        }
    }

    fn eex(&mut self) {
        if self.calc.radix() != Radix::Dec {
            self.msg = Some("reals are decimal only");
            return;
        }
        match &mut self.entry {
            None => self.entry = Some("1e".to_string()),
            Some(b) if b.contains('e') => self.msg = Some("misplaced EEX"),
            Some(b) => b.push('e'),
        }
    }

    /// CHS: during entry flip the exponent sign (after EEX) or the mantissa
    /// sign; otherwise negate X in the engine.
    fn chs(&mut self) {
        match &mut self.entry {
            Some(b) => {
                let at = b.find('e').map(|i| i + 1).unwrap_or(0);
                if b[at..].starts_with('-') {
                    b.remove(at);
                } else {
                    b.insert(at, '-');
                }
            }
            None => self.run("chs"),
        }
    }

    /// Backspace edits the live entry only (cancels it on the last digit).
    fn backspace(&mut self) {
        if let Some(b) = &mut self.entry {
            b.pop();
            if b.is_empty() || b == "-" {
                self.entry = None;
            }
        }
    }

    /// CLx: cancel a pending entry, else drop X.
    fn clear_x(&mut self) {
        if self.entry.take().is_none() {
            self.run("drop");
        }
    }

    /// ENTER: push a pending entry, else duplicate X.
    fn enter(&mut self) {
        if self.entry.is_some() {
            self.flush();
        } else {
            self.run("enter");
        }
    }

    /// Push the pending entry onto the stack (no-op without one). An entry cut
    /// short mid-exponent (`3e`, `3e-`) or bare-sign is completed/dropped.
    fn flush(&mut self) {
        let Some(mut s) = self.entry.take() else { return };
        for suffix in ["e-", "e+", "e", ".", "-"] {
            if let Some(t) = s.strip_suffix(suffix) {
                s = t.to_string();
            }
        }
        if s.is_empty() {
            return;
        }
        self.run_owned(&s);
    }

    fn run(&mut self, tok: &'static str) {
        self.run_owned(tok);
    }

    fn run_owned(&mut self, tok: &str) {
        if let Err(e) = self.calc.input(tok) {
            self.msg = Some(match e {
                CalcError::Parse(_) => "parse error",
                CalcError::Empty => "stack empty",
                CalcError::TypeError(t) => t,
                CalcError::DivZero => "divide by zero",
            });
        }
    }

    // ---- display ------------------------------------------------------------
    /// The display rows as text, index 0 = top. X (or the live entry, cursor
    /// `_`) is the bottom row, Y above it, … — the top of the RPN stack.
    pub fn text_rows(&self) -> [String; DISPLAY_ROWS] {
        let mut items: Vec<String> = self
            .calc
            .stack()
            .iter()
            .map(|v| crate::format::format(v, self.calc.radix(), self.calc.prec()))
            .collect();
        if let Some(b) = &self.entry {
            let mut line = b.clone();
            line.push('_');
            items.push(line);
        }
        let mut rows: [String; DISPLAY_ROWS] = Default::default();
        for i in 0..DISPLAY_ROWS {
            // bottom row (last index) gets the last item (X / entry)
            if let Some(item) = items.len().checked_sub(1 + i).map(|n| &items[n]) {
                rows[DISPLAY_ROWS - 1 - i] = item.clone();
            }
        }
        rows
    }

    /// The display rows as TM1640 segment bytes, index 0 = top — exactly what
    /// the hardware shows (the emulator renders these same bytes).
    pub fn seg_rows(&self) -> [[u8; DIGITS_PER_ROW]; DISPLAY_ROWS] {
        self.text_rows().map(|t| seg7::encode_row(&t))
    }
}

/// Engine token for a logical key; `None` = not implemented in the engine yet
/// (rotate/bit ops, sto/rcl, %, round, float/sign modes — see DESIGN.md).
fn token_for(k: Key) -> Option<&'static str> {
    Some(match k {
        Key::Add => "+",
        Key::Sub => "-",
        Key::Mul => "*",
        Key::Div => "/",
        Key::Swap => "swap",
        Key::RollDn => "rolldn",
        Key::RollUp => "rollup",
        Key::LastX => "lastx",
        Key::Hex => "hex",
        Key::Dec => "dec",
        Key::Oct => "oct",
        Key::Bin => "bin",
        Key::WordSize => "wsize",
        Key::Prec => "prec",
        Key::And => "and",
        Key::Or => "or",
        Key::Xor => "xor",
        Key::Not => "not",
        Key::Shl => "shl",
        Key::Shr => "shr",
        Key::Rmd => "mod",
        Key::Sin => "sin",
        Key::Cos => "cos",
        Key::Tan => "tan",
        Key::Asin => "asin",
        Key::Acos => "acos",
        Key::Atan => "atan",
        Key::Sinh => "sinh",
        Key::Cosh => "cosh",
        Key::Tanh => "tanh",
        Key::Ln => "ln",
        Key::Exp => "exp",
        Key::Log10 => "log",
        Key::Exp10 => "exp10",
        Key::Sqrt => "sqrt",
        Key::Sq => "sq",
        Key::Pow => "pow",
        Key::Recip => "inv",
        Key::Pi => "pi",
        Key::Fact => "fact",
        _ => return None,
    })
}
