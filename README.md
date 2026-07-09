# Calcumaker 16

[![CI](https://github.com/calcumaker/calcumaker/actions/workflows/ci.yml/badge.svg)](https://github.com/calcumaker/calcumaker/actions/workflows/ci.yml)

> Repo: `calcumaker` · Product: **Calcumaker 16** (see [`NAMING.md`](NAMING.md))

A wide-format, full-size **Cherry MX** **programmer's / technical RPN
calculator**. It carries the HP-16C tradition — hexadecimal / octal / binary /
decimal entry, bitwise operators, and selectable word sizes — and extends it far
past what any single Voyager could do, with **arbitrary-precision** math: **GNU
MP** for big integers, **MPFR** for correctly-rounded floating-point and
transcendentals, and **MPC** for complex numbers. The 16C is the grounding
default (a faithful programmer's machine — `√-1` is `Error 0`, just like the
real one); switchable **personalities** (16C / 15C / SCI / FIN) and a SETUP menu
turn on the scientific and complex features on top. The top of the RPN stack is
shown live on a **multi-row 7-segment display** (2–3 rows) that mounts on its
**own angled PCB**. Battery + USB-C powered; a low-power STM32 keeps it alive
between keystrokes.

> Real keyswitches, a real stack you can see, and answers that are correct to as
> many digits as you ask for.

## Features

- **Programmer's RPN core (HP-16C lineage):** HEX/DEC/OCT/BIN modes, bitwise and
  shift/rotate ops, selectable word size, two's-complement / unsigned / one's-
  complement integer modes.
- **Arbitrary precision (single math path — no fallback):**
  - **GNU MP (libgmp)** — unbounded integers (huge hex/decimal, exact bitwise).
  - **MPFR (libmpfr)** — correctly-rounded floating point + transcendentals
    (sin/cos/exp/ln/…) at user-selectable precision.
  - **MPC (libmpc)** — arbitrary-precision **complex numbers** (HP-42S/15C model:
    one stack object, rectangular `a+bi` or polar `r∠θ`, complex-aware functions,
    CPXRES auto-promotion so `√-4 → 2i`). Off on the 16C, on for the scientific
    personalities and toggleable in SETUP.
  - Packaged as the **`calcumaker-core`** library (`no_std`) over our own
    **`gmp-mpfr-nostd`** FFI bindings — host-testable and runnable today
    (`cargo test`, `cargo run --example repl`).
- **Switchable personalities:** **16C** (programmer, the grounding default),
  **15C** (advanced scientific — the one that natively had complex), **SCI**
  (general scientific), **FIN** (HP-12C-style financial) — one keyboard, chosen
  in SETUP, each just a keymap + defaults over the same superset engine.
- **Field updates over USB-C:** flash new firmware with `dfu-util` via the STM32
  ROM DFU bootloader (`make dfu`) — no ST-Link needed; BOOT0 is the backup.
- **Host emulator:** `calcumaker-emu` runs the whole device UI — keymap, f/g
  shifts, digit entry, the multi-row 7-seg display rendered from the real
  TM1640 segment bytes — on a standard terminal (`cargo run`).
- **Visible RPN stack:** multi-row 7-segment display (2–3 rows) shows the top of
  the stack at once.
- **Split design:** the display lives on its **own PCB** that angles upward;
  only power, the display serial bus, and optional aux-display I²C cross the FFC
  to the MCU board — which keeps wiring simple.
- **Full-size Cherry MX keyswitches:** a wide, tactile, technical-use keypad.
- **Battery + USB-C:** 1S Li-ion with USB-C charging + buck-boost rail; aggressive
  sleep between keypresses for long runtime.
- **Rust firmware:** `no_std` main loop on the **STM32U575RGT6** (Cortex-M33,
  1 MB / 768 KB, ULP — see `DESIGN.md`).

## Hardware Stack

Calcumaker 16 is a three-board KiCad 10 design:

- **`calcumaker-mcu`** is the bottom brain/PSU board: STM32U575, USB-C power and
  charging, the 3V3 rail, the gated 5V display rail, SWD, clocking, the display
  FFC connector, and the keyboard mezzanine.
- **`calcumaker-keyboard`** is the top/front-panel board: 49 Cherry MX keys (a 2U ENTER),
  per-key diodes, annunciator LEDs, and a small STM32G0 keyboard scanner that
  reports key events to the U575 instead of routing the raw matrix across the
  stack.
- **`calcumaker-display`** is the angled display board: three repeated 16-digit
  7-segment rows driven by TM1640s, plus the display FFC and optional aux OLED
  socket.

The schematics are generated from per-board manifests under `hardware/scripts/`
and then placed/wired in KiCad. The display board uses a fully wired
multi-channel row sheet; the MCU and keyboard boards carry the generated sheet
structure plus on-canvas wiring notes. See [`hardware/README.md`](hardware/README.md)
and [`DESIGN.md`](DESIGN.md) for the current hardware source of truth.

## Architecture

```
   DISPLAY BOARD (calcumaker-display) — angled, cabled to the MCU board
   ┌──────────────────────────────────────────────────┐
   │  7-segment RPN stack: 3 rows × 16 digits (2–3 on) │
   │   48× FJ5161AH 0.56" single-digit CC (THT)        │
   │   3× TM1640 driver (1 per row, 2-wire bus)        │
   └──────────────────────────────────────────────────┘
       │ 12-position 0.5mm FFC: +5V, GND, CLK + DIN×3
       │ (5V logic) + 3V3/I²C for the aux OLED — cables down to the MCU board
       ╧
   KEYBOARD BOARD (calcumaker-keyboard) — TOP of stack, keys face up
   ┌──────────────────────────────────────────────────┐
   │  full-size Cherry MX key matrix (+ per-key diode) │
   │  STM32G0 scanner + annunciator LEDs (f g C G lo-b)│
   └──────────────────────┬┬──────────────────────────┘
     low-profile Hirose DF40 mezzanine (0.4mm, 1.5mm stack) + standoffs
        ││  I²C + UART + KB_IRQ (wake) + 3V3/GND  (matrix stays on the G0)
   MCU BOARD (calcumaker-mcu) — BOTTOM of stack
   ┌──────────────────────┴┴──────────────────────────┐
   │  EN-gated 5V boost ──► display │ 74HCT125 (3V3→5V)│
   │   ┌───────────────────────────────────────────┐   │  USB-C ── console /
   │   │ STM32U575RGT6 (Cortex-M33, 1MB/768KB, ULP) │   │          provisioning
   │   │  Rust no_std main loop (embassy)           │   │
   │   │   calcumaker-core: RPN engine              │   │
   │   │        └ GMP + MPFR  (single path)         │   │
   │   │   heap: embedded-alloc (TLSF)              │   │
   │   └────────────────────────────────────────────┘   │
   └───────┬───────────────────────────────────────────┘
           │ VSYS
   1S Li-ion ── USB-C charger ── load-share ── VSYS ──┬── buck-boost → 3V3 (MCU, ULP)
                                                      └── 5V boost (EN-gated) → display
```

## Firmware Stack

The firmware is deliberately split so there is one calculator and multiple thin
I/O bindings around it:

```
   host terminal                         STM32U575 firmware
   ┌─────────────────────┐               ┌─────────────────────┐
   │ calcumaker-emu      │               │ calcumaker-fw       │
   │ crossterm UI        │               │ no_std / embassy    │
   │ host keys + ASCII   │               │ keyboard link +     │
   │ 7-seg rendering     │               │ TM1640 bus + heap   │
   └──────────┬──────────┘               └──────────┬──────────┘
              │                                     │
              └──────────────┬──────────────────────┘
                             │
                    ┌────────▼────────┐
                    │ calcumaker-core │
                    │ Calc + App +    │
                    │ keys + seg7     │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │ gmp-mpfr-nostd  │
                    │ Integer + Float │
                    └────────┬────────┘
                             │
          host system GMP/MPFR or target cross-built GMP/MPFR
```

- **`gmp-mpfr-nostd`** is the math FFI layer: `no_std` + `alloc` wrappers for
  GMP integers and MPFR floats. On the host it links system/Homebrew GMP+MPFR;
  on the MCU it links the cross-built static libraries.
- **`calcumaker-core`** is the calculator: RPN stack, exact integer/programmer
  operations, MPFR real math, modes, keymap and f/g shift layers, entry editing,
  errors, and TM1640 7-segment byte generation. This crate is the single numeric
  path: no `rug`, no `std`, no pure-Rust fallback.
- **`calcumaker-emu`** runs the same `App` on a desktop terminal. It maps host
  keys to the physical matrix and renders the real segment bytes as 7-segment
  art, so scripted examples exercise the same key handling and display pipeline
  the device uses.
- **`calcumaker-fw`** is the U575 board binding: heap setup, power/display
  bring-up, keyboard event intake, and the TM1640 bus. The keyboard board's G0
  firmware owns matrix scan, debounce, wake/IRQ, and annunciator drive; the U575
  consumes `(row,col)` key events and feeds them to `calcumaker_core::App`.

Useful firmware entry points:

```sh
cd firmware/calcumaker-core
cargo test
cargo run --example repl

cd ../calcumaker-emu
cargo run
cargo run -- --press "2;3+"

cd ../..
firmware/scripts/build-gmp-mpfr-arm.sh
```

Host engine tests, the token REPL, and the terminal emulator work today against
the real GMP/MPFR libraries. The target GMP/MPFR build is cross-built and
link-verified; remaining firmware work is MCU/HAL bring-up, newlib/libm link
cleanup, heap routing for GMP allocations, and the keyboard-G0 firmware.

## Repository Structure

```
calcumaker/
├── hardware/                     # PCB design (KiCad 10) — split, three boards
│   ├── lib/                      # project-specific symbols/footprints/3D (shared)
│   │   ├── symbols/ footprints.pretty/ 3dmodels/
│   ├── calcumaker-mcu/           # MCU board (brain/PSU): MCU + PSU + clock + SWD + display-IF + keyboard mezzanine
│   │   ├── calcumaker-mcu.kicad_pro
│   │   ├── (root + mcu / clock / prog / psu / display_if / keyboard_if sub-sheets — generated)
│   │   ├── sym-lib-table · fp-lib-table
│   ├── calcumaker-keyboard/      # keyboard board (stacks above MCU): Cherry MX matrix + annunciators + MCU mezzanine
│   │   ├── calcumaker-keyboard.kicad_pro
│   │   ├── (root + keypad / annunc / main_if sub-sheets — generated)
│   │   ├── sym-lib-table · fp-lib-table
│   ├── calcumaker-display/       # display board (angled, cabled): 7-seg stack + driver + interconnect
│   │   ├── calcumaker-display.kicad_pro
│   │   ├── (root + display_row ×3 / interconnect / aux sub-sheets — generated)
│   │   ├── sym-lib-table · fp-lib-table
│   ├── scripts/                  # schgen engine + per-board manifests, check/render, JLCPCB
│   ├── Makefile                  # PROJECTS = calcumaker-mcu calcumaker-keyboard calcumaker-display
│   ├── LICENSE                   # CERN-OHL-S v2 (hardware)
│   ├── README.md
│   ├── sym-lib-table
│   └── fp-lib-table
├── firmware/
│   ├── gmp-mpfr-nostd/           # own no_std FFI to GMP/MPFR ("rug, but no_std")
│   ├── calcumaker-core/          # the CALCULATOR: engine + keymap + App + 7-seg
│   │   ├── src/                  #   lib, calc, value, format, keys, app, seg7
│   │   ├── tests/ · examples/repl.rs
│   │   └── Cargo.toml            #   single math path (no fallback)
│   ├── calcumaker-emu/           # HOST EMULATOR: the same App on a terminal
│   ├── calcumaker-fw/            # Rust no_std board binary (STM32U575 / embassy)
│   │   ├── Cargo.toml · .cargo/config.toml
│   │   ├── memory.x · build.rs · rust-toolchain.toml
│   │   └── src/                  #   main, keyboard input/link, display (TM1640 bus)
│   ├── scripts/                  # build-gmp-mpfr-arm.sh (cross-build for thumbv8m)
│   ├── common/                   # shared HAL/utilities
│   └── shared/                   # shared protocol/definitions
├── reference/                    # datasheets + dependency pointers
├── DESIGN.md                     # full hardware/firmware design + open questions
├── NAMING.md                     # product naming (Calcumaker 16)
└── LICENSE                       # AGPL-3.0 (firmware)
```

## Key Components

| Component | Part | Status |
|-----------|------|--------|
| MCU | STM32U575RGT6 (1MB/768KB, M33, ULP, LQFP-64) | ✅ selected — LCSC C5270980, JLCPCB Extended |
| Display | 3 rows × 16 digits: 3× TM1640 + 48× FJ5161AH 0.56" CC (THT) | ✅ LCSC C5337152 / C8093 |
| Keys | 5×10 grid, 49 full-size Cherry MX (2U double-height ENTER) + keyboard STM32G0 scanner | electrical/keymap decided; physical layout details still TBD |
| Display interconnect | 12-position 0.5mm FFC: +5V/GND, CLK + DIN×3, 3V3/I²C aux lines | selected in `DESIGN.md`; verify cable length/orientation at layout |
| Power | 1S Li-ion + USB-C charge; **3V3 (TPS63900, MCU)** + **EN-gated 5V boost (display)** | 3V3 ✅; 5V boost + 74HCT125 level shifter TBD (research) |
| Math | GNU MP + MPFR via `calcumaker-core` + own `gmp-mpfr-nostd` (single path) | ✅ no_std, host-tested + REPL + emulator; cross-built + link-verified for the target |

## Status

**The calculator works on the host today**: engine + keymap + display pipeline
are host-tested, and `calcumaker-emu` runs the full device UI on a terminal.
GMP/MPFR are cross-built + link-verified for the STM32 target. **MCU, keypad
layout (5×10), software stack, display BOM, and power architecture are decided** (see
`DESIGN.md` / `hardware/PARTS.md`). The three-board hardware split is in the
repo; the display board is fully wired as a multi-channel KiCad design, while
the MCU and keyboard boards generate from their manifests and still need
eeschema wiring/layout work. Remaining firmware work: U575 MCU bring-up
(embassy + newlib/libm link + heap routing) and the keyboard-G0 scanner
firmware. Battery sizing and fabrication are still ahead.

## License

- **Firmware:** [AGPL-3.0](LICENSE) (compatible with the LGPLv3 GMP/MPFR).
- **Hardware:** [CERN-OHL-S v2](hardware/LICENSE) (strongly reciprocal — matches
  the AGPL copyleft posture).

Copyright (c) 2026 calcumaker authors.
