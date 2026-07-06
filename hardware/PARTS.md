# Calcumaker 16 — Parts

Source-of-truth mapping: each line → a real **LCSC** part (for the JLCPCB BOM).
LCSC/MPN/Manufacturer are set as KiCad symbol fields when parts are placed, so
`make bom-<board>` emits a JLCPCB BOM. `✅` = resolved by availability research;
`TBD` = pending an Open Question (see `../DESIGN.md`).

> Prices/stock are point-in-time (fetched during scaffolding, 2026-06); re-verify
> at order time. All ICs/displays below are JLCPCB **Extended** (no Basic option
> in these categories).

## calcumaker-mcu (MCU / PSU / clock / SWD / display-IF / keyboard mezzanine / QSPI flash)

*Bottom of the stack; the dense fine-pitch SMT brain board.*

| Block | Part | LCSC | Pkg / footprint | Status |
|-------|------|------|-----------------|--------|
| MCU | **STM32U575RGT6** | **C5270980** | LQFP-64 | ✅ ~$4.90 — **1MB** flash (fw links ~323KB); smaller pkg now the matrix scans off-board |
| **QSPI flash** | **W25Q32JVSSIQ** (4MB / 32Mbit quad-SPI NOR) | **C179173** | SOIC-8 (`SOIC-8_5.3x5.3mm`) | ✅ ~$0.30 (23k stock) — on OCTOSPI1; XIP constants + state/program storage; U7 + R9 CS-pullup + C26 |
| USB-C | GCT USB4105 (USB2.0 16P) | C2927039 | TopMnt horizontal | ✅ (from ephemerkey PSU) |
| ESD | USBLC6-2SC6 | C2687116 | SOT-23-6 | ✅ |
| Charger | MCP73831T-2ACI/OT | C424093 | SOT-23-5 | ✅ (PROG sized to cell) |
| Load-share FET | AO3401A (P-MOS) | C15127 | SOT-23 | ✅ |
| Load-share diode | B5819W (Schottky) | C8598 | SOD-123 | ✅ |
| Buck-boost 3V3 (MCU) | TPS63900DSKR | C1518762 | WSON-10 | ✅ MCU-only (light load); ULP low-Iq |
| 3V3 inductor | 2.2µH | TBD | **0805/2016** | smallest reasonable; verify Isat for TPS63900 |
| 5V boost (display) | **TPS61022RWUR** (EN-gated, adj.) | **C915088** | VQFN-7 2×2 | ✅ ~$0.32; FB divider R6 732k/R7 100k → 5V |
| 5V boost inductor | **FTC201610S1R0MBCA** 1µH | **C5832342** | 2.0×1.6mm | ✅ ~$0.04 (smallest reasonable for ~2A) |
| 5V boost caps | 10µF in + 2×22µF out (16V) | (PARTS) | **0603** | smallest at voltage |
| Level shifter | **SN74HCT125DR** (quad, VCC=5V, 3V3→5V) | **C352957** | SOIC-14 | ✅ ~$0.20; KiCad symbol = `74AHCT125` |
| Battery conn | JST S2B-PH-K-S | C173752 | PH 2.0 | ✅ |
| RTC xtal | Epson 32.768 kHz | C32346 | SMD 3215 2-pin | ✅ LSE (Clock sheet) + 2× 12pF load caps |
| Display interconnect (J3) | **AFC01-S12FCA-00** 0.5mm 12P FFC | **C262661** | `Hirose_FH12-12S-0.5SH` | ✅ FFC to display; +5V/GND doubled for LED current (0.5mm ≈ 0.4A/cond). **Cable = GCT FFC05-TIN `05-12-A-<len>-A-4-06-4-T`, DigiKey (non-BOM; length TBD).** |
| I2C pull-ups | 4.7k 0402 ×2 (R14/R15) | C25900 | 0402 | ✅ DNP — populate with the aux OLED |
| **Keyboard mezzanine receptacle (J5)** | **Hirose DF40B-12DS-0.4V** 2×6 0.4mm B2B | **C3641147** | `Connector_Hirose_DF40:…DF40B-12DS-0.4V_2x06` | ✅ ~1.5mm stack (3D-modelled); I²C+UART+KB_IRQ+**VSYS**+power to the keyboard G0 (matrix does NOT cross); mates DF40C-12DP. DF40C-12DS unstocked → DF40B (same 1.5mm stack; DF40 height = the suffix, not the B/C letter) |
| Programming | SWD Tag-Connect TC2030-NL | — | pogo pad | ✅ (no part placed) |

## calcumaker-keyboard (Cherry MX matrix + STM32G0 scanner + annunciators + per-key RGB + MCU mezzanine)

*Top of the stack; the front-panel board the user types on. Mezzanine-stacks
above the MCU board. Its own STM32G0 scans the matrix + drives the annunciators
and talks to the U575 over I²C/UART (keyscanning is off the MCU board).*

| Block | Part | LCSC | Pkg / footprint | Status |
|-------|------|------|-----------------|--------|
| **Scanner MCU (U1)** | **STM32G031K8U6** | **C432207** | UFQFPN-32 (`UFQFPN-32-1EP_5x5mm`) | ✅ ~$0.60; scans matrix + drives LEDs + I²C/UART to U575; QFN like tsumikoro's G0 |
| G0 decoupling | 100nF ×3 + 4.7µF (C1–C4) | C1525 / — | 0402 / 0603 | VDD/VDDA + bulk |
| G0 NRST / BOOT0 | 100nF (C5) + 10k (R6) | C1525 / C25744 | 0402 | reset cap + BOOT0 pulldown |
| Keyswitches ×50 | Cherry MX (full size) | — | SW_Cherry_MX_1.00u_PCB | 5×10 matrix → the **G0** (local scan); Kailh hot-swap optional; SW1–50 |
| Key diodes ×50 | 1N4148W | C81598 | SOD-123 | ✅ one per key (NKRO); D1–D50 |
| Annunciator LED f (gold) | Everlight 19-213/Y2C (yellow) | C72038 | **0603** | ✅ D51, beside the f key (driven by the G0) |
| Annunciator LED g (blue) | XL-1608UBC-04 | C965807 | **0603** | ✅ D52, beside the g key |
| Annunciator LEDs C / G / low-batt | KT-0603R (red) ×3 | C2286 | **0603** | ✅ D53–D55, top edge under the display bezel |
| Annunciator resistors ×5 | 470 Ω | C25117 | 0402 | ✅ R1–R5 (~1.3–2.8 mA @3V3; tune per color at bring-up) |
| G0 programming (J2) | SWD Tag-Connect TC2030-NL | — | pogo pad | ✅ (bare land) — or reflash via the UART/DFU bootloader over J1 |
| **Per-key RGB ×50 (D56–D105)** | **SK6805-EC15** 1.5×1.5mm single-wire addressable RGB | **C2890035** | `LED_SMD:LED_SK6812_EC15_1.5x1.5mm` | ✅ ~$0.06; smallest serial RGB w/ KiCad fp + stock; one beside each key (hint lighting); daisy-chained off the G0 |
| RGB level shifter (U2) | **SN74LVC1G125** single buffer (3V3→VLED data) | **C23654** | SOT-23-5 | ✅ powered from the gated LED rail; /OE→GND; lifts data to LED V_IH |
| RGB load switch (Q1/Q2) | **AO3401A** P-FET + **2N7002** N-FET + R7–R10/C6/C7 | **C15127** / **C8545** | SOT-23 | ✅ high-side gate on VSYS→VLED; G0 `LED_EN` cuts LEDs in sleep (near-zero leakage) |
| **MCU mezzanine header (J1)** | **Hirose DF40C-12DP-0.4V** 2×6 0.4mm B2B | **C6224952** | `Connector_Hirose_DF40:…DF40C-12DP-0.4V_2x06` | ✅ mates the MCU receptacle (J5, DF40B-12DS); ~1.5mm stack; carries +VSYS+GND for the RGB; pinout MUST match |

## calcumaker-display (7-seg stack + drivers + interconnect)

| Block | Part | LCSC | Pkg / footprint | Status |
|-------|------|------|-----------------|--------|
| Driver ×3 | TM1640 | C5337152 | SOP-28 | ✅ ~$0.12 — 1 chip = 1 row of 16 CC digits |
| Digits ×48 | FJ5161AH (0.56" **single-digit**, **common-cathode**) | C8093 | **THROUGH-HOLE** | ✅ ~$0.10 — **16 per row** (single digit, NOT a 4-up module) |
| Interconnect (J1) | **AFC01-S12FCA-00** 0.5mm 12P FFC | **C262661** | `Hirose_FH12-12S-0.5SH` | ✅ FFC ← MCU board (mcu J3); +5V/GND doubled for LED current. **Cable = GCT FFC05-TIN `05-12-A-<len>-A-4-06-4-T`, DigiKey (non-BOM; length TBD).** |
| Aux OLED socket | PZ254V-11-04P (1×4 2.54mm, J2) | C2691448 | header THT | ✅ DNP — receives a 0.91″ SSD1306 128×32 I2C module (sourced separately, not in the JLC library) |

**Topology:** 3 rows × 16 digits = **48 single digits**. Each row = 1× TM1640
driving **16× FJ5161AH** over a 2-wire bus (shared **CLK/DISP_CLK** + per-row
**DIN1/2/3**); segments a–g,dp = the shared **SEG1–8** bus, each digit's cathode
= one of **GRID1–16**. The **top row (U3 / DS33–48) is optional** → builds as a
2- or 3-row display. This repeats identically per row, so the schematic is a
**KiCad multi-channel** design: one reusable `display_row.kicad_sch` instantiated
3× (Row1→U1/DS1–16, Row2→U2/DS17–32, Row3→U3/DS33–48).

## Non-BOM accessories (ordered separately, not JLCPCB-assembled)

| Item | Part | Source | Notes |
|------|------|--------|-------|
| Display FFC cable | **GCT FFC05-TIN** `05-12-A-<len>-A-4-06-4-T` | **DigiKey** (cheap) | 0.5mm, 12-cond; **length TBD at layout**; `A`=same-side / `D`=opposite-side contacts per the two connectors' mounting |
| Aux OLED module | 0.91″ SSD1306 128×32 I2C | any | plugs into display J2 (DNP socket) |
| Stacking standoffs ×4 | M2 (matched to the DF40 ~1.5mm stack) | any | set the MCU↔keyboard gap; take mechanical load |
| Battery cell | 1S Li-ion + JST-PH lead | any | capacity TBD |

## Important assembly note — through-hole digits

No SMD multi-digit 7-segment displays are stocked on LCSC; the well-stocked
parts are **through-hole**. So `calcumaker-display` needs **THT assembly**
(JLCPCB through-hole add-on, or hand/wave solder) in addition to SMT for the
TM1640s. The MCU and keyboard boards are mostly SMT plus their connectors and
through-hole switches as applicable. If an all-SMT display is a hard
requirement, revisit driver+digit selection (would likely mean discrete SMD
single-digit displays — more parts).

## KiCad symbols

- **Digits (FJ5161AH):** **single-digit** 0.56" common-cathode (LCSC C8093 —
  confirmed a **1-digit** part, NOT a 4-up module). Symbol **authored** as
  `calcumaker:FJ5161AH` in `lib/symbols/calcumaker.kicad_sym` (standard 5161
  pinout: 1=E 2=D 3=COM 4=C 5=DP 6=B 7=A 8=COM 9=F 10=G) + the dimensionally-
  matched 0.56" single-digit land `Display_7Segment:7SegmentLED_LTS6760_LTS6780`.
  *Verify the FJ5161AH pad map vs the LTS6760 land at layout.*
  ⚠ **The earlier `Display_Character:CC56-12EWA` / `CC56-12GWA` mapping was WRONG**
  — that's a **4-digit** symbol/footprint/3D, which is where the phantom "clock
  colon" came from. There is no colon on the real single-digit part.
- **Driver (TM1640):** not in KiCad — **authored** from the datasheet pinout in
  `lib/symbols/calcumaker.kicad_sym` (28-pin SOP-28: GRID12–16=1–5, VSS=6, DIN=7,
  SCLK=8, SEG1–8=9–16, VDD=17, GRID1–11=18–28), registered via `register_lib` in
  `scripts/calcumaker-display.schgen.py`. ✅ Generates + passes the structure
  check. *Confirm the SOIC-28W footprint vs the TM1640 SOP-28 package drawing.*
- **Level shifter (SN74HCT125):** use the **stock** `74xx:74AHCT125` symbol
  (pin-identical quad buffer; value = `74HCT125`) + `Package_SO:SOIC-14_3.9x8.7mm`.
  ✅ No custom symbol.
- **5V boost (TPS61022):** stock `Converter_DCDC:TPS61022` symbol +
  `Package_DFN_QFN:Texas_RWU0007A_VQFN-7_2x2mm_P0.5mm` footprint. ✅ No custom
  symbol.
- **MCU (STM32U575RGT6):** stock `MCU_ST_STM32U5:STM32U575RGTx` symbol +
  `Package_QFP:LQFP-64_10x10mm_P0.5mm`. ✅ No custom symbol.
- **QSPI flash (W25Q32JVSSIQ):** stock `Memory_Flash:W25Q32JVSS` symbol +
  `Package_SO:SOIC-8_5.3x5.3mm_P1.27mm`. ✅ No custom symbol.

## Passive size policy

Per project policy: **0402** for resistors + decoupling/small caps; **0603** for
bulk MLCCs (10/22µF — true-0402 10µF is 6.3V-only, too low for the 5V rail).
Magnetics: **smallest reasonable** from the JLCPCB catalog (boost L = 2.0×1.6mm
1µH; the 3V3 buck-boost L downsized to 0805/2016 — verify Isat).
