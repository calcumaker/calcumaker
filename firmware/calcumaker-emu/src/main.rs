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

/// Printable name for a host key.
fn host_key_name(ch: char) -> String {
    match ch {
        '\x08' => "Bks".into(),
        '\n' => "Ent".into(),
        c => c.to_string(),
    }
}

/// Help = the ACTIVE personality's key grid, rendered live from the same
/// tables the device uses (`keydoc::label`), with **the host key to press
/// inside each box**. Follows PErS switches automatically and can't drift.
fn help_text(app: &App) -> String {
    use calcumaker_core::keydoc::label;
    let km = app.keymap();

    // column width: widest of labels and bracketed host keys
    let mut w = 5; // "[Bks]"
    for layer in [&km.base, &km.f, &km.g] {
        for row in layer.iter() {
            for &k in row.iter() {
                w = w.max(label(k).len());
            }
        }
    }

    let mut out = format!(
        "PERSONALITY: {}   (each box: [key you press] / f gold / KEY FACE / g blue)\n\n",
        km.name
    );
    let mut border = String::new();
    for _ in 0..COLS {
        border += "+";
        border += &"-".repeat(w + 2);
    }
    border += "+\n";
    for r in 0..ROWS {
        out += &border;
        // host key line
        for c in 0..COLS {
            out += &format!("| {:<w$} ", format!("[{}]", host_key_name(HOST_KEYS[r][c])));
        }
        out += "|\n";
        for layer in [&km.f, &km.base, &km.g] {
            for c in 0..COLS {
                out += &format!("| {:<w$} ", label(layer[r][c]));
            }
            out += "|\n";
        }
    }
    out += &border;
    out += "\nF/G = gold/blue shifts. STO/RCL: m or r, then a digit 0-f = the register.\n\
G+X = SETUP menu, F+X = STATUS view. Esc clears a pending shift.\n\
--personality 16C|SCI|FIN (or SETUP > PErS). ? = help. Ctrl-C quits.\n";
    out
}

/// Render one full frame: 7-seg rows, annunciators, the untruncated X, the
/// mock aux OLED (unless the base --no-oled build), footer.
fn frame(app: &App, help: bool, style: Style, oled: bool) -> String {
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
    // number-type mode: FLEX is the quiet default; INT/REAL are announced
    let numty = match c.num_mode() {
        calcumaker_core::NumMode::Flex => "",
        calcumaker_core::NumMode::Int => "INT  ",
        calcumaker_core::NumMode::Real => "REAL  ",
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
    out.push_str(&format!(" {pers}{radix}  {angle}  {numty}prec {}  {word}{flags}{fmt}{win}{shift}{reg}{msg}\n", c.prec()));

    // The glass rounds to its 16 digits (HP-style); this line is the SHOW
    // view — X at full precision, where the arbitrary precision is visible.
    out.push_str(&format!(" X: {}\n", app.x_full()));

    // Mock of the DNP-optional aux OLED (128x32 = 4 lines x 21 chars): the
    // SAME App::aux_lines the firmware panel will draw. SETUP > OLEd toggles
    // the flags header.
    if oled {
        out.push_str(&format!(" ┌{}┐ aux OLED\n", "─".repeat(23)));
        for l in app.aux_lines() {
            out.push_str(&format!(" │ {l:<21} │\n"));
        }
        out.push_str(&format!(" └{}┘\n", "─".repeat(23)));
    }

    if help {
        out.push('\n');
        out.push_str(&help_text(app));
        out.push('\n');
    } else {
        out.push_str(" [?] keys   [Ctrl-C] quit\n");
    }
    out
}

fn draw(stdout: &mut impl Write, app: &App, help: bool, style: Style, oled: bool) -> io::Result<()> {
    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;
    for line in frame(app, help, style, oled).lines() {
        write!(stdout, "{line}\r\n")?;
    }
    stdout.flush()
}

fn interactive(mut app: App, style: Style, oled: bool) -> io::Result<()> {
    let mut stdout = io::stdout();
    terminal::enable_raw_mode()?;
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
    let mut help = false;

    let result = (|| -> io::Result<()> {
        draw(&mut stdout, &app, help, style, oled)?;
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
            draw(&mut stdout, &app, help, style, oled)?;
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
    let mut oled = true;
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
            "--no-oled" => oled = false,
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
                    "calcumaker-emu [--prec <bits>] [--press <keys>] [--ascii] [--no-suffix] [--no-oled] [--personality 16C|SCI|FIN]\n"
                );
                let app = App::new(256);
                println!("{}", help_text(&app));
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
        print!("{}", frame(&app, false, style, oled));
        return Ok(());
    }
    interactive(app, style, oled)
}

fn usage(msg: &str) -> ! {
    eprintln!(
        "calcumaker-emu: {msg}\nusage: calcumaker-emu [--prec <bits>] [--press <keys>] [--ascii] [--no-suffix] [--no-oled] [--personality 16C|SCI|FIN]"
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
