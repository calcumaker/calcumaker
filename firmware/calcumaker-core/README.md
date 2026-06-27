# calcumaker-core

The **Calcumaker 16 calculator engine** — the RPN stack + arbitrary-precision
math, as a plain library you can unit-test and run against on the host.

**Single math path.** Real **GNU MP** (integers) + **MPFR** (correctly-rounded
floats and transcendentals) via our own `no_std` bindings crate
**`gmp-mpfr-nostd`** — **no `std`, no feature-gated pure-Rust fallback.** The
engine itself is `#![no_std]` + `alloc`, so the *same* crate runs on the host and
compiles for the MCU.

## Test it

```sh
brew install gmp mpfr     # one-time host deps (apt: libgmp-dev libmpfr-dev)
cargo test
```

Builds in well under a second (it links the system GMP/MPFR — no build from
source). The tests exercise the real C libraries: `sqrt(2)` and `e` to hundreds
of digits, `cos(0)=1`, `100!` as a 158-digit integer, hex bitwise ops, word-size
masking, and integer-vs-real promotion.

## Run it

```sh
cargo run --example repl
```

```
[Dec 256b]  > 2 sqrt
[Dec 256b] 1.4142135623730950488016887242096980785696718753769480731766797... > prec 64
[Dec 64b] ... > hex  ff 0f and          # -> F
[Hex 64b] F > dec  20 fact              # -> 2432902008176640000
```

Tokens: numbers · `+ - * / chs sqrt sin cos tan ln exp inv sq pi` ·
`and or xor not shl shr fact` · radix `hex dec oct bin` ·
meta `prec <bits>`, `words <bits|none>`, `stack`, `clear`, `quit`.

## API

```rust
use calcumaker_core::{Calc, Radix};
let mut c = Calc::new(256);                  // 256-bit working precision
for t in ["2", "3", "+"] { c.input(t)?; }
assert_eq!(c.display(), "5");
```

`Calc::input(tok)` pushes a number or applies a command; `display()` formats X.
`set_radix`, `set_prec`, `set_word_bits` control modes.

## Relationship to the firmware

This crate is the **canonical engine** — `no_std`, math via `gmp-mpfr-nostd`. On
the host it links the system GMP/MPFR for dev + testing; the **same crate**
compiles for `thumbv8m`, where the STM32 firmware (`../calcumaker-fw`) links the
**cross-built** GMP/MPFR (GMP allocator → the firmware heap). That cross-build is
the remaining integration; see `../../DESIGN.md` → "GMP/MPFR on the target".

## License

AGPL-3.0 (see repo root). GMP/MPFR are LGPLv3 — compatible.
