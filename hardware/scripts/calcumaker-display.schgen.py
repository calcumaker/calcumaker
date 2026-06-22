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
K.register_stdlib("Connector_Generic", "Conn_01x06")
# TODO(display): register the driver + 7-seg symbols once chosen by availability:
#   K.register_stdlib("Display_Driver", "MAX7219")     # or HT16K33 / TM1640 ...
#   K.register_stdlib("Display_7Segment", "...")       # per chosen digit module

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


# ============================ Display sheet (TODO parts) =====================
# Multi-row 7-seg RPN stack: 3 rows laid out, top row optionally populated (=> 2
# or 3 visible rows), each row ~12-16 digits. Driver + digits PENDING the
# availability research (DESIGN.md Display). Skeleton shows per-chip support
# passives only; add the real driver chip(s) + digit modules when chosen.
DISPLAY = dict(name="Display", file="display.kicad_sch",
    title="7-seg RPN stack (2-3 rows) + driver", page="2",
    big=[
        # TODO(display): N driver chips (e.g. MAX7219 cascade over SPI), +
        # the 7-seg digit modules tiling 2-3 rows of ~12-16 digits.
        # dict(ref="U1", lib_id="Display_Driver:MAX7219", value="MAX7219", ...),
    ],
    small=[
        # Per driver chip: 100nF + 10uF bulk + ISET resistor (segment current).
        C("C1", "100nF"), C("C2", "10uF", C0805),
        R("R1", "10k"),   # ISET (segment current) — value per driver datasheet
    ],
    note=(15, 120, "Calcumaker 16 display — multi-row 7-seg RPN stack (2-3 rows; "
          "board laid out for 3, top row optional). PENDING driver + digit parts "
          "(DESIGN.md Display, chosen by LCSC availability). Add N driver chips + "
          "digit modules (rows = top of stack), per-chip 100nF+10uF + ISET. "
          "*** LED current dominates active power — budget it; it is drawn from "
          "+3V3 across the interconnect, so it gates the main board's buck-boost "
          "sizing. ***"))

# ============================ Interconnect sheet (TODO) ======================
# Connector back to the main board. Pinout MUST match calcumaker-main J3.
INTERCONNECT = dict(name="Interconnect", file="interconnect.kicad_sch",
    title="Main board interconnect", page="3", big=[
        dict(ref="J1", lib_id="Connector_Generic:Conn_01x06", value="TO MAIN",
             fp="Connector_PinHeader_2.54mm:PinHeader_1x06_P2.54mm_Vertical"),  # TODO: FFC/FPC vs header (availability)
    ],
    small=[
        C("C3", "10uF", C0805),   # local 3V3 bulk at the connector
    ],
    note=(15, 95, "Calcumaker 16 display — Interconnect to the main board. J1 "
          "pinout (MUST match calcumaker-main J3): 1=+3V3, 2=GND, 3=SPI SCK, "
          "4=SPI MOSI, 5=CS/LOAD, 6=BLANK/spare. Wide +3V3/GND (LED current). "
          "Connector type/part PENDING availability research. See DESIGN.md "
          "Board Partition."))

# ============================ generate =======================================
K.build(
    project="calcumaker-display", proj_dir=PROJ_DIR, root_uuid=ROOT_UUID,
    title=dict(title="Calcumaker 16 — Display", date="2026-06-21", rev="0.1",
               company="calcumaker authors",
               comments=["Programmer's/technical arbitrary-precision RPN calculator",
                         "Display board: 7-seg RPN stack + driver + interconnect (DRAFT)"]),
    sheets=[DISPLAY, INTERCONNECT],
)
