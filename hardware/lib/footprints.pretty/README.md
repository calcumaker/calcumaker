# calcumaker project footprints

Most parts resolve to **KiCad bundled libraries** (symbols + footprints) — see
the "KiCad Library Map" in `../../../DESIGN.md`.

This directory is reserved for any part that is **not** in KiCad's standard
libraries. The most likely candidates for this project are:

- the **Cherry MX keyswitch** footprints (plate/PCB-mount, with or without
  Kailh hot-swap sockets) — KiCad's `keyswitch-kicad-library` is the usual
  source; vendor here if not installed system-wide;
- specific **7-segment display** modules whose pinout/dimensions don't match a
  bundled `Display_7Segment:*` footprint;
- the chosen **MCU package** if its exact footprint isn't bundled.

Add `*.kicad_mod` files here and register them via `../../fp-lib-table` /
`../../calcumaker/fp-lib-table` (lib name `calcumaker`).
