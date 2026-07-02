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

/// One display row of segment bytes as three lines of ASCII art (4 columns per
/// digit: the 7 segments + the dp position).
fn seg_art(row: &[u8; DIGITS_PER_ROW]) -> [String; 3] {
    let mut l: [String; 3] = Default::default();
    for &b in row {
        let seg = |m: u8, on: char| if b & m != 0 { on } else { ' ' };
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
    F then H/D/O/B = FLOAT (int X -> real)  F then W = sign mode (2's/1's/unsgn)
    F then v = roll-up
    G then S/C/T = sinh/cosh/tanh     G then L = log10   G then Q = 10^x
    G then 4/5/6 = x! / % / round (real X -> int)
    G then H/D/O/B = FIX/SCI/ENG/auto (digit count from X)

  STO/RCL: press m (STO) or r (RCL), then a digit 0-f = the register.
  Modes: bits then W = wsize (0 = unbounded); annunciators show C (carry) and
  G (overflow) in word mode. Esc cancels a pending shift. ? = help. Ctrl-C quits.";

/// Render one full frame: 7-seg rows, annunciators, the untruncated X, footer.
fn frame(app: &App, help: bool) -> String {
    let mut out = String::new();
    let width = DIGITS_PER_ROW * 4;
    out.push_str("Calcumaker 16 - emulator\n");
    out.push_str(&format!("+{}+\n", "-".repeat(width + 2)));
    for row in &app.seg_rows() {
        for line in seg_art(row) {
            out.push_str(&format!("| {line} |\n"));
        }
        out.push_str(&format!("|{}|\n", " ".repeat(width + 2)));
    }
    out.push_str(&format!("+{}+\n", "-".repeat(width + 2)));

    let c = app.calc();
    let radix = format!("{:?}", c.radix()).to_uppercase();
    let word = match c.word_bits() {
        Some(b) => {
            let mode = match c.sign_mode() {
                calcumaker_core::SignMode::Unsigned => "unsgn",
                calcumaker_core::SignMode::Ones => "1's",
                calcumaker_core::SignMode::Twos => "2's",
            };
            let flags = format!(
                "{}{}",
                if c.carry() { "  C" } else { "" },
                if c.overflow() { "  G" } else { "" }
            );
            format!("word {b} {mode}{flags}")
        }
        None => "word unbounded".into(),
    };
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
    let msg = match app.message() {
        Some(m) => format!("  << {m}"),
        None => String::new(),
    };
    out.push_str(&format!(" {radix}  prec {}  {word}{fmt}{shift}{reg}{msg}\n", c.prec()));

    // The 16-digit window truncates long values; show X in full here — this is
    // where the arbitrary precision is visible on the host.
    let x = app.text_rows()[calcumaker_core::seg7::DISPLAY_ROWS - 1].clone();
    out.push_str(&format!(" X: {x}\n"));

    if help {
        out.push('\n');
        out.push_str(HELP);
        out.push('\n');
    } else {
        out.push_str(" [?] keys   [Ctrl-C] quit\n");
    }
    out
}

fn draw(stdout: &mut impl Write, app: &App, help: bool) -> io::Result<()> {
    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    for line in frame(app, help).lines() {
        write!(stdout, "{line}\r\n")?;
    }
    stdout.flush()
}

fn interactive(mut app: App) -> io::Result<()> {
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
    let mut help = false;

    let result = (|| -> io::Result<()> {
        draw(&mut stdout, &app, help)?;
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
            draw(&mut stdout, &app, help)?;
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
            "--help" | "-h" => {
                println!("calcumaker-emu [--prec <bits>] [--press <keys>]\n\n{HELP}");
                return Ok(());
            }
            other => usage(&format!("unknown argument {other}")),
        }
    }

    let mut app = App::new(prec);
    if let Some(s) = script {
        for ch in s.replace("\\n", "\n").chars() {
            feed(&mut app, ch);
        }
        print!("{}", frame(&app, false));
        return Ok(());
    }
    interactive(app)
}

fn usage(msg: &str) -> ! {
    eprintln!("calcumaker-emu: {msg}\nusage: calcumaker-emu [--prec <bits>] [--press <keys>]");
    std::process::exit(2);
}
