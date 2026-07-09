//! Interactive RPN REPL to run the engine against locally:
//!
//! ```sh
//! cargo run -p calcumaker-core --example repl
//! # or, from this crate dir:  cargo run --example repl
//! ```
//!
//! Enter whitespace-separated tokens. Numbers push; commands apply.
//! Precision and word size are RPN too: `256 prec`, `16 wsize` (`0 wsize` =
//! unbounded). Meta: `stack`, `quit` (`clear` is an engine command).

use std::io::{self, BufRead, Write};

use calcumaker_core::Calc;

fn main() {
    let mut calc = Calc::new(256);
    // Seed RAN# from host entropy so it varies between runs (SEED still repeats).
    calc.reseed(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1),
    );
    println!(
        "Calcumaker 16 — RPN engine (GMP + MPFR), {}-bit precision.",
        calc.prec()
    );
    println!("arith : + - * / chs abs pow  inv sq sqrt  fact mod pct");
    println!("trig  : sin cos tan asin acos atan  sinh cosh tanh");
    println!("log   : ln log exp exp10 e pi");
    println!("prog  : and or xor not  sl sr asr rl rr (X by 1; shl shr rln rrn = Y by X)");
    println!("bits  : bset bclr btest maskl maskr popcnt  | radix: hex dec oct bin");
    println!("conv  : float round trunc floor ceil frac");
    println!("stack : enter dup drop swap over rolldn rollup lastx clear  | sto0-f rcl0-f");
    println!(
        "modes : <bits> prec | <bits> wsize (0=unbounded) | unsgn 1s 2s | <d> fix/sci/eng, std"
    );
    println!("meta  : stack, quit\n");

    let stdin = io::stdin();
    loop {
        let word = match calc.word_bits() {
            Some(n) => format!(
                " w{n}{}",
                match calc.sign_mode() {
                    calcumaker_core::SignMode::Unsigned => "u",
                    calcumaker_core::SignMode::Ones => "·1s",
                    calcumaker_core::SignMode::Twos => "·2s",
                },
            ),
            None => String::new(),
        };
        let flags = format!(
            "{}{}",
            if calc.carry() { " C" } else { "" },
            if calc.overflow() { " G" } else { "" },
        );
        let angle = match calc.angle_mode() {
            calcumaker_core::AngleMode::Rad => "",
            calcumaker_core::AngleMode::Deg => " DEG",
            calcumaker_core::AngleMode::Grad => " GRAD",
        };
        print!(
            "[{:?} {}b{angle}{word}{flags}] {} > ",
            calc.radix(),
            calc.prec(),
            calc.display()
        );
        io::stdout().flush().ok();

        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            println!();
            break;
        }

        for tok in line.split_whitespace() {
            match tok {
                "quit" | "q" | "exit" => return,
                "stack" => {
                    for (i, v) in calc.stack().iter().enumerate().rev() {
                        println!("  {i}: {}", calcumaker_core::display_value(v, &calc));
                    }
                }
                _ => {
                    if let Err(e) = calc.input(tok) {
                        eprintln!("  ? {tok}: {e:?}");
                    }
                }
            }
        }
    }
}
