#!/usr/bin/env python3
"""Regenerate the Calcumaker 16 **display board** schematic — a KiCad
*multi-channel* design.

    CALCUMAKER_SCHGEN_DRAFT_OK=1 python3 scripts/calcumaker-display.schgen.py
    (or: CALCUMAKER_SCHGEN_DRAFT_OK=1 make gen-calcumaker-display)

The display board is a SEPARATE PCB (split design — it angles up; only the
display serial bus + power cross the interconnect from the MCU board). It holds
the multi-row 7-segment RPN stack (**2-3 rows**; the board is laid out for 3
rows with the top row optionally populated) plus its driver ICs and the
interconnect back to the MCU board.

**Multi-channel structure.** Every row is electrically identical — one TM1640
driving 16 common-cathode digits over the shared 8-segment bus. So the row is
authored ONCE as a reusable, fully-wired child sheet (``display_row.kicad_sch``)
and instantiated **three times** at the root (Row1/Row2/Row3), each annotating
to its own reference designators (U1/DS1-16, U2/DS17-32, U3/DS33-48). This kills
the old redundancy (three hand-copied rows) and the off-page sprawl, and means a
wiring fix in one row propagates to all three.

Digit = **FJ5161AH** (LCSC C8093): a *single-digit* 0.56" common-cathode THT
7-segment (confirmed via LCSC — NOT a 4-digit module; an earlier scaffold
wrongly mapped it to the 4-digit ``CC56-12`` symbol/footprint/3D, which is where
the phantom "clock colon" came from). 16 digits/row => **48 digits** total. The
symbol is authored in ``lib/symbols/calcumaker.kicad_sym`` (standard 5161
pinout); the land is the dimensionally-matched 0.56" single-digit ``LTS6760``.

This is DATA; the engine is scripts/kschgen.py. Verify each lib_id/footprint
exists in your KiCad 10 install before relying on it.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kschgen as K

# --- DRAFT guard -------------------------------------------------------------
if not os.environ.get("CALCUMAKER_SCHGEN_DRAFT_OK"):
    sys.exit(
        "calcumaker-display.schgen.py is a DRAFT (display parts pending final "
        "layout/Isat/pinout verification — see DESIGN.md Display). Set "
        "CALCUMAKER_SCHGEN_DRAFT_OK=1 to generate anyway."
    )

HW = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))   # hardware/
PROJ_DIR = os.path.join(HW, "calcumaker-display")
PROJECT = "calcumaker-display"
PAPER_ROOT = "A3"
PAPER_ROW = "A2"          # a full 16-digit row + its labels wants the bigger sheet

# ---- stable UUIDs (only ROOT is truly load-bearing; the rest keep regen diffs
#      readable). Component element UUIDs still churn per regen (uuid4).
ROOT_UUID = "ca1c0000-0000-4000-8000-0000000d1501"
ROW_FILE  = "ca1c0000-0000-4000-8000-0000000d1510"   # display_row.kicad_sch file id
R1        = "ca1c0000-0000-4000-8000-0000000d1511"   # Row1 sheet symbol
R2        = "ca1c0000-0000-4000-8000-0000000d1512"
R3        = "ca1c0000-0000-4000-8000-0000000d1513"
IC        = "ca1c0000-0000-4000-8000-0000000d1520"   # interconnect sheet + file
AX        = "ca1c0000-0000-4000-8000-0000000d1530"   # aux sheet + file
ROWS      = [R1, R2, R3]

# ---- symbol libraries -------------------------------------------------------
K.register_stdlib("Connector_Generic", "Conn_01x12", "Conn_01x04")
K.register_lib("calcumaker",
               os.path.join(HW, "lib", "symbols", "calcumaker.kicad_sym"),
               "TM1640", "FJ5161AH")
K.register_stdlib("Device", "C")

# ---- footprint shorthands ---------------------------------------------------
C0402 = "Capacitor_SMD:C_0402_1005Metric"
C0603 = "Capacitor_SMD:C_0603_1608Metric"
TM1640_FP = "Package_SO:SOIC-28W_7.5x18.7mm_P1.27mm"   # verify vs TM1640 SOP-28
DIGIT_FP  = "Display_7Segment:7SegmentLED_LTS6760_LTS6780"  # 0.56" single-digit land
TITLE = dict(title="Calcumaker 16 — Display", date="2026-07-04", rev="0.2",
             company="calcumaker authors",
             comments=["Programmer's/technical arbitrary-precision RPN calculator",
                       "Display board: multi-channel 7-seg RPN stack + drivers + interconnect (DRAFT)"])

# segment (FJ5161AH pin name) -> TM1640 SEG line. a..g,dp = SEG1..SEG8 (matches
# firmware seg7 bit order: bit0=a .. bit7=dp -> TM1640 SEG1..SEG8).
SEGMAP = {"A": "SEG1", "B": "SEG2", "C": "SEG3", "D": "SEG4",
          "E": "SEG5", "F": "SEG6", "G": "SEG7", "DP": "SEG8"}

G = 2.54          # 100-mil grid; placements MUST snap to it so pin endpoints,
                  # stubs and labels stay on the 50-mil ERC grid.


def g(n):
    return n * G


def _C(value, fp):
    return dict(lib_id="Device:C", value=value, fp=fp,
                lcsc={"10uF": "C15850", "100nF": "C1525"}.get(value, ""))


# ============================ reusable ROW sheet =============================
# One TM1640 + 16 single-digit FJ5161AH, fully wired. Instantiated x3 at root.
def build_row():
    comps = []      # (comp_dict, [(path, ref), ...])
    wiring = ""

    def paths_refs(fn):
        return [(f"/{ROOT_UUID}/{ROWS[i]}", fn(i)) for i in range(3)]

    # --- driver -------------------------------------------------------------
    U = dict(lib_id="calcumaker:TM1640", value="TM1640", fp=TM1640_FP,
             lcsc="C5337152", mpn="TM1640", mfr="TitanMicro", x=g(25), y=g(60))
    comps.append((U, paths_refs(lambda i: f"U{i+1}")))
    for n in range(1, 9):
        wiring += K.net_pin(U, f"SEG{n}", f"SEG{n}")                  # left bus
    for n in range(1, 17):
        wiring += K.net_pin(U, f"GRID{n}", f"GRID{n}")               # right, per digit
    wiring += K.net_pin(U, "VDD", "+5V", kind="glabel")
    wiring += K.net_pin(U, "VSS", "GND", kind="glabel")
    wiring += K.net_pin(U, "SCLK", "DISP_CLK", kind="glabel")         # shared clock
    wiring += K.net_pin(U, "DIN", "DIN", kind="hlabel", shape="input")  # per-row -> sheet pin

    # --- 16 digits (8 across x 2 down) --------------------------------------
    for k in range(1, 17):
        col, row = (k - 1) % 8, (k - 1) // 8
        dg = dict(lib_id="calcumaker:FJ5161AH", value="FJ5161AH", fp=DIGIT_FP,
                  lcsc="C8093", mpn="FJ5161AH", mfr="Shenzhen Zhihao",
                  x=g(52 + col * 22), y=g(36 + row * 35))
        comps.append((dg, paths_refs(lambda i, kk=k: f"DS{kk + 16 * i}")))
        for seg, net in SEGMAP.items():
            wiring += K.net_pin(dg, seg, net)          # segments -> shared SEG bus
        wiring += K.net_pin(dg, 3, f"GRID{k}")         # both commons -> this digit's grid
        wiring += K.net_pin(dg, 8, f"GRID{k}")

    # --- decoupling + bulk per row ------------------------------------------
    cdec = dict(_C("100nF", C0402), x=g(16), y=g(78))    # below the driver, clear of its labels
    comps.append((cdec, paths_refs(lambda i: f"C{i+1}")))          # C1/C2/C3
    wiring += K.net_pin(cdec, 1, "+5V", kind="glabel")
    wiring += K.net_pin(cdec, 2, "GND", kind="glabel")
    cbulk = dict(_C("10uF", C0603), x=g(26), y=g(78))
    comps.append((cbulk, paths_refs(lambda i: f"C{i+4}")))         # C4/C5/C6
    wiring += K.net_pin(cbulk, 1, "+5V", kind="glabel")
    wiring += K.net_pin(cbulk, 2, "GND", kind="glabel")

    note = (20.0, 240.0, K.note_block(
        "REUSABLE ROW  (multi-channel: instantiated x3 as Row1/Row2/Row3)",
        "  Row1 -> U1 / DS1-16     Row2 -> U2 / DS17-32     Row3 -> U3 / DS33-48",
        "",
        "One TM1640 (C5337152) drives 16x FJ5161AH single-digit 0.56\" common-",
        "cathode (C8093, THROUGH-HOLE):",
        "  SEG1..8 (a..g,dp)  -  shared 8-line bus to all 16 digits' segments",
        "  GRID1..16          -  one per digit  (GRID k -> digit k cathode)",
        "  VDD  -> +5V              VSS  -> GND",
        "  SCLK -> DISP_CLK (global net, shared by all 3 rows)",
        "  DIN  -> hierarchical pin (Row1<-DIN1, Row2<-DIN2, Row3<-DIN3 @ root)",
        "",
        "Runs at +5V (VDD + LED) from the MCU-board EN-gated 5V boost.",
        "Digit->GRID is 1:1 sequential; firmware seg7/App is the source of truth.",
        "*** THT digits (no SMD multi-digit 7-seg on LCSC). ***",
        "Verify FJ5161AH pad map vs the LTS6760 0.56\" land before fab."))
    return dict(uuid=ROW_FILE, file="display_row.kicad_sch", page="2",
                title="Reusable 7-seg row (1x TM1640 + 16 digits)",
                comps=comps, wiring=wiring, notes=[note], _dir=PROJ_DIR)


# ============================ interconnect sheet ============================
def build_interconnect():
    path = f"/{ROOT_UUID}/{IC}"
    # 0.5mm 12P FFC to the MCU board. The flat-flex CABLE is a NON-ASSEMBLED
    # DigiKey accessory: GCT FFC05-TIN series, 12-position -> PN
    # 05-12-A-<length>-A-4-06-4-T ("-T"=tin). LENGTH + contact orientation
    # ("A"=same-side / "D"=opposite-side) set at layout. Power conductors DOUBLED
    # for LED current:
    # a single 0.5mm FFC conductor is ~0.4A and the 3 multiplexed TM1640s peak
    # ~0.3-0.5A on +5V (less with brightness capping), so 2x+5V + 3x GND.
    J1 = dict(lib_id="Connector_Generic:Conn_01x12", value="TO MCU (FFC)",
              fp="Connector_FFC-FPC:Hirose_FH12-12S-0.5SH_1x12-1MP_P0.50mm_Horizontal",
              lcsc="C262661", mpn="AFC01-S12FCA-00", mfr="JUSHUO", x=g(28), y=g(28))
    # J1 pinout (MUST match calcumaker-mcu J3).
    j1nets = {1: "+5V", 2: "+5V", 3: "GND", 4: "DISP_CLK", 5: "DIN1", 6: "DIN2",
              7: "DIN3", 8: "GND", 9: "+3V3", 10: "SDA", 11: "SCL", 12: "GND"}
    wiring = ""
    for pin, net in j1nets.items():
        wiring += K.net_pin(J1, pin, net, kind="glabel")
    C7 = dict(_C("10uF", C0603), x=g(48), y=g(28))
    wiring += K.net_pin(C7, 1, "+5V", kind="glabel")
    wiring += K.net_pin(C7, 2, "GND", kind="glabel")
    comps = [(J1, [(path, "J1")]), (C7, [(path, "C7")])]
    note = (20.0, 120.0, K.note_block(
        "DISPLAY INTERCONNECT  -  J1  AFC01-S12FCA-00  (LCSC C262661)",
        "0.5mm 12-pos FFC to the MCU board.  Pinout MUST match mcu J3.",
        "",
        K.pin_table([(1, "+5V"), (2, "+5V"), (3, "GND"), (4, "CLK (shared)"),
                     (5, "DIN1"), (6, "DIN2"), (7, "DIN3"), (8, "GND"),
                     (9, "+3V3"), (10, "SDA"), (11, "SCL"), (12, "GND")]),
        "",
        "+5V/GND doubled: a 0.5mm FFC conductor is ~0.4A; 3x TM1640 peak",
        "~0.3-0.5A on +5V (less with brightness capping).",
        "CLK/DIN1-3 are 5V-logic from the MCU-board 74HCT125.",
        "Pins 9-11 (+3V3/SDA/SCL) feed the DNP aux OLED (unused if bare).",
        "",
        "CABLE (non-BOM): GCT FFC05-TIN  05-12-A-<len>-A-4-06-4-T  (DigiKey);",
        "                 length + A(same-side)/D(opposite) contacts TBD @ layout.",
        "Verify the FFC land vs the AFC01 datasheet at layout.  See DESIGN.md."))
    return dict(uuid=IC, file="interconnect.kicad_sch", page="6",
                title="MCU board interconnect (0.5mm FFC)", comps=comps,
                wiring=wiring, notes=[note], _dir=PROJ_DIR)


# ====================== aux display sheet (DNP-optional) ====================
def build_aux():
    path = f"/{ROOT_UUID}/{AX}"
    J2 = dict(lib_id="Connector_Generic:Conn_01x04", value="OLED 128x32 (DNP)",
              fp="Connector_PinHeader_2.54mm:PinHeader_1x04_P2.54mm_Vertical",
              lcsc="C2691448", mpn="PZ254V-11-04P", mfr="XKB", dnp=True,
              x=g(28), y=g(28))
    j2nets = {1: "+3V3", 2: "GND", 3: "SCL", 4: "SDA"}
    wiring = ""
    for pin, net in j2nets.items():
        wiring += K.net_pin(J2, pin, net, kind="glabel")
    note = (20.0, 110.0, K.note_block(
        "AUX OLED  -  J2   (OPTIONAL, DNP by default)",
        "Receives a 0.91\" SSD1306 128x32 I2C module (sourced separately,",
        "hand-placed with the THT digits).  I2C at 3V3 straight from the MCU",
        "(pull-ups on the MCU board, DNP alongside).",
        "",
        K.pin_table([(1, "VCC <- +3V3"), (2, "GND"), (3, "SCL"), (4, "SDA")], cols=1),
        "",
        "Shows full error text / SETUP / STATUS; the 7-seg glass stays primary."))
    return dict(uuid=AX, file="aux.kicad_sch", page="7",
                title="Aux OLED 128x32 (SSD1306 I2C) — DNP-optional",
                comps=[(J2, [(path, "J2")])], wiring=wiring, notes=[note],
                _dir=PROJ_DIR)


# ============================ root sheet ====================================
def build_root_strings():
    # 3 reused row instances + interconnect + aux, with per-row DIN routed to
    # DIN1/2/3 global labels.
    sym = ""
    wiring = ""
    for i, (uuid, name) in enumerate([(R1, "Row1"), (R2, "Row2"), (R3, "Row3")]):
        x, y = g(28), g(12 + i * 12)
        w, h = g(24), g(8)
        py = y + g(4)
        sym += K.w_sheet(name, "display_row.kicad_sch", uuid, x, y, w, h,
                         pins=[("DIN", "input", x, py, 180)])
        wiring += K.w_wire(x, py, x - g(3), py)
        wiring += K.w_glabel(f"DIN{i+1}", x - g(3), py, 180, shape="output")
    sym += K.w_sheet("Interconnect", "interconnect.kicad_sch", IC,
                     g(64), g(12), g(20), g(8), pins=[])
    sym += K.w_sheet("AuxDisplay", "aux.kicad_sch", AX,
                     g(64), g(24), g(20), g(8), pins=[])
    wiring += K.text_note(
        "Calcumaker 16 — Display (MULTI-CHANNEL). Row1/2/3 are three instances "
        "of ONE reusable sheet (display_row.kicad_sch). Shared bus is carried on "
        "global nets +5V / GND / DISP_CLK; each row's serial data DIN is a "
        "hierarchical pin fed by DIN1/DIN2/DIN3 from the interconnect. Top row "
        "(Row3 / U3 / DS33-48) is optional -> 2- or 3-row build.", 20.0, 105.0)
    pro_sheets = [[R1, "Row1"], [R2, "Row2"], [R3, "Row3"],
                  [IC, "Interconnect"], [AX, "AuxDisplay"]]
    return sym, wiring, pro_sheets


# ============================ generate ======================================
row = build_row()
interconnect = build_interconnect()
aux = build_aux()

print("child sheets:")
K.write_wired_child(row, PROJECT, ROOT_UUID, TITLE, PAPER_ROW)
K.write_wired_child(interconnect, PROJECT, ROOT_UUID, TITLE, PAPER_ROOT)
K.write_wired_child(aux, PROJECT, ROOT_UUID, TITLE, PAPER_ROOT)

sym, wiring, pro_sheets = build_root_strings()
K.write_root(PROJECT, PROJ_DIR, ROOT_UUID, TITLE, sym, wiring, pro_sheets,
             paper=PAPER_ROOT)

# remove the now-obsolete flat display.kicad_sch (replaced by display_row.kicad_sch)
_old = os.path.join(PROJ_DIR, "display.kicad_sch")
if os.path.exists(_old):
    os.remove(_old)
    print("removed obsolete display.kicad_sch (replaced by display_row.kicad_sch)")
