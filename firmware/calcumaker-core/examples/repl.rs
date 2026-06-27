//! Interactive RPN REPL to run the engine against locally:
//!
//! ```sh
//! cargo run -p calcumaker-core --example repl
//! # or, from this crate dir:  cargo run --example repl
//! ```
//!
//! Enter whitespace-separated tokens. Numbers push; commands apply.
//! Meta: `prec <bits>`, `words <bits|none>`, `stack`, `clear`, `quit`.

use std::io::{self, BufRead, Write};

use calcumaker_core::Calc;

fn main() {
    let mut calc = Calc::new(256);
    println!("Calcumaker 16 — RPN engine (GMP + MPFR), {}-bit precision.", calc.prec());
    println!("arith : + - * / chs abs pow  inv sq sqrt  fact mod");
    println!("trig  : sin cos tan asin acos atan  sinh cosh tanh");
    println!("log   : ln log exp e pi");
    println!("prog  : and or xor not shl shr  | radix: hex dec oct bin");
    println!("stack : enter dup drop swap over rolldn rollup lastx");
    println!("meta  : prec <bits>, words <bits|none>, stack, clear, quit\n");

    let stdin = io::stdin();
    loop {
        print!("[{:?} {}b] {} > ", calc.radix(), calc.prec(), calc.display());
        io::stdout().flush().ok();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            println!();
            break;
        }

        let mut it = line.split_whitespace().peekable();
        while let Some(tok) = it.next() {
            match tok {
                "quit" | "q" | "exit" => return,
                "clear" => calc = Calc::new(calc.prec()),
                "stack" => {
                    for (i, v) in calc.stack().iter().enumerate().rev() {
                        println!("  {i}: {}", calcumaker_core::display_value(v, calc.radix(), calc.prec()));
                    }
                }
                "prec" => match it.next().and_then(|s| s.parse::<u32>().ok()) {
                    Some(b) => calc.set_prec(b),
                    None => eprintln!("usage: prec <bits>"),
                },
                "words" => match it.next() {
                    Some("none") => calc.set_word_bits(None),
                    Some(s) => match s.parse::<u32>() {
                        Ok(b) => calc.set_word_bits(Some(b)),
                        Err(_) => eprintln!("usage: words <bits|none>"),
                    },
                    None => eprintln!("usage: words <bits|none>"),
                },
                _ => {
                    if let Err(e) = calc.input(tok) {
                        eprintln!("  ? {tok}: {e:?}");
                    }
                }
            }
        }
    }
}
