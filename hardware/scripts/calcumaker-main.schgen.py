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
# TODO(mcu): register the STM32U575ZGT6 symbol (selected), e.g.:
#   K.register_stdlib("MCU_ST_STM32U5", "STM32U575ZGTx")   # verify exact symbol (LQFP-144)
# TODO(boost): register the chosen 5V display boost symbol once picked by
#   availability (MT3608 etc. likely need a custom symbol in calcumaker.kicad_sym).
# TODO(keypad): Cherry MX switch symbol — KiCad's Switch:SW_Push works as a
#   placeholder; the keyswitch-kicad-library 'SW_Cherry_MX_*' is preferred.

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

RLCSC = {"5.1k": "C25905", "4.7k": "C25900", "10k": "C25744",
         "100k": "C25741", "1k": "C11702", "0R": "C17168"}
# Per-rail voltage differs, so 10uF/22uF LCSC# live in hardware/PARTS.md, not here.
CLCSC = {"100nF": "C1525", "1uF": "C29266"}


def R(ref, val):
    return dict(ref=ref, lib_id="Device:R", value=val, fp=R0402,
                lcsc=RLCSC.get(val, ""))


def C(ref, val, fp=C0402):
    return dict(ref=ref, lib_id="Device:C", value=val, fp=fp,
                lcsc=CLCSC.get(val, ""))


# ============================ MCU sheet (TODO) ===============================
# STM32U575ZGT6 selected (DESIGN.md). Still needs its parts placed: VDD/VDDA/
# VDDIO decoupling, LSE 32.768kHz + load caps, NRST cap, BOOT0 pulldown, USB FS
# (PA11/PA12) + ESD (on PSU sheet), SWD (PA13/PA14) -> Tag-Connect, VDDUSB, and
# bus pins to the interconnect (display) + keypad sheets.
MCU = dict(name="MCU", file="mcu.kicad_sch", title="MCU / clock / programming",
    page="2", big=[], small=[],
    note=(15, 150, "Calcumaker 16 main — MCU (STM32U575ZGT6, LQFP-144). See "
          "DESIGN.md MCU + Pin Budget. Add: VDD/VDDA/VDDIO decoupling, LSE "
          "32.768kHz + 2x load caps, NRST 100nF, BOOT0 10k pulldown, USB FS "
          "PA11/PA12 (ESD on PSU), SWD PA13/PA14 -> Tag-Connect, display bus "
          "(TM1640 2-wire: shared CLK + 3x DIN, 3V3 GPIO/bit-bang) -> Display-IF "
          "sheet (74HCT125 level shifter) + DISP_PWR_EN GPIO -> 5V boost EN, "
          "GPIO matrix -> Keypad sheet (one col -> EXTI wake)."))

# ============================ PSU sheet (concrete) ===========================
# Mirrors the proven ephemerkey power path. NOTE: TPS63900 is ultra-low-Iq but
# modest max current (~hundreds of mA) — re-evaluate against the display LED
# current budget (DESIGN.md Power Tree); a higher-current buck-boost may be
# needed. LCSC values carried over from ephemerkey; re-verify stock.
PSU = dict(name="PSU", file="psu.kicad_sch",
    title="USB-C / Li-ion charge / load-share / buck-boost", page="3",
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

# ============================ Keypad sheet (TODO counts) =====================
# Cherry MX matrix, wide HP-16C layout (DESIGN.md). Skeleton shows the pattern;
# replicate SW+diode per key across an R x C matrix (~40-45 keys).
KEYPAD = dict(name="Keypad", file="keypad.kicad_sch",
    title="Cherry MX key matrix", page="4", big=[],
    small=[
        # Per key: SW_Push (placeholder for Cherry MX) + series diode 1N4148W.
        dict(ref="SW1", lib_id="Switch:SW_Push", value="MX",
             fp="Button_Switch_Keyboard:SW_Cherry_MX_1.00u_PCB"),   # TODO verify fp lib
        dict(ref="D3", lib_id="Device:D", value="1N4148W", fp=SOD123,
             lcsc="C81598", mpn="1N4148W", mfr="onsemi"),
        # ... repeat SWn + Dn for the full matrix (generate in a loop once the
        # key map + matrix dimensions are fixed).
    ],
    note=(15, 120, "Calcumaker 16 main — Keypad (Cherry MX matrix, wide HP-16C "
          "layout). PENDING key map + dimensions (DESIGN.md). ~40-45 keys on an "
          "R x C matrix (e.g. 6x8); ONE 1N4148W per key (cathode->col) for "
          "n-key rollover. Rows driven by GPIO, cols read with pull-ups; route "
          "one col to an EXTI line for wake-from-Stop on keypress. Optional "
          "Kailh hot-swap sockets."))

# ===================== Display power + interface sheet =======================
# The display runs at 5V (TM1640 is 5V-nominal; VIH=0.7*VDD=3.5V > MCU 3.3V).
# This sheet generates the EN-gated 5V display rail and translates the 4 control
# lines 3V3->5V, then hands +5V + 5V-logic to the display board via J3.
#   - U5: TPS61022 EN-gated 5V boost (off in sleep). LCSC C915088, VQFN-7.
#     Adjustable -> R6/R7 FB divider sets +5V. Custom symbol TODO (not in KiCad).
#   - U6: 74HCT125 quad buffer @5V = 3V3->5V level shift for CLK + DIN1/2/3
#     (KiCad symbol 74AHCT125 is pin-compatible; value=74HCT125, LCSC C352957).
#   - J3: 1x8 2.54mm header to the display board.
# The 3V3 TPS63900 (PSU sheet) now feeds only the MCU, so it stays as-is.
DISPLAY_IF = dict(name="DisplayIF", file="display_if.kicad_sch",
    title="Display 5V rail (TPS61022) + 74HCT125 level shifter + interconnect",
    page="5",
    big=[
        # 5V boost TPS61022 (adjustable). Symbol is TODO (author into
        # calcumaker.kicad_sym like the MCU); footprint exists in KiCad.
        dict(ref="U5", lib_id="calcumaker:TPS61022", value="TPS61022RWUR",
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
    sheets=[MCU, PSU, KEYPAD, DISPLAY_IF],
)
