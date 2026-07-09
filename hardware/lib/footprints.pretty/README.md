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
- **`SW_MX_HS_CPG151101S11_2u_Vertical.kicad_mod`** — the **2U vertical** ENTER
  variant of the hot-swap footprint: the 1u footprint above plus the four
  **PCB-mount 2U stabilizer** NPTH holes, taken from stock
  `Button_Switch_Keyboard:SW_Cherry_MX_2.00u_Vertical_PCB` and re-expressed about
  our origin (which is already the switch centre): `y = ±11.90 mm` (23.8 mm stab
  spacing), each wing a `Ø3.05` hole at `x = −7.00` and a `Ø4.00` at `x = +8.24`
  (15.24 mm apart). Courtyard grown to ±10.5 × ±14.2; the 2U keycap envelope
  (19.05 × 38.10) is drawn on `F.Fab`. This is `MX_2U_FP`.
  - *Place-on-back is safe:* mirroring X maps the stab-hole set onto itself
    rotated 180°, and a 2U key is symmetric — a standard stabilizer still fits.
  - **Only needed for PCB-mount stabilizers.** We default to **plate-mount**
    stabs (they clip into the switch plate the hot-swap sockets already require),
    for which KiCad's own `SW_Cherry_MX_2.00u_Vertical_Plate` has *the same holes
    as the 1u plate footprint* — i.e. the PCB needs no stabilizer holes at all and
    ENTER just reuses the 1u footprint. Choosing PCB-mount would **also** force a
    Row5 variant sheet, because ENTER's switch lives on the shared 10-key sheet
    and multi-channel instances must share footprints. See DESIGN.md "The 2U ENTER".

- **`SW_Cherry_MX_1.00u_PCB.kicad_mod`** — plain **solder-in-only** 1u footprint
  (stock KiCad copy + the kiswitch 3D). Use this for a solder-in board revision
  (a separate board, since the hot-swap fp above isn't solderable).
