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
- **Bindings: our own `firmware/gmp-mpfr-nostd/`** — thin `no_std` FFI to
  GMP/MPFR (*like `rug`, but no_std*). Host links system GMP/MPFR (`brew install
  gmp mpfr`); target links cross-built. **Single math path — no pure-Rust
  fallback, no `rug`/`std` in the engine.**
- For the target: cross-built GMP/MPFR (`--host=arm-none-eabi --disable-assembly`
  + picolibc); see DESIGN.md → "GMP/MPFR on the target".
- Embedded Rust: **embassy-stm32** (async HAL), **cortex-m** / **cortex-m-rt**,
  **embedded-alloc** (heap for the bignum allocator), **probe-rs** (flash/debug).

## Datasheets (download into this folder via the `digikey`/`datasheets` skills)
Populate as the layout moves toward fabrication. Current expected set:

| Block | Part | Notes |
|-------|------|-------|
| MCU | STM32U575ZGT6 (LCSC C5271004) | flash/RAM, low-power modes, USB FS, FPU, AF map, SRAM banking |
| Keyboard scanner | STM32G031K8U6 (LCSC C432207) | GPIO, Stop/EXTI wake, I2C/UART, package |
| Display driver | TM1640 (LCSC C5337152) | digit/segment drive, current set, 2-wire interface |
| 7-seg display | FJ5161AH (LCSC C8093) | 0.56 in single-digit common-cathode THT, pinout |
| Display FFC | AFC01-S12FCA-00 + GCT FFC05-TIN cable | 0.5 mm 12-position connector/cable orientation and current |
| Keyboard mezzanine | Hirose DF40C-10DS/DP-0.4V | stack height, land pattern, 3D clearance |
| Keyswitch | Cherry MX (full size) | mechanical/electrical, hot-swap socket option |
| Power | USB-C charger, TPS63900 3V3, TPS61022 5V boost, SN74HCT125 level shifter | charge current, Iq, input range, boost sizing, logic thresholds |
| Battery | 1S Li-ion/LiPo | capacity, protection |

> Use the `digikey`/`datasheets` skills to populate this folder, then the
> `kicad`/`emc`/`spice` analyzers can consume verified specs.
