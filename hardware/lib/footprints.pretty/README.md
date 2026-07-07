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

## Vendored

- **`SW_MX_HS_CPG151101S11_1u.kicad_mod`** — the keyswitch **hot-swap** footprint
  (Kailh CPG151101S11 socket). From
  [ebastler/marbastlib](https://github.com/ebastler/marbastlib) (**CERN-OHL-P**);
  3D `(model)` lines repointed to `${KIPRJMOD}/../lib/3dmodels/` — socket (as
  authored) + the switch body added on the **B side** (`rotate 180`, so it renders
  opposite the socket for **place-on-back** placement). This is `MX_FP`.
  **Hot-swap only** — the thru-holes are 0.15 mm-ring socket pass-throughs, not
  solder pads; place switch fps on the board's **back** layer (socket → bottom).
- **`SW_Cherry_MX_1.00u_PCB.kicad_mod`** — plain **solder-in-only** 1u footprint
  (stock KiCad copy + the kiswitch 3D). Use this for a solder-in board revision
  (a separate board, since the hot-swap fp above isn't solderable).
