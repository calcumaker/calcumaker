# Third-party & authored library parts

Vendored / authored KiCad symbols, footprints and 3D models live in this `lib/`
directory and are referenced project-locally through each board's `sym-lib-table` /
`fp-lib-table` (the shared **`calcumaker:`** library). **Record every part here with
its source and license.**

This matters more than usual for this repo: `hardware/LICENSE` is **CERN-OHL-S v2**
(strongly reciprocal), so the provenance of every imported land pattern and symbol has
to be traceable. It also has a practical payoff — an attribution line stating *where a
pinout came from* is what lets a future reader trust a part over a guess. (That is not
hypothetical: the MAX17048 footprint below was confirmed against the sibling
`notchdeck` repo precisely because its attribution recorded "from ADI datasheet".)

Import workflow (matches the other BenchBits hardware projects):

```sh
# 1. drop the pre-built KiCad part in place
cp <download>/Foo.kicad_mod   lib/footprints.pretty/
cp <download>/Foo.step        lib/3dmodels/
# 2. append the (symbol "Foo" ...) block to lib/symbols/calcumaker.kicad_sym
#    (drop any vendor library prefix on the embedded Footprint property so it
#     resolves against our project-local "calcumaker:" library)
# 3. repoint the footprint's 3D path to our local copy:
sed -i 's|\${KICAD.*_3RD_PARTY}/3dmodels/.*/|\${KIPRJMOD}/../lib/3dmodels/|' \
    lib/footprints.pretty/Foo.kicad_mod
```

## Imported / authored parts

| Part | Symbol | Footprint | 3D | Source | License |
|---|---|---|---|---|---|
| **Kailh CPG151101S11 hot-swap MX (1u)** | stdlib | ✅ `SW_MX_HS_CPG151101S11_1u.kicad_mod` | ✅ `HS_CPG151101S11_MX.step` + `SW_Cherry_MX_1.00u_PCB.step` | [ebastler/marbastlib](https://github.com/ebastler/marbastlib) | **CERN-OHL-P** |
| **MX 2U vertical (ENTER), hot-swap + PCB stabs** | stdlib | ✅ `SW_MX_HS_CPG151101S11_2u_Vertical.kicad_mod` (derived) | ↑ same | in-house: marbastlib 1u + stab holes re-expressed from stdlib `SW_Cherry_MX_2.00u_Vertical_PCB` | **CERN-OHL-P** (marbastlib) + CC-BY-SA-4.0 w/ exception (KiCad) |
| **Cherry MX solder-in 1u** (alt board rev) | stdlib | ✅ `SW_Cherry_MX_1.00u_PCB.kicad_mod` (stdlib copy) | ✅ `SW_Cherry_MX_1.00u_PCB.step` | footprint: KiCad stdlib · 3D: [kiswitch/kiswitch](https://github.com/kiswitch/kiswitch) (no official KiCad model exists) | CC-BY-SA-4.0 w/ exception · 3D **MIT** (dual MIT/CC-BY-SA-4.0, used under MIT) |
| **Titan Micro TM1640** (7-seg driver) | ✅ `calcumaker:TM1640` (authored) | stdlib `Package_SO:SOIC-28W_7.5x18.7mm_P1.27mm` | stdlib | in-house, from the Titan Micro datasheet | own work |
| **FJ5161AH** (0.56" single-digit 7-seg, CC) | ✅ `calcumaker:FJ5161AH` (authored) | stdlib (`LTS6760` land) | stdlib | in-house, from the vendor datasheet | own work |
| **ST TCPP01-M12** (USB-C port protection) | ✅ `calcumaker:TCPP01-M12` (authored) | stdlib `Package_DFN_QFN:QFN-12-1EP_3x3mm_P0.5mm_EP1.45x1.45mm_ThermalVias` | stdlib | in-house, from **ST DS12900 rev 4** — pinout Table 1; package Table 15 (D=E=3.00, e=0.50, EP D2=E2=1.45) | own work |
| **ADI MAX17048** (1-cell fuel gauge) | ✅ `calcumaker:MAX17048` (authored) | stdlib `Package_DFN_QFN:TDFN-8-1EP_2x2mm_P0.5mm_EP0.8x1.2mm` | stdlib | in-house, from the ADI datasheet pinout; **independently cross-checked against sibling repo `notchdeck`** (same symbol + footprint) | own work |
| **XINGLIGHT XL-1010RGBC-2812B-S** (C51900942, 1010 addressable RGB) | stdlib `LED:SK6812` (pad-compatible) | ✅ `LED_XL1010RGBC_1.0x1.0mm.kicad_mod` (authored) | ☐ none | in-house, from the XINGLIGHT datasheet | own work |
| Everything else | stdlib | stdlib | stdlib | KiCad standard libraries | CC-BY-SA-4.0 w/ library exception |

(☐ = not present. "stdlib" = KiCad-shipped, nothing vendored.)

## License compatibility with CERN-OHL-S v2

The hardware is licensed **CERN-OHL-S v2** (`hardware/LICENSE`), which is *strongly
reciprocal*. Each imported part is compatible with being incorporated into it:

- **CERN-OHL-P** (marbastlib) — the *permissive* variant of the same family; it may be
  combined into an OHL-S design, and OHL-S is the license of the resulting whole.
- **MIT** (kiswitch 3D model) — permissive; compatible.
- **KiCad standard libraries** — CC-BY-SA-4.0 **with the KiCad library exception**,
  which explicitly permits using the libraries in a design without the design itself
  becoming a CC-BY-SA derivative.

Keep the source-location notice required by CERN-OHL-S in `hardware/README.md`.

## Verify-before-fab

Authored land patterns carry the fab risk, so they are listed here explicitly rather
than left in a footprint `descr` where nobody reads them:

- ⚠ **`LED_XL1010RGBC_1.0x1.0mm`** — authored 1 mm / 1010 land. **Verify pad geometry
  and the net-to-corner mapping against the XINGLIGHT datasheet before fab.** Pads are
  numbered to match the stdlib `LED:SK6812` symbol (1=VSS 2=DIN 3=VDD 4=DOUT).
- ✅ **TCPP01-M12** — EP 1.45 × 1.45 mm, matches ST DS12900 Table 15.
- ✅ **MAX17048** — EP 0.8 × 1.2 mm; KiCad's footprint `descr` cites **Maxim package
  outline 21-0168**, the authoritative drawing for this package.

> **Exposed pads are visible pins** in the authored `TCPP01-M12` and `MAX17048` symbols
> (pad 13 / pad 9), *not* hidden power pins. Our sheets are generated **placed-not-wired**
> and finished by hand, and a hidden EP is invisible to both the person wiring the board
> and to ERC — so a floating thermal/ground pad would pass every check we run. On the
> TCPP01 the EP is a genuine ground return. **Wire them.**

## Details

Per-part specifics (why the hot-swap footprint is place-on-back, the 2U stabilizer
geometry, why no official Cherry MX 3D model exists upstream) live in:

- `footprints.pretty/README.md`
- `3dmodels/README.md`
