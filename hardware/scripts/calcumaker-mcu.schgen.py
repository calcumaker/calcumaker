#!/usr/bin/env python3
"""Regenerate the Calcumaker 16 **MCU board** hierarchical schematic.

    CALCUMAKER_SCHGEN_DRAFT_OK=1 python3 scripts/calcumaker-mcu.schgen.py
    (or: CALCUMAKER_SCHGEN_DRAFT_OK=1 make gen-calcumaker-mcu)

*** DRAFT ***
The MCU board is the **brain/PSU logic board** of a THREE-board split (see
DESIGN.md → Board Partition): it carries the **MCU (STM32U575RGT6)**, **PSU**
(USB-C/charge/buck-boost), clock, SWD, the **display 5V rail + level shifter +
interconnect** (0.5mm FFC) to the angled display board, and a **fine-pitch
mezzanine** up to the **keyboard board** that stacks above it (the Cherry MX
matrix + its own STM32G0 scanner + annunciator LEDs live there — a dense LQFP-64
and 50 through-hole keys don't belong on one PCB). Keyscanning is off the main
board: only an **I2C + UART link + KB_IRQ wake + power** cross the mezzanine (not
the raw matrix). The PSU sheet is concrete; the MCU config and connector pinouts
are PLACEHOLDERS pending front-panel layout. A guard refuses to generate first.

This is DATA; the engine is scripts/kschgen.py. Components are PLACED, not wired
— wire them in eeschema using the per-sheet notes as the spec (regenerate BEFORE
wiring; regen reassigns UUIDs). 0402 passives; bulk MLCCs 0603. Verify each
lib_id/footprint exists in your KiCad 10 install before relying on it.
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
K.register_stdlib("Device", "R", "C", "L", "LED", "Crystal", "D_Schottky", "D")
K.register_stdlib("Regulator_Switching", "TPS63900")
K.register_stdlib("Battery_Management", "MCP73831-2-OT")
K.register_stdlib("Power_Protection", "USBLC6-2SC6")
K.register_stdlib("Transistor_FET", "Q_PMOS_GSD")
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
C0603 = "Capacitor_SMD:C_0603_1608Metric"   # bulk MLCCs (10/22uF @ >=16V)
L2016 = "Inductor_SMD:L_0805_2012Metric"     # ~2x1.6mm power inductor (verify land vs part)
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
# STM32U575RGTx (LQFP-64) + power decoupling + reset/boot. Clock and programming
# are their own subsheets. NOTE: the U5 core can run from the internal LDO or the
# internal SMPS; SMPS mode needs an external inductor on VLXSMPS + VDD12 caps
# (datasheet) — placed/configured at layout. VDDA/VREF+ and VDDUSB decoupled.
MCU = dict(name="MCU", file="mcu.kicad_sch", title="MCU core (STM32U575)",
    page="2",
    big=[
        dict(ref="U1", lib_id="MCU_ST_STM32U5:STM32U575RGTx", value="STM32U575RGT6",
             fp=LQFP64, lcsc="C5270980", mpn="STM32U575RGT6", mfr="STMicroelectronics"),
    ],
    small=[
        # Refs are globally unique across the board: PSU uses C1-C7/R1-R5,
        # DisplayIF C8-C11/R6-R7, so MCU starts at C12/R8.
        C("C12", "100nF"), C("C13", "100nF"), C("C14", "100nF"), C("C15", "100nF"),
        C("C16", "100nF"),                                 # VDD x5 decoupling
        C("C17", "10uF", C0603),                           # VDD bulk
        C("C18", "1uF"), C("C19", "100nF"),                # VDDA/VREF+ filter
        C("C20", "100nF"),                                 # VDDUSB
        C("C21", "100nF"),                                 # NRST cap
        R("R8", "10k"),                                    # BOOT0 pulldown
        C("C22", "2.2uF", C0603), C("C23", "2.2uF", C0603),  # VCORE/VCAP (LDO/SMPS) — verify per mode
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
        "OFF-SHEET",
        "  USB   PA11/PA12  -> PSU ESD (U3)",
        "  SWD   PA13/PA14  -> Programming J4  (+ NRST)",
        "  LSE   OSC32      -> Clock Y1",
        "  DISP  SPI1 + DISP_IRQ/NRST/BOOT  -> DisplayIF (unified module bus)",
        "  KBD   I2C+UART+KB_IRQ/NRST/BOOT0 -> KeyboardIF J5",
        "  QSPI  OCTOSPI1 CLK/NCS/IO0-3     -> QSPIFlash U7",
        "",
        "Verify OCTOSPI1 pin mapping is available on LQFP-64.")))

# ============================ Clock sheet ====================================
CLOCK = dict(name="Clock", file="clock.kicad_sch", title="LSE 32.768 kHz (RTC)",
    page="3", big=[],
    small=[
        dict(ref="Y1", lib_id="Device:Crystal", value="32.768kHz", fp=XTAL_FP,
             lcsc="C32346", mpn="Q13FC13500004", mfr="Epson"),
        C("C24", "12pF"), C("C25", "12pF"),                # LSE load caps
    ],
    note=(15, 100, K.note_block(
        "CLOCK  -  LSE 32.768 kHz   (Y1  Q13FC13500004, LCSC C32346)",
        "",
        "  Y1.1 -> OSC32_IN  (PC14)",
        "  Y1.2 -> OSC32_OUT (PC15)",
        "  C24 / C25 -> LSE load caps to GND   (12pF shown)",
        "",
        "Load caps: CL match = 2*(CL - Cstray); trim with the RTC SMOOTHCALIB.",
        "Drives the RTC for sleep timing.")))

# ============================ Programming sheet ==============================
# PSU uses J1/J2, DisplayIF uses J3, so SWD = J4.
PROG = dict(name="Programming", file="prog.kicad_sch", title="SWD programming",
    page="4", big=[
        dict(ref="J4", lib_id="Connector:Conn_ARM_SWD_TagConnect_TC2030-NL",
             value="SWD TC2030-NL", fp=SWD_FP),
    ], small=[],
    note=(15, 95, K.note_block(
        "SWD PROGRAMMING  -  J4  Tag-Connect TC2030-NL  (no-legs pogo pad)",
        "Bare land, no part mounted.",
        "",
        K.pin_table([(1, "+3V3 (VTref)"), (2, "SWDIO (PA13)"), (3, "NRST"),
                     (4, "SWCLK (PA14)"), (5, "GND"), (6, "SWO (PB3, opt)")], cols=1))))

# ============================ PSU sheet (concrete) ===========================
# Mirrors the proven ephemerkey power path. NOTE: TPS63900 is ultra-low-Iq but
# modest max current (~hundreds of mA) — re-evaluate against the display LED
# current budget (DESIGN.md Power Tree); a higher-current buck-boost may be
# needed. LCSC values carried over from ephemerkey; re-verify stock.
PSU = dict(name="PSU", file="psu.kicad_sch",
    title="USB-C / Li-ion charge / load-share / buck-boost", page="5",
    big=[
        dict(ref="J1", lib_id="Connector:USB_C_Receptacle_USB2.0_16P", value="USB-C",
             fp="Connector_USB:USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal",
             lcsc="C2927039", mpn="USB-TYPE-C-019", mfr="GCT"),
        dict(ref="J2", lib_id="Connector_Generic:Conn_01x02", value="BAT 1S Li-ion",
             fp="Connector_JST:JST_PH_S2B-PH-K_1x02_P2.00mm_Horizontal",
             lcsc="C173752", mpn="S2B-PH-K-S", mfr="JST"),
        dict(ref="U2", lib_id="Regulator_Switching:TPS63900", value="TPS63900DSKR",
             fp="Package_SON:WSON-10-1EP_2.5x2.5mm_P0.5mm_EP1.2x2mm",
             lcsc="C1518762", mpn="TPS63900DSKR", mfr="Texas Instruments"),  # TODO: current sizing
    ],
    small=[
        dict(ref="U3", lib_id="Power_Protection:USBLC6-2SC6", value="USBLC6-2SC6",
             fp=SOT236, lcsc="C2687116", mpn="USBLC6-2SC6", mfr="STMicroelectronics"),
        R("R1", "5.1k"), R("R2", "5.1k"),                # CC1/CC2 (sink)
        dict(ref="U4", lib_id="Battery_Management:MCP73831-2-OT",
             value="MCP73831-2-OT", fp=SOT235, lcsc="C424093",
             mpn="MCP73831T-2ACI/OT", mfr="Microchip"),
        R("R3", "4.7k"),                                 # PROG (charge current — size to cell)
        C("C1", "10uF", C0603), C("C2", "10uF", C0603),  # charger in/out
        dict(ref="Q1", lib_id="Transistor_FET:Q_PMOS_GSD", value="AO3401A",
             fp=SOT23, lcsc="C15127", mpn="AO3401A", mfr="AOS"),
        dict(ref="D1", lib_id="Device:D_Schottky", value="B5819W", fp=SOD123,
             lcsc="C8598", mpn="B5819W", mfr="Slkor"),
        R("R4", "100k"),                                 # load-share gate pulldown
        dict(ref="L1", lib_id="Device:L", value="2.2uH", fp=L2016),  # smallest reasonable 0805/2016 power L; verify Isat for TPS63900 (PARTS.md)
        C("C3", "10uF", C0603), C("C4", "10uF", C0603), C("C5", "10uF", C0603),
        C("C6", "10uF", C0603), C("C7", "10uF", C0603),
        dict(ref="D2", lib_id="Device:LED", value="CHG", fp=LED0402, lcsc="C130719"),
        R("R5", "1k"),
    ],
    note=(15, 165, K.note_block(
        "POWER  -  USB-C -> charge -> load-share -> buck-boost 3V3",
        "PLACED, not wired.  See DESIGN.md Power Tree.",
        "",
        "USB-C  J1   CC1->R1, CC2->R2 (5.1k sink); D+/D- -> U3 ESD -> MCU USB;",
        "            VBUS bulk C6.",
        "CHARGER U4  MCP73831: VDD<-VBUS, VBAT->BAT+, PROG R3 (size to cell),",
        "            STAT->D2+R5; C1/C2 in/out.",
        "LOAD-SHR Q1 AO3401A: src=BAT+, drn=VSYS, gate<-VBUS via R4;",
        "            D1 B5819W VBUS->VSYS.",
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
    title="Keyboard link -- DF40 stack (J5) OR 16-pin FFC cable (J6), populate one", page="7",
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
        "load-shared batt/USB rail (PSU sheet) -> the keyboard per-key RGB (gated",
        "on the keyboard). FFC cable (non-BOM): GCT FFC05-TIN 05-16-A-<len>-A-4-",
        "06-4-T (DigiKey). Stacked build: MCU board under a keyless region /",
        "board edge. Verify lands + 3D clearance at layout.")))

# ===================== Display-module interface sheet ========================
# Each display is now a self-contained MODULE (7-seg OR RGB matrix) with its OWN
# MCU, plugging into a UNIFIED SPI connector. So the old EN-gated 5V boost +
# 74HCT125 shifter MOVED onto the (7-seg) display board, and J3 is technology-
# agnostic: power + SPI "display intent" + reset/boot. A separate 2-pin VSYS
# outlet (J7) feeds the RGB-matrix module's LED rail directly from the PSU (amps,
# kept off the signal FFC). The 3V3 TPS63900 (PSU sheet) still feeds the MCU.
DISPLAY_IF = dict(name="DisplayIF", file="display_if.kicad_sch",
    title="Unified display-module interface (SPI FFC J3 + VSYS outlet J7)",
    page="6",
    big=[
        # Unified 12-pos 0.5mm FFC to the display module. CABLE = GCT FFC05-TIN
        # 05-12-A-<length>-A-4-06-4-T (DigiKey accessory, NOT assembled; len TBD).
        dict(ref="J3", lib_id="Connector_Generic:Conn_01x12", value="TO DISPLAY (unified SPI FFC)",
             fp="Connector_FFC-FPC:Hirose_FH12-12S-0.5SH_1x12-1MP_P0.50mm_Horizontal",
             lcsc="C262661", mpn="AFC01-S12FCA-00", mfr="JUSHUO"),
        # VSYS outlet -> the RGB-matrix module's LED inlet (its own 2-pin JST).
        dict(ref="J7", lib_id="Connector_Generic:Conn_01x02", value="VSYS -> matrix LED pwr",
             fp="Connector_JST:JST_PH_S2B-PH-K_1x02_P2.00mm_Horizontal",
             lcsc="C173752", mpn="S2B-PH-K-S", mfr="JST"),
    ],
    small=[],
    note=(15, 110, K.note_block(
        "UNIFIED DISPLAY-MODULE INTERFACE",
        "",
        "J3 = 0.5mm 12-pos FFC to the display module (AFC01-S12FCA-00, C262661).",
        "SAME pinout on BOTH display boards (7-seg + RGB matrix) -> interchangeable.",
        "Technology-agnostic: power + SPI 'display intent' + reset/boot. The module",
        "MCU (STM32G031 on 7-seg / RP2040 on the matrix) is the SPI slave + renders",
        "locally; 5V + any level-shifting are generated ON the module now.",
        "",
        K.pin_table([(1, "VSYS"), (2, "VSYS"), (3, "GND"), (4, "GND"), (5, "+3V3"),
                     (6, "SPI_SCLK"), (7, "SPI_MOSI"), (8, "SPI_CS"), (9, "DISP_IRQ"),
                     (10, "DISP_NRST"), (11, "DISP_BOOT"), (12, "GND")]),
        "",
        "U575: SPI1 SCLK/MOSI/CS + DISP_IRQ (EXTI) + DISP_NRST/DISP_BOOT (reflash",
        "the module MCU). +3V3 from the MCU rail; VSYS from the PSU load-share.",
        "",
        "J7 = 2-pin JST-PH VSYS outlet -> the RGB-matrix module's LED inlet (J2):",
        "the matrix pulls amps for 2304 LEDs, so its LED current takes this direct",
        "lead, NOT the signal FFC (the 7-seg module boosts from VSYS on the FFC).",
        "CABLE (non-BOM): GCT FFC05-TIN 05-12-A-<len>-A-4-06-4-T (DigiKey; len TBD).",
        "See DESIGN.md Unified display-module interface / Power Tree.")))

# NOTE: the Keypad (Cherry MX matrix) and Annunciator-LED sheets moved to the
# separate, stacked **calcumaker-keyboard** board (2026-07-05 split). They reach
# the MCU across the KeyboardIF mezzanine (J5) above.

# ======================= QSPI flash memory sheet =============================
# 4MB (32Mbit) quad-SPI NOR flash on the STM32U575 OCTOSPI1 peripheral (quad
# I/O). Memory-mappable (XIP) for constants/tables, and usable as storage for
# state persistence / keystroke programs. CS# pulled up so the flash stays
# deselected during MCU reset/boot.
QSPI_FLASH = dict(name="QSPIFlash", file="qspi_flash.kicad_sch",
    title="4MB quad-SPI NOR flash (OCTOSPI1)", page="8",
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
        K.pin_table([("1", "CS#       <- OCTOSPI NCS  (+ R9 10k pull-up to +3V3)"),
                     ("6", "CLK       <- OCTOSPI CLK"),
                     ("5", "IO0 / DI  <-> OCTOSPI IO0"),
                     ("2", "IO1 / DO  <-> OCTOSPI IO1"),
                     ("3", "IO2 / WP# <-> OCTOSPI IO2"),
                     ("7", "IO3 / HOLD# <-> OCTOSPI IO3"),
                     ("8", "VCC = +3V3   (C26 100nF at pin 8)"),
                     ("4", "GND")], cols=1),
        "",
        "Assign OCTOSPI1 to LQFP-64 pins (PB/PC bank); keep the 4 IO + CLK",
        "short and length-matched at layout (>=50 MHz quad).",
        "Use: memory-mapped XIP for constant tables + state/program storage.",
        "1.8V-IO variant = W25Q32JW.")))

# ============================ generate =======================================
K.build(
    project="calcumaker-mcu", proj_dir=PROJ_DIR, root_uuid=ROOT_UUID,
    title=dict(title="Calcumaker 16 — MCU", date="2026-07-06", rev="0.3",
               company="calcumaker authors",
               comments=["Programmer's/technical arbitrary-precision RPN calculator",
                         "MCU board: STM32U575RGT6 (LQFP-64) + PSU + clock + SWD + display-IF + keyboard mezzanine + 4MB QSPI flash (DRAFT)"]),
    sheets=[MCU, CLOCK, PROG, PSU, DISPLAY_IF, KEYBOARD_IF, QSPI_FLASH],
)
