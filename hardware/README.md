# Calcumaker 16 — hardware

KiCad 10 design for the **Calcumaker 16** programmer's / technical RPN
calculator. **Split design** — three PCBs (two stacked, one cabled):

- **`calcumaker-mcu/`** — the brain/PSU board: MCU (STM32U575RGT6), PSU (USB-C
  charge + buck-boost), clock, SWD, the display 5V rail + level shifter +
  interconnect, and a fine-pitch mezzanine up to the keyboard board. *Bottom of
  the stack.*
- **`calcumaker-keyboard/`** — the front-panel board: the 49-key Cherry MX matrix (2U ENTER)
  + per-key diodes + the annunciator LEDs + the mating mezzanine header.
  *Mezzanine-stacks directly above the MCU board* (keeps a dense LQFP-64 off the
  through-hole key matrix).
- **`calcumaker-display/`** — the multi-row 7-segment RPN stack (2–3 rows) + its
  driver ICs + the interconnect back to the MCU board. Mounts at an upward angle,
  cabled; power, the display serial bus, and optional aux-display I2C cross the
  FFC.

See `../DESIGN.md` for the full design and `scripts/README.md` for the
schematic-generation flow. Build docs/BOMs/fab packages with the `Makefile`
(`make help`).

## Library

`lib/{symbols,footprints.pretty,3dmodels}` holds project-specific parts (shared
by all boards, lib name `calcumaker`); most parts resolve to KiCad bundled
libraries.

## License — Hardware: CERN-OHL-S v2

The hardware (schematics, PCB layouts, and associated design files under
`hardware/`) is licensed under the **CERN Open Hardware Licence Version 2 —
Strongly Reciprocal** (`CERN-OHL-S-2.0`). Full text: [`LICENSE`](LICENSE).
Chosen to match the strong-copyleft posture of the AGPL-3.0 firmware.

**Third-party library parts** (vendored symbols / footprints / 3D models) carry their
own upstream licenses — CERN-OHL-P, MIT, and CC-BY-SA-4.0-with-exception — all of which
are compatible with incorporation into a CERN-OHL-S v2 design. Every one is indexed,
with its source and license, in **[`lib/ATTRIBUTIONS.md`](lib/ATTRIBUTIONS.md)**.

> Copyright (c) 2026 calcumaker authors.
>
> This source describes Open Hardware and is licensed under the CERN-OHL-S v2.
>
> You may redistribute and modify this source and make products using it under
> the terms of the CERN-OHL-S v2 (https://ohwr.org/cern_ohl_s_v2.txt).
>
> This source is distributed WITHOUT ANY EXPRESS OR IMPLIED WARRANTY, INCLUDING
> OF MERCHANTABILITY, SATISFACTORY QUALITY AND FITNESS FOR A PARTICULAR PURPOSE.
> Please see the CERN-OHL-S v2 for applicable conditions.
>
> Source location: https://github.com/calcumaker/calcumaker
>
> As per CERN-OHL-S v2 § 4, if you produce a device using this source, you
> should where practicable keep the above Source Location visible on the device.
