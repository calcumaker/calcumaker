# calcumaker firmware

Rust firmware for the Calcumaker 16 programmer's / technical RPN calculator.

## Layout

```
firmware/
├── gmp-mpfr-nostd/    # own no_std FFI bindings to GMP/MPFR (Integer + Float).
│   │                  #   "Like rug, but for a no_std world." Links system libs
│   │                  #   on host, cross-built libs on target.
│   └── src/           #   lib · ffi · integer · float
├── calcumaker-core/   # the CALCULATOR — everything device-independent:
│   │                  #   engine (RPN + arbitrary precision, SINGLE math path),
│   │                  #   keymap + f/g shift layers (keys), key handling +
│   │                  #   entry editing (App), 7-seg encoding (seg7).
│   ├── src/           #   lib · calc · value · format · keys · app · seg7
│   ├── tests/         #   engine + app + seg7 tests against real GMP/MPFR
│   └── examples/repl.rs
├── calcumaker-emu/    # HOST EMULATOR — the same App on a terminal; ASCII 7-seg
│   └── src/           #   rendered from the real TM1640 segment bytes.
├── calcumaker-fw/     # the embedded binary (Cortex-M33, no_std): board bring-up
│   └── src/           #   main · keypad (matrix scan) · display (TM1640 bus)
├── scripts/           # build-gmp-mpfr-arm.sh — cross-build GMP+MPFR for thumbv8m
├── vendor/            # cross-built libgmp.a/libmpfr.a land here (gitignored)
├── common/            # shared HAL / board glue (reserved)
├── shared/            # shared protocol / definitions (reserved)
└── LICENSE            # (repo LICENSE is AGPL-3.0)
```

## The calculator is real and testable today

`calcumaker-core` uses **GNU MP + MPFR** through our own `no_std` bindings
(`gmp-mpfr-nostd`). On the host they link the system libraries, so it's a normal
library you develop and test on the desktop (builds in <1 s):

```bash
brew install gmp mpfr      # one-time host deps (apt: libgmp-dev libmpfr-dev)
cd calcumaker-core
cargo test                 # engine + app + 7-seg tests vs real GMP/MPFR
cargo run --example repl   # token RPN REPL against the engine
```

And the whole device UI runs on a terminal — see `calcumaker-emu/README.md`:

```bash
cd calcumaker-emu
cargo run                       # interactive emulator (? = key map)
cargo run -- --press "2;3+"     # scripted keys → prints the final frame
```

The core is `#![no_std]`; the *same* crate also compiles for the MCU
(`cargo build --target thumbv8m.main-none-eabihf`).

There is **one** numeric path — no `numeric-gmp`/`numeric-pure` feature split,
no pure-Rust fallback — and **one** UI path: firmware and emulator are thin I/O
bindings (matrix scan / host keys in, TM1640 bytes / ASCII art out) around the
same `App`.

## The embedded crate

`calcumaker-fw` is the board binary (heap, Cherry MX matrix scan → `(row,col)`,
TM1640 display bus). It builds and links for `thumbv8m.main-none-eabihf` today
(the default target in its `.cargo/config.toml`).

GMP + MPFR are **cross-built + link-verified** for the target
(`scripts/build-gmp-mpfr-arm.sh` → `vendor/gmp-mpfr-arm/`; `build.rs` links
them when `GMP_MPFR_LIBDIR` is set). Remaining bring-up (tracked in
`../DESIGN.md`): embassy clocks/GPIO, newlib libc/libm at the final link, and
routing GMP's allocator to the firmware heap — then the main loop is exactly
what the emulator runs: `app.press(row, col)` → `display.render(&app.seg_rows())`.

## License

AGPL-3.0 (see `../LICENSE`). GMP and MPFR are LGPLv3 — compatible.
