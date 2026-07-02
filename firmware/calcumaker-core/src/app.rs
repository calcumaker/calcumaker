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

/// A pending STO/RCL waiting for its register digit (0–F).
#[derive(Clone, Copy, PartialEq, Eq)]
enum RegOp {
    Sto,
    Rcl,
}

/// One SETUP-menu entry: a runtime tunable with a 7-seg-renderable name,
/// a value renderer, and a cycle/toggle action. Numeric settings (prec,
/// wsize, FIX digits) stay RPN-postfix — they need digit entry, which the
/// keypad already does well. Future: the personality selector (DESIGN-MODES).
#[derive(Clone, Copy, PartialEq, Eq)]
enum SetupItem {
    Suffix,
    LeadZeros,
    Angle,
    Sign,
}

const SETUP_ITEMS: [SetupItem; 4] =
    [SetupItem::Suffix, SetupItem::LeadZeros, SetupItem::Angle, SetupItem::Sign];

impl SetupItem {
    fn name(self) -> &'static str {
        match self {
            SetupItem::Suffix => "SUFF",   // the h/o/b base letter on the glass
            SetupItem::LeadZeros => "LEAd 0",
            SetupItem::Angle => "AnGLE",
            SetupItem::Sign => "SIGn",
        }
    }

    fn value(self, c: &Calc) -> &'static str {
        let onoff = |on: bool| if on { "on" } else { "oFF" };
        match self {
            SetupItem::Suffix => onoff(c.radix_suffix()),
            SetupItem::LeadZeros => onoff(c.leading_zeros()),
            SetupItem::Angle => match c.angle_mode() {
                crate::calc::AngleMode::Rad => "rAd",
                crate::calc::AngleMode::Deg => "dEG",
                crate::calc::AngleMode::Grad => "GrAd",
            },
            SetupItem::Sign => match c.sign_mode() {
                crate::calc::SignMode::Twos => "2S",
                crate::calc::SignMode::Ones => "1S",
                crate::calc::SignMode::Unsigned => "UnS",
            },
        }
    }

    fn cycle(self, c: &mut Calc) {
        match self {
            SetupItem::Suffix => c.set_radix_suffix(!c.radix_suffix()),
            SetupItem::LeadZeros => c.set_leading_zeros(!c.leading_zeros()),
            SetupItem::Angle => c.set_angle_mode(match c.angle_mode() {
                crate::calc::AngleMode::Rad => crate::calc::AngleMode::Deg,
                crate::calc::AngleMode::Deg => crate::calc::AngleMode::Grad,
                crate::calc::AngleMode::Grad => crate::calc::AngleMode::Rad,
            }),
            SetupItem::Sign => c.set_sign_mode(match c.sign_mode() {
                crate::calc::SignMode::Twos => crate::calc::SignMode::Ones,
                crate::calc::SignMode::Ones => crate::calc::SignMode::Unsigned,
                crate::calc::SignMode::Unsigned => crate::calc::SignMode::Twos,
            }),
        }
    }
}

pub struct App {
    calc: Calc,
    shift: Shift,
    entry: Option<String>,
    pending_reg: Option<RegOp>,
    /// Display window over an X wider than the row (0 = default view).
    win: usize,
    /// STATUS view active — the glass shows modes/flags instead of the stack.
    status_view: bool,
    /// SETUP menu active — index of the selected item.
    setup: Option<usize>,
    msg: Option<String>,
}

impl App {
    /// New calculator app at `prec` bits of MPFR working precision.
    pub fn new(prec: u32) -> Self {
        Self {
            calc: Calc::new(prec),
            shift: Shift::None,
            entry: None,
            pending_reg: None,
            win: 0,
            status_view: false,
            setup: None,
            msg: None,
        }
    }

    // ---- state for annunciators --------------------------------------------
    pub fn calc(&self) -> &Calc {
        &self.calc
    }
    /// Mutable engine access for configuration tunables (frontends: CLI
    /// flags, saved settings). Key-driven changes go through `press`.
    pub fn calc_mut(&mut self) -> &mut Calc {
        &mut self.calc
    }
    /// Pending shift for the annunciator: `Some('f')` / `Some('g')`.
    pub fn shift(&self) -> Option<char> {
        match self.shift {
            Shift::None => None,
            Shift::F => Some('f'),
            Shift::G => Some('g'),
        }
    }
    /// Status blip from the last press (error, SHOW view, unassigned key, …).
    pub fn message(&self) -> Option<&str> {
        self.msg.as_deref()
    }
    /// STO/RCL waiting for a register digit: `Some("STO")` / `Some("RCL")`.
    pub fn pending_register(&self) -> Option<&'static str> {
        self.pending_reg.map(|r| match r {
            RegOp::Sto => "STO",
            RegOp::Rcl => "RCL",
        })
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
        // SETUP menu captures navigation until dismissed.
        if let Some(i) = self.setup {
            match k {
                Key::RollDn => self.setup = Some((i + 1) % SETUP_ITEMS.len()),
                Key::RollUp => self.setup = Some((i + SETUP_ITEMS.len() - 1) % SETUP_ITEMS.len()),
                Key::Enter => SETUP_ITEMS[i].cycle(&mut self.calc),
                Key::Setup | Key::ClrX | Key::Back => self.setup = None,
                Key::ShiftF | Key::ShiftG | Key::Nop => {}
                _ => self.msg = Some("SEtUP: R-dn/up moves, ENTER changes, CLx exits".into()),
            }
            return;
        }
        if k == Key::Setup {
            self.setup = Some(0);
            self.status_view = false;
            self.win = 0;
            return;
        }
        // STATUS is momentary (16C): it takes the glass until the next key.
        if k == Key::Status {
            self.status_view = true;
            self.win = 0;
            return;
        }
        self.status_view = false;
        // Window scrolling is pure display navigation; any other key resets
        // the window to the default view.
        if matches!(k, Key::WinL | Key::WinR) {
            let (cur, total) = self.window();
            self.win = if k == Key::WinR {
                (cur + 1).min(total - 1)
            } else {
                cur.saturating_sub(1)
            };
            return;
        }
        self.win = 0;
        // A pending STO/RCL claims the next digit key as its register (0-F,
        // radix-independent); any other key cancels it.
        if let Some(op) = self.pending_reg.take() {
            match k {
                Key::Digit(n) => {
                    let prefix = match op {
                        RegOp::Sto => "sto",
                        RegOp::Rcl => "rcl",
                    };
                    let tok = alloc::format!("{prefix}{n:x}");
                    self.run_owned(&tok);
                    return;
                }
                _ => self.msg = Some("register select cancelled".into()),
            }
            return;
        }
        match k {
            Key::Digit(n) => self.digit(n),
            Key::Sto => {
                self.flush();
                self.pending_reg = Some(RegOp::Sto);
            }
            Key::Rcl => {
                self.flush();
                self.pending_reg = Some(RegOp::Rcl);
            }
            Key::Dot => self.dot(),
            Key::Eex => self.eex(),
            Key::Chs => self.chs(),
            Key::Back => self.backspace(),
            Key::ClrX => self.clear_x(),
            Key::Enter => self.enter(),
            Key::ShowHex => self.show(Radix::Hex),
            Key::ShowDec => self.show(Radix::Dec),
            Key::ShowOct => self.show(Radix::Oct),
            Key::ShowBin => self.show(Radix::Bin),
            Key::Off => self.msg = Some("off".into()),
            Key::Nop | Key::ShiftF | Key::ShiftG => {}
            other => match token_for(other) {
                Some(tok) => {
                    self.flush();
                    self.run(tok);
                }
                None => self.msg = Some("not implemented".into()),
            },
        }
    }

    /// SHOW — X momentarily in another base, in the status line (16C f-SHOW).
    fn show(&mut self, r: Radix) {
        self.flush();
        let tag = match r {
            Radix::Hex => "hex",
            Radix::Dec => "dec",
            Radix::Oct => "oct",
            Radix::Bin => "bin",
        };
        self.msg = Some(alloc::format!("{tag}: {}", self.calc.show_in(r)));
    }

    // ---- entry editing ------------------------------------------------------
    fn digit(&mut self, n: u8) {
        if i32::from(n) >= self.calc.radix().base() {
            self.msg = Some("bad digit for radix".into());
            return;
        }
        let c = char::from_digit(u32::from(n), 16).unwrap().to_ascii_uppercase();
        self.entry.get_or_insert_with(String::new).push(c);
    }

    fn dot(&mut self) {
        if self.calc.radix() != Radix::Dec {
            self.msg = Some("reals are decimal only".into());
            return;
        }
        match &mut self.entry {
            None => self.entry = Some("0.".to_string()),
            Some(b) if b.contains('.') || b.contains('e') => self.msg = Some("misplaced .".into()),
            Some(b) => b.push('.'),
        }
    }

    fn eex(&mut self) {
        if self.calc.radix() != Radix::Dec {
            self.msg = Some("reals are decimal only".into());
            return;
        }
        match &mut self.entry {
            None => self.entry = Some("1e".to_string()),
            Some(b) if b.contains('e') => self.msg = Some("misplaced EEX".into()),
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
            self.msg = Some(
                match e {
                    CalcError::Parse(_) => "parse error",
                    CalcError::Empty => "stack empty",
                    CalcError::TypeError(t) => t,
                    CalcError::DivZero => "divide by zero",
                }
                .into(),
            );
        }
    }

    // ---- display ------------------------------------------------------------
    /// X at full precision (or the live entry with its cursor) — the SHOW view;
    /// the glass rows round AUTO reals to the window instead.
    pub fn x_full(&self) -> String {
        if let Some(b) = &self.entry {
            let mut line = b.clone();
            line.push('_');
            return line;
        }
        self.calc
            .stack()
            .last()
            .map(|v| crate::format::format(v, &self.calc))
            .unwrap_or_default()
    }

    /// The STATUS view (16C f-STATUS) as three glass lines, every character
    /// 7-seg renderable ('X'/'W'/'Z' don't exist, hence the spellings):
    /// base + sign mode + angle unit, precision + word bits (`b0` =
    /// unbounded), real format + the six flags as bits 543210
    /// (G · C · lz · F2 · F1 · F0).
    fn status_rows(&self) -> [String; DISPLAY_ROWS] {
        let c = &self.calc;
        let sign = match c.sign_mode() {
            crate::calc::SignMode::Twos => "2S",
            crate::calc::SignMode::Ones => "1S",
            crate::calc::SignMode::Unsigned => "UnS",
        };
        let angle = match c.angle_mode() {
            crate::calc::AngleMode::Rad => "rAd",
            crate::calc::AngleMode::Deg => "dEG",
            crate::calc::AngleMode::Grad => "GrAd",
        };
        let fmt = match c.float_fmt() {
            crate::calc::FloatFmt::Auto => "AUtO".to_string(),
            crate::calc::FloatFmt::Fix(d) => alloc::format!("FI {d}"),
            crate::calc::FloatFmt::Sci(d) => alloc::format!("SCI {d}"),
            crate::calc::FloatFmt::Eng(d) => alloc::format!("EnG {d}"),
        };
        let bits: String = [
            c.overflow(),
            c.carry(),
            c.leading_zeros(),
            c.user_flag(2),
            c.user_flag(1),
            c.user_flag(0),
        ]
        .iter()
        .map(|&f| if f { '1' } else { '0' })
        .collect();
        let mut rows = [
            alloc::format!("bASE {} {sign} {angle}", c.radix().base()),
            alloc::format!("P{} b{}", c.prec(), c.word_bits().unwrap_or(0)),
            alloc::format!("{fmt} {bits}"),
        ];
        for r in &mut rows {
            // pad to the row width so the text renders left-aligned
            while r.len() < DIGITS_PER_ROW {
                r.push(' ');
            }
        }
        rows
    }

    /// The display rows as text, index 0 = top. X (or the live entry, cursor
    /// `_`) is the bottom row, Y above it, … — the top of the RPN stack.
    /// AUTO-mode reals are display-rounded to the row width (HP behaviour —
    /// the stored value keeps full precision, see [`App::x_full`]). With the
    /// STATUS view active, the rows are the mode summary instead.
    pub fn text_rows(&self) -> [String; DISPLAY_ROWS] {
        if let Some(i) = self.setup {
            let item = SETUP_ITEMS[i];
            let mut rows = [
                "SEtUP".to_string(),
                alloc::format!("{} {}", i + 1, item.name()),
                item.value(&self.calc).to_string(),
            ];
            for r in &mut rows {
                while r.len() < DIGITS_PER_ROW {
                    r.push(' ');
                }
            }
            return rows;
        }
        if self.status_view {
            return self.status_rows();
        }
        let mut items: Vec<String> = self
            .calc
            .stack()
            .iter()
            .map(|v| crate::format::format_fit(v, &self.calc, DIGITS_PER_ROW))
            .collect();
        // 16C radix letter on the X readout — the only base indicator the
        // glass has (hardware carries no radix lamps). Non-decimal integers
        // get " h"/" o"/" b"; a bare number is decimal (deviation from the
        // 16C, which also marks `d`). Skipped when the value alone already
        // fills the row (the window annunciator + STATUS carry the base).
        // A display tunable — `suffix` toggles it (Calc::radix_suffix).
        if self.entry.is_none() && self.calc.radix_suffix() {
            let letter = match self.calc.radix() {
                Radix::Hex => Some('h'),
                Radix::Oct => Some('o'),
                Radix::Bin => Some('b'),
                Radix::Dec => None,
            };
            if let (Some(l), Some(crate::Value::Int(_)), Some(last)) =
                (letter, self.calc.stack().last(), items.last_mut())
            {
                // integer texts carry no dots: chars == display cells
                if last.chars().count() + 2 <= DIGITS_PER_ROW {
                    last.push(' ');
                    last.push(l);
                }
            }
        }
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
    /// the hardware shows (the emulator renders these same bytes). With a
    /// window selected (16C `<`/`>`), the X row scrolls: window 0 is the
    /// fitted view (15 cells + the overflow marker), and window k ≥ 1 picks
    /// up **exactly where the marker cut off** (cell 15 + 16·(k−1)),
    /// left-aligned — every digit is reachable.
    pub fn seg_rows(&self) -> [[u8; DIGITS_PER_ROW]; DISPLAY_ROWS] {
        let texts = self.text_rows();
        let mut rows = [[0u8; DIGITS_PER_ROW]; DISPLAY_ROWS];
        for (i, t) in texts.iter().enumerate() {
            rows[i] = seg7::encode_row(t);
        }
        if self.win > 0 {
            let cells = seg7::encode_cells(&texts[DISPLAY_ROWS - 1]);
            let start = DIGITS_PER_ROW * self.win - 1;
            let mut row = [0u8; DIGITS_PER_ROW];
            for (i, c) in cells.iter().skip(start).take(DIGITS_PER_ROW).enumerate() {
                row[i] = *c;
            }
            rows[DISPLAY_ROWS - 1] = row;
        }
        rows
    }

    /// Display window position: `(current, total)` — total > 1 means X is
    /// wider than the row and the window keys will scroll it.
    pub fn window(&self) -> (usize, usize) {
        let len = seg7::encode_cells(&self.text_rows()[DISPLAY_ROWS - 1]).len();
        let total = if len <= DIGITS_PER_ROW {
            1
        } else {
            // window 0 shows 15 cells (+ marker); the rest come in 16s
            1 + (len - (DIGITS_PER_ROW - 1)).div_ceil(DIGITS_PER_ROW)
        };
        (self.win.min(total - 1), total)
    }
}

/// Engine token for a logical key; `None` = not a calculator function
/// (Sto/Rcl run through the pending-register flow, Off is a system key).
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
        Key::SignMode => "signmode",
        Key::Float => "float",
        Key::Prec => "prec",
        Key::And => "and",
        Key::Or => "or",
        Key::Xor => "xor",
        Key::Not => "not",
        // the panel shift/rotate keys act on X by one bit (16C style)
        Key::Shl => "sl",
        Key::Shr => "sr",
        Key::Asr => "asr",
        Key::Rotl => "rl",
        Key::Rotr => "rr",
        Key::Rlc => "rlc",
        Key::Rrc => "rrc",
        Key::Lj => "lj",
        Key::DblMul => "dbl*",
        Key::DblDiv => "dbl/",
        Key::DblRem => "dblr",
        Key::Sf => "sf",
        Key::Cf => "cf",
        Key::Ftest => "ftest",
        Key::ClrReg => "clreg",
        Key::BitSet => "bset",
        Key::BitClr => "bclr",
        Key::BitTest => "btest",
        Key::MaskL => "maskl",
        Key::MaskR => "maskr",
        Key::BitCount => "popcnt",
        Key::Rmd => "mod",
        Key::Pct => "pct",
        Key::Round => "round",
        Key::Fix => "fix",
        Key::Sci => "sci",
        Key::Eng => "eng",
        Key::FmtAuto => "std",
        Key::AngleMode => "anglemode",
        Key::Lz => "lz",
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
