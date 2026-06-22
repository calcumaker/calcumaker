# calcumaker firmware

Rust **`no_std`** firmware for the calcumaker programmer's / technical RPN
calculator.

## Layout

```
firmware/
├── calcumaker-fw/   # the application crate (Cortex-M, no_std)
│   └── src/         #   main · rpn · keypad · display · numeric/
├── common/          # shared HAL / board glue (reserved)
├── shared/          # shared protocol / definitions (reserved)
└── LICENSE          # (repo LICENSE is AGPL-3.0)
```

## Status: skeleton

The crate is a structured **skeleton** and does **not** build yet — by design.
Two things must be selected and wired first (tracked in `../DESIGN.md`):

1. **MCU / HAL.** Pin the STM32 part, then set:
   - the Cargo target (`thumbv8m.main-none-eabihf` for Cortex-M33 / STM32U5/L5,
     or `thumbv7em-none-eabihf` for Cortex-M4F / STM32L4+),
   - the `embassy-stm32` chip feature,
   - `memory.x` FLASH/RAM sizes,
   - the `.cargo/config.toml` `probe-rs` runner chip name.
2. **Numeric backend.** Pick a Cargo feature:
   - `numeric-gmp` — **GNU MP + MPFR via FFI** (preferred; correctly-rounded
     transcendentals). Requires a cross-built `libgmp`/`libmpfr` (see
     `calcumaker-fw/build.rs` and `../DESIGN.md` → "GMP/MPFR on no_std").
   - `numeric-pure` — **pure-Rust** arbitrary precision (fallback; fully no_std).

## Build (once the above are wired)

```bash
cd calcumaker-fw
cargo build --release                       # default (numeric-pure) backend
cargo build --release --no-default-features --features numeric-gmp
cargo run   --release                        # flash+run via probe-rs
```

## Host-testable units

`rpn.rs` and `numeric/` are intended to be exercised on the host (a small `std`
test harness / the pure-Rust backend) before targeting silicon — keep the RPN
logic free of HAL dependencies.

## License

AGPL-3.0 (see `../LICENSE`). GMP and MPFR are LGPLv3 — compatible.
