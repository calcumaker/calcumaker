#!/usr/bin/env python3
"""Regenerate the Calcumaker 16 **main board** hierarchical schematic.

    CALCUMAKER_SCHGEN_DRAFT_OK=1 python3 scripts/calcumaker-main.schgen.py
    (or: CALCUMAKER_SCHGEN_DRAFT_OK=1 make gen-calcumaker-main)

*** DRAFT ***
The main board carries the **MCU (STM32U575ZGT6)**, **PSU** (USB-C/charge/
buck-boost), the **Cherry MX key matrix**, and the **interconnect** to the
separate display board (split design — the display assembly angles up; only the
serial bus + power cross the connector). The PSU sheet is concrete; the MCU,
keypad counts, and connector pinout are PLACEHOLDERS pending the front-panel
layout (see ../../DESIGN.md → Open Questions). A guard refuses to generate until
you opt in. Fill the TODOs, then remove the guard.

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
        "calcumaker-main.schgen.py is a DRAFT: the MCU/keypad/interconnect are "
        "placeholders (see DESIGN.md Open Questions). Set "
        "CALCUMAKER_SCHGEN_DRAFT_OK=1 to generate anyway."
    )

HW = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))   # hardware/
PROJ_DIR = os.path.join(HW, "calcumaker-main")
ROOT_UUID = "ca1c0000-0000-4000-8000-00000000ma01"   # keep stable across regens

# ---- symbol libraries -------------------------------------------------------
K.register_stdlib("Device", "R", "C", "L", "LED", "Crystal", "D_Schottky", "D")
K.register_stdlib("Regulator_Switching", "TPS63900")
K.register_stdlib("Battery_Management", "MCP73831-2-OT")
K.register_stdlib("Power_Protection", "USBLC6-2SC6")
K.register_stdlib("Transistor_FET", "Q_PMOS_GSD")
K.register_stdlib("Switch", "SW_Push")
K.register_stdlib("Connector", "USB_C_Receptacle_USB2.0_16P",
                  "Conn_ARM_SWD_TagConnect_TC2030-NL")
K.register_stdlib("Connector_Generic", "Conn_01x02", "Conn_01x08")
K.register_stdlib("74xx", "74AHCT125")  # level shifter symbol (use value "74HCT125"); 3V3->5V
K.register_stdlib("MCU_ST_STM32U5", "STM32U575ZGTx")   # LQFP-144 (stock)
K.register_stdlib("Converter_DCDC", "TPS61022")        # 5V boost (stock)
# Note: Cherry MX keys use Switch:SW_Push (placeholder symbol); the
# keyswitch-kicad-library 'SW_Cherry_MX_*' is a nicer swap if installed.

# ---- footprint shorthands ---------------------------------------------------
# Size policy (user): 0402 for resistors + small/decoupling caps; bulk MLCCs
# (>=10uF) at 0603 (smallest with voltage margin on the 5V rail; true-0402 10uF
# is 6.3V-only). Magnetics: smallest reasonable from the JLCPCB catalog.
R0402 = "Resistor_SMD:R_0402_1005Metric"
C0402 = "Capacitor_SMD:C_0402_1005Metric"
C0603 = "Capacitor_SMD:C_0603_1608Metric"   # bulk MLCCs (10/22uF @ >=16V)
L2016 = "Inductor_SMD:L_0805_2012Metric"     # ~2x1.6mm power inductor (verify land vs part)
LED0402 = "LED_SMD:LED_0402_1005Metric"
SOT235 = "Package_TO_SOT_SMD:SOT-23-5"
SOT236 = "Package_TO_SOT_SMD:SOT-23-6"
SOT23 = "Package_TO_SOT_SMD:SOT-23"
SOD123 = "Diode_SMD:D_SOD-123"
LQFP144 = "Package_QFP:LQFP-144_20x20mm_P0.5mm"
XTAL_FP = "Crystal:Crystal_SMD_3215-2Pin_3.2x1.5mm"
SWD_FP = "Connector:Tag-Connect_TC2030-IDC-NL_2x03_P1.27mm_Vertical"

RLCSC = {"5.1k": "C25905", "4.7k": "C25900", "10k": "C25744",
         "100k": "C25741", "1k": "C11702", "0R": "C17168"}
# Per-rail voltage differs, so 10uF/22uF LCSC# live in hardware/PARTS.md, not here.
CLCSC = {"100nF": "C1525", "1uF": "C29266", "12pF": "C1547"}


def R(ref, val):
    return dict(ref=ref, lib_id="Device:R", value=val, fp=R0402,
                lcsc=RLCSC.get(val, ""))


def C(ref, val, fp=C0402):
    return dict(ref=ref, lib_id="Device:C", value=val, fp=fp,
                lcsc=CLCSC.get(val, ""))


# ============================ MCU core sheet =================================
# STM32U575ZGTx (LQFP-144) + power decoupling + reset/boot. Clock and programming
# are their own subsheets. NOTE: the U5 core can run from the internal LDO or the
# internal SMPS; SMPS mode needs an external inductor on VLXSMPS + VDD12 caps
# (datasheet) — placed/configured at layout. VDDA/VREF+ and VDDUSB decoupled.
MCU = dict(name="MCU", file="mcu.kicad_sch", title="MCU core (STM32U575)",
    page="2",
    big=[
        dict(ref="U1", lib_id="MCU_ST_STM32U5:STM32U575ZGTx", value="STM32U575ZGT6",
             fp=LQFP144, lcsc="C5271004", mpn="STM32U575ZGT6", mfr="STMicroelectronics"),
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
    note=(15, 150, "Calcumaker 16 main — MCU core (U1 STM32U575ZGTx, LQFP-144). "
          "POWER: VDD pins -> +3V3 (5x 100nF C12-C16 + C17 10uF bulk); VDDA/VREF+ "
          "-> C18 1uF + C19 100nF; VDDUSB -> C20 100nF; EP/VSS -> GND. "
          "VCORE: choose LDO or internal SMPS — SMPS needs an inductor on VLXSMPS "
          "+ VDD12; C22/C23 are VCAP placeholders (set per mode, datasheet). "
          "RESET/BOOT: NRST + C21 100nF; BOOT0 -> R8 10k to GND. "
          "OFF-SHEET: USB PA11/PA12 -> PSU ESD; SWD PA13/PA14 + NRST -> Programming; "
          "LSE OSC32_IN/OUT -> Clock; display bus (CLK+DIN1/2/3) + DISP_PWR_EN -> "
          "DisplayIF; 5 rows + 10 cols -> Keypad (one col -> EXTI wake)."))

# ============================ Clock sheet ====================================
CLOCK = dict(name="Clock", file="clock.kicad_sch", title="LSE 32.768 kHz (RTC)",
    page="3", big=[],
    small=[
        dict(ref="Y1", lib_id="Device:Crystal", value="32.768kHz", fp=XTAL_FP,
             lcsc="C32346", mpn="Q13FC13500004", mfr="Epson"),
        C("C24", "12pF"), C("C25", "12pF"),                # LSE load caps
    ],
    note=(15, 100, "Calcumaker 16 main — LSE 32.768kHz (Y1) -> MCU OSC32_IN/"
          "OSC32_OUT (PC14/PC15). C24/C25 load caps: match to Y1 CL via "
          "2*(CL - Cstray); 12pF shown — trim with the RTC SMOOTHCALIB. Drives "
          "the RTC for sleep timing."))

# ============================ Programming sheet ==============================
# PSU uses J1/J2, DisplayIF uses J3, so SWD = J4.
PROG = dict(name="Programming", file="prog.kicad_sch", title="SWD programming",
    page="4", big=[
        dict(ref="J4", lib_id="Connector:Conn_ARM_SWD_TagConnect_TC2030-NL",
             value="SWD TC2030-NL", fp=SWD_FP),
    ], small=[],
    note=(15, 95, "Calcumaker 16 main — SWD programming (J4 Tag-Connect TC2030-NL, "
          "no-legs pogo pad). Pins: +3V3, GND, SWDIO(PA13), SWCLK(PA14), NRST. "
          "Bare land — no part mounted."))

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
    note=(15, 165, "Calcumaker 16 main — Power (USB-C -> charge -> load-share -> "
          "buck-boost 3V3). PLACED, not wired. See DESIGN.md Power Tree.\n"
          "USB-C J1: CC1->R1, CC2->R2 (5.1k sink); D+/D- -> U3 ESD -> MCU USB. "
          "VBUS bulk C6.\nCHARGER U4 MCP73831: VDD<-VBUS; VBAT->BAT+; PROG R3 "
          "(size to cell); STAT->D2+R5. C1/C2 in/out.\nLOAD-SHARE: Q1 AO3401A "
          "src=BAT+, drn=VSYS, gate<-VBUS via R4; D1 B5819W VBUS->VSYS.\n"
          "BUCK-BOOST U2 TPS63900: VIN<-VSYS, L1 2.2uH, Cin/Cout C3/C4/C5. "
          "CFG strap=3.3V. VOUT=+3V3 -> MCU ONLY (display is on its own EN-gated "
          "5V boost, Display-IF sheet), so the TPS63900 stays lightly loaded / "
          "low-Iq for sleep.\nBATTERY J2 JST-PH: 1=BAT+, 2=GND (1S)."))

# ============================ Keypad sheet ===================================
# Wide HP-16C-style layout: 5 rows x 10 cols = 50 full-size Cherry MX keys, with
# f/g shifts (3 functions per key). See DESIGN.md "Keypad" for the keymap.
# Matrix: ROWr (GPIO out) -> SW -> 1N4148W (anode@SW, cathode@COLc) -> COLc
# (GPIO in, internal pull-up). 50 keys => SW1..SW50 + D11..D60. Key index =
# (r-1)*10 + c. (Diodes start at D11 to avoid the PSU's D1/D2.)
MX_FP = "Button_Switch_Keyboard:SW_Cherry_MX_1.00u_PCB"   # plate/PCB-mount 1u; Kailh hot-swap variant optional
KEY_SW = [dict(ref="SW%d" % i, lib_id="Switch:SW_Push", value="MX", fp=MX_FP)
          for i in range(1, 51)]
KEY_D = [dict(ref="D%d" % (10 + i), lib_id="Device:D", value="1N4148W", fp=SOD123,
              lcsc="C81598", mpn="1N4148W", mfr="onsemi") for i in range(1, 51)]
KEYPAD = dict(name="Keypad", file="keypad.kicad_sch",
    title="Cherry MX key matrix (5x10, 50 keys)", page="6",
    big=KEY_SW, small=KEY_D,
    note=(15, 130, "Calcumaker 16 main — Keypad: 50 Cherry MX keys in a 5x10 "
          "scanned matrix (wide HP-16C layout + f/g shifts; keymap in DESIGN.md). "
          "PLACED, not wired. WIRING: ROW1..ROW5 = GPIO outputs; COL1..COL10 = "
          "GPIO inputs with INTERNAL pull-ups (no external R; STM32U5 keeps "
          "pull-ups in Stop). Each key SWn in series with Dn (anode at switch, "
          "cathode to its COL) for n-key rollover. Key (row r, col c): SW#=(r-1)*"
          "10+c, D#=10+(r-1)*10+c. Scan: drive one ROW low, read COLs. WAKE: "
          "drive all ROWs low + EXTI on one COL (or all) -> any keypress wakes "
          "from Stop. Optional Kailh hot-swap sockets (same footprint family)."))

# ===================== Display power + interface sheet =======================
# The display runs at 5V (TM1640 is 5V-nominal; VIH=0.7*VDD=3.5V > MCU 3.3V).
# This sheet generates the EN-gated 5V display rail and translates the 4 control
# lines 3V3->5V, then hands +5V + 5V-logic to the display board via J3.
#   - U5: TPS61022 EN-gated 5V boost (off in sleep). LCSC C915088, VQFN-7.
#     Adjustable -> R6/R7 FB divider sets +5V. Symbol: stock Converter_DCDC:TPS61022.
#   - U6: 74HCT125 quad buffer @5V = 3V3->5V level shift for CLK + DIN1/2/3
#     (KiCad symbol 74AHCT125 is pin-compatible; value=74HCT125, LCSC C352957).
#   - J3: 1x8 2.54mm header to the display board.
# The 3V3 TPS63900 (PSU sheet) now feeds only the MCU, so it stays as-is.
DISPLAY_IF = dict(name="DisplayIF", file="display_if.kicad_sch",
    title="Display 5V rail (TPS61022) + 74HCT125 level shifter + interconnect",
    page="7",
    big=[
        # 5V boost TPS61022 (adjustable). Symbol is TODO (author into
        # calcumaker.kicad_sym like the MCU); footprint exists in KiCad.
        dict(ref="U5", lib_id="Converter_DCDC:TPS61022", value="TPS61022RWUR",
             fp="Package_DFN_QFN:Texas_RWU0007A_VQFN-7_2x2mm_P0.5mm",
             lcsc="C915088", mpn="TPS61022RWUR", mfr="Texas Instruments"),
        dict(ref="U6", lib_id="74xx:74AHCT125", value="74HCT125",
             fp="Package_SO:SOIC-14_3.9x8.7mm_P1.27mm",
             lcsc="C352957", mpn="SN74HCT125DR", mfr="Texas Instruments"),
        dict(ref="J3", lib_id="Connector_Generic:Conn_01x08", value="TO DISPLAY",
             fp="Connector_PinHeader_2.54mm:PinHeader_1x08_P2.54mm_Vertical",
             lcsc="C492407", mpn="PZ254V-11-08P", mfr="XKB"),
    ],
    small=[
        dict(ref="L2", lib_id="Device:L", value="1uH", fp=L2016,
             lcsc="C5832342", mpn="FTC201610S1R0MBCA", mfr="Sunlord"),  # 2.0x1.6mm, boost L
        C("C8", "10uF", C0603), C("C9", "22uF", C0603), C("C10", "22uF", C0603),  # boost in / out (2x22u)
        C("C11", "100nF"),                                # 74HCT125 VCC(5V) decoupling
        R("R6", "732k"), R("R7", "100k"),                 # FB divider: Vout=0.6*(1+R6/R7)=~5.0V
    ],
    note=(15, 110, "Calcumaker 16 main — Display 5V rail + interface. "
          "5V BOOST U5 TPS61022 (C915088, EN-gated): VIN<-VSYS (3.0-4.7V), L2 "
          "1uH (C5832342), Cin C8 10uF, Cout C9/C10 2x22uF; FB R6/R7 divider "
          "-> +5V (Vref 0.6V); EN<-MCU DISP_PWR_EN (low in sleep). LEVEL SHIFT "
          "U6 74HCT125 @ +5V (VIH=2V accepts 3V3): IN<-MCU CLK,DIN1,DIN2,DIN3 "
          "(3V3); OUT-> J3 at 5V logic; C11 100nF; tie all 4 /OE low. J3 to "
          "display (MUST match calcumaker-display J1): 1=+5V, 2=GND, 3=CLK, "
          "4=DIN1, 5=DIN2, 6=DIN3, 7=GND, 8=spare. Wide +5V/GND (LED current). "
          "See DESIGN.md Power Tree / Board Partition."))

# ============================ generate =======================================
K.build(
    project="calcumaker-main", proj_dir=PROJ_DIR, root_uuid=ROOT_UUID,
    title=dict(title="Calcumaker 16 — Main", date="2026-06-21", rev="0.1",
               company="calcumaker authors",
               comments=["Programmer's/technical arbitrary-precision RPN calculator",
                         "Main board: MCU + PSU + keypad + display interconnect (DRAFT)"]),
    sheets=[MCU, CLOCK, PROG, PSU, KEYPAD, DISPLAY_IF],
)
