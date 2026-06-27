# calcumaker firmware

Rust firmware for the Calcumaker 16 programmer's / technical RPN calculator.

## Layout

```
firmware/
├── calcumaker-core/   # the calculator ENGINE — RPN + arbitrary precision (GMP+MPFR
│   │                  #   via rug). Plain library, host-tested + a REPL. SINGLE math
│   │                  #   path (no fallback). This is where the logic lives.
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

`calcumaker-core` uses **GNU MP + MPFR** through `rug` (which builds the C libs
itself — no system packages). It's a normal library you develop and test on the
host:

```bash
cd calcumaker-core
cargo test                 # engine tests vs real GMP/MPFR (first build compiles them)
cargo run --example repl   # interactive RPN, run against it
```

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
