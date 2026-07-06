# calcumaker-fw

Rust **`no_std`** board firmware for the Calcumaker 16 MCU board
(`STM32U575ZGT6`, Cortex-M33).

This crate is not the calculator engine. The calculator lives in
[`../calcumaker-core`](../calcumaker-core): RPN stack, GMP/MPFR math, keymap,
entry editing, modes, errors, and TM1640 segment-byte generation. This crate is
the hardware binding around that core: heap setup, MCU/HAL bring-up, keyboard
event intake from the keyboard-board G0, and the display bus.

## Modules

| Module | Role |
|--------|------|
| `main.rs` | Heap init, placeholder board loop, eventual embassy bring-up. |
| `keypad.rs` | Provisional keyboard-event intake. The real matrix scan/debounce/IRQ lives on the keyboard board's STM32G0 firmware; the U575 consumes `(row,col)` events. |
| `display.rs` | TM1640 bus driver skeleton for the multi-row 7-segment display. |

## Calculator Core

There is one numeric path: **GNU MP + MPFR** through the repo's
`gmp-mpfr-nostd` crate. No `numeric-pure`/`numeric-gmp` feature split, no
pure-Rust fallback, and no `std`/`rug` in the engine.

On the host, `calcumaker-core` links system GMP/MPFR and is tested directly:

```bash
cd ../calcumaker-core
cargo test
cargo run --example repl
```

For the target, GMP/MPFR are cross-built out of tree and linked by this crate's
`build.rs` when `GMP_MPFR_LIBDIR` points at the install prefix:

```bash
cd ../..
firmware/scripts/build-gmp-mpfr-arm.sh
GMP_MPFR_LIBDIR=firmware/vendor/gmp-mpfr-arm \
  cargo build --manifest-path firmware/calcumaker-fw/Cargo.toml --target thumbv8m.main-none-eabihf
```

## Bring-Up Status

The MCU is pinned and the crate carries a provisional `memory.x`, heap, target,
and GMP/MPFR linker hook. Remaining work is board bring-up: embassy clocks/GPIO,
newlib/libm link cleanup for the C libraries, routing GMP allocations through
`mp_set_memory_functions`, the keyboard-G0 firmware/link protocol, and the real
TM1640 bit-bang driver. See [`../../DESIGN.md`](../../DESIGN.md) for the full
firmware checklist.
