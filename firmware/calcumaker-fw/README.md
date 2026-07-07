# calcumaker-fw

Rust **`no_std`** board firmware for the Calcumaker 16 MCU board
(`STM32U575RGT6`, Cortex-M33).

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

## Nucleo-U575ZI-Q validation target

A permanent, non-production **validation target**: the same firmware, flashed to
an off-the-shelf **Nucleo-U575ZI-Q** (STM32U575ZIT6Q — a 2 MB/144-pin sibling of
the production RGT6). It has no keyboard matrix or display, so `main` runs the
shared engine **self-test** (`src/selftest.rs`) — golden RPN cases whose results
were captured from the host engine — and streams PASS/FAIL over **RTT (defmt)**
through the board's on-board ST-LINK-V3. This proves the cross-built on-target
GMP/MPFR reproduces the host results *byte-for-byte* (MPFR is deterministic at a
fixed precision/rounding), i.e. the whole arbitrary-precision stack computes
correctly on real U5 silicon.

Everything is shared with the production image. The `nucleo` cargo feature only
swaps the panic handler (`panic-halt` → `panic-probe`) and turns on the defmt
result logging (compiled out otherwise, via the `log_info!`/`log_error!` shim in
`main.rs`). `memory.x` (1 MB/768 KB) is a strict subset of the ZI's resources,
so it links unchanged — only the flash-time `--chip` differs.

```bash
# from this crate dir (GMP_MPFR_LIBDIR defaults to ../vendor/gmp-mpfr-arm):
make run-nucleo     # build + flash + stream the self-test (Ctrl-C to detach; fw idles in wfi)
make build-nucleo   # build only
make check-nucleo   # type-check the nucleo config
```

Requires `probe-rs >= 0.24` (decodes the defmt v4 wire format; **0.31** verified).
`make build` (default features) still produces the production RGT6 link-smoke
image with logging compiled out.

## Bring-Up Status

The MCU is pinned and the crate carries a provisional `memory.x`, heap, target,
and GMP/MPFR linker hook. Remaining work is board bring-up: embassy clocks/GPIO,
newlib/libm link cleanup for the C libraries, routing GMP allocations through
`mp_set_memory_functions`, the keyboard-G0 firmware/link protocol, and the real
TM1640 bit-bang driver. See [`../../DESIGN.md`](../../DESIGN.md) for the full
firmware checklist.
