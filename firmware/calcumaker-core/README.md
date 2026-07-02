# calcumaker-core

The **Calcumaker 16 calculator** — everything device-independent, as a plain
library you can unit-test and run against on the host:

- **`Calc`** — the RPN stack + arbitrary-precision math engine;
- **`keys`** — the 50-key matrix keymap + f/g shift layers (design source of
  truth, shared by firmware and emulator);
- **`App`** — key handling over the engine: HP-style digit-by-digit entry,
  shift resolution, dispatch, display rows;
- **`seg7`** — text → TM1640 segment bytes (what the glass actually shows).

**Single math path.** Real **GNU MP** (integers) + **MPFR** (correctly-rounded
floats and transcendentals) via our own `no_std` bindings crate
**`gmp-mpfr-nostd`** — **no `std`, no feature-gated pure-Rust fallback.** The
crate is `#![no_std]` + `alloc`, so the *same* code runs on the host and
compiles for the MCU.

## Test it

```sh
brew install gmp mpfr     # one-time host deps (apt: libgmp-dev libmpfr-dev)
cargo test
```

Builds in well under a second (it links the system GMP/MPFR — no build from
source). The tests exercise the real C libraries: `sqrt(2)` and `e` to hundreds
of digits, `cos(0)=1`, `100!` as a 158-digit integer, hex bitwise ops, word-size
masking, integer-vs-real promotion — plus key-press sequences through `App` and
the 7-seg byte encoding.

## Run it

Two ways. The token REPL (engine only):

```sh
cargo run --example repl
```

```
[Dec 256b]  > 2.0 sqrt
[Dec 256b] 1.4142135623730950488016887242096980785696718753769480731766797... > 64 prec
[Dec 64b] ... > hex  ff 0f and          # -> F
[Hex 64b] F > dec  20 fact              # -> 2432902008176640000
[Dec 64b] ... > 2 100 pow               # -> exact 1267650600228229401496703205376
```

**Integers stay exact** (the point of GMP): `pow`/`sq`/`exp10` on integers give
every digit, and `sqrt` on an integer is the 16C-style *integer* root
(`17 sqrt` → `4`, carry flag set when inexact) — enter `2.0` or use `float`
for the real root.

Or the full device UI — keymap, shifts, entry editing, 7-seg display — in the
emulator: `../calcumaker-emu` (`cargo run`).

Keymap diagrams (ASCII, one per personality, generated + freshness-tested):
`../../doc/keymap-*.txt` — regenerate with `cargo run --example keymaps`.

Tokens:
- arith: `+ - * / chs abs pow inv sq sqrt fact mod pct`
- trig: `sin cos tan asin acos atan sinh cosh tanh` · angle unit `rad deg grad`
  (`anglemode` cycles; RAD default — DEG/GRAD reduce mod the circle exactly and
  hit exact angles: `deg 180 sin` = 0, `deg 30 sin` = 0.5, `deg 1 atan` = 45)
- log/const: `ln log exp exp10 e pi`
- programmer: `and or xor not` · `sl sr asr rl rr` (X by one bit) ·
  `shl shr rln rrn` (Y by X bits) · `rlc rrc rlcn rrcn` (through carry) ·
  `bset bclr btest maskl maskr popcnt lj` · `dbl* dbl/ dblr` (double-word) ·
  flags `sf cf ftest` (0-2 user; 3/4/5 = lz/C/G) · radix `hex dec oct bin`
- statistics/combinatorics (SCI pack): `s+ s- mean sdev lr yhat corr clstat`
  · exact `ncr npr` · `ran seed`
- conversions: `float round trunc floor ceil frac`
- stack/memory: `enter dup drop swap over rolldn rollup lastx clear` ·
  registers `sto0`–`stof` / `rcl0`–`rclf` / `clreg`
- modes (RPN postfix, pop X): `<bits> prec` · `<bits> wsize` (`0` = unbounded)
  with sign modes `2s 1s unsgn` (or `signmode` to cycle) · real formats
  `<d> fix / sci / eng`, `std` = auto · `lz` = leading zeros to the word width
  (16C flag 3) · `suffix` = toggle the glass's `h o b` base letter (on by
  default) · angle `rad deg grad` / `anglemode` · stack `stack4` (classic HP
  X/Y/Z/T with T-replication + lift discipline) / `stackfree` (unbounded,
  default) — all also in the on-device SETUP menu (g-CLx)
- word-mode flags: `carry()` (C) and `overflow()` (G) — add/sub carry-borrow,
  shifted/rotated-out bits, wrapped results
- REPL meta: `stack`, `quit`

## API

```rust
use calcumaker_core::{Calc, Radix};
let mut c = Calc::new(256);                  // 256-bit working precision
for t in ["2", "3", "+"] { c.input(t)?; }
assert_eq!(c.display(), "5");
```

`Calc::input(tok)` pushes a number or applies a command; `display()` formats X.
`set_radix`, `set_prec`, `set_word_bits` control modes.

One level up, `App` speaks key presses instead of tokens:

```rust
use calcumaker_core::{App, Key};
let mut app = App::new(256);
app.press(4, 6);                      // matrix (row,col): the '0' key…
app.press_key(Key::Digit(2));         // …or logical keys directly
app.press_key(Key::Enter);
let rows = app.seg_rows();            // [[u8; 16]; 3] TM1640 segment bytes
```

## Relationship to the firmware

This crate is the **canonical calculator** — `no_std`, math via
`gmp-mpfr-nostd`. On the host it links the system GMP/MPFR for dev + testing
(and powers `../calcumaker-emu`); the **same crate** compiles for `thumbv8m`,
where the STM32 firmware (`../calcumaker-fw`) links the **cross-built** GMP/MPFR
(built + link-verified — see `../../DESIGN.md` → "GMP/MPFR on the target") and
contributes only the matrix scan and the TM1640 bus.

## License

AGPL-3.0 (see repo root). GMP/MPFR are LGPLv3 — compatible.
