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
| ✅ Cherry MX keyswitch | `SW_Cherry_MX_1.00u_PCB.step` | **vendored** from `kiswitch/kiswitch` (MIT) — see below |
| 7-segment display module(s) | `<display>.step` | Vendor (e.g. Kingbright) product page / SnapMagic |
| Display driver IC (MAX7219 / HT16K33 / TLC59xx — TBD) | `<MPN>.step` | SnapMagic / DigiKey models |

## Vendored: `SW_Cherry_MX_1.00u_PCB.step`

KiCad's stock `Button_Switch_Keyboard.3dshapes` **isn't in the packages3D repo at
all** (the bundled footprint's `(model)` line is dangling *upstream* — no official
model exists — which is why it's not installed and can't be downloaded from
KiCad), so the switch showed no 3D. We vendor the community model from the
**[kiswitch/kiswitch](https://github.com/kiswitch/kiswitch)** library
(`library/3dmodels/3d-library.3dshapes/SW_Cherry_MX_PCB.stp`), **dual-licensed
MIT / CC-BY-SA-4.0** — used here under **MIT** (© kiswitch contributors). The
vendored footprint `../footprints.pretty/SW_Cherry_MX_1.00u_PCB.kicad_mod` (lib
`calcumaker`) references it via `${KIPRJMOD}/../lib/3dmodels/…`.

## Vendored: `HS_CPG151101S11_MX.step` (Kailh hot-swap socket)

The keyswitches use a **hot-swap footprint**
(`../footprints.pretty/SW_MX_HS_CPG151101S11_1u.kicad_mod`, Kailh CPG151101S11) —
vendored from **[ebastler/marbastlib](https://github.com/ebastler/marbastlib)**
(**CERN-OHL-P**, © marbastlib contributors). Its footprint references **two**
models here: `SW_Cherry_MX_1.00u_PCB.step` (the switch body) + `HS_CPG151101S11_MX.step`
(the socket). Both resolve via `${KIPRJMOD}/../lib/3dmodels/…`. The switch-body
`(model)` is on the **B side** (`rotate 180 0 0`, `offset 0 0 -1.6`) so it renders
opposite the socket — correct for **place-on-back** placement (socket on bottom).

For the 2u-Enter option later, marbastlib also has `STAB_MX_2u` (stabilizer
footprint + 3D) and KiCad's own `SW_Cherry_MX_2.00u_PCB` already carries the
stabilizer mounts.

## Attaching a downloaded model to its (bundled) footprint

1. Place the part, open **Footprint Properties → 3D Models**.
2. Add `${KIPRJMOD}/../lib/3dmodels/<file>.step`.
3. Set offset/rotation if the vendor model isn't origin-centered.

(`${KIPRJMOD}` is the board dir — `hardware/calcumaker-mcu/`,
`hardware/calcumaker-keyboard/`, or `hardware/calcumaker-display/` — so
`../lib/3dmodels/` resolves here for all three.)
These sources need a **free login**, so the files can't be auto-fetched in CI —
download once and commit them here (STEP only; ~tens of KB each).
