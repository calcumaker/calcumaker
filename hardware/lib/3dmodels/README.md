# calcumaker 3D models

Most parts get their 3D model automatically from KiCad's bundled libraries
(0402/0805 R/C/L/LED, crystals, JST-PH, USB-C, SOT/SON/QFN packages, diodes,
headers). Parts **without** a bundled model should be downloaded as STEP and
dropped here, then attached per-board in the PCB editor.

Likely candidates for this project (fill in once parts are finalized — see
`DESIGN.md`):

| Part | Drop file here as | Where to download |
|------|-------------------|-------------------|
| MCU (package TBD — see DESIGN.md part selection) | `<MPN>.step` | ST product page → CAD Resources / Ultra Librarian / SnapMagic |
| Cherry MX keyswitch | `Cherry_MX.step` | Cherry CAD downloads / SnapMagic / `keyswitch-kicad-library` |
| 7-segment display module(s) | `<display>.step` | Vendor (e.g. Kingbright) product page / SnapMagic |
| Display driver IC (MAX7219 / HT16K33 / TLC59xx — TBD) | `<MPN>.step` | SnapMagic / DigiKey models |

## Attaching a downloaded model to its (bundled) footprint

1. Place the part, open **Footprint Properties → 3D Models**.
2. Add `${KIPRJMOD}/../lib/3dmodels/<file>.step`.
3. Set offset/rotation if the vendor model isn't origin-centered.

(`${KIPRJMOD}` is the board dir — `hardware/calcumaker-main/` or
`hardware/calcumaker-display/` — so `../lib/3dmodels/` resolves here for both.)
These sources need a **free login**, so the files can't be auto-fetched in CI —
download once and commit them here (STEP only; ~tens of KB each).
