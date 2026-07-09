//! Human-readable keymap diagrams — pure-ASCII key grids generated from the
//! [`crate::keys`] tables (the design source of truth), one file per
//! personality in `doc/keymap-<name>.txt`.
//!
//! Regenerate with `cargo run --example keymaps`; the `keymap_docs_are_fresh`
//! golden test fails when the committed files drift from the tables, so the
//! diagrams can never lie. Every key box shows, HP-keycap style:
//! top = f (gold) shift, middle = the key face, bottom = g (blue) shift.

use alloc::format;
use alloc::string::String;

use crate::keys::{Key, Keymap, COLS, ROWS};

/// Short printable label for a key (ASCII only — these files are meant to be
/// readable anywhere, including keycap-legend planning). Exhaustive on
/// purpose: adding a `Key` forces choosing its label.
pub fn label(k: Key) -> &'static str {
    match k {
        Key::Digit(0) => "0",
        Key::Digit(1) => "1",
        Key::Digit(2) => "2",
        Key::Digit(3) => "3",
        Key::Digit(4) => "4",
        Key::Digit(5) => "5",
        Key::Digit(6) => "6",
        Key::Digit(7) => "7",
        Key::Digit(8) => "8",
        Key::Digit(9) => "9",
        Key::Digit(10) => "A",
        Key::Digit(11) => "B",
        Key::Digit(12) => "C",
        Key::Digit(13) => "D",
        Key::Digit(14) => "E",
        Key::Digit(15) => "F",
        Key::Digit(_) => "?",
        Key::Dot => ".",
        Key::Chs => "CHS",
        Key::Eex => "EEX",
        Key::Back => "BSP",
        Key::ClrX => "CLX",
        Key::Add => "+",
        Key::Sub => "-",
        Key::Mul => "*",
        Key::Div => "/",
        Key::Enter => "ENTER",
        Key::Swap => "x<>y",
        Key::RollDn => "Rv",
        Key::RollUp => "R^",
        Key::LastX => "LSTx",
        Key::Sto => "STO",
        Key::Rcl => "RCL",
        Key::ClrReg => "CLREG",
        Key::Sf => "SF",
        Key::Cf => "CF",
        Key::Ftest => "F?",
        Key::ShowHex => "SHOWh",
        Key::ShowDec => "SHOWd",
        Key::ShowOct => "SHOWo",
        Key::ShowBin => "SHOWb",
        Key::Status => "STATUS",
        Key::Setup => "SETUP",
        Key::Hex => "HEX",
        Key::Dec => "DEC",
        Key::Oct => "OCT",
        Key::Bin => "BIN",
        Key::WordSize => "WSIZE",
        Key::SignMode => "SGN",
        Key::Float => "FLOAT",
        Key::And => "AND",
        Key::Or => "OR",
        Key::Xor => "XOR",
        Key::Not => "NOT",
        Key::Shl => "SL",
        Key::Shr => "SR",
        Key::Asr => "ASR",
        Key::Rotl => "RL",
        Key::Rotr => "RR",
        Key::Rlc => "RLC",
        Key::Rrc => "RRC",
        Key::Lj => "LJ",
        Key::BitSet => "SB",
        Key::BitClr => "CB",
        Key::BitTest => "B?",
        Key::MaskL => "MASKL",
        Key::MaskR => "MASKR",
        Key::BitCount => "#B",
        Key::Rmd => "RMD",
        Key::DblMul => "DBLx",
        Key::DblDiv => "DBL/",
        Key::DblRem => "DBLR",
        Key::Sin => "SIN",
        Key::Cos => "COS",
        Key::Tan => "TAN",
        Key::Asin => "ASIN",
        Key::Acos => "ACOS",
        Key::Atan => "ATAN",
        Key::Sinh => "SINH",
        Key::Cosh => "COSH",
        Key::Tanh => "TANH",
        Key::Ln => "LN",
        Key::Exp => "e^x",
        Key::Log10 => "LOG",
        Key::Exp10 => "10^x",
        Key::Sqrt => "SQRT",
        Key::Sq => "x^2",
        Key::Pow => "y^x",
        Key::Recip => "1/x",
        Key::Complex => "CPLX",
        Key::CplxDisp => "R<>P",
        Key::Conj => "CONJ",
        Key::Arg => "ARG",
        Key::Re => "Re",
        Key::Im => "Im",
        Key::ReIm => "Re<>Im",
        Key::ToPolar => ">P",
        Key::ToRect => ">R",
        Key::MatNew => "MDIM",
        Key::MatSet => "MSTO",
        Key::Det => "DET",
        Key::Transpose => "TRSP",
        Key::Minv => "1/M",
        Key::Matsolve => "M/",
        Key::Pi => "PI",
        Key::Fact => "x!",
        Key::Pct => "%",
        Key::Round => "RND",
        Key::IntPart => "INT",
        Key::Fix => "FIX",
        Key::Sci => "SCI",
        Key::Eng => "ENG",
        Key::FmtAuto => "AUTO",
        Key::AngleMode => "ANGLE",
        Key::SigmaPlus => "S+",
        Key::SigmaMinus => "S-",
        Key::Mean => "MEAN",
        Key::Sdev => "SDEV",
        Key::Lr => "L.R.",
        Key::Yhat => "YHAT",
        Key::Corr => "CORR",
        Key::ClStat => "CLSUM",
        Key::Ncr => "nCr",
        Key::Npr => "nPr",
        Key::Ran => "RAN#",
        Key::Seed => "SEED",
        Key::TvmN => "n",
        Key::TvmI => "i",
        Key::TvmPv => "PV",
        Key::TvmPmt => "PMT",
        Key::TvmFv => "FV",
        Key::X12Mul => "12x",
        Key::X12Div => "12div",
        Key::BegKey => "BEG",
        Key::EndKey => "END",
        Key::ClFin => "CLFIN",
        Key::Cf0 => "CF0",
        Key::Cfj => "CFj",
        Key::NjKey => "Nj",
        Key::Npv => "NPV",
        Key::Irr => "IRR",
        Key::ClCf => "CLCF",
        Key::PctChg => "d%",
        Key::PctT => "%T",
        Key::Wmean => "xw",
        Key::Ddays => "dDYS",
        Key::DateAdd => "DATE+",
        Key::Dow => "DOW",
        Key::DepSl => "SL",
        Key::DepSoyd => "SOYD",
        Key::DepDb => "DB",
        Key::Prec => "PREC",
        Key::WinL => "<WIN",
        Key::WinR => "WIN>",
        Key::Lz => "LZ",
        Key::ShiftF => "f",
        Key::ShiftG => "g",
        Key::Off => "OFF",
        Key::Nop => "",
        // No switch here — the upper half of the 2U ENTER cap. Drawn as a gap
        // merged into the ENTER box below (see `render`).
        Key::Absent => "",
    }
}

/// Render one personality as an ASCII key-grid diagram.
pub fn render(km: &Keymap) -> String {
    // column width = the longest label anywhere in this personality
    let mut w = 1;
    for layer in [&km.base, &km.f, &km.g] {
        for row in layer.iter() {
            for &k in row.iter() {
                w = w.max(label(k).len());
            }
        }
    }

    let mut out = String::new();
    out += &format!("CALCUMAKER 16 - PERSONALITY: {}\n", km.name);
    out += "GENERATED from firmware/calcumaker-core/src/keys.rs - do not edit.\n";
    out += "Regenerate: cargo run --example keymaps   (in firmware/calcumaker-core)\n";
    out += "\n";
    out += "Each key:   f (gold) shift function\n";
    out += "            KEY FACE\n";
    out += "            g (blue) shift function\n";
    out += "\n";
    out += "ENTER is a 2U (double-height) key: one switch in its lower cell, the\n";
    out += "cell above carries no switch (drawn as the merged box).\n";
    out += "\n";

    // The border above row `r`; a cell whose neighbour above is Absent renders as
    // a gap, merging the two cells into one tall key box. `None` = bottom edge.
    let border = |below: Option<usize>| {
        let mut s = String::new();
        for c in 0..COLS {
            s.push('+');
            let merged =
                matches!(below, Some(r) if r > 0 && matches!(km.base[r - 1][c], Key::Absent));
            s += &(if merged { " " } else { "-" }).repeat(w + 2);
        }
        s.push('+');
        s.push('\n');
        s
    };

    for r in 0..ROWS {
        out += &border(Some(r));
        for layer in [&km.f, &km.base, &km.g] {
            for &key in &layer[r] {
                out += &format!("| {:<w$} ", label(key));
            }
            out += "|\n";
        }
    }
    out += &border(None);
    out
}
