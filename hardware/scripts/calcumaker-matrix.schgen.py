#!/usr/bin/env python3
"""Regenerate the Calcumaker 16 **RGB dot-matrix display board** — a KiCad
*nested multi-channel* design.

    CALCUMAKER_SCHGEN_DRAFT_OK=1 KSCHGEN_FORCE=1 python3 scripts/calcumaker-matrix.schgen.py
    (or: CALCUMAKER_SCHGEN_DRAFT_OK=1 KSCHGEN_FORCE=1 make gen-calcumaker-matrix)

*** DRAFT ***
An alternative display *module* to the 7-seg board: a full-color addressable-RGB
dot matrix. It plugs into the SAME unified SPI connector on the MCU board as the
7-seg board (they are interchangeable), and carries its OWN brain — an **RP2040**
(its PIO is the ideal WS2812 engine) — that receives semantic "display intent"
over SPI and renders it locally into the pixel array. So the MCU board speaks ONE
protocol regardless of which display is attached.

**Pixel** = **XL-1010RGBC-2812B-S** (LCSC C51900942): a 1x1mm WS2812/SK6812-
protocol addressable RGB LED, 3.5-5.5V (runs straight off VSYS, no boost). The
KiCad symbol is the stock ``LED:SK6812`` (value/fp/lcsc overridden — kschgen does
not resolve ``extends``); the land is the authored ``calcumaker:LED_XL1010RGBC_
1.0x1.0mm`` (pad numbers 1=VSS 2=DIN 3=VDD 4=DOUT, matching SK6812).

**Nested multi-channel structure (cluster -> row -> board).**
  * a **cluster** = an 8x8 = 64-LED block, authored ONCE as a reusable, fully-
    wired child sheet ``led_cluster.kicad_sch`` (LEDs chained DIN->DOUT).
  * a **row** = a reusable child sheet ``led_row.kicad_sch`` that instantiates the
    cluster **12 times** (chained cluster-to-cluster) = 96x8 px = one stack row.
  * the **board** = the row instantiated **3 times** at the root = 96x24 = 2304 px.
So the leaf pixel is defined once, and repeats 3x12 = 36 times via the nested
sheet instances (paths /root/Row_r/Cluster_c). Refs D1..D2304. Each row is one
**data chain** (768 LEDs) driven by one RP2040 PIO line via the quad level shifter.

**Power.** LED current (amps at brightness) does NOT cross the signal FFC: the
board takes a dedicated 2-pin VSYS inlet (J2) from the MCU-board PSU, gated by a
high-side load switch (LED_EN off in sleep). Firmware MUST cap total brightness.

This is DATA; the engine is scripts/kschgen.py. Verify each lib_id/footprint
exists in your KiCad 10 install before relying on it.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kschgen as K

# --- DRAFT guard -------------------------------------------------------------
if not os.environ.get("CALCUMAKER_SCHGEN_DRAFT_OK"):
    sys.exit(
        "calcumaker-matrix.schgen.py is a DRAFT (1mm-pitch land + RP2040 min-"
        "system + pixel count pending layout/JLC-assembly quote). Set "
        "CALCUMAKER_SCHGEN_DRAFT_OK=1 to generate anyway."
    )

HW = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))   # hardware/
PROJ_DIR = os.path.join(HW, "calcumaker-matrix")
PROJECT = "calcumaker-matrix"
PAPER_ROOT = "A3"
PAPER_CLUSTER = "A3"
PAPER_ROW = "A3"

# ---- grid / tiling ----------------------------------------------------------
CW, CH = 8, 8              # pixels per cluster (8x8)
NCLUST = 12               # clusters per row  -> 96 px wide
NROWS = 3                 # rows on the board -> 24 px tall, 3 data chains
PER_CLUSTER = CW * CH      # 64

# ---- stable UUIDs (last group = 0000000d3xxx; existing files keep on-disk ids) --
def _u(suffix):
    return f"ca1c0000-0000-4000-8000-0000000d3{suffix}"
ROOT_UUID    = _u("001")
CLUSTER_FILE = _u("010")                                   # led_cluster.kicad_sch file id
ROW_FILE     = _u("020")                                   # led_row.kicad_sch file id
ROW_INST     = [_u(f"02{r + 1}") for r in range(NROWS)]    # Row1..Row3 (sheet-symbol ids at root)
CLUST_INST   = [_u(f"{0x030 + c:03x}") for c in range(NCLUST)]  # Cluster1..12 (ids inside led_row)
RP2040_U     = _u("200")
RGBPOWER     = _u("210")
MATRIX_IF    = _u("220")
AUX          = _u("230")

# ---- symbol libraries -------------------------------------------------------
K.register_stdlib("Device", "R", "C", "Crystal")
K.register_stdlib("LED", "SK6812")                     # pixel (base sym; value/fp overridden to XL-1010)
K.register_stdlib("MCU_RaspberryPi", "RP2040")         # STOCK symbol (QFN-56)
K.register_stdlib("Memory_Flash", "W25Q32JVSS")        # RP2040 boot/code QSPI flash
K.register_stdlib("74xx", "74LVC125")                  # quad 3V3->VLED data level shifter
K.register_stdlib("Transistor_FET", "Q_PMOS_GSD", "Q_NMOS_GSD")   # LED-rail load switch
K.register_stdlib("Power_Protection", "USBLC6-2SC6")   # USB ESD (BOOTSEL/DFU port)
K.register_stdlib("Switch", "SW_Push")                 # BOOTSEL button
K.register_stdlib("Connector", "USB_C_Receptacle_USB2.0_16P")     # J4 program (BOOTSEL/DFU)
K.register_stdlib("Connector_Generic", "Conn_01x12", "Conn_01x04", "Conn_01x02")

# ---- footprint shorthands ---------------------------------------------------
R0402 = "Resistor_SMD:R_0402_1005Metric"
C0402 = "Capacitor_SMD:C_0402_1005Metric"
C0603 = "Capacitor_SMD:C_0603_1608Metric"
SOT23 = "Package_TO_SOT_SMD:SOT-23"
SOT236 = "Package_TO_SOT_SMD:SOT-23-6"
PIXEL_FP = "calcumaker:LED_XL1010RGBC_1.0x1.0mm"        # authored 1x1mm 4-pad land
RP2040_FP = "Package_DFN_QFN:QFN-56-1EP_7x7mm_P0.4mm_EP3.2x3.2mm"   # STOCK
FLASH_FP = "Package_SO:SOIC-8_5.3x5.3mm_P1.27mm"
XTAL_FP = "Crystal:Crystal_SMD_3225-2Pin_3.2x2.5mm"
SHIFT_FP = "Package_SO:SOIC-14_3.9x8.7mm_P1.27mm"       # 74LVC125 quad
BTN_FP = "Button_Switch_SMD:SW_SPST_TL3342"
USBC_FP = "Connector_USB:USB_C_Receptacle_GCT_USB4105-xx-A_16P_TopMnt_Horizontal"
FFC12_FP = "Connector_FFC-FPC:Hirose_FH12-12S-0.5SH_1x12-1MP_P0.50mm_Horizontal"
JST2_FP = "Connector_JST:JST_PH_S2B-PH-K_1x02_P2.00mm_Horizontal"
OLED_FP = "Connector_PinHeader_2.54mm:PinHeader_1x04_P2.54mm_Vertical"

RLCSC = {"27": "C25100", "1k": "C11702", "10k": "C25744", "4.7k": "C25900", "100k": "C25741"}
CLCSC = {"100nF": "C1525", "1uF": "C29266", "10uF": "C15850", "22uF": "C45783", "15pF": ""}

# ---- UNIFIED display-module connector pinout (MUST match mcu J3 + 7-seg J1) --
UNIFIED_PINS = {1: "VSYS", 2: "VSYS", 3: "GND", 4: "GND", 5: "+3V3",
                6: "SPI_SCLK", 7: "SPI_MOSI", 8: "SPI_CS",
                9: "DISP_IRQ", 10: "DISP_NRST", 11: "DISP_BOOT", 12: "GND"}

TITLE = dict(title="Calcumaker 16 — RGB Matrix", date="2026-07-07", rev="0.1",
             company="calcumaker authors",
             comments=["Programmer's/technical arbitrary-precision RPN calculator",
                       "RGB dot-matrix display module: 96x24 1mm WS2812 (cluster->row->board) + RP2040 + unified SPI (DRAFT)"])

G = 2.54


def g(n):
    return n * G


def R(ref, val, x=0, y=0):
    return dict(ref=ref, lib_id="Device:R", value=val, fp=R0402,
                lcsc=RLCSC.get(val, ""), x=x, y=y)


def C(ref, val, fp=C0402, x=0, y=0):
    return dict(ref=ref, lib_id="Device:C", value=val, fp=fp,
                lcsc=CLCSC.get(val, ""), x=x, y=y)


def place1(path, specs, x0=g(8), y0=g(14), dx=g(16), dy=g(14), per=6):
    comps = []
    for idx, sp in enumerate(specs):
        ref = sp["ref"]
        c = {k: v for k, v in sp.items() if k != "ref"}
        c["x"] = x0 + (idx % per) * dx
        c["y"] = y0 + (idx // per) * dy
        comps.append((c, [(path, ref)]))
    return comps


# ============= LEVEL 1: reusable 8x8 LED CLUSTER (leaf, nested) ===============
# 64 XL-1010 chained DIN->DOUT. Instantiated 3 rows x 12 clusters = 36 times, so
# each LED symbol carries all 36 nested instance paths /root/Row_r/Cluster_c.
def build_led_cluster():
    comps = []
    wiring = ""

    def pr(fn):   # nested instance paths: for every (row r, cluster c)
        return [(f"/{ROOT_UUID}/{ROW_INST[r]}/{CLUST_INST[c]}", fn(r, c))
                for r in range(NROWS) for c in range(NCLUST)]

    for j in range(1, PER_CLUSTER + 1):
        cc, rr = (j - 1) % CW, (j - 1) // CW
        led = dict(lib_id="LED:SK6812", value="XL-1010RGBC-2812B-S", fp=PIXEL_FP,
                   lcsc="C51900942", mpn="XL-1010RGBC-2812B-S", mfr="XINGLIGHT",
                   x=g(8 + cc * 6), y=g(10 + rr * 6))
        # ref: global cluster index gc = r*12 + c ; 64 LEDs per cluster
        comps.append((led, pr(lambda r, c, jj=j: f"D{(r * NCLUST + c) * PER_CLUSTER + jj}")))
        wiring += K.net_pin(led, "VDD", "VLED", kind="glabel")
        wiring += K.net_pin(led, "VSS", "GND", kind="glabel")
        if j == 1:
            wiring += K.net_pin(led, "DIN", "DIN", kind="hlabel", shape="input")
        else:
            wiring += K.net_pin(led, "DIN", f"CN{j}", kind="label")
        if j == PER_CLUSTER:
            wiring += K.net_pin(led, "DOUT", "DOUT", kind="hlabel", shape="output")
        else:
            wiring += K.net_pin(led, "DOUT", f"CN{j + 1}", kind="label")

    note = (g(4), g(66), K.note_block(
        "REUSABLE 8x8 LED CLUSTER  (leaf of the nested design)",
        "Instantiated 3 rows x 12 clusters = 36x  ->  paths /root/Row_r/Cluster_c.",
        "Refs D1..D2304: cluster gc = r*12+c  ->  D(64*gc+1) .. D(64*gc+64).",
        "",
        "64x XL-1010RGBC-2812B-S (C51900942, 1x1mm WS2812/SK6812, 3.5-5.5V):",
        "  DIN -> LED1 -> ... -> LED64 -> DOUT  (serpentine; DIN/DOUT = hier pins,",
        "         chained cluster-to-cluster in led_row, row-to-row at the root)",
        "  VDD -> VLED (gated LED rail);  VSS -> GND",
        "",
        "Symbol = stock LED:SK6812 (value/fp/lcsc overridden). Land = authored",
        "calcumaker:LED_XL1010RGBC_1.0x1.0mm (pads 1=VSS 2=DIN 3=VDD 4=DOUT).",
        "*** 1mm/1010 fine pitch -> 4-LAYER board (VLED+GND planes) + fine JLC",
        "    placement; VERIFY the 1010 land vs the XINGLIGHT datasheet + get a",
        "    JLCPCB assembly quote (2304 placements) EARLY. ***"))
    return dict(uuid=CLUSTER_FILE, file="led_cluster.kicad_sch", page="2",
                title="Reusable 8x8 RGB cluster (64x XL-1010)",
                comps=comps, wiring=wiring, notes=[note], _dir=PROJ_DIR)


# ============= LEVEL 2: reusable ROW = 12 clusters (nested x3) ================
# Instantiates led_cluster 12x, chained DIN->DOUT, exposing the row's own DIN/DOUT
# hier pins. Reused 3x at the root (one per stack row / data chain). comps=[] —
# its body is sub-sheets, injected via the wiring string.
def build_led_row():
    wiring = ""
    for c in range(NCLUST):
        cx, cy = g(6 + c * 14), g(20)
        cw, ch = g(10), g(12)
        dpy, opy = cy + g(4), cy + g(8)
        wiring += K.w_sheet(f"Cluster{c + 1}", "led_cluster.kicad_sch", CLUST_INST[c],
                            cx, cy, cw, ch,
                            pins=[("DIN", "input", cx, dpy, 180),
                                  ("DOUT", "output", cx + cw, opy, 0)])
        # DIN side: cluster 0 = row DIN (hier); else the intra-row chain node RC{c}
        wiring += K.w_wire(cx, dpy, cx - g(2), dpy)
        if c == 0:
            wiring += K.w_hlabel("DIN", cx - g(2), dpy, 180, shape="input")
        else:
            wiring += K.w_label(f"RC{c}", cx - g(2), dpy, 180)
        # DOUT side: cluster 11 = row DOUT (hier); else RC{c+1}
        wiring += K.w_wire(cx + cw, opy, cx + cw + g(2), opy)
        if c == NCLUST - 1:
            wiring += K.w_hlabel("DOUT", cx + cw + g(2), opy, 0, shape="output")
        else:
            wiring += K.w_label(f"RC{c + 1}", cx + cw + g(2), opy, 0)
    note = K.text_note(K.note_block(
        "REUSABLE ROW  (led_row.kicad_sch)  -  instantiated x3 at the root (Row1-3)",
        "= 12x led_cluster chained DIN -> Cluster1 -> ... -> Cluster12 -> DOUT.",
        "96 x 8 px = 768 LEDs = one WS2812 data chain (one RP2040 PIO line).",
        "VLED / GND are global nets (shared everywhere); only DIN/DOUT are hier."),
        g(6), g(40))
    return dict(uuid=ROW_FILE, file="led_row.kicad_sch", page="2",
                title="Reusable row = 12x 8x8 cluster (96x8 = 768 px)",
                comps=[], wiring=wiring + note, notes=[], _dir=PROJ_DIR)


# ===================== RP2040 min-system sheet (single instance) =============
def build_rp2040():
    path = f"/{ROOT_UUID}/{RP2040_U}"
    specs = [
        dict(ref="U1", lib_id="MCU_RaspberryPi:RP2040", value="RP2040", fp=RP2040_FP,
             lcsc="C2040", mpn="RP2040", mfr="Raspberry Pi"),
        dict(ref="U2", lib_id="Memory_Flash:W25Q32JVSS", value="W25Q32JVSSIQ",
             fp=FLASH_FP, lcsc="C179173", mpn="W25Q32JVSSIQ", mfr="Winbond"),
        dict(ref="U3", lib_id="Power_Protection:USBLC6-2SC6", value="USBLC6-2SC6",
             fp=SOT236, lcsc="C2687116", mpn="USBLC6-2SC6", mfr="STMicroelectronics"),
        dict(ref="Y1", lib_id="Device:Crystal", value="12MHz", fp=XTAL_FP),   # LCSC TBD @ BOM
        dict(ref="J4", lib_id="Connector:USB_C_Receptacle_USB2.0_16P", value="USB-C (BOOTSEL)",
             fp=USBC_FP, lcsc="C2927039", mpn="USB-TYPE-C-019", mfr="GCT"),
        dict(ref="SW1", lib_id="Switch:SW_Push", value="BOOTSEL", fp=BTN_FP),   # LCSC TBD @ BOM
        R("R1", "1k"), R("R2", "27"), R("R3", "27"), R("R4", "10k"),
        C("C1", "100nF"), C("C2", "100nF"), C("C3", "100nF"), C("C4", "100nF"),
        C("C5", "100nF"), C("C6", "100nF"), C("C7", "100nF"),        # IOVDD/USB/ADC/VREG_VIN decouple
        C("C8", "1uF"),                                              # DVDD (internal 1.1V LDO out)
        C("C9", "10uF", C0603), C("C10", "10uF", C0603),            # 3V3 bulk
        C("C11", "15pF"), C("C12", "15pF"),                         # Y1 load caps (LCSC TBD)
    ]
    note = (15, 165, K.note_block(
        "RP2040 MIN-SYSTEM  -  U1 RP2040 (C2040, QFN-56)  -  PLACED, not wired",
        "PIO drives the WS2812 chains; embassy-rp firmware (a 2nd MCU ecosystem).",
        "",
        "POWER  IOVDD/USBVDD/ADC_AVDD/VREG_VIN -> +3V3 (from unified connector);",
        "       C1-C7 100nF decouple; C9/C10 10uF bulk. DVDD = internal 1.1V LDO",
        "       out -> C8 1uF. VSS/EP -> GND.",
        "CLOCK  Y1 12MHz + C11/C12 15pF + R1 1k series on XIN (USB needs the xtal).",
        "FLASH  U2 W25Q32JVSSIQ (C179173) on QSPI (SD0-3/SCLK/CSn); R4 10k CSn",
        "       pull-up. RP2040 boots from it.",
        "USB    J4 USB-C (BOOTSEL/DFU drag-drop UF2 programming); U3 USBLC6 ESD;",
        "       R2/R3 27R series on D+/D-. SW1 = BOOTSEL button (hold at power-on).",
        "LINK   unified SPI slave <- SPI_SCLK/MOSI/CS (main MCU 'display intent');",
        "       DISP_IRQ out (ready); DISP_NRST -> RUN; DISP_BOOT -> BOOTSEL.",
        "OUT    3x LED data (one per stack row) + LED_EN -> RGBPower (U4 shifter).",
        "       Drive the aux OLED locally over I2C (AuxOLED J3) from OLED intent.",
        "Y1 12MHz + SW1 BOOTSEL: pick LCSC at BOM.  See DESIGN.md display-module IF."))
    return dict(uuid=RP2040_U, file="rp2040.kicad_sch", page="3",
                title="RP2040 min-system (MCU + QSPI flash + USB BOOTSEL)",
                comps=place1(path, specs), wiring="", notes=[note], _dir=PROJ_DIR)


# ===================== RGB power + data gate sheet (single instance) =========
def build_rgb_power():
    path = f"/{ROOT_UUID}/{RGBPOWER}"
    specs = [
        dict(ref="U4", lib_id="74xx:74LVC125", value="74LVC125", fp=SHIFT_FP,
             lcsc="C460512", mpn="74LVC125AS14-13", mfr="Diodes Incorporated"),
        dict(ref="Q1", lib_id="Transistor_FET:Q_PMOS_GSD", value="AO3401A",
             fp=SOT23, lcsc="C15127", mpn="AO3401A", mfr="AOS"),
        dict(ref="Q2", lib_id="Transistor_FET:Q_NMOS_GSD", value="2N7002",
             fp=SOT23, lcsc="C8545", mpn="2N7002", mfr="onsemi"),
        dict(ref="J2", lib_id="Connector_Generic:Conn_01x02", value="VSYS IN (LED pwr)",
             fp=JST2_FP, lcsc="C173752", mpn="S2B-PH-K-S", mfr="JST"),
        R("R5", "100k"), R("R6", "10k"), R("R7", "100k"), R("R8", "100k"),
        C("C13", "100nF"), C("C14", "22uF", C0603), C("C15", "22uF", C0603),
    ]
    note = (15, 130, K.note_block(
        "RGB POWER + DATA GATE  -  drives the 3 WS2812 chains (Row1-3)",
        "PLACED, not wired.  (The 2304 LEDs are in led_cluster, nested in led_row.)",
        "",
        "DATA   RP2040 3x LED data (3V3) -> U4 74LVC125 (3 of 4 buffers) ->",
        "       CH1_DATA / CH2_DATA / CH3_DATA (VLED level). U4 /OE -> GND; VCC->VLED.",
        "GATE   J2 VSYS (dedicated 2-pin inlet from the MCU-board PSU, ~3.5-4.7V)",
        "       -> Q1 AO3401A P-FET -> VLED. Q1 gate R5 100k pull-up to VSYS = OFF;",
        "       Q2 2N7002 pulls the gate low: RP2040 LED_EN -> R6 10k -> Q2 (R7 100k",
        "       pulldown = OFF at boot). LED_EN low in sleep -> LEDs + U4 fully off.",
        "BULK   C14/C15 22uF at VLED; ADD a bulk electrolytic (>=470uF) at layout.",
        "",
        "POWER BUDGET  2304x WS2812 at full white ~ many amps -> NEVER full white.",
        "  Firmware MUST enforce a global brightness/current cap; VLED comes from",
        "  the dedicated VSYS inlet (J2), NOT over the signal FFC. -S pixel = 3.5V",
        "  floor -> gate/dim on a VBAT sense when the cell is low.",
        "R8 100k spare (a ~330R data series may help; tune @ layout)."))
    return dict(uuid=RGBPOWER, file="rgb_power.kicad_sch", page="4",
                title="RGB power + data gate (quad level shift + VLED load switch)",
                comps=place1(path, specs), wiring="", notes=[note], _dir=PROJ_DIR)


# ===================== unified SPI connector sheet (single instance) =========
def build_matrix_if():
    path = f"/{ROOT_UUID}/{MATRIX_IF}"
    J1 = dict(lib_id="Connector_Generic:Conn_01x12", value="TO MCU (unified SPI FFC)",
              fp=FFC12_FP, lcsc="C262661", mpn="AFC01-S12FCA-00", mfr="JUSHUO",
              x=g(28), y=g(30))
    wiring = ""
    for pin, net in UNIFIED_PINS.items():
        wiring += K.net_pin(J1, pin, net, kind="glabel")
    note = (20.0, 130.0, K.note_block(
        "UNIFIED DISPLAY-MODULE CONNECTOR  -  J1  AFC01-S12FCA-00  (LCSC C262661)",
        "0.5mm 12-pos FFC to the MCU board. SAME connector + pinout as the 7-seg",
        "board's J1 and mcu J3 -> the two display modules are INTERCHANGEABLE.",
        "Technology-agnostic: power + SPI 'display intent' + reset/boot (no I2C,",
        "no display-specific lines). Distinct pinout from the old CLK/DIN display",
        "FFC and from the 16-pin keyboard link (can't cross-plug).",
        "",
        K.pin_table([(1, "VSYS"), (2, "VSYS"), (3, "GND"), (4, "GND"), (5, "+3V3"),
                     (6, "SPI_SCLK"), (7, "SPI_MOSI"), (8, "SPI_CS"), (9, "DISP_IRQ"),
                     (10, "DISP_NRST (RP2040 RUN)"), (11, "DISP_BOOT (RP2040 BOOTSEL)"),
                     (12, "GND")]),
        "",
        "VSYS here powers module logic only (LED current uses the separate J2 inlet).",
        "+3V3 -> RP2040. SPI = main MCU writes display intent; DISP_IRQ = module",
        "ready/attention. CABLE (non-BOM): GCT FFC05-TIN 05-12-A-<len>-A-4-06-4-T."))
    return dict(uuid=MATRIX_IF, file="matrix_if.kicad_sch", page="5",
                title="Unified SPI display-module connector (0.5mm FFC)",
                comps=[(J1, [(path, "J1")])], wiring=wiring, notes=[note],
                _dir=PROJ_DIR)


# ===================== aux OLED sheet (DNP-optional) =========================
def build_aux():
    path = f"/{ROOT_UUID}/{AUX}"
    J3 = dict(lib_id="Connector_Generic:Conn_01x04", value="OLED 128x32 (DNP)",
              fp=OLED_FP, lcsc="C2691448", mpn="PZ254V-11-04P", mfr="XKB", dnp=True,
              x=g(28), y=g(28))
    j3nets = {1: "+3V3", 2: "GND", 3: "OLED_SCL", 4: "OLED_SDA"}
    wiring = ""
    for pin, net in j3nets.items():
        wiring += K.net_pin(J3, pin, net, kind="glabel")
    note = (20.0, 110.0, K.note_block(
        "AUX OLED  -  J3   (OPTIONAL, DNP by default)",
        "0.91\" SSD1306 128x32 I2C module, sourced separately + hand-placed.",
        "Driven LOCALLY by the RP2040 over a private I2C (OLED_SDA/OLED_SCL), NOT",
        "the unified connector -- the main MCU sends OLED content inside the SPI",
        "display-intent stream. Add 4.7k pull-ups (DNP with the module).",
        "",
        K.pin_table([(1, "VCC <- +3V3"), (2, "GND"), (3, "SCL <- OLED_SCL"),
                     (4, "SDA <- OLED_SDA")], cols=1),
        "",
        "Shows full-precision X / error text / SETUP; the matrix stays primary."))
    return dict(uuid=AUX, file="aux.kicad_sch", page="6",
                title="Aux OLED 128x32 (SSD1306 I2C) — DNP-optional",
                comps=[(J3, [(path, "J3")])], wiring=wiring, notes=[note],
                _dir=PROJ_DIR)


# ============================ root ===========================================
def build_root_strings():
    sym = ""
    wiring = ""
    for r in range(NROWS):
        rx, ry = g(6), g(10 + r * 14)
        rw, rh = g(30), g(10)
        dpy, opy = ry + g(3), ry + g(6)
        sym += K.w_sheet(f"Row{r + 1}", "led_row.kicad_sch", ROW_INST[r], rx, ry, rw, rh,
                         pins=[("DIN", "input", rx, dpy, 180),
                               ("DOUT", "output", rx + rw, opy, 0)])
        wiring += K.w_wire(rx, dpy, rx - g(2), dpy)
        wiring += K.w_glabel(f"CH{r + 1}_DATA", rx - g(2), dpy, 180, shape="input")
        wiring += K.w_wire(rx + rw, opy, rx + rw + g(2), opy)
        wiring += K.w_glabel(f"CH{r + 1}_END", rx + rw + g(2), opy, 0, shape="output")

    yb = g(10 + NROWS * 14 + 2)
    for k, (nm, fn, uu) in enumerate([("RP2040", "rp2040.kicad_sch", RP2040_U),
                                      ("RGBPower", "rgb_power.kicad_sch", RGBPOWER),
                                      ("MatrixIF", "matrix_if.kicad_sch", MATRIX_IF),
                                      ("AuxOLED", "aux.kicad_sch", AUX)]):
        sym += K.w_sheet(nm, fn, uu, g(6 + k * 36), yb, g(30), g(8), pins=[])
    wiring += K.text_note(K.note_block(
        "Calcumaker 16 - RGB Matrix (NESTED MULTI-CHANNEL: cluster -> row -> board).",
        "Row1..Row3 = 3 instances of led_row.kicad_sch; each led_row = 12 instances",
        "of led_cluster.kicad_sch (8x8 = 64x XL-1010). Grid 96x24 = 2304 px. Shared",
        "globals VLED (gated) / GND; DIN/DOUT are hier pins chained cluster->cluster",
        "then row->row: CHr_DATA -> Row_r -> CHr_END. RP2040 PIO drives 3 data lines",
        "via RGBPower's quad shifter; RGBPower gates VLED off in sleep. Wire the",
        "one-off sheets in eeschema. Same unified SPI connector as the 7-seg board."),
        g(6), yb + g(12))
    pro_sheets = [[ROW_INST[r], f"Row{r + 1}"] for r in range(NROWS)] + \
                 [[CLUST_INST[c], f"Cluster{c + 1}"] for c in range(NCLUST)] + \
                 [[RP2040_U, "RP2040"], [RGBPOWER, "RGBPower"],
                  [MATRIX_IF, "MatrixIF"], [AUX, "AuxOLED"]]
    return sym, wiring, pro_sheets


# ============================ generate =======================================
print("child sheets:")
K.write_wired_child(build_led_cluster(), PROJECT, ROOT_UUID, TITLE, PAPER_CLUSTER)
K.write_wired_child(build_led_row(), PROJECT, ROOT_UUID, TITLE, PAPER_ROW)
K.write_wired_child(build_rp2040(), PROJECT, ROOT_UUID, TITLE, PAPER_ROOT)
K.write_wired_child(build_rgb_power(), PROJECT, ROOT_UUID, TITLE, PAPER_ROOT)
K.write_wired_child(build_matrix_if(), PROJECT, ROOT_UUID, TITLE, PAPER_ROOT)
K.write_wired_child(build_aux(), PROJECT, ROOT_UUID, TITLE, PAPER_ROOT)

sym, wiring, pro_sheets = build_root_strings()
K.write_root(PROJECT, PROJ_DIR, ROOT_UUID, TITLE, sym, wiring, pro_sheets,
             paper=PAPER_ROOT)
