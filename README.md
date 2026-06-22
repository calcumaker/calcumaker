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
- **Arbitrary precision:**
  - **GNU MP (libgmp)** — unbounded integers (huge hex/decimal, exact bitwise).
  - **MPFR (libmpfr)** — correctly-rounded floating point + transcendentals
    (sin/cos/exp/ln/…) at user-selectable precision.
  - A **pure-Rust** arbitrary-precision backend is the fallback if GMP/MPFR
    can't be made to work `no_std` (selected at build time).
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
   │  multi-row 7-segment RPN stack (2–3 rows)         │
   │      ▲ segment/digit drive                        │
   │   driver IC(s) (MAX7219 SPI / HT16K33 I²C — TBD)  │
   └───────────────────────▲──────────────────────────┘
                           │  interconnect: +3V3, GND, serial bus (SPI)
                           │  (board-to-board / FFC — "simplifies wiring")
   MAIN BOARD (calcumaker-main)
   ┌───────────────────────┴──────────────────────────┐
   │  full-size Cherry MX key matrix (+ per-key diode) │
   │      │ GPIO matrix scan                           │
   │   ┌──┴───────────────────────────────────────┐   │  USB-C ── console /
   │   │ STM32U575ZGT6 (Cortex-M33, 2MB/786KB, ULP)│   │          provisioning
   │   │  Rust no_std main loop (embassy)          │   │
   │   │   RPN engine ──► numeric core             │   │
   │   │        ├ GMP/MPFR via FFI       (preferred)│   │
   │   │        └ pure-Rust bignum        (fallback)│   │
   │   │   heap: embedded-alloc (TLSF)             │   │
   │   └───────────────────────────────────────────┘   │
   └───────┬───────────────────────────────────────────┘
           │ 3V3
   1S Li-ion ── USB-C charger ── load-share ── buck-boost ── 3V3 rail
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
│   ├── calcumaker-fw/            # Rust no_std application (STM32U575 / embassy)
│   │   ├── Cargo.toml            #   numeric core: GMP/MPFR FFI ⟷ pure-Rust
│   │   ├── .cargo/config.toml
│   │   ├── memory.x · build.rs · rust-toolchain.toml
│   │   └── src/                  #   main, rpn, display, keypad, numeric/
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
| Display | 7-segment, 2–3 rows + driver (MAX7219 / HT16K33 / TM-series) | driver+digits being chosen by LCSC price/availability |
| Keys | full-size Cherry MX (wide HP-16C-style layout) | layout TBD |
| Interconnect | board-to-board (FFC / header) main↔display | part TBD by availability |
| Power | 1S Li-ion + USB-C charge + buck-boost | parts TBD (buck-boost sized to display LED current) |
| Math | GNU MP + MPFR (pure-Rust fallback) | path confirmed (FFI to cross-built libs) |

## Status

Repo scaffold (split hardware framework + Rust firmware skeleton + design doc)
is in. **MCU + software stack are decided** (see `DESIGN.md`); the display BOM
and the keypad layout are the next part-selection steps. Hardware/firmware are
not yet built or fabricated.

## License

- **Firmware:** [AGPL-3.0](LICENSE) (compatible with the LGPLv3 GMP/MPFR).
- **Hardware:** [CERN-OHL-S v2](hardware/LICENSE) (strongly reciprocal — matches
  the AGPL copyleft posture).

Copyright (c) 2026 calcumaker authors.
