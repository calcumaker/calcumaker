# Calcumaker 16

> Repo: `calcumaker` · Product: **Calcumaker 16** (see [`NAMING.md`](NAMING.md))

A wide-format, full-size **Cherry MX** **programmer's / technical RPN
calculator**. It carries the HP-16C tradition — hexadecimal / octal / binary /
decimal entry, bitwise operators, and selectable word sizes — and extends it
with **arbitrary-precision** math: **GNU MP** for big integers and **MPFR** for
correctly-rounded floating-point and transcendental functions. The top of the
RPN stack is shown live on a **multi-row 7-segment display** (2–3 rows) that
mounts on its **own angled PCB**. Battery + USB-C powered; a low-power STM32
keeps it alive between keystrokes.

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
  - Packaged as the **`calcumaker-core`** library (`no_std`) over our own
    **`gmp-mpfr-nostd`** FFI bindings — host-testable and runnable today
    (`cargo test`, `cargo run --example repl`).
- **Host emulator:** `calcumaker-emu` runs the whole device UI — keymap, f/g
  shifts, digit entry, the multi-row 7-seg display rendered from the real
  TM1640 segment bytes — on a standard terminal (`cargo run`).
- **Visible RPN stack:** multi-row 7-segment display (2–3 rows) shows the top of
  the stack at once.
- **Split design:** the display lives on its **own PCB** that angles upward;
  only +3V3, GND, and the display serial bus cross the interconnect to the main
  board — which keeps wiring simple.
- **Full-size Cherry MX keyswitches:** a wide, tactile, technical-use keypad.
- **Battery + USB-C:** 1S Li-ion with USB-C charging + buck-boost rail; aggressive
  sleep between keypresses for long runtime.
- **Rust firmware:** `no_std` main loop on the **STM32U575ZGT6** (Cortex-M33,
  2 MB / 786 KB, ULP — see `DESIGN.md`).

## Architecture

```
   DISPLAY BOARD (calcumaker-display) — angled                        
   ┌──────────────────────────────────────────────────┐
   │  7-segment RPN stack: 3 rows × 16 digits (2–3 on) │
   │      ▲ FJ5161AH 0.56" common-cathode (THT)        │
   │   3× TM1640 driver (1 per row, 2-wire bus)        │
   └───────────────────────▲──────────────────────────┘
                           │  interconnect (1×8 2.54mm header):
                           │  +5V, GND, CLK + DIN×3 (5V logic)  ("simplifies wiring")
   MAIN BOARD (calcumaker-main)
   ┌───────────────────────┴──────────────────────────┐
   │  EN-gated 5V boost ──► display │ 74HCT125 (3V3→5V)│
   │  full-size Cherry MX key matrix (+ per-key diode) │
   │      │ GPIO matrix scan                           │
   │   ┌──┴───────────────────────────────────────┐   │  USB-C ── console /
   │   │ STM32U575ZGT6 (Cortex-M33, 2MB/786KB, ULP)│   │          provisioning
   │   │  Rust no_std main loop (embassy)          │   │
   │   │   calcumaker-core: RPN engine          │   │
   │   │        └ GMP + MPFR  (single path)      │   │
   │   │   heap: embedded-alloc (TLSF)             │   │
   │   └───────────────────────────────────────────┘   │
   └───────┬───────────────────────────────────────────┘
           │ VSYS
   1S Li-ion ── USB-C charger ── load-share ── VSYS ──┬── buck-boost → 3V3 (MCU, ULP)
                                                      └── 5V boost (EN-gated) → display
```

## Repository Structure

```
calcumaker/
├── hardware/                     # PCB design (KiCad 10) — split, two boards
│   ├── lib/                      # project-specific symbols/footprints/3D (shared)
│   │   ├── symbols/ footprints.pretty/ 3dmodels/
│   ├── calcumaker-main/          # main board: MCU + PSU + keypad + interconnect
│   │   ├── calcumaker-main.kicad_pro
│   │   ├── (root + mcu / psu / keypad / interconnect sub-sheets — generated)
│   │   ├── sym-lib-table · fp-lib-table
│   ├── calcumaker-display/       # display board: 7-seg stack + driver + interconnect
│   │   ├── calcumaker-display.kicad_pro
│   │   ├── (root + display / interconnect sub-sheets — generated)
│   │   ├── sym-lib-table · fp-lib-table
│   ├── scripts/                  # schgen engine + per-board manifests, check/render, JLCPCB
│   ├── Makefile                  # PROJECTS = calcumaker-main calcumaker-display
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
│   │   └── src/                  #   main, keypad (scan), display (TM1640 bus)
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
| MCU | STM32U575ZGT6 (2MB/786KB, M33, ULP, LQFP-144) | ✅ selected — LCSC C5271004, JLCPCB Extended |
| Display | 3 rows × 16 digits: 3× TM1640 + 12× FJ5161AH 0.56" CC (THT) | ✅ LCSC C5337152 / C8093 |
| Keys | full-size Cherry MX (wide HP-16C-style layout) | layout TBD |
| Interconnect | 1×8 2.54mm header (PZ254V-11-08P), carries +5V | ✅ LCSC C492407 |
| Power | 1S Li-ion + USB-C charge; **3V3 (TPS63900, MCU)** + **EN-gated 5V boost (display)** | 3V3 ✅; 5V boost + 74HCT125 level shifter TBD (research) |
| Math | GNU MP + MPFR via `calcumaker-core` + own `gmp-mpfr-nostd` (single path) | ✅ no_std, host-tested + REPL + emulator; cross-built + link-verified for the target |

## Status

**The calculator works on the host today**: engine + keymap + display pipeline
are host-tested, and `calcumaker-emu` runs the full device UI on a terminal.
GMP/MPFR are cross-built + link-verified for the STM32 target. **MCU, keypad
layout (5×10), software stack, display BOM, and power rails are decided** (see
`DESIGN.md` / `hardware/PARTS.md`); both boards generate from their manifests
and pass structure checks. Remaining: eeschema wiring, firmware MCU bring-up
(embassy + newlib link + heap routing), battery sizing. Not yet fabricated.

## License

- **Firmware:** [AGPL-3.0](LICENSE) (compatible with the LGPLv3 GMP/MPFR).
- **Hardware:** [CERN-OHL-S v2](hardware/LICENSE) (strongly reciprocal — matches
  the AGPL copyleft posture).

Copyright (c) 2026 calcumaker authors.
