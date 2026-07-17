#!/usr/bin/env python3
"""Regenerate the Calcumaker 16 **MCU board** hierarchical schematic.

    CALCUMAKER_SCHGEN_DRAFT_OK=1 python3 scripts/calcumaker-mcu.schgen.py
    (or: CALCUMAKER_SCHGEN_DRAFT_OK=1 make gen-calcumaker-mcu)

*** DRAFT ***
The MCU board is the **brain/PSU logic board** of a THREE-board split (see
DESIGN.md → Board Partition): it carries the **MCU (STM32U575RGT6)**, **PSU**
(USB-C/charge/buck-boost), clock, SWD, the **display-module interconnect**
(0.5mm FFC) to the angled display board, and a **fine-pitch mezzanine** up to
the **keyboard board** that stacks above it (the Cherry MX matrix + its own
STM32G0 scanner + annunciator LEDs live there — a dense LQFP-64 and 50
through-hole keys don't belong on one PCB). Keyscanning is off the main board:
only an **I2C + UART link + KB_IRQ wake + power** cross the mezzanine (not the
raw matrix). The 5V rail + level shifter now live on the display module, not
here. The PSU sheet is concrete; the connector pinouts are PLACEHOLDERS pending
front-panel layout. A guard refuses to generate first.

FOUR sheets: MCU, PSU, KeyboardIF, QSPIFlash. The former one-off Clock (Y1),
Programming (J4) and DisplayIF (J3/J7) sheets were folded into the MCU sheet on
2026-07-09 and their .kicad_sch files deleted; their parts and notes live on the
MCU sheet now. The OCTOSPI1 pin map is committed (see the MCU + QSPIFlash notes).

This is DATA; the engine is scripts/kschgen.py. Components are PLACED, not wired
— wire them in eeschema using the per-sheet notes as the spec (regenerate BEFORE
wiring; regen reassigns UUIDs). 0402 passives; bulk MLCCs 0603. Verify each
lib_id/footprint exists in your KiCad 10 install before relying on it.

*** WIRING IS IN PROGRESS in eeschema — do NOT regenerate this board. ***
kschgen keeps existing .kicad_sch files unless KSCHGEN_FORCE=1; forcing a regen
would discard the manual wiring and reassign every UUID.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kschgen as K

# --- DRAFT guard -------------------------------------------------------------
if not os.environ.get("CALCUMAKER_SCHGEN_DRAFT_OK"):
    sys.exit(
        "calcumaker-mcu.schgen.py is a DRAFT: the MCU config + connector pinouts "
        "are placeholders (see DESIGN.md Open Questions). Set "
        "CALCUMAKER_SCHGEN_DRAFT_OK=1 to generate anyway."
    )

HW = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))   # hardware/
PROJ_DIR = os.path.join(HW, "calcumaker-mcu")
ROOT_UUID = "ca1c0000-0000-4000-8000-00000000ma01"   # keep stable across regens

# ---- symbol libraries -------------------------------------------------------
K.register_stdlib("Device", "R", "C", "L", "LED", "Crystal", "D")
K.register_stdlib("Regulator_Switching", "TPS63900")
K.register_stdlib("Battery_Management", "BQ25601")   # 3A sync buck + NVDC power path
K.register_stdlib("Power_Protection", "USBLC6-2SC6")
K.register_stdlib("Transistor_FET", "Q_NMOS_GSD")    # VBUS OVP pass FET (TCPP01 gate driver)
# TCPP01-M12 is NOT in the KiCad stdlib (only its DRP sibling TCPP03-M20 is), so the
# sink part is authored in the project lib alongside TM1640 / FJ5161AH.
K.register_lib("calcumaker",
               os.path.join(HW, "lib", "symbols", "calcumaker.kicad_sym"),
               "TCPP01-M12", "MAX17048")
K.register_stdlib("Connector", "USB_C_Receptacle_USB2.0_16P",
                  "Conn_ARM_SWD_TagConnect_TC2030-NL")
K.register_stdlib("Connector_Generic", "Conn_01x02", "Conn_01x12", "Conn_01x16",
                  "Conn_02x06_Odd_Even")   # 01x12 = display FFC (J3); 02x06 = keyboard DF40 stack (J5);
#                                            01x16 = keyboard FFC-cable alternative (J6)
K.register_stdlib("74xx", "74AHCT125")  # level shifter symbol (use value "74HCT125"); 3V3->5V
K.register_stdlib("MCU_ST_STM32U5", "STM32U575RGTx")   # LQFP-64 (stock) — smaller pkg now the matrix is off-board
K.register_stdlib("Converter_DCDC", "TPS61022")        # 5V boost (stock)
K.register_stdlib("Memory_Flash", "W25Q32JVSS")        # 4MB (32Mbit) quad-SPI NOR on OCTOSPI1

# ---- footprint shorthands ---------------------------------------------------
# Size policy (user): 0402 for resistors + small/decoupling caps; bulk MLCCs
# (>=10uF) at 0603 (smallest with voltage margin on the 5V rail; true-0402 10uF
# is 6.3V-only). Magnetics: smallest reasonable from the JLCPCB catalog.
R0402 = "Resistor_SMD:R_0402_1005Metric"
C0402 = "Capacitor_SMD:C_0402_1005Metric"
C0603 = "Capacitor_SMD:C_0603_1608Metric"   # bulk MLCCs (10/22uF @ >=16V) on VSYS/VBAT (<=4.4V)
C0805 = "Capacitor_SMD:C_0805_2012Metric"   # the 12V-PD VBUS rail: 50V parts, see below
L2016 = "Inductor_SMD:L_0805_2012Metric"     # ~2x1.6mm power inductor (verify land vs part)
L4020 = "Inductor_SMD:L_Changjiang_FNR4020S"  # charger power L: 2.2uH >=4A Isat, 4x4x2.0 shielded
#                                              (TI typ. 1-2.2uH @ 1.5MHz -- verify vs datasheet at layout)
BQ_FP = "Package_DFN_QFN:Texas_RTW_WQFN-24-1EP_4x4mm_P0.5mm_EP2.7x2.7mm_ThermalVias"
# TCPP01-M12 = QFN12, D=E=3.00mm, e=0.50mm, EP D2=E2=1.45mm (datasheet Table 15)
TCPP_FP = "Package_DFN_QFN:QFN-12-1EP_3x3mm_P0.5mm_EP1.45x1.45mm_ThermalVias"
# MAX17048 = TDFN-8 2x2, 0.5mm pitch, EP 0.8x1.2mm (pad 9).  VERIFIED: KiCad's own
# footprint cites Maxim package outline 21-0168 -- the authoritative drawing for this
# package -- in its descr field, and the sibling notchdeck repo independently landed on
# the same part.  (An earlier DFN-8...EP0.9x1.5mm guess was WRONG: the oversized EP
# would have risked solder-bridging to the signal pins.)
MAX_FP = "Package_DFN_QFN:TDFN-8-1EP_2x2mm_P0.5mm_EP0.8x1.2mm"
LED0402 = "LED_SMD:LED_0402_1005Metric"
LED0603 = "LED_SMD:LED_0603_1608Metric"      # annunciators (visible indicators)
SOT235 = "Package_TO_SOT_SMD:SOT-23-5"
SOT236 = "Package_TO_SOT_SMD:SOT-23-6"
SOT23 = "Package_TO_SOT_SMD:SOT-23"
SOD123 = "Diode_SMD:D_SOD-123"
LQFP64 = "Package_QFP:LQFP-64_10x10mm_P0.5mm"
QSPI_FP = "Package_SO:SOIC-8_5.3x5.3mm_P1.27mm"       # W25Q32JVSS (SOIC-8 208mil)
XTAL_FP = "Crystal:Crystal_SMD_3215-2Pin_3.2x1.5mm"
SWD_FP = "Connector:Tag-Connect_TC2030-IDC-NL_2x03_P1.27mm_Vertical"

RLCSC = {"5.1k": "C25905", "4.7k": "C25900", "10k": "C25744",
         "100k": "C25741", "1k": "C11702", "470": "C25117", "0R": "C17168"}
# Per-rail voltage differs, so 10uF/22uF LCSC# live in hardware/PARTS.md, not here.
CLCSC = {"100nF": "C1525", "1uF": "C29266", "12pF": "C1547"}


def R(ref, val):
    return dict(ref=ref, lib_id="Device:R", value=val, fp=R0402,
                lcsc=RLCSC.get(val, ""))


def C(ref, val, fp=C0402):
    return dict(ref=ref, lib_id="Device:C", value=val, fp=fp,
                lcsc=CLCSC.get(val, ""))


# ============================ MCU core sheet =================================
# STM32U575RGTx (LQFP-64) + power decoupling + reset/boot. NOTE: the U5 core can
# run from the internal LDO or the internal SMPS; SMPS mode needs an external
# inductor on VLXSMPS + VDD12 caps (datasheet) — placed/configured at layout.
# VDDA/VREF+ and VDDUSB decoupled.
#
# 2026-07-09: the former Clock (Y1/C24/C25), Programming (J4) and DisplayIF
# (J3/J7) one-off subsheets were FOLDED IN HERE and their .kicad_sch files
# deleted — they were single-part sheets whose only job was to sit next to the
# MCU. Their parts, refs and notes now live on this sheet; the pinouts they
# documented are captured in the PIN MAP below.
MCU = dict(name="MCU", file="mcu.kicad_sch",
    title="MCU core (STM32U575) + clock + SWD + display-module interface",
    page="2",
    big=[
        dict(ref="U1", lib_id="MCU_ST_STM32U5:STM32U575RGTx", value="STM32U575RGT6",
             fp=LQFP64, lcsc="C5270980", mpn="STM32U575RGT6", mfr="STMicroelectronics"),
        # --- was the Programming sheet: PSU uses J1/J2, display FFC is J3, SWD = J4.
        dict(ref="J4", lib_id="Connector:Conn_ARM_SWD_TagConnect_TC2030-NL",
             value="SWD TC2030-NL", fp=SWD_FP),
        # --- was the DisplayIF sheet: unified 12-pos 0.5mm FFC to the display
        # module. CABLE = GCT FFC05-TIN 05-12-A-<length>-A-4-06-4-T (DigiKey
        # accessory, NOT assembled; len TBD).
        dict(ref="J3", lib_id="Connector_Generic:Conn_01x12", value="TO DISPLAY (unified SPI FFC)",
             fp="Connector_FFC-FPC:Hirose_FH12-12S-0.5SH_1x12-1MP_P0.50mm_Horizontal",
             lcsc="C262661", mpn="AFC01-S12FCA-00", mfr="JUSHUO"),
        # VSYS outlet -> the RGB-matrix module's LED inlet (its own 2-pin JST).
        dict(ref="J7", lib_id="Connector_Generic:Conn_01x02", value="VSYS -> matrix LED pwr",
             fp="Connector_JST:JST_PH_S2B-PH-K_1x02_P2.00mm_Horizontal",
             lcsc="C173752", mpn="S2B-PH-K-S", mfr="JST"),
    ],
    small=[
        # Refs are globally unique across the board: PSU uses C1-C7/R1-R5, so the
        # MCU sheet starts at C12/R8 (C8-C11 / R6-R7 are retired — they belonged
        # to the 5V boost + 74HCT125 that moved onto the display module).
        C("C12", "100nF"), C("C13", "100nF"), C("C14", "100nF"), C("C15", "100nF"),
        C("C16", "100nF"),                                 # VDD x5 decoupling
        C("C17", "10uF", C0603),                           # VDD bulk
        C("C18", "1uF"), C("C19", "100nF"),                # VDDA/VREF+ filter
        C("C20", "100nF"),                                 # VDDUSB
        C("C21", "100nF"),                                 # NRST cap
        R("R8", "10k"),                                    # BOOT0 pulldown
        C("C22", "2.2uF", C0603), C("C23", "2.2uF", C0603),  # VCORE/VCAP (LDO/SMPS) — verify per mode
        # --- was the Clock sheet: LSE 32.768 kHz -> RTC (sleep timing).
        dict(ref="Y1", lib_id="Device:Crystal", value="32.768kHz", fp=XTAL_FP,
             lcsc="C32346", mpn="Q13FC13500004", mfr="Epson"),
        C("C24", "12pF"), C("C25", "12pF"),                # LSE load caps
    ],
    note=(15, 150, K.note_block(
        "MCU CORE  -  U1  STM32U575RGT6   (LQFP-64, Cortex-M33)",
        "Smaller pkg now the key matrix scans on the keyboard G0, not here.",
        "",
        "POWER",
        "  VDD (each pin) -> +3V3   C12-C16 100nF + C17 10uF bulk",
        "                           (LQFP-64 has fewer VDD than the 5 caps;",
        "                            spares are extra bypass, trim @ layout)",
        "  VDDA / VREF+   -> +3V3   C18 1uF + C19 100nF",
        "  VDDUSB         -> +3V3   C20 100nF",
        "  VSS / EP       -> GND",
        "  VCORE          -> LDO or internal SMPS (SMPS: L on VLXSMPS + VDD12);",
        "                    C22/C23 = VCAP placeholders (set per mode)",
        "RESET / BOOT",
        "  NRST -> C21 100nF          BOOT0 -> R8 10k to GND",
        "",
        "PIN MAP  (verified vs the ST CubeMX pin DB for LQFP-64)",
        "  OCTOSPI1 -> U7      OCTOSPIM Port 1; IO0-3 forced, 1 pin each",
        "    CLK  PB10 AF10 p29      IO0  PB1  AF10 p27",
        "    NCS  PA4  AF3  p20      IO1  PB0  AF10 p26",
        "                            IO2  PA7  AF10 p23",
        "                            IO3  PA6  AF10 p22",
        "  SPI1 -> display J3  (PA6/PA7 are QSPI now; the FFC has no MISO)",
        "    SCK  PA5  AF5  p21      MOSI PB5  AF5  p57",
        "    CS   PA8       p41      (plain GPIO -- MOVED off PA15, now UCPD1_CC1)",
        "  UCPD1  -> USB-C CC   CC1 PA15 p50 / CC2 PB15 p63   (dedicated analog,",
        "                       NO AF alt -- this is why SPI1 CS had to move.",
        "                       Rd is INTERNAL: no 5.1k parts.  DBCC1/DBCC2 unused,",
        "                       so PB5 stays SPI1 MOSI.)",
        "  USART2 -> keyboard   TX PA2 p16 / RX PA3 p17   AF7",
        "  I2C1   -> keyboard + CHARGER   SCL PB6 p58 / SDA PB7 p59 AF4",
        "                       (shared bus; BQ25601 = 0x6B, no addr clash)",
        "",
        "ON THIS SHEET  (Clock / Programming / DisplayIF merged in here)",
        "  SWD   PA13/PA14 (+ PB3 SWO)      -> J4 Tag-Connect (+ NRST)",
        "  LSE   PC14/PC15 OSC32            -> Y1 + C24/C25",
        "  DISP  SPI1 + DISP_IRQ/NRST/BOOT  -> J3 FFC ; J7 VSYS outlet",
        "",
        "OFF-SHEET",
        "  USB   PA11/PA12                     -> PSU ESD (U3)",
        "  CC    PA15/PB15 (UCPD1)             -> PSU ESD (U5) -> USB-C CC1/CC2",
        "  CHG   I2C1 + CHG_INT/CHG_PG/CHG_CE  -> PSU charger U4",
        "  KBD   I2C1+USART2+KB_IRQ/NRST/BOOT0 -> KeyboardIF J5/J6",
        "  QSPI  OCTOSPI1 CLK/NCS/IO0-3        -> QSPIFlash U7",
        "",
        "KB_IRQ must land on a WKUP pin (keypress wake from Stop) -- still open.",
        "",
        "---- J3  UNIFIED DISPLAY-MODULE FFC  (was the DisplayIF sheet) --------",
        "0.5mm 12-pos FFC (AFC01-S12FCA-00, C262661). SAME pinout on BOTH display",
        "boards (7-seg + RGB matrix) -> interchangeable. Technology-agnostic:",
        "power + SPI 'display intent' + reset/boot. The module MCU (STM32G031 on",
        "7-seg / RP2040 on the matrix) is the SPI slave + renders locally; 5V and",
        "any level-shifting are generated ON the module now.",
        "",
        K.pin_table([(1, "VSYS"), (2, "VSYS"), (3, "GND"), (4, "GND"), (5, "+3V3"),
                     (6, "SPI_SCLK"), (7, "SPI_MOSI"), (8, "SPI_CS"), (9, "DISP_IRQ"),
                     (10, "DISP_NRST"), (11, "DISP_BOOT"), (12, "GND")]),
        "",
        "J7 = 2-pin JST-PH VSYS outlet -> the RGB-matrix module's LED inlet (J2):",
        "the matrix pulls amps for 2304 LEDs, so its LED current takes this direct",
        "lead, NOT the signal FFC (the 7-seg module boosts from VSYS on the FFC).",
        "CABLE (non-BOM): GCT FFC05-TIN 05-12-A-<len>-A-4-06-4-T (DigiKey; len TBD).",
        "",
        "---- J4  SWD PROGRAMMING  (was the Programming sheet) ----------------",
        "Tag-Connect TC2030-NL (no-legs pogo pad). Bare land, no part mounted.",
        "",
        K.pin_table([(1, "+3V3 (VTref)"), (2, "SWDIO (PA13)"), (3, "NRST"),
                     (4, "SWCLK (PA14)"), (5, "GND"), (6, "SWO (PB3, opt)")], cols=1),
        "",
        "---- Y1  LSE 32.768 kHz  (was the Clock sheet) -----------------------",
        "Q13FC13500004, LCSC C32346.  Y1.1 -> OSC32_IN (PC14)",
        "                             Y1.2 -> OSC32_OUT (PC15)",
        "  C24 / C25 -> LSE load caps to GND   (12pF shown)",
        "Load caps: CL match = 2*(CL - Cstray); trim with the RTC SMOOTHCALIB.",
        "Drives the RTC for sleep timing.")))

# NOTE: the former Clock (Y1 + C24/C25) and Programming (J4) one-off sheets were
# folded into the MCU sheet above on 2026-07-09 and clock.kicad_sch /
# prog.kicad_sch deleted. Their parts and notes live on MCU now.

# ============================ PSU sheet (concrete) ===========================
# Revised 2026-07-12: the MCP73831 linear charger + the discrete load-share
# (P-FET + Schottky) were replaced by a BQ25601 -- a 3A synchronous buck charger
# whose NVDC power path IS the load-share (regulated SYS output). See DESIGN.md
# "Power" for the full rationale. What changed here:
#   - U4 MCP73831 -> BQ25601RTWR (C468236, WQFN-24 4x4, stock KiCad symbol)
#   - DELETED: Q1 (AO3401A), D1 (B5819W), R4 (gate pulldown), R3 (PROG resistor
#     -- charge current is now the ICHG I2C register, sized to the cell)
#   - ADDED: L2 charger power inductor + its support passives (BTST/REGN/PMID),
#     the charger's I2C + /INT + /PG + /CE lines to the MCU, the TS bias, and
#     the CC1/CC2 ADC taps (R6/R7 + C13/C14) that read the Type-C current
#     advertisement so firmware can raise IINDPM to 1.5A / 3.0A.
# NOTE: TPS63900 is ultra-low-Iq but modest max current (~hundreds of mA) --
# it feeds the MCU rail ONLY, so this is fine. LCSC stock re-verified 2026-07-12.
PSU = dict(name="PSU", file="psu.kicad_sch",
    title="USB-C / CC sense / BQ25601 charger (NVDC power path) / buck-boost 3V3", page="3",
    big=[
        dict(ref="J1", lib_id="Connector:USB_C_Receptacle_USB2.0_16P", value="USB-C",
             fp="Connector_USB:USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal",
             lcsc="C2927039", mpn="USB-TYPE-C-019", mfr="GCT"),
        dict(ref="J2", lib_id="Connector_Generic:Conn_01x02", value="BAT 1S Li-ion",
             fp="Connector_JST:JST_PH_S2B-PH-K_1x02_P2.00mm_Horizontal",
             lcsc="C173752", mpn="S2B-PH-K-S", mfr="JST"),
        dict(ref="U4", lib_id="Battery_Management:BQ25601", value="BQ25601RTWR",
             fp=BQ_FP, lcsc="C468236", mpn="BQ25601RTWR", mfr="Texas Instruments"),
        dict(ref="U2", lib_id="Regulator_Switching:TPS63900", value="TPS63900DSKR",
             fp="Package_SON:WSON-10-1EP_2.5x2.5mm_P0.5mm_EP1.2x2mm",
             lcsc="C1518762", mpn="TPS63900DSKR", mfr="Texas Instruments"),
        # TCPP01-M12: ST's Type-C port protection -- the companion chip to STM32
        # UCPD, and the missing half of the reference sink front-end.  It does the
        # two things a bare USBLC6 cannot:
        #   1. DEAD-BATTERY HANDOFF.  It carries its OWN Rd and clamps CC while the
        #      MCU is unpowered, so a flat board still gets VBUS.  Once UCPD boots
        #      and enables its own Rd, fw drives DB/ high -> TCPP01 drops its clamp
        #      and closes the CC switches, handing CC over to UCPD.  This is what
        #      closes the old "dead-battery Rd" risk (was Open Question 8).
        #   2. CC SHORT-TO-VBUS OVP (6.0V clamp).  A defective cable/source putting
        #      20V on CC would otherwise destroy PA15/PB15 -- a USBLC6 is an ESD
        #      clamp (ns transients), NOT sustained-overvoltage protection.
        # Plus IEC61000-4-2 level 4 ESD on CC, and adjustable VBUS OVP via an
        # external N-FET it gate-drives.  Sourced by the user: 300 pcs on JLCPCB.
        # NOT in the KiCad stdlib -> symbol authored in lib/symbols/calcumaker.
        dict(ref="U5", lib_id="calcumaker:TCPP01-M12", value="TCPP01-M12",
             fp=TCPP_FP, lcsc="C1121848", mpn="TCPP01-M12", mfr="STMicroelectronics"),
        # MAX17048 -- 1-cell ModelGauge fuel gauge.  The BQ25601 has NO ADC (it gives
        # charge status/faults only), so without this there is no real state-of-charge
        # -- only a crude voltage guess off the battery ADC, and Li-ion's flat 20-80%
        # plateau makes that poor.  ModelGauge needs NO current-sense resistor.
        #   VDD + CELL both -> BAT+ (the RAW CELL, not VSYS) so it keeps gauging while
        #     the system is off.  ~3uA hibernate -- fine to leave on the battery.
        #   SDA/SCL -> the SHARED I2C1 bus.  addr 0x36 -- no clash with BQ25601 (0x6B).
        #   ALRT -> open-drain low-SoC interrupt -> MCU GPIO (+R12 pull-up).  Route it
        #     to a WKUP pin: this is what drives the LOWBAT annunciator, off a real SoC
        #     threshold rather than a voltage guess.
        #   CTG + QSTRT -> GND  (CTG must be grounded in normal operation).
        # Footprint VERIFIED: TDFN-8 2x2, EP 0.8x1.2mm (pad 9) -- KiCad's footprint
        # descr cites Maxim outline 21-0168, the drawing for this exact package.
        dict(ref="U6", lib_id="calcumaker:MAX17048", value="MAX17048G+T10",
             fp=MAX_FP, lcsc="C2682616", mpn="MAX17048G+T10", mfr="Analog Devices"),
        R("R12", "100k"),                                # ALRT pull-up (open-drain)
        C("C32", "100nF"),                               # MAX17048 VDD bypass
        # VBUS OVP pass FET, gate-driven by TCPP01 (VGS 5-6V from its charge pump).
        # In series on VBUS: connector -> IN_GD (drain) -> FET -> SOURCE -> system.
        # On an OVP/OTP/UVLO fault TCPP01 pulls the gate down and disconnects VBUS.
        # AO3400A: 30V / 5.7A / 28mOhm @ VGS=4.5V, and a JLCPCB BASIC part.
        dict(ref="Q1", lib_id="Transistor_FET:Q_NMOS_GSD", value="AO3400A",
             fp=SOT23, lcsc="C20917", mpn="AO3400A", mfr="AOS"),
    ],
    small=[
        # D+/D- ESD.  TCPP01 protects VBUS + CC only -- the datalines still need this.
        dict(ref="U3", lib_id="Power_Protection:USBLC6-2SC6", value="USBLC6-2SC6",
             fp=SOT236, lcsc="C2687116", mpn="USBLC6-2SC6", mfr="STMicroelectronics"),
        # -- USB-C CC -> TCPP01 (connector side CC1c/CC2c, 22V tolerant) -> MCU side
        # CC1/CC2 -> the U575's UCPD1 (CC1=PA15, CC2=PB15; dedicated analog pins, NO
        # AF alternative -- which is why the display SPI1 CS moved off PA15 to PA8).
        # There are NO 5.1k Rd resistors: in the run state UCPD generates Rd
        # INTERNALLY (CcPull::Sink).  It is either/or -- an external 5.1k in parallel
        # with the internal Rd = 2.55k, and the source would misread us as an audio
        # accessory.  UCPD decodes the source's Rp in HARDWARE
        # (UCPD_SR.TYPEC_VSTATE_CC: LOWEST=detached LOW=500mA HIGH=1.5A HIGHEST=3.0A);
        # fw reads it and writes IINDPM over I2C.  UCPD also carries the BMC PHY, so
        # real USB-PD later is FIRMWARE-ONLY -- no respin.
        #   DB/ is a plain 3V3 GPIO (NOT the UCPD DBCC1/DBCC2 pins) -- so PB5 stays
        #   SPI1 MOSI and DBCC1/DBCC2 remain unused.  Datasheet 6.4.
        R("R11", "100k"),                                # FLT/ pull-up (open-drain)
        # VBUS OVP threshold: VBUS_CTRL trips at Vovp = 1.20 (min) / 1.25 (typ) /
        # 1.34 (max) V.  Sized for a 12V PD contract:
        #     trip = Vovp * (R6+R7)/R7      R6=10k, R7=976R  =>  13.5 .. 15.1 V
        # It must sit ABOVE the highest LEGAL VBUS and BELOW what stresses the parts:
        #   - a 12V PDO is spec'd +/-5% => up to 12.6V.  Min trip 13.5V clears it.
        #   - BQ25601 abs max is 22V.     Max trip 15.1V is far below it.
        # !! Do NOT use ST's Table 13 "13V" row (R7=1.1k) -- its MIN trip is ~12.1V,
        # !! which is BELOW a legal 12V source, so it would nuisance-trip on a good
        # !! charger.  The table rows are nominal-only; they ignore the Vovp spread.
        R("R6", "10k"),                                  # VBUS_CTRL top    (ST's R1)
        R("R7", "976R"),                                 # VBUS_CTRL bottom (ST's R2) -> ~14V OVP
        # VBUS voltage sense -> MCU ADC.  ST datasheet 6.5.5: R3=200k / R4=40.2k.
        # Not needed for 5V-only operation, but a PD sink policy engine REQUIRES
        # VBUS sensing (vSafe0V / vSafe5V), so fitting it now keeps PD firmware-only.
        R("R4", "200k"),                                 # VBUS sense top   (ST's R3)
        R("R10", "40.2k"),                               # VBUS sense bottom (ST's R4)
        # CC line capacitance: USB-PD requires the CC receiver to total 200-600pF.
        # TCPP01 contributes 40-100pF and the MCU 60-90pF -> 150pF each lands inside
        # the window at both extremes (min 250pF, max 340pF).  Datasheet Table 12.
        C("C28", "150pF", C0402), C("C29", "150pF", C0402),   # CC1c / CC2c
        # ESD capacitor -- the system-level ESD rating DEPENDS on this part.  It is
        # NOT a generic 100nF: must be >=50V X7R (X7R loses capacitance as voltage
        # rises, so the derating matters) and placed hard against U5 (datasheet
        # 6.5.1 + section 7).  Explicit part, NOT the C0402 100nF used elsewhere.
        dict(ref="C30", lib_id="Device:C", value="100nF 50V X7R", fp=C0603,
             lcsc="C14663", mpn="CC0603KRX7R9BB104", mfr="YAGEO"),  # ST's recommended series
        C("C31", "100nF", C0402),                        # TCPP01 VCC decoupling
        # -- BQ25601 support.  Charge current (ICHG) + input limit (IINDPM) are
        # I2C registers, NOT resistors -- there is no PROG/ISET part any more.
        # PSEL is tied HIGH so the power-on default is the safe 500mA USB limit:
        # on a dead cell the NVDC path instant-on's the MCU, firmware boots, reads
        # CC, and only THEN raises the limit.  If firmware never runs we still
        # charge -- just slowly.  Fail-safe by construction.
        # NOTE on refs: deleting the old Rd (R1/R2), PROG (R3) and load-share gate
        # (R4) freed those numbers, so the new charger passives reuse them.  R5
        # (STAT LED) is unchanged; R8 belongs to the MCU sheet and R9 to QSPIFlash
        # -- do NOT reuse those here.
        R("R1", "100k"),                                 # PSEL pullup -> VBUS (500mA default)
        dict(ref="L2", lib_id="Device:L", value="2.2uH", fp=L4020),  # charger power L, >=4A Isat
        # *** THE VBUS RAIL IS NOW UP TO 12V (PD), AND UP TO ~15V ON AN OVP EXCURSION.
        # *** Every cap that sits on it must be a 50V part -- NOT the 16V bulk MLCCs
        # *** used on VSYS/VBAT.  Two reasons: (1) 16V is no margin at all against a
        # *** 15.1V OVP trip, and (2) DC-BIAS DERATING -- an X5R at 12V on a 16V part
        # *** loses most of its capacitance, so a "10uF" would not be 10uF where it
        # *** matters most.  50V/0805 keeps the real capacitance up.
        # VBUS-side (12V): C1, C2, C6, C30.   VSYS/VBAT-side (<=4.4V): C8-C11, C3-C5, C7.
        dict(ref="C1", lib_id="Device:C", value="1uF 50V", fp=C0603,
             lcsc="C15849", mpn="CL10A105KB8NNNC", mfr="Samsung"),      # charger VBUS
        dict(ref="C2", lib_id="Device:C", value="10uF 50V", fp=C0805,
             lcsc="C440198", mpn="GRM21BR61H106KE43L", mfr="Murata"),   # PMID (sits at ~VBUS)
        C("C8", "10uF", C0603), C("C9", "10uF", C0603),  # SYS (>=20uF)   [VSYS, <=4.4V]
        C("C10", "10uF", C0603),                         # BAT            [VBAT, <=4.4V]
        C("C11", "4.7uF", C0603),                        # REGN LDO       [~4.8V internal]
        C("C27", "47nF", C0402),                         # BTST (-> SW)   [C12 = MCU sheet]
        # -- TS: the BQ25601 REFUSES TO CHARGE if TS is out of range.  Either a
        # 103AT NTC in the pack (preferred -- real pack thermal protection) or
        # this fixed REGN divider faking 25C.  Decide with the cell (DESIGN.md Q6).
        R("R2", "5.23k"), R("R3", "30.1k"),              # TS bias (REGN / GND)
        # VBUS bulk -- on VBUS_PROT (downstream of Q1), so it too is a 12V/50V part.
        dict(ref="C6", lib_id="Device:C", value="10uF 50V", fp=C0805,
             lcsc="C440198", mpn="GRM21BR61H106KE43L", mfr="Murata"),
        # -- buck-boost 3V3 (MCU rail only; VSYS input <=4.4V, 16V parts are fine)
        dict(ref="L1", lib_id="Device:L", value="2.2uH", fp=L2016),  # verify Isat for TPS63900 (PARTS.md)
        C("C3", "10uF", C0603), C("C4", "10uF", C0603), C("C5", "10uF", C0603),
        C("C7", "10uF", C0603),
        dict(ref="D2", lib_id="Device:LED", value="CHG", fp=LED0402, lcsc="C130719"),
        R("R5", "1k"),                                   # STAT LED series
    ],
    note=(15, 165, K.note_block(
        "POWER  -  USB-C -> BQ25601 (3A buck + NVDC power path) -> buck-boost 3V3",
        "PLACED, not wired.  See DESIGN.md Power / Power Tree.",
        "",
        "USB-C  J1   D+/D- -> U3 ESD -> MCU USB.  (TCPP01 covers VBUS+CC, not data.)",
        "",
        "TCPP01 U5   ST Type-C port protection -- the companion chip to STM32 UCPD.",
        "  VBUS PATH (U5 gate-drives Q1; VBUS passes THROUGH the FET):",
        "    J1 VBUS -> C30 (ESD cap, 50V X7R, keep CLOSE to U5) -> IN_GD(8)",
        "    IN_GD = Q1 drain; GATE(5) -> Q1 gate; SOURCE(4) = Q1 source = VBUS_PROT",
        "    VBUS_PROT -> U4 charger VBUS.  On OVP/OTP/UVLO U5 pulls the gate down",
        "    and DISCONNECTS the system from a defective charger.",
        "  VBUS OVP THRESHOLD:  J1 VBUS -> R6 10k -> VBUS_CTRL(6) -> R7 976R -> GND",
        "    trip = Vovp*(R6+R7)/R7, Vovp=1.20/1.25/1.34 => 13.5 .. 15.1 V.",
        "    Sized for a 12V PD contract: a 12V PDO is +/-5% (=> 12.6V max), so the",
        "    MIN trip (13.5V) clears it; the MAX trip (15.1V) is far under the",
        "    BQ25601's 22V abs max.",
        "    !! do NOT use ST Table 13's '13V' row (1.1k): its MIN trip is ~12.1V,",
        "    !! BELOW a legal 12V source -> nuisance trips on a good charger.",
        "  VBUS SENSE:  J1 VBUS -> R4 200k -> VBUS_SENSE (ADC) -> R10 40.2k -> GND",
        "    (not needed at 5V; a PD policy engine REQUIRES it -> keeps PD fw-only)",
        "  CC:  J1 CC1 -> CC1c(7) ... CC1(3) -> PA15 (UCPD1_CC1)   [+C28 150pF]",
        "       J1 CC2 -> CC2c(9) ... CC2(1) -> PB15 (UCPD1_CC2)   [+C29 150pF]",
        "    connector side is 22V tolerant; U5 clamps CC at 6.0V -> a CC short to",
        "    VBUS can no longer destroy the MCU's CC pins.",
        "  CONTROL:  VCC(12) <- TCPP01_EN GPIO (+C31 100nF)  -- powering VCC from a",
        "    GPIO gives NULL quiescent current when unplugged (datasheet 5).",
        "            DB/(10)  <- TCPP01_DB  GPIO   (plain 3V3 GPIO, NOT UCPD DBCC!",
        "                                          so PB5 stays SPI1 MOSI)",
        "            FLT/(11) -> TCPP01_FLT GPIO   open-drain, + R11 100k pull-up",
        "            GND(2) + EP(13) -> GND   <-- EP IS A REAL GROUND RETURN, not just",
        "                                         thermal.  It is a VISIBLE pin: WIRE IT.",
        "",
        "  *** NO 5.1k Rd PARTS ANYWHERE *** -- Rd comes from U5 when the MCU is",
        "  unpowered (dead battery) and from UCPD (CcPull::Sink) once it boots.",
        "  An external 5.1k would parallel the internal one to 2.55k -> the source",
        "  misreads us as an audio accessory.  Do NOT add one.",
        "",
        "  DEAD-BATTERY / WAKE SEQUENCE  (datasheet Fig.17) -- fw MUST follow it:",
        "    1. flat board: U5 clamps CC (1.1V) with its OWN Rd; DB/ low = clamp on",
        "    2. source sees the clamp -> applies 5V on VBUS",
        "    3. Q1 turns on -> VBUS_PROT feeds U4 -> NVDC instant-on -> MCU boots",
        "    4. fw powers U5 (TCPP01_EN) and enables Rd in UCPD",
        "    5. ONLY THEN drive DB/ HIGH -> U5 drops its clamp, closes CC switches,",
        "       and UCPD owns the CC lines.  ORDER MATTERS (datasheet 6.4).",
        "    6. fw reads VSTATE (500mA/1.5A/3.0A) -> sets IINDPM on U4 over I2C.",
        "CHARGER U4  BQ25601 (I2C 0x6B):",
        "            VBUS<-VBUS_PROT (Q1 SOURCE, NOT the raw connector!)  C1 1uF",
        "            PMID->C2 10uF  REGN->C11 4.7uF",
        "            SW(19,20) -> L2 2.2uH -> SYS(15,16) = VSYS, C8/C9 10uF",
        "            BTST -> C27 47nF -> SW     BAT(13,14) -> BAT+, C10 10uF",
        "            PSEL -> R1 100k -> VBUS  (HIGH = 500mA safe default)",
        "            TS   -> R2 5.23k to REGN + R3 30.1k to GND  (or pack NTC)",
        "            SCL/SDA -> I2C1 (SHARED with keyboard G0 -- no addr clash)",
        "            /INT -> CHG_INT (EXTI, ideally WKUP)   /PG -> CHG_PG",
        "            /CE  -> CHG_CE      STAT -> D2 + R5",
        "            /QON: ship-mode exit -- leave for a pad/button (see DESIGN.md)",
        "            GND(17,18) + EP(25) -> GND, thermal vias.",
        "  NOTE: SYS is NVDC -- it tracks ~VBAT with a 3.5V floor (NOT 4.6V like the",
        "        old Schottky-OR).  See the RGB headroom warning in DESIGN.md.",
        "  ICHG/IINDPM are I2C REGISTERS.  No PROG resistor.  fw MUST kick the 40s",
        "        I2C watchdog or the charger reverts to 500mA defaults.",
        "GAUGE  U6   MAX17048 ModelGauge (I2C 0x36 -- no clash with U4's 0x6B):",
        "            VDD + CELL -> BAT+  (the RAW CELL, not VSYS -- it must keep",
        "                                 gauging while the system is off; ~3uA)",
        "            C32 100nF on VDD.   CTG -> GND.   QSTRT -> GND (unused).",
        "            GND(4) + EP(9) -> GND  (EP is a visible pin -- WIRE IT).",
        "            SDA/SCL -> shared I2C1.  ALRT -> MCU GPIO + R12 100k pull-up;",
        "            put ALRT on a WKUP pin -- it drives the LOWBAT annunciator off a",
        "            REAL state-of-charge, not a voltage guess.  ModelGauge needs NO",
        "            current-sense resistor.  (The BQ25601 has no ADC at all.)",
        "            Pkg TDFN-8 2x2, EP 0.8x1.2 (pad 9) -- per Maxim outline 21-0168.",
        "",
        "BUCK-BST U2 TPS63900: VIN<-VSYS, L1 2.2uH, Cin/Cout C3/C4/C5;",
        "            CFG strap=3.3V.  VOUT=+3V3 -> MCU ONLY (display has its own",
        "            EN-gated 5V boost) so the TPS63900 stays low-Iq in sleep.",
        "BATTERY J2  JST-PH 1S:  1=BAT+, 2=GND.")))

# ===================== Keyboard mezzanine sheet ==============================
# Fine-pitch board-to-board mezzanine UP to the keyboard board that stacks above.
# The keyboard board now has its OWN scanner MCU (STM32G031K8U6) that scans the
# matrix + drives the annunciators locally, so ONLY a serial link + power crosses
# the mezzanine (NOT the raw matrix): a shared I2C bus + a UART, plus a KB_IRQ
# wake line (keyboard -> MCU) and reset/boot for reflashing the keyboard MCU.
# J5 = Hirose DF40 2x6 (12-pin) 0.4mm RECEPTACLE (DF40B-12DS, LCSC C3641147); the
# keyboard board carries the mating header (DF40C-12DP C6224952). Grown 10 -> 12
# pins to carry VSYS + a dedicated GND up to the keyboard for its per-key RGB
# lighting. (DF40C-12DS isn't LCSC-stocked, so the receptacle is DF40B-12DS --
# same 1.5mm stack: on DF40 the receptacle SUFFIX sets the height [none=1.5mm,
# (2.0)=2.0mm], not the B/C letter, and the DF40C plug is the common header.)
MEZZ_SOCKET_FP = "Connector_Hirose_DF40:Hirose_DF40B-12DS-0.4V_2x06-1MP_P0.4mm"
FFC16_FP = "Connector_FFC-FPC:Hirose_FH12-16S-0.5SH_1x16-1MP_P0.50mm_Horizontal"
KEYBOARD_IF = dict(name="KeyboardIF", file="keyboard_if.kicad_sch",
    title="Keyboard link -- DF40 stack (J5) OR 16-pin FFC cable (J6), populate one", page="4",
    big=[
        dict(ref="J5", lib_id="Connector_Generic:Conn_02x06_Odd_Even",
             value="TO KEYBOARD (stack)", fp=MEZZ_SOCKET_FP,
             lcsc="C3641147", mpn="DF40B-12DS-0.4V(58)", mfr="Hirose"),
        dict(ref="J6", lib_id="Connector_Generic:Conn_01x16", value="TO KEYBOARD (FFC)",
             fp=FFC16_FP, lcsc="C262665", mpn="AFC01-S16FCA-00", mfr="JUSHUO"),
    ],
    small=[],
    note=(15, 95, K.note_block(
        "KEYBOARD LINK  -  TWO options on the SAME nets; POPULATE ONE:",
        "  J5 STACK = DF40B-12DS 2x6 0.4mm (C3641147), ~1.5mm rigid mezzanine up",
        "     to the keyboard header (kbd J1 DF40C-12DP). Compact, BUT the MCU",
        "     board then sits under the keys -> tight vs the keyboard's bottom",
        "     Kailh sockets/LEDs.",
        "  J6 CABLE = 16-pin 0.5mm FFC (AFC01-S16FCA-00, C262665) -> the MCU board",
        "     mounts FREELY in the case (fixes the spacing/interference). 16-pin",
        "     so its cable CAN'T cross-plug the 12-pin display FFC (J3).",
        "Keyscanning is on the keyboard G0 -> only a serial link + power cross.",
        "PLACED, not wired.  The keyboard end (MainIF) mirrors both connectors.",
        "",
        "J5 DF40 (12-pin):",
        K.pin_table([(1, "+3V3"), (2, "GND"), (3, "SDA"), (4, "SCL"), (5, "UART_TX"),
                     (6, "UART_RX"), (7, "KB_IRQ"), (8, "KB_NRST"), (9, "KB_BOOT0"),
                     (10, "GND"), (11, "VSYS"), (12, "GND")]),
        "J6 FFC (16-pin: VSYS x2, GND x3 for LED current + 2 spare):",
        K.pin_table([(1, "+3V3"), (2, "GND"), (3, "SDA"), (4, "SCL"), (5, "UART_TX"),
                     (6, "UART_RX"), (7, "KB_IRQ"), (8, "KB_NRST"), (9, "KB_BOOT0"),
                     (10, "GND"), (11, "VSYS"), (12, "VSYS"), (13, "GND"), (14, "GND"),
                     (15, "NC"), (16, "NC")]),
        "",
        "I2C = G0 reports keys + gets annunciator state; KB_IRQ = G0->MCU WKUP",
        "wake; UART = alt/bootloader; NRST/BOOT0 = MCU reflashes the G0. VSYS =",
        "the BQ25601 NVDC SYS rail (PSU sheet; ~VBAT, 3.5V floor -- NOT the old",
        "4.6V Schottky-OR) -> the keyboard per-key RGB (gated on the keyboard;",
        "check SK6812 headroom at 3.5V).  FFC cable (non-BOM): GCT FFC05-TIN 05-16-A-<len>-A-4-",
        "06-4-T (DigiKey). Stacked build: MCU board under a keyless region /",
        "board edge. Verify lands + 3D clearance at layout.")))

# NOTE: the DisplayIF sheet (J3 unified SPI FFC + J7 VSYS outlet) was folded into
# the MCU sheet above on 2026-07-09 and display_if.kicad_sch deleted. Each display
# is a self-contained MODULE (7-seg OR RGB matrix) with its OWN MCU, plugging into
# the unified SPI connector, so the old EN-gated 5V boost + 74HCT125 shifter MOVED
# onto the (7-seg) display board. J7 feeds the RGB-matrix module's LED rail
# directly from VSYS (amps, kept off the signal FFC). The 3V3 TPS63900 (PSU sheet)
# still feeds the MCU.

# NOTE: the Keypad (Cherry MX matrix) and Annunciator-LED sheets moved to the
# separate, stacked **calcumaker-keyboard** board (2026-07-05 split). They reach
# the MCU across the KeyboardIF mezzanine (J5) above.

# ======================= QSPI flash memory sheet =============================
# 4MB (32Mbit) quad-SPI NOR flash on the STM32U575 OCTOSPI1 peripheral (quad
# I/O). Memory-mappable (XIP) for constants/tables, and usable as storage for
# state persistence / keystroke programs. CS# pulled up so the flash stays
# deselected during MCU reset/boot.
QSPI_FLASH = dict(name="QSPIFlash", file="qspi_flash.kicad_sch",
    title="4MB quad-SPI NOR flash (OCTOSPI1)", page="5",
    big=[
        dict(ref="U7", lib_id="Memory_Flash:W25Q32JVSS", value="W25Q32JVSSIQ",
             fp=QSPI_FP, lcsc="C179173", mpn="W25Q32JVSSIQ", mfr="Winbond"),
    ],
    small=[
        C("C26", "100nF"),      # flash VCC decoupling (close to pin 8)
        R("R9", "10k"),         # CS# pull-up to +3V3 (deselect during reset/boot)
    ],
    note=(15, 95, K.note_block(
        "QSPI FLASH  -  U7  W25Q32JVSSIQ  (LCSC C179173)",
        "4MB (32Mbit) quad-SPI NOR, SOIC-8, 2.7-3.6V, on the STM32U575 OCTOSPI1.",
        "",
        K.pin_table([("1", "CS#         <-  NCS     PA4   AF3    p20  (+R9 10k)"),
                     ("6", "CLK         <-  CLK     PB10  AF10   p29"),
                     ("5", "IO0 / DI    <-> IO0     PB1   AF10   p27"),
                     ("2", "IO1 / DO    <-> IO1     PB0   AF10   p26"),
                     ("3", "IO2 / WP#   <-> IO2     PA7   AF10   p23"),
                     ("7", "IO3 / HOLD# <-> IO3     PA6   AF10   p22"),
                     ("8", "VCC = +3V3   (C26 100nF at pin 8)"),
                     ("4", "GND")], cols=1),
        "",
        "OCTOSPI on U5 goes through the OCTOSPI I/O manager: the GPIO AFs are",
        "OCTOSPIM_P1_* and OCTOSPIM routes OCTOSPI1 -> Port 1 (straight-thru).",
        "LQFP-64 bonds out NO Port-2 bus (ports E/F/G absent), so Port 1 is",
        "mandatory. On this package IO0-3 have exactly ONE pin each -- only",
        "CLK (PA3|PB10) and NCS (PA2|PA4|PC11) were a choice. PB10+PA4 keep",
        "the whole bus in a pin 20-29 cluster and leave PA2/PA3 intact as the",
        "USART2 TX/RX pair for the keyboard link. Costs SPI1_NSS(PA4)+WKUP2.",
        "Verified against the ST CubeMX pin DB for STM32U575RGTx / LQFP-64.",
        "",
        "Keep the 4 IO + CLK short and length-matched at layout (>=50 MHz quad).",
        "Use: memory-mapped XIP for constant tables + state/program storage.",
        "1.8V-IO variant = W25Q32JW.")))

# ============================ generate =======================================
K.build(
    project="calcumaker-mcu", proj_dir=PROJ_DIR, root_uuid=ROOT_UUID,
    title=dict(title="Calcumaker 16 — MCU", date="2026-07-06", rev="0.3",
               company="calcumaker authors",
               comments=["Programmer's/technical arbitrary-precision RPN calculator",
                         "MCU board: STM32U575RGT6 (LQFP-64) + PSU + clock + SWD + display-IF + keyboard mezzanine + 4MB QSPI flash (DRAFT)"]),
    sheets=[MCU, PSU, KEYBOARD_IF, QSPI_FLASH],
)
