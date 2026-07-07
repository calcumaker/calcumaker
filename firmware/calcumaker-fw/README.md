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
| `main.rs` | Heap init, GMP/MPFR C-alloc shim, and the async entry on embassy's **low-power executor**; runs the self-test then hands off to USB. |
| `clock.rs` | RCC config: 160 MHz SYSCLK (PLL1-R/HSI), 48 MHz USB (HSI48+CRS), LSI for the RTC. Shared by both boards. |
| `usb.rs` | USB OTG_FS **composite device**: CDC-ACM console wired to a live RPN REPL over `calcumaker_core`, + a HID keyboard that types the X value to the host. |
| `selftest.rs` | Golden-case engine self-test (see the validation target below). |
| `keypad.rs` | Provisional keyboard-event intake. The real matrix scan/debounce/IRQ lives on the keyboard board's STM32G0 firmware; the U575 consumes `(row,col)` events. |
| `display.rs` | TM1640 bus driver skeleton for the multi-row 7-segment display. |

## MCU bring-up (embassy-stm32)

The crate runs on `embassy-stm32` (async). Brought up and **validated on the
Nucleo-U575ZI-Q**:

- **Clocks** (`clock.rs`): 160 MHz SYSCLK, 48 MHz crystal-free USB clock (HSI48
  CRS-trimmed to the USB SOF), LSI enabled for the RTC.
- **USB** (`usb.rs`): an IAD composite device on OTG_FS (PA11/PA12,
  `vbus_detection=false`). Interface 0/1 = CDC-ACM, a full RPN REPL over the real
  engine (`2 3 +` → `5`, `float 2 sqrt` → 256-bit MPFR, `100 !` → the 158-digit
  factorial); interface 2 = a HID keyboard — the `type` command taps the current
  X value to the focused host app.
- **Low power**: uses embassy's low-power executor, so the part enters the
  deepest sleep the peripheral **stop-refcounts** allow when idle. USB OTG is
  `stop_mode=Stop1`, so while the console is enumerated it stays in Sleep (WFI,
  USB-safe) and wakes on USB activity; deeper STOP is only reached once USB is
  gated. **Deferred:** STOP2-with-timekeeping needs an RTC time driver that
  embassy-stm32 0.6 doesn't yet provide for the U5 (its LPTIM driver miscompiles
  against the current metapac), and a dedicated VBUS-EXTI wake-on-plug source —
  both revisited on an embassy bump / with the production board's VBUS routing.

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

Everything is shared with the production image. The `nucleo` cargo feature swaps
the panic handler (`panic-halt` → `panic-probe`), turns on the defmt result
logging (compiled out otherwise, via the `log_info!`/`log_error!` shim in
`main.rs`), and selects the ZI chip. The linker memory map comes from
embassy-stm32's `memory-x` per chip (RG = 1 MB, ZI = 2 MB), so only the chip
feature + flash-time `--chip` differ between the two boards.

After the self-test the Nucleo image brings up USB, so the board enumerates as a
composite CDC-ACM + HID keyboard device you can drive interactively (see *MCU
bring-up* above).

```bash
# from this crate dir (GMP_MPFR_LIBDIR defaults to ../vendor/gmp-mpfr-arm):
make run-nucleo     # build + flash + stream the self-test over RTT (Ctrl-C to detach)
make build-nucleo   # build only
make check-nucleo   # type-check the nucleo config
```

Requires `probe-rs >= 0.24` (decodes the defmt v4 wire format; **0.31** verified).
`make build` (default features) still produces the production RGT6 link-smoke
image with logging compiled out.

## Bring-Up Status

Done + validated on hardware (Nucleo-U575ZI-Q): the full GMP/MPFR engine
cross-compiles/links and computes correctly on-target, 160 MHz clocks, the USB
composite (CDC-ACM REPL + HID keyboard), and low-power-executor Sleep with
USB-gated STOP.

Remaining board bring-up: the keyboard-G0 link protocol (matrix `(row,col)`
intake), the real TM1640 bit-bang display driver, routing GMP allocations
through `mp_set_memory_functions` (vs. the current global-heap malloc shim),
deep STOP2 + VBUS wake (see *MCU bring-up* deferrals), and moving the engine off
the USB REPL onto the shared `App`. See [`../../DESIGN.md`](../../DESIGN.md) for
the full firmware checklist.
