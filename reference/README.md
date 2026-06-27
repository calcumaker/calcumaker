# Reference Material

Pointers to source material and dependencies this project is built from.

## Repo skeleton
Modeled after the sibling BenchBits hardware+firmware repos:
- **`github/ephemerkey/`** — freshest scaffold (KiCad 10, `hardware/` + `scripts/`
  schematic-generation flow, `Makefile`, lib-tables, JLCPCB script, `DESIGN.md`
  style). Closest structural template.
- **`github/notchdeck/`**, **`github/tsumikoro/`** — the same hardware/firmware
  layout and `hardware/scripts/` tooling.

The firmware departs from those (which are C / Zephyr / ESPHome): calcumaker's
main loop is **Rust (no_std)**.

## Firmware dependencies (see DESIGN.md)
- **GNU MP (libgmp)** — arbitrary-precision integers (the HP-16C "programmer"
  side: big hex/dec/oct/bin, bitwise). https://gmplib.org/ (LGPLv3 / GPLv2 dual)
- **MPFR (libmpfr)** — arbitrary-precision floats with correct rounding (the
  scientific side: transcendentals). https://www.mpfr.org/ (LGPLv3)
- **rug / gmp-mpfr-sys** — Rust bindings; `rug` self-builds GMP/MPFR/MPC.
  https://crates.io/crates/rug · used by `calcumaker-core` for host dev+test.
  **Single math path — no pure-Rust fallback.**
- For the target: cross-built GMP/MPFR (`--host=arm-none-eabi --disable-assembly`
  + picolibc); see DESIGN.md → "GMP/MPFR on the target".
- Embedded Rust: **embassy-stm32** (async HAL), **cortex-m** / **cortex-m-rt**,
  **embedded-alloc** (heap for the bignum allocator), **probe-rs** (flash/debug).

## Datasheets (download into this folder via the `digikey`/`datasheets` skills)
Populate once parts are finalized (DESIGN.md). Expected set:

| Block | Part | Notes |
|-------|------|-------|
| MCU | STM32U575ZGT6 (LCSC C5271004) | flash/RAM, low-power modes, USB FS, FPU, AF map, SRAM banking |
| Display driver | TBD (MAX7219 / HT16K33 / TLC59xx) | digit/segment drive, current set, interface |
| 7-seg display | TBD | digit height, common-anode/cathode, pinout |
| Keyswitch | Cherry MX (full size) | mechanical/electrical, hot-swap socket option |
| Power | USB-C charger + buck-boost (TBD) | charge current, Iq, input range |
| Battery | 1S Li-ion/LiPo | capacity, protection |

> Use the `digikey`/`datasheets` skills to populate this folder, then the
> `kicad`/`emc`/`spice` analyzers can consume verified specs.
