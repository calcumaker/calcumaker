//! Calcumaker 16 emulator — the real calculator app on a host terminal.
//!
//! Runs the same `calcumaker_core::App` the firmware hosts: host keys map to
//! the 50-key matrix ([`HOST_KEYS`]), presses resolve through the f/g shift
//! layers, and the display is drawn from the **same TM1640 segment bytes** the
//! hardware would receive — rendered as ASCII 7-segment art.
//!
//! ```sh
//! cargo run                       # interactive (raw-mode) emulator
//! cargo run -- --press "2;3+"     # scripted: press keys, print the frame, exit
//! cargo run -- --prec 512         # working precision in bits
//! ```
//!
//! `;` (or a newline) is ENTER in `--press` scripts.

use std::io::{self, Write};

use calcumaker_core::keys::{COLS, ROWS};
use calcumaker_core::seg7::DIGITS_PER_ROW;
use calcumaker_core::App;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{cursor, event, execute, terminal};

/// Host key for each matrix cell (base-layer faces in the comments; f/g layers
/// are reached through `F` / `G` exactly like the device). `\x08` = Backspace,
/// `\n` = Enter.
#[rustfmt::skip]
const HOST_KEYS: [[char; COLS]; ROWS] = [
    // sin  cos  tan  ln   sqrt y^x  1/x  EEX  back clx
    ['S',  'C', 'T', 'L', 'Q', 'P', 'I', 'E', '\x08', 'X'],
    // A    B    C    D    E    F    7    8    9    ÷
    ['a',  'b', 'c', 'd', 'e', 'f', '7', '8', '9', '/'],
    // and  or   xor  not  shl  shr  4    5    6    ×
    ['&',  '|', '^', '~', '<', '>', '4', '5', '6', '*'],
    // hex  dec  oct  bin  wsiz swap 1    2    3    −
    ['H',  'D', 'O', 'B', 'W', 'x', '1', '2', '3', '-'],
    // f    g    sto  rcl  R↓   ENT  0    .    chs  +
    ['F',  'G', 'm', 'r', 'v', '\n', '0', '.', 'n', '+'],
];

/// Matrix cell for a host key. `;` doubles as ENTER for shell-friendly scripts.
fn cell_for(ch: char) -> Option<(usize, usize)> {
    let ch = if ch == ';' { '\n' } else { ch };
    for r in 0..ROWS {
        for c in 0..COLS {
            if HOST_KEYS[r][c] == ch {
                return Some((r, c));
            }
        }
    }
    None
}

// TM1640 segment bits (see calcumaker_core::seg7): a b c d e f g dp.
const SEG_A: u8 = 0x01;
const SEG_B: u8 = 0x02;
const SEG_C: u8 = 0x04;
const SEG_D: u8 = 0x08;
const SEG_E: u8 = 0x10;
const SEG_F: u8 = 0x20;
const SEG_G: u8 = 0x40;
const SEG_DP: u8 = 0x80;

/// Rendering style for the 7-seg glass.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Unicode block elements — LED-like (default). 5 columns per digit.
    Block,
    /// Plain `_` / `|` — any terminal. 4 columns per digit.
    Ascii,
}

impl Style {
    fn digit_cols(self) -> usize {
        match self {
            Style::Block => 6,
            Style::Ascii => 4,
        }
    }
}

/// One display row of segment bytes as three lines of terminal art.
///
/// Block style, per digit: the top bar spans the full digit width so it meets
/// the corner posts, and the bottom bar falls back into the corner columns
/// when e/c are unlit — segments connect like real glass ('9.' below). The dp
/// is a lower-right quadrant in its own column (half a cell of air on its
/// left) followed by a gap column, so it never merges into a digit:
/// ```text
/// ▄▄▄▄     <- a
/// █▄▄█     <- f g b
/// ▄▄▄█ ▗   <- (d corner) d c, dp
/// ```
fn seg_art(row: &[u8; DIGITS_PER_ROW], style: Style) -> [String; 3] {
    let mut l: [String; 3] = Default::default();
    for &b in row {
        let on = |m: u8| b & m != 0;
        match style {
            Style::Block => {
                let bar = |m: u8| if on(m) { "▄▄" } else { "  " };
                // corner: full-height post, else the horizontal bar passes through
                let corner = |post: u8, bar: u8| {
                    if on(post) {
                        '█'
                    } else if on(bar) {
                        '▄'
                    } else {
                        ' '
                    }
                };
                l[0].push_str(if on(SEG_A) { "▄▄▄▄  " } else { "      " });
                l[1].push(if on(SEG_F) { '█' } else { ' ' });
                l[1].push_str(bar(SEG_G));
                l[1].push(if on(SEG_B) { '█' } else { ' ' });
                l[1].push_str("  ");
                l[2].push(corner(SEG_E, SEG_D));
                l[2].push_str(bar(SEG_D));
                l[2].push(corner(SEG_C, SEG_D));
                l[2].push(if on(SEG_DP) { '▗' } else { ' ' });
                l[2].push(' ');
            }
            Style::Ascii => {
                let seg = |m: u8, ch: char| if on(m) { ch } else { ' ' };
                l[0].push(' ');
                l[0].push(seg(SEG_A, '_'));
                l[0].push(' ');
                l[0].push(' ');
                l[1].push(seg(SEG_F, '|'));
                l[1].push(seg(SEG_G, '_'));
                l[1].push(seg(SEG_B, '|'));
                l[1].push(' ');
                l[2].push(seg(SEG_E, '|'));
                l[2].push(seg(SEG_D, '_'));
                l[2].push(seg(SEG_C, '|'));
                l[2].push(seg(SEG_DP, '.'));
            }
        }
    }
    l
}

const HELP: &str = "\
Host keyboard -> Calcumaker 16 keys (f = gold shift, g = blue shift):

  S sin    C cos    T tan    L ln     Q sqrt   P y^x    I 1/x    E EEX    Bksp back  X CLx
  a..f hex digits A-F                          7 8 9    / divide
  & AND    | OR     ^ XOR    ~ NOT    < SHL    > SHR    4 5 6    * multiply
  H HEX    D DEC    O OCT    B BIN    W wsize  x swap   1 2 3    - subtract
  F f      G g      m STO    r RCL    v roll-dn  Enter ENTER  0  . dot  n CHS  + add

  Shifted (press F or G first, like the device):
    F then S/C/T = asin/acos/atan     F then L = e^x     F then Q = x^2
    F then I = prec (X bits)          F then E = pi      F then Bksp = LASTx
    F then a-f = bit-set/clr/test, maskL/maskR, popcount (index/width from X)
    F then &/|/^  = rotate-l/rotate-r/asr   F then ~ = rmd (Y mod X)
    F then </> = RLC/RRC (rotate through carry)   F then 7 = LJ (left justify)
    F then 4/5/6 = DBLx/DBL-div/DBLR (double-word: product splits into Y:X,
                   dividend Z:Y over X)
    F then H/D/O/B = SHOW x in hex/dec/oct/bin (transient, in the status line)
    F then x = FLOAT (int X -> real)        F then W = sign mode (2's/1's/unsgn)
    F then 8/9 = SF/CF, F then / = F? (flag 0-5 from X; 3=LZ 4=C 5=G)
    F then m = CLR-REG (wipe all registers) F then v = roll-up
    F then X (CLx cell) = STATUS: the glass shows base/sign/angle, prec/word,
                          format + flags until the next key
    G then S/C/T = sinh/cosh/tanh     G then L = log10   G then Q = 10^x
    G then 4/5/6 = x! / % / round (real X -> int)
    G then H/D/O/B = FIX/SCI/ENG/auto (digit count from X)
    G then W = angle mode (RAD -> DEG -> GRAD)
    G then & = leading zeros toggle (pad hex/oct/bin to the word width)
    G then </> = scroll the display window over values wider than 16 digits
    G then X (CLx cell) = SETUP menu: R-dn/R-up (v / F,v) moves, ENTER changes
                          the value, CLx exits (suffix, leading zeros, angle,
                          sign mode; numeric settings stay RPN: 256 prec etc.)

  STO/RCL: press m (STO) or r (RCL), then a digit 0-f = the register.

  SCI personality (--personality sci, or SETUP > PErS): digits/ENTER/shifts/
  arithmetic keep their positions; S/C/T row unchanged; a-f row = asin/acos/
  atan/log10/e^x/10^x; &/|/^/~ row = Sigma+/Sigma-/mean/sdev, </> = x!/%;
  H/D/O/B/W row = FIX/SCI/ENG/auto/angle-mode. F layer: sinh/cosh/tanh, L.R./
  yhat/corr/CLstat. G layer: nCr nPr RAN# seed (over &/|/^/~). Defaults on
  switch: DEG, FIX 4, decimal.

  FIN personality (--personality fin, or SETUP > PErS): a-e cells = the 12C
  TVM row n i PV PMT FV — a keyed number STORES, a bare press SOLVES; f = %;
  &/|/^/~/</> row = CF0 CFj Nj NPV IRR d%; H/D/O/B/W row = FIX/SCI/ENG/auto/
  %T. F layer: 12x 12div (over n/i), SL/SOYD/DB (over 7/8/9), dDYS DATE+ DOW
  x-bar-w (over the CF row), CLFIN (over RCL). G layer: BEG/END (over
  PMT/FV), CLCF (over CF0). Defaults on switch: FIX 2, decimal.

  Modes: bits then W = wsize (0 = unbounded); annunciators show C (carry) and
  G (overflow) in word mode. Esc cancels a pending shift. ? = help. Ctrl-C quits.";

/// Render one full frame: 7-seg rows, annunciators, the untruncated X, footer.
fn frame(app: &App, help: bool, style: Style) -> String {
    let mut out = String::new();
    let width = DIGITS_PER_ROW * style.digit_cols();
    let (tl, tr, bl, br, h, v) = match style {
        Style::Block => ('┌', '┐', '└', '┘', '─', '│'),
        Style::Ascii => ('+', '+', '+', '+', '-', '|'),
    };
    out.push_str("Calcumaker 16 - emulator\n");
    out.push_str(&format!("{tl}{}{tr}\n", h.to_string().repeat(width + 2)));
    let rows = app.seg_rows();
    for (i, row) in rows.iter().enumerate() {
        for line in seg_art(row, style) {
            out.push_str(&format!("{v} {line} {v}\n"));
        }
        if i + 1 < rows.len() {
            out.push_str(&format!("{v}{}{v}\n", " ".repeat(width + 2)));
        }
    }
    out.push_str(&format!("{bl}{}{br}\n", h.to_string().repeat(width + 2)));

    let c = app.calc();
    let pers = if app.keymap().name == "16C" {
        String::new()
    } else {
        format!("{}  ", app.keymap().name)
    };
    let radix = format!("{:?}", c.radix()).to_uppercase();
    let angle = match c.angle_mode() {
        calcumaker_core::AngleMode::Rad => "RAD",
        calcumaker_core::AngleMode::Deg => "DEG",
        calcumaker_core::AngleMode::Grad => "GRAD",
    };
    let word = match c.word_bits() {
        Some(b) => {
            let mode = match c.sign_mode() {
                calcumaker_core::SignMode::Unsigned => "unsgn",
                calcumaker_core::SignMode::Ones => "1's",
                calcumaker_core::SignMode::Twos => "2's",
            };
            format!("word {b} {mode}")
        }
        None => "word unbounded".into(),
    };
    let mut flags = format!(
        "{}{}{}",
        if c.carry() { "  C" } else { "" },
        if c.overflow() { "  G" } else { "" },
        if c.leading_zeros() { "  LZ" } else { "" }
    );
    for i in 0..3 {
        if c.user_flag(i) {
            flags.push_str(&format!("  F{i}"));
        }
    }
    let fmt = match c.float_fmt() {
        calcumaker_core::FloatFmt::Auto => String::new(),
        calcumaker_core::FloatFmt::Fix(d) => format!("  FIX{d}"),
        calcumaker_core::FloatFmt::Sci(d) => format!("  SCI{d}"),
        calcumaker_core::FloatFmt::Eng(d) => format!("  ENG{d}"),
    };
    let shift = match app.shift() {
        Some(s) => format!("  [{s}]"),
        None => String::new(),
    };
    let reg = match app.pending_register() {
        Some(r) => format!("  {r} _"),
        None => String::new(),
    };
    let win = {
        let (w, total) = app.window();
        if total > 1 {
            format!("  win {}/{total}", w + 1)
        } else {
            String::new()
        }
    };
    let msg = match app.message() {
        Some(m) => format!("  << {m}"),
        None => String::new(),
    };
    out.push_str(&format!(" {pers}{radix}  {angle}  prec {}  {word}{flags}{fmt}{win}{shift}{reg}{msg}\n", c.prec()));

    // The glass rounds to its 16 digits (HP-style); this line is the SHOW
    // view — X at full precision, where the arbitrary precision is visible.
    out.push_str(&format!(" X: {}\n", app.x_full()));

    if help {
        out.push('\n');
        out.push_str(HELP);
        out.push('\n');
    } else {
        out.push_str(" [?] keys   [Ctrl-C] quit\n");
    }
    out
}

fn draw(stdout: &mut impl Write, app: &App, help: bool, style: Style) -> io::Result<()> {
    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    for line in frame(app, help, style).lines() {
        write!(stdout, "{line}\r\n")?;
    }
    stdout.flush()
}

fn interactive(mut app: App, style: Style) -> io::Result<()> {
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
    let mut help = false;

    let result = (|| -> io::Result<()> {
        draw(&mut stdout, &app, help, style)?;
        loop {
            let Event::Key(KeyEvent { code, modifiers, kind, .. }) = event::read()? else {
                continue;
            };
            if kind == KeyEventKind::Release {
                continue;
            }
            if modifiers.contains(KeyModifiers::CONTROL)
                && matches!(code, KeyCode::Char('c') | KeyCode::Char('d') | KeyCode::Char('q'))
            {
                return Ok(());
            }
            match code {
                KeyCode::Char('?') => help = !help,
                // Esc cancels a pending shift by toggling that shift key again.
                KeyCode::Esc => match app.shift() {
                    Some('f') => app.press_key(calcumaker_core::Key::ShiftF),
                    Some('g') => app.press_key(calcumaker_core::Key::ShiftG),
                    _ => {}
                },
                KeyCode::Enter => feed(&mut app, '\n'),
                KeyCode::Backspace => feed(&mut app, '\x08'),
                KeyCode::Char(c) => feed(&mut app, c),
                _ => {}
            }
            draw(&mut stdout, &app, help, style)?;
        }
    })();

    execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    result
}

fn feed(app: &mut App, ch: char) {
    if let Some((r, c)) = cell_for(ch) {
        app.press(r, c);
    }
}

fn main() -> io::Result<()> {
    let mut prec = 256u32;
    let mut script: Option<String> = None;
    let mut style = Style::Block;
    let mut no_suffix = false;
    let mut personality: Option<&'static calcumaker_core::keys::Keymap> = None;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--prec" => {
                prec = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage("--prec needs a bit count"))
            }
            "--press" => {
                script = Some(args.next().unwrap_or_else(|| usage("--press needs a key string")))
            }
            "--ascii" => style = Style::Ascii,
            "--no-suffix" => no_suffix = true,
            "--personality" => {
                let name = args.next().unwrap_or_else(|| usage("--personality needs a name"));
                personality = Some(
                    calcumaker_core::keys::PERSONALITIES
                        .iter()
                        .find(|km| km.name.eq_ignore_ascii_case(&name))
                        .copied()
                        .unwrap_or_else(|| usage(&format!("unknown personality {name} (16C, SCI, FIN)"))),
                );
            }
            "--help" | "-h" => {
                println!(
                    "calcumaker-emu [--prec <bits>] [--press <keys>] [--ascii] [--no-suffix] [--personality 16C|SCI|FIN]\n\n{HELP}"
                );
                return Ok(());
            }
            other => usage(&format!("unknown argument {other}")),
        }
    }

    let mut app = App::new(prec);
    if let Some(km) = personality {
        app.set_keymap(km);
    }
    if no_suffix {
        app.calc_mut().set_radix_suffix(false);
    }
    if let Some(s) = script {
        for ch in s.replace("\\n", "\n").chars() {
            feed(&mut app, ch);
        }
        print!("{}", frame(&app, false, style));
        return Ok(());
    }
    interactive(app, style)
}

fn usage(msg: &str) -> ! {
    eprintln!(
        "calcumaker-emu: {msg}\nusage: calcumaker-emu [--prec <bits>] [--press <keys>] [--ascii] [--no-suffix] [--personality 16C|SCI|FIN]"
    );
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use calcumaker_core::seg7;

    /// The decimal point must be visible: its glyph inks the right half of its
    /// own column (air on its left, toward the digit) and a gap column follows
    /// (air on its right, toward the next digit). Regression: 3.89 read as 389.
    #[test]
    fn block_dp_is_visible_and_separated() {
        let row = seg7::encode_row("3.89");
        let art = seg_art(&row, Style::Block);
        let bottom: Vec<char> = art[2].chars().collect();
        let i = bottom.iter().position(|&c| c == '▗').expect("dp glyph rendered");
        assert_eq!(bottom[i + 1], ' ', "gap column after the dp");
    }

    /// Every art line is exactly rows × digit_cols wide, both styles.
    #[test]
    fn art_width_matches_style() {
        for style in [Style::Block, Style::Ascii] {
            let row = seg7::encode_row("8.8");
            for line in seg_art(&row, style) {
                assert_eq!(line.chars().count(), seg7::DIGITS_PER_ROW * style.digit_cols());
            }
        }
    }
}
