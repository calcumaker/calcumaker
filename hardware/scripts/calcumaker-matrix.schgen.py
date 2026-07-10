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
K.register_stdlib("74xGxx", "74LVC1G125")              # single-gate 3V3->VLED data shifter (x3 chains)
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
SOT235 = "Package_TO_SOT_SMD:SOT-23-5"
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
    comps, w = [], ""

    def one(c, ref):
        comps.append((c, [(path, ref)]))
        return c

    # --- RP2040 (57-pin, single unit) ---
    U1 = one(dict(lib_id="MCU_RaspberryPi:RP2040", value="RP2040", fp=RP2040_FP,
                  lcsc="C2040", mpn="RP2040", mfr="Raspberry Pi", x=g(44), y=g(44)), "U1")
    for n in (1, 10, 22, 33, 42, 49):                    # IOVDD
        w += K.net_pin(U1, n, "+3V3", kind="glabel")
    for n in (44, 48, 43):                               # VREG_VIN / USB_VDD / ADC_AVDD
        w += K.net_pin(U1, n, "+3V3", kind="glabel")
    for n in (45, 23, 50):                               # VREG_VOUT + DVDD (1.1V core)
        w += K.net_pin(U1, n, "DVDD", kind="glabel")
    w += K.net_pin(U1, 57, "GND", kind="glabel")          # thermal pad / GND
    w += K.net_pin(U1, 19, "GND", kind="glabel")          # TESTEN -> GND
    w += K.net_pin(U1, 20, "XIN", kind="glabel")
    w += K.net_pin(U1, 21, "XOUT", kind="glabel")
    for n, net in [(52, "QSPI_SCLK"), (53, "QSPI_SD0"), (55, "QSPI_SD1"),
                   (54, "QSPI_SD2"), (51, "QSPI_SD3"), (56, "QSPI_CS")]:
        w += K.net_pin(U1, n, net, kind="glabel")
    w += K.net_pin(U1, 47, "USB_DP", kind="glabel")
    w += K.net_pin(U1, 46, "USB_DM", kind="glabel")
    w += K.net_pin(U1, 26, "DISP_NRST", kind="glabel")    # RUN <- reset from U575
    w += K.net_pin(U1, 24, "SWCLK", kind="glabel")
    w += K.net_pin(U1, 25, "SWDIO", kind="glabel")
    # peripheral GPIOs (by pin number)
    for n, net in [(4, "SPI_SCLK"), (2, "SPI_MOSI"), (3, "SPI_CS"), (38, "DISP_IRQ"),
                   (27, "LED_DATA1"), (28, "LED_DATA2"), (29, "LED_DATA3"),
                   (34, "LED_EN"), (6, "OLED_SDA"), (7, "OLED_SCL")]:
        w += K.net_pin(U1, n, net, kind="glabel")

    # --- decoupling + bulk ---
    for i, ref in enumerate(("C1", "C2", "C3", "C4", "C5", "C6")):
        c = one(dict(C(ref, "100nF"), x=g(8 + i * 5), y=g(6)), ref)
        w += K.net_pin(c, 1, "+3V3", kind="glabel")
        w += K.net_pin(c, 2, "GND", kind="glabel")
    C8 = one(dict(C("C8", "1uF"), x=g(40), y=g(6)), "C8")           # DVDD (1.1V LDO out)
    w += K.net_pin(C8, 1, "DVDD", kind="glabel")
    w += K.net_pin(C8, 2, "GND", kind="glabel")
    for i, ref in enumerate(("C9", "C10")):
        c = one(dict(C(ref, "10uF", C0603), x=g(46 + i * 5), y=g(6)), ref)
        w += K.net_pin(c, 1, "+3V3", kind="glabel")
        w += K.net_pin(c, 2, "GND", kind="glabel")

    # --- 12 MHz crystal (XIN--Y1--XOSC, R1 series on XOSC->XOUT, 15pF loads) ---
    Y1 = one(dict(lib_id="Device:Crystal", value="12MHz", fp=XTAL_FP, x=g(8), y=g(70)), "Y1")
    w += K.net_pin(Y1, 1, "XIN", kind="glabel")
    w += K.net_pin(Y1, 2, "XOSC", kind="label")
    R1 = one(dict(R("R1", "1k"), x=g(16), y=g(70)), "R1")
    w += K.net_pin(R1, 1, "XOSC", kind="label")
    w += K.net_pin(R1, 2, "XOUT", kind="glabel")
    C11 = one(dict(C("C11", "15pF"), x=g(6), y=g(78)), "C11")
    w += K.net_pin(C11, 1, "XIN", kind="glabel")
    w += K.net_pin(C11, 2, "GND", kind="glabel")
    C12 = one(dict(C("C12", "15pF"), x=g(12), y=g(78)), "C12")
    w += K.net_pin(C12, 1, "XOSC", kind="label")
    w += K.net_pin(C12, 2, "GND", kind="glabel")

    # --- QSPI boot flash ---
    U2 = one(dict(lib_id="Memory_Flash:W25Q32JVSS", value="W25Q32JVSSIQ", fp=FLASH_FP,
                  lcsc="C179173", mpn="W25Q32JVSSIQ", mfr="Winbond", x=g(78), y=g(44)), "U2")
    for n, net in [(1, "QSPI_CS"), (2, "QSPI_SD1"), (3, "QSPI_SD2"), (4, "GND"),
                   (5, "QSPI_SD0"), (6, "QSPI_SCLK"), (7, "QSPI_SD3"), (8, "+3V3")]:
        w += K.net_pin(U2, n, net, kind="glabel")
    R4 = one(dict(R("R4", "10k"), x=g(88), y=g(36)), "R4")           # CS# pull-up
    w += K.net_pin(R4, 1, "+3V3", kind="glabel")
    w += K.net_pin(R4, 2, "QSPI_CS", kind="glabel")
    C7 = one(dict(C("C7", "100nF"), x=g(88), y=g(50)), "C7")         # flash decouple
    w += K.net_pin(C7, 1, "+3V3", kind="glabel")
    w += K.net_pin(C7, 2, "GND", kind="glabel")
    # BOOTSEL: hold flash CS low at power-on
    SW1 = one(dict(lib_id="Switch:SW_Push", value="BOOTSEL", fp=BTN_FP, x=g(78), y=g(64)), "SW1")
    w += K.net_pin(SW1, 1, "QSPI_CS", kind="glabel")
    w += K.net_pin(SW1, 2, "GND", kind="glabel")

    # --- USB-C (BOOTSEL/DFU) + ESD. Redundant VBUS/GND pins + CC 5.1k @ layout. ---
    U3 = one(dict(lib_id="Power_Protection:USBLC6-2SC6", value="USBLC6-2SC6", fp=SOT236,
                  lcsc="C2687116", mpn="USBLC6-2SC6", mfr="STMicroelectronics",
                  x=g(30), y=g(74)), "U3")
    w += K.net_pin(U3, "I/O1", "USB_DP_C", kind="label")
    w += K.net_pin(U3, "I/O2", "USB_DM_C", kind="label")
    w += K.net_pin(U3, "VBUS", "VBUS", kind="glabel")
    w += K.net_pin(U3, "GND", "GND", kind="glabel")
    R2 = one(dict(R("R2", "27"), x=g(40), y=g(72)), "R2")
    w += K.net_pin(R2, 1, "USB_DP", kind="glabel")
    w += K.net_pin(R2, 2, "USB_DP_C", kind="label")
    R3 = one(dict(R("R3", "27"), x=g(40), y=g(78)), "R3")
    w += K.net_pin(R3, 1, "USB_DM", kind="glabel")
    w += K.net_pin(R3, 2, "USB_DM_C", kind="label")
    J4 = one(dict(lib_id="Connector:USB_C_Receptacle_USB2.0_16P", value="USB-C (BOOTSEL)",
                  fp=USBC_FP, lcsc="C2927039", mpn="USB-TYPE-C-019", mfr="GCT",
                  x=g(52), y=g(74)), "J4")
    w += K.net_pin(J4, "VBUS", "VBUS", kind="glabel")
    w += K.net_pin(J4, "GND", "GND", kind="glabel")
    w += K.net_pin(J4, "D+", "USB_DP_C", kind="label")
    w += K.net_pin(J4, "D-", "USB_DM_C", kind="label")

    note = (g(4), g(84), K.note_block(
        "RP2040 MIN-SYSTEM (WIRED)  -  U1 RP2040 (C2040, QFN-56). PIO drives WS2812.",
        "POWER  IOVDD/VREG_VIN/USB_VDD/ADC_AVDD -> +3V3 (C1-C6); VREG_VOUT+DVDD =",
        "       1.1V core -> C8 1uF; C9/C10 10uF bulk. GND pad + TESTEN -> GND.",
        "CLOCK  Y1 12MHz (XIN..R1..XOUT), C11/C12 15pF loads.  FLASH  U2 W25Q32 on",
        "       QSPI (SD0-3/SCLK/CS); R4 10k CS pull-up; SW1 BOOTSEL pulls CS low.",
        "USB    U1 D+/D- -> R2/R3 27R -> U3 USBLC6 ESD -> J4 USB-C (drag-drop UF2).",
        "       TODO @layout: tie the redundant VBUS/GND pins + add CC1/CC2 5.1k Rd.",
        "LINK   SPI slave <- SPI_SCLK/MOSI/CS; DISP_IRQ out; RUN <- DISP_NRST.",
        "OUT    LED_DATA1/2/3 + LED_EN -> RGBPower; OLED_SDA/SCL -> AuxOLED J3.",
        "Y1 12MHz + SW1 BOOTSEL: pick LCSC at BOM."))
    return dict(uuid=RP2040_U, file="rp2040.kicad_sch", page="3",
                title="RP2040 min-system (MCU + QSPI flash + USB BOOTSEL) — wired",
                comps=comps, wiring=w, notes=[note], _dir=PROJ_DIR)


# ===================== RGB power + data gate sheet (single instance) =========
def build_rgb_power():
    path = f"/{ROOT_UUID}/{RGBPOWER}"
    comps, w = [], ""

    def one(c, ref):
        comps.append((c, [(path, ref)]))
        return c

    # VSYS inlet (dedicated LED-power lead from mcu J7 — amps, off the signal FFC)
    J2 = one(dict(lib_id="Connector_Generic:Conn_01x02", value="VSYS IN (LED pwr)",
                  fp=JST2_FP, lcsc="C173752", mpn="S2B-PH-K-S", mfr="JST",
                  x=g(8), y=g(12)), "J2")
    w += K.net_pin(J2, 1, "VSYS", kind="glabel")
    w += K.net_pin(J2, 2, "GND", kind="glabel")

    # High-side P-FET: VSYS -> VLED, gate = QG (R5 pull-up to VSYS = OFF default)
    Q1 = one(dict(lib_id="Transistor_FET:Q_PMOS_GSD", value="AO3401A", fp=SOT23,
                  lcsc="C15127", mpn="AO3401A", mfr="AOS", x=g(26), y=g(16)), "Q1")
    w += K.net_pin(Q1, "S", "VSYS", kind="glabel")
    w += K.net_pin(Q1, "D", "VLED", kind="glabel")
    w += K.net_pin(Q1, "G", "QG", kind="label")
    R5 = one(dict(R("R5", "100k"), x=g(26), y=g(6)), "R5")
    w += K.net_pin(R5, 1, "VSYS", kind="glabel")
    w += K.net_pin(R5, 2, "QG", kind="label")
    # N-FET pulls QG low when LED_EN is high (R7 pulldown = OFF at boot/sleep)
    Q2 = one(dict(lib_id="Transistor_FET:Q_NMOS_GSD", value="2N7002", fp=SOT23,
                  lcsc="C8545", mpn="2N7002", mfr="onsemi", x=g(40), y=g(16)), "Q2")
    w += K.net_pin(Q2, "D", "QG", kind="label")
    w += K.net_pin(Q2, "S", "GND", kind="glabel")
    w += K.net_pin(Q2, "G", "QEN", kind="label")
    R6 = one(dict(R("R6", "10k"), x=g(40), y=g(26)), "R6")
    w += K.net_pin(R6, 1, "LED_EN", kind="glabel")
    w += K.net_pin(R6, 2, "QEN", kind="label")
    R7 = one(dict(R("R7", "100k"), x=g(48), y=g(26)), "R7")
    w += K.net_pin(R7, 1, "QEN", kind="label")
    w += K.net_pin(R7, 2, "GND", kind="glabel")
    # VLED bulk
    for i, (ref, val, fp) in enumerate([("C13", "22uF", C0603), ("C14", "22uF", C0603),
                                        ("C15", "100nF", C0402)]):
        c = one(dict(C(ref, val, fp), x=g(56 + i * 5), y=g(12)), ref)
        w += K.net_pin(c, 1, "VLED", kind="glabel")
        w += K.net_pin(c, 2, "GND", kind="glabel")

    # 3x single-gate shifter: A <- LED_DATAk (RP2040 3V3), Y -> CHk_DATA (VLED);
    # VCC = VLED, /OE = GND (always enabled), one per WS2812 chain.
    for k in range(1, 4):
        U = one(dict(lib_id="74xGxx:74LVC1G125", value="74LVC1G125", fp=SOT235,
                     lcsc="C23654", mpn="SN74LVC1G125DBVR", mfr="Texas Instruments",
                     x=g(10 + (k - 1) * 18), y=g(44)), f"U{3 + k}")
        w += K.net_pin(U, 5, "VLED", kind="glabel")            # VCC
        w += K.net_pin(U, 3, "GND", kind="glabel")             # GND
        w += K.net_pin(U, 1, "GND", kind="glabel")             # /OE low = enabled
        w += K.net_pin(U, 2, f"LED_DATA{k}", kind="glabel")    # A  <- RP2040 (3V3)
        w += K.net_pin(U, 4, f"CH{k}_DATA", kind="glabel")     # Y  -> chain k
        cc = one(dict(C(f"C{15 + k}", "100nF"), x=g(10 + (k - 1) * 18), y=g(54)), f"C{15 + k}")
        w += K.net_pin(cc, 1, "VLED", kind="glabel")
        w += K.net_pin(cc, 2, "GND", kind="glabel")

    note = (g(4), g(64), K.note_block(
        "RGB POWER + DATA GATE  (WIRED)  -  feeds the 3 WS2812 chains (Row1-3)",
        "GATE   J2 VSYS -> Q1 AO3401A P-FET -> VLED; Q1 gate QG (R5 100k pull-up",
        "       to VSYS = OFF). RP2040 LED_EN -> R6 10k -> Q2 2N7002 (R7 100k",
        "       pulldown) pulls QG low = ON. LED_EN low in sleep -> all off.",
        "DATA   RP2040 LED_DATA1/2/3 (3V3) -> U4/U5/U6 74LVC1G125 (VCC=VLED, /OE=",
        "       GND) -> CH1/2/3_DATA. C16-18 decouple; C13/C14 22uF VLED bulk.",
        "POWER  2304x WS2812 full-white ~ many amps -> firmware MUST cap brightness;",
        "       VLED from the J2 inlet, NOT the FFC. ADD a >=470uF bulk @ layout."))
    return dict(uuid=RGBPOWER, file="rgb_power.kicad_sch", page="4",
                title="RGB power + data gate (load switch + 3x level shift)",
                comps=comps, wiring=w, notes=[note], _dir=PROJ_DIR)


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
    return dict(uuid=AUX, file="aux-oled.kicad_sch", page="6",
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
                                      ("AuxOLED", "aux-oled.kicad_sch", AUX)]):
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
