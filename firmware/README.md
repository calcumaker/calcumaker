# calcumaker firmware

Rust firmware for the Calcumaker 16 programmer's / technical RPN calculator.

## Layout

```
firmware/
├── gmp-mpfr-nostd/    # own no_std FFI bindings to GMP/MPFR (Integer + Float).
│   │                  #   "Like rug, but for a no_std world." Links system libs
│   │                  #   on host, cross-built libs on target.
│   └── src/           #   lib · ffi · integer · float
├── calcumaker-core/   # the calculator ENGINE — RPN + arbitrary precision over
│   │                  #   gmp-mpfr-nostd. no_std lib, host-tested + a REPL. SINGLE
│   │                  #   math path (no fallback). This is where the logic lives.
│   ├── src/           #   lib · calc · value · format
│   ├── tests/         #   engine tests against real GMP/MPFR
│   └── examples/repl.rs
├── calcumaker-fw/     # the embedded application (Cortex-M, no_std): board bring-up,
│   └── src/           #   main · keypad · display  (it will host the core engine)
├── common/            # shared HAL / board glue (reserved)
├── shared/            # shared protocol / definitions (reserved)
└── LICENSE            # (repo LICENSE is AGPL-3.0)
```

## The engine is real and testable today

`calcumaker-core` uses **GNU MP + MPFR** through our own `no_std` bindings
(`gmp-mpfr-nostd`). On the host they link the system libraries, so it's a normal
library you develop and test on the desktop (builds in <1 s):

```bash
brew install gmp mpfr      # one-time host deps (apt: libgmp-dev libmpfr-dev)
cd calcumaker-core
cargo test                 # 12 engine tests vs real GMP/MPFR
cargo run --example repl   # interactive RPN, run against it
```

The engine is `#![no_std]`; the *same* crate also compiles for the MCU
(`cargo build --target thumbv8m.main-none-eabihf`).

There is **one** numeric path — no `numeric-gmp`/`numeric-pure` feature split,
no pure-Rust fallback.

## The embedded crate

`calcumaker-fw` is the board skeleton (heap, Cherry MX matrix scan, 7-segment
driver). It does **not** build standalone yet — the MCU/HAL must be wired (Cargo
target, `embassy-stm32` chip feature, `memory.x`, `probe-rs` chip in
`.cargo/config.toml`).

It will host the **same** `calcumaker-core` engine. The one hard step (tracked in
`../DESIGN.md` → "Numeric core / GMP/MPFR on the target"): build GMP + MPFR for
`thumbv8m` (`--host=arm-none-eabi --disable-assembly`, against picolibc) and link
the engine against them with GMP's allocator routed to the firmware heap. Same
code, cross-built C libraries — not a second implementation.

## License

AGPL-3.0 (see `../LICENSE`). GMP and MPFR are LGPLv3 — compatible.
