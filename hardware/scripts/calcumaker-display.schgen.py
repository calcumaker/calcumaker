#!/usr/bin/env python3
"""Regenerate the Calcumaker 16 **display board** hierarchical schematic.

    CALCUMAKER_SCHGEN_DRAFT_OK=1 python3 scripts/calcumaker-display.schgen.py
    (or: CALCUMAKER_SCHGEN_DRAFT_OK=1 make gen-calcumaker-display)

*** DRAFT ***
The display board is a SEPARATE PCB (split design — it angles up; only the
display serial bus + power cross the interconnect from the main board). It holds
the multi-row 7-segment RPN stack (**2-3 rows**; the board is laid out for 3
rows with the top row optionally populated) plus its driver IC(s), and the
interconnect connector back to the main board.

The driver IC + 7-seg digit parts are chosen by **price + LCSC availability**
(research) and are PLACEHOLDERS here until that lands (see ../../DESIGN.md →
Display). A guard refuses to generate until you opt in.

This is DATA; the engine is scripts/kschgen.py. Components are PLACED, not wired.
Verify each lib_id/footprint exists in your KiCad 10 install before relying on it.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kschgen as K

# --- DRAFT guard -------------------------------------------------------------
if not os.environ.get("CALCUMAKER_SCHGEN_DRAFT_OK"):
    sys.exit(
        "calcumaker-display.schgen.py is a DRAFT: the driver + 7-seg digit "
        "parts are pending availability research (see DESIGN.md Display). Set "
        "CALCUMAKER_SCHGEN_DRAFT_OK=1 to generate anyway."
    )

HW = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))   # hardware/
PROJ_DIR = os.path.join(HW, "calcumaker-display")
ROOT_UUID = "ca1c0000-0000-4000-8000-0000000d1501"   # keep stable across regens

# ---- symbol libraries -------------------------------------------------------
K.register_stdlib("Device", "R", "C")
K.register_stdlib("Connector_Generic", "Conn_01x08")
# Digits: the FJ5161AH is a generic 0.56" 4-digit COMMON-CATHODE display — KiCad
# bundles this exact topology as Display_Character:CC56-12* (12-pin). We reuse
# CC56-12EWA (verify the FJ5161AH pinout matches CC56-12 at layout).
K.register_stdlib("Display_Character", "CC56-12EWA")
# Driver: TM1640 is NOT in KiCad — vendored symbol authored from the datasheet
# pinout in hardware/lib/symbols/calcumaker.kicad_sym.
K.register_lib("calcumaker",
               os.path.join(HW, "lib", "symbols", "calcumaker.kicad_sym"),
               "TM1640")

# ---- footprint shorthands ---------------------------------------------------
C0402 = "Capacitor_SMD:C_0402_1005Metric"
C0805 = "Capacitor_SMD:C_0805_2012Metric"
R0402 = "Resistor_SMD:R_0402_1005Metric"
CLCSC = {"100nF": "C1525", "10uF": "C15850"}


def R(ref, val):
    return dict(ref=ref, lib_id="Device:R", value=val, fp=R0402)


def C(ref, val, fp=C0402):
    return dict(ref=ref, lib_id="Device:C", value=val, fp=fp,
                lcsc=CLCSC.get(val, ""))


# ============================ Display sheet ==================================
# Multi-row 7-seg RPN stack: 3 rows laid out, top row optionally populated (=> 2
# or 3 visible rows). Each row = ONE TM1640 driving 16 common-cathode digits =
# 4x FJ5161AH (0.56" 4-digit CC). Parts chosen by LCSC stock/price (research):
#   TM1640   C5337152  SOP-28 (SOIC-28W)   ~$0.12  (2-wire, 16-dig x 8-seg, CC)
#   FJ5161AH C8093     0.56" 4-digit common-cathode, THROUGH-HOLE  ~$0.19
#       -> symbol = stock Display_Character:CC56-12EWA; fp = Display_7Segment:CC56-12GWA
# *** THT digits: no SMD multi-digit 7-seg are stocked on LCSC, so the display
#     board needs THT assembly (JLCPCB THT add-on or hand-solder). ***
TM1640_FP = "Package_SO:SOIC-28W_7.5x18.7mm_P1.27mm"   # verify vs TM1640 SOP-28 drawing
DIGIT_FP = "Display_7Segment:CC56-12GWA"               # CC56-12 land = FJ5161AH 12-pin
# 3 row drivers (U3/top row optional for a 2-row build).
DRIVERS = [dict(ref=f"U{r}", lib_id="calcumaker:TM1640", value="TM1640",
                fp=TM1640_FP, lcsc="C5337152", mpn="TM1640", mfr="TitanMicro")
           for r in (1, 2, 3)]
# 12 digit modules: 4 per row x 3 rows (DS9-12 = top row, optional).
DIGITS = [dict(ref=f"DS{n}", lib_id="Display_Character:CC56-12EWA",
               value="FJ5161AH", fp=DIGIT_FP, lcsc="C8093", mpn="FJ5161AH",
               mfr="Forge") for n in range(1, 13)]
DISPLAY = dict(name="Display", file="display.kicad_sch",
    title="7-seg RPN stack (3 rows x 16 digits) + TM1640 drivers", page="2",
    big=DRIVERS + DIGITS,
    small=[
        # Per TM1640: 100nF decoupling; shared 3V3 bulk.
        C("C1", "100nF"), C("C2", "100nF"), C("C3", "100nF"),  # U1/U2/U3 bypass
        C("C4", "10uF", C0805),                                 # 3V3 bulk
    ],
    note=(15, 120, "Calcumaker 16 display — 7-seg RPN stack. 3 rows x 16 digits "
          "(top row U3/DS9-12 optional => 2- or 3-row build). PER ROW: 1x TM1640 "
          "(C5337152) drives 4x FJ5161AH 0.56\" 4-digit common-cathode (C8093) "
          "over a 2-wire bus. *** DIGITS ARE THROUGH-HOLE (no SMD multi-digit "
          "7-seg on LCSC) -> THT assembly. *** TM1640: shared CLK, per-chip DIN "
          "(DIN1/2/3) from the interconnect; GRID->digit commons, SEG->segments. "
          "LED current dominates active power (drawn from +3V3 across the "
          "interconnect) -> gates the main buck-boost sizing. Use brightness "
          "(TM1640 dimming) + blank-on-idle."))

# ============================ Interconnect sheet =============================
# Connector back to the main board. Pinout MUST match calcumaker-main J3.
# 2.54mm 1x8 header (research): PZ254V-11-08P (C492407, straight) or PZ254R
# (C492416, right-angle for a fixed display angle); join boards with a short
# ribbon/cable for the upward-angled mount.
INTERCONNECT = dict(name="Interconnect", file="interconnect.kicad_sch",
    title="Main board interconnect", page="3", big=[
        dict(ref="J1", lib_id="Connector_Generic:Conn_01x08", value="TO MAIN",
             fp="Connector_PinHeader_2.54mm:PinHeader_1x08_P2.54mm_Vertical",
             lcsc="C492407", mpn="PZ254V-11-08P", mfr="XKB"),
    ],
    small=[
        C("C5", "10uF", C0805),   # local 3V3 bulk at the connector
    ],
    note=(15, 95, "Calcumaker 16 display — Interconnect to the main board. J1 "
          "pinout (MUST match calcumaker-main J3): 1=+3V3, 2=GND, 3=CLK (shared), "
          "4=DIN1, 5=DIN2, 6=DIN3, 7=GND, 8=spare. (TM1640 2-wire: one shared "
          "CLK + one DIN per row driver U1/U2/U3.) Wide +3V3/GND for LED "
          "current. 2.54mm 1x8 header (C492407); short cable to the angled "
          "display. See DESIGN.md Board Partition."))

# ============================ generate =======================================
K.build(
    project="calcumaker-display", proj_dir=PROJ_DIR, root_uuid=ROOT_UUID,
    title=dict(title="Calcumaker 16 — Display", date="2026-06-21", rev="0.1",
               company="calcumaker authors",
               comments=["Programmer's/technical arbitrary-precision RPN calculator",
                         "Display board: 7-seg RPN stack + driver + interconnect (DRAFT)"]),
    sheets=[DISPLAY, INTERCONNECT],
)
