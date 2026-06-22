# Calcumaker 16 — Parts

Source-of-truth mapping: each line → a real **LCSC** part (for the JLCPCB BOM).
LCSC/MPN/Manufacturer are set as KiCad symbol fields when parts are placed, so
`make bom-<board>` emits a JLCPCB BOM. `✅` = resolved by availability research;
`TBD` = pending an Open Question (see `../DESIGN.md`).

> Prices/stock are point-in-time (fetched during scaffolding, 2026-06); re-verify
> at order time. All ICs/displays below are JLCPCB **Extended** (no Basic option
> in these categories).

## calcumaker-main (MCU / PSU / keypad / interconnect)

| Block | Part | LCSC | Pkg / footprint | Status |
|-------|------|------|-----------------|--------|
| MCU | STM32U575ZGT6 | C5271004 | LQFP-144 | ✅ ~$4.90 |
| USB-C | GCT USB4105 (USB2.0 16P) | C2927039 | TopMnt horizontal | ✅ (from ephemerkey PSU) |
| ESD | USBLC6-2SC6 | C2687116 | SOT-23-6 | ✅ |
| Charger | MCP73831T-2ACI/OT | C424093 | SOT-23-5 | ✅ (PROG sized to cell) |
| Load-share FET | AO3401A (P-MOS) | C15127 | SOT-23 | ✅ |
| Load-share diode | B5819W (Schottky) | C8598 | SOD-123 | ✅ |
| Buck-boost 3V3 (MCU) | TPS63900DSKR | C1518762 | WSON-10 | ✅ MCU-only now (light load); ULP low-Iq |
| 3V3 inductor | FNR3015S2R2MT 2.2µH | C167747 | 3015 | ✅ (per TPS63900) |
| 5V boost (display) | EN-gated boost, ~0.6–0.8A | TBD | TBD | researching (MT3608/TPS6102x…) |
| 5V boost inductor | per boost | TBD | TBD | per boost choice |
| Level shifter | 74HCT125 (quad, VCC=5V, 3V3→5V) | TBD | SOIC-14 | researching LCSC# |
| Battery conn | JST S2B-PH-K-S | C173752 | PH 2.0 | ✅ |
| RTC xtal | 32.768 kHz | — | — | TBD (e.g. Epson) |
| Keyswitches | Cherry MX (full size) | — | MX PCB / hot-swap | TBD count + layout |
| Key diodes | 1N4148W ×N | C81598 | SOD-123 | ✅ part (qty per matrix) |
| Interconnect | PZ254V-11-08P (1×8 2.54mm) | C492407 | header THT | ✅ (J3 → display; carries **+5V**) |
| Programming | SWD Tag-Connect TC2030-NL | — | pogo pad | ✅ (no part placed) |

## calcumaker-display (7-seg stack + drivers + interconnect)

| Block | Part | LCSC | Pkg / footprint | Status |
|-------|------|------|-----------------|--------|
| Driver ×3 | TM1640 | C5337152 | SOP-28 | ✅ ~$0.12 — 1 chip = 1 row of 16 CC digits |
| Digits ×12 | FJ5161AH (0.56" 4-digit, **common-cathode**) | C8093 | **THROUGH-HOLE** | ✅ ~$0.19 — 4 per row |
| Interconnect | PZ254V-11-08P (1×8 2.54mm) | C492407 | header THT | ✅ (J1 ← main) |

**Topology:** 3 rows × 16 digits. Each row = 1× TM1640 driving 4× FJ5161AH over
a 2-wire bus (shared **CLK** + per-row **DIN1/2/3**). The **top row (U3 / DS9–12)
is optional** → builds as a 2- or 3-row display.

## Important assembly note — through-hole digits

No SMD multi-digit 7-segment displays are stocked on LCSC; the well-stocked
parts are **through-hole**. So `calcumaker-display` needs **THT assembly**
(JLCPCB through-hole add-on, or hand/wave solder) in addition to SMT for the
TM1640s. The main board is all-SMT (plus the THT header + battery/USB connectors
as applicable). If an all-SMT display is a hard requirement, revisit driver+digit
selection (would likely mean discrete SMD single-digit displays — more parts).

## KiCad symbols

- **Digits (FJ5161AH):** use the **stock** KiCad symbol
  `Display_Character:CC56-12EWA` (generic 0.56" 4-digit **common-cathode**, the
  same 12-pin topology) with footprint `Display_7Segment:CC56-12GWA`. ✅ No custom
  symbol needed. *Verify the FJ5161AH pinout matches the CC56-12 land at layout.*
- **Driver (TM1640):** not in KiCad — **authored** from the datasheet pinout in
  `lib/symbols/calcumaker.kicad_sym` (28-pin SOP-28: GRID12–16=1–5, VSS=6, DIN=7,
  SCLK=8, SEG1–8=9–16, VDD=17, GRID1–11=18–28), registered via `register_lib` in
  `scripts/calcumaker-display.schgen.py`. ✅ Generates + passes the structure
  check. *Confirm the SOIC-28W footprint vs the TM1640 SOP-28 package drawing.*
