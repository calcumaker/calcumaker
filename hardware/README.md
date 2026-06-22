# Calcumaker 16 — hardware

KiCad 10 design for the **Calcumaker 16** programmer's / technical RPN
calculator. **Split design** — two PCBs:

- **`calcumaker-main/`** — MCU (STM32U575ZGT6), PSU (USB-C charge + buck-boost),
  the Cherry MX key matrix, and the interconnect to the display board.
- **`calcumaker-display/`** — the multi-row 7-segment RPN stack (2–3 rows) + its
  driver IC(s) + the interconnect back to the main board. Mounts at an upward
  angle; only +3V3, GND, and the display serial bus cross the connector (this is
  what "simplifies wiring").

See `../DESIGN.md` for the full design and `scripts/README.md` for the
schematic-generation flow. Build docs/BOMs/fab packages with the `Makefile`
(`make help`).

## Library

`lib/{symbols,footprints.pretty,3dmodels}` holds project-specific parts (shared
by both boards, lib name `calcumaker`); most parts resolve to KiCad bundled
libraries.

## License — Hardware: CERN-OHL-S v2

The hardware (schematics, PCB layouts, and associated design files under
`hardware/`) is licensed under the **CERN Open Hardware Licence Version 2 —
Strongly Reciprocal** (`CERN-OHL-S-2.0`). Full text: [`LICENSE`](LICENSE).
Chosen to match the strong-copyleft posture of the AGPL-3.0 firmware.

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
