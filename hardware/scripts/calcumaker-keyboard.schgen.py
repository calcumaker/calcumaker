#!/usr/bin/env python3
"""Regenerate the Calcumaker 16 **keyboard board** schematic — a KiCad
*multi-channel* design.

    CALCUMAKER_SCHGEN_DRAFT_OK=1 KSCHGEN_FORCE=1 python3 scripts/calcumaker-keyboard.schgen.py

*** DRAFT ***
The keyboard board is the **top board** of the three-board split (DESIGN.md ->
Board Partition). It stacks ABOVE the MCU board on a fine-pitch mezzanine and
carries the front-panel: the **49-key Cherry MX matrix** (2U ENTER), its **own
STM32G031K8U6 scanner** (U1), the **annunciator LEDs**, **per-key RGB** hint
lighting, and the mezzanine header (J1) down to the MCU board.

**Multi-channel structure.** The 5x10 matrix is *almost* five identical rows, so a
row is authored ONCE as a reusable, fully-wired child sheet (``key_row.kicad_sch``)
and instantiated at the root, each annotating to its own reference designators.
Each key = **MX switch + 1N4148W diode + SK6812MINI-E RGB LED**; the matrix
(ROW/COL) and the RGB daisy-chain (DIN->DOUT) are wired in the one sheet, so a fix
propagates to every row that uses it. Shared buses (COL1..10, VLED, GND) are global
nets; each row's ROW line + RGB DIN/DOUT are hierarchical pins wired at the root.

**The 2U ENTER row variant.** ENTER is a double-height (2U) keycap spanning Row4
and Row5 of COL6, with its single switch wired to Row5 (firmware ``keys.rs``:
``ENTER_SWITCH_CELL = (4,5)``, ``ENTER_SPAN_CELL = (3,5)``, 0-based). So the
**Row4/COL6 cell carries no switch, no diode, and no RGB LED**. NOTE the switch
body is physically centered on the 2U cap (on the Row4/Row5 boundary, 9.525mm from
either 1U cell center) -- the Row5 assignment is a NET, not a coordinate. The 2U
stabilizer is plate-mount (no PCB holes); see DESIGN.md "The 2U ENTER". Multi-channel instances must be identical, so Row4 gets its own
**9-key** sheet (``key_row_9.kicad_sch``); Row1/2/3/5 still share the reusable
10-key sheet. Row4's RGB daisy-chain is re-stitched around the gap (LED5 -> LED7),
and the global reference numbering keeps its hole (no SW36 / D36 / D91). The board
is therefore **49 keys / 49 RGB LEDs**, not 50. `enter_is_2u_in_every_personality`
in calcumaker-core pins this geometry so the schematic and firmware scan agree.

Peripheral one-off sheets (Annunciators, KbdMCU, RGBPower, MainIF) are
single-instance and PLACED-not-wired -- wire them + the G0 pin assignment
(KB_ROW1-5, COL1-10, KB_LED_DATA, KB_LED_EN) in eeschema from the per-sheet notes.

This is DATA; the engine is scripts/kschgen.py. 0402 passives. Verify each
lib_id/footprint exists in your KiCad 10 install before relying on it.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import kschgen as K

# --- DRAFT guard -------------------------------------------------------------
if not os.environ.get("CALCUMAKER_SCHGEN_DRAFT_OK"):
    sys.exit(
        "calcumaker-keyboard.schgen.py is a DRAFT: keypad layout + mezzanine "
        "pinout are placeholders (see DESIGN.md Open Questions). Set "
        "CALCUMAKER_SCHGEN_DRAFT_OK=1 to generate anyway."
    )

HW = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))   # hardware/
PROJ_DIR = os.path.join(HW, "calcumaker-keyboard")
PROJECT = "calcumaker-keyboard"
PAPER_ROOT = "A3"
PAPER_ROW = "A2"          # a 10-key row + its labels wants the bigger sheet

# ---- stable UUIDs (sheet-symbol + file ids; existing files keep their on-disk
#      uuids, these seed fresh ones + keep forced-regen diffs readable) --------
ROOT_UUID = "ca1c0000-0000-4000-8000-00000000eb01"
ROW_FILE  = "ca1c0000-0000-4000-8000-00000000eb10"   # key_row.kicad_sch file id
ROW9_FILE = "ca1c0000-0000-4000-8000-00000000eb1a"   # key_row_9.kicad_sch file id
ROW_INST  = ["ca1c0000-0000-4000-8000-00000000eb1%d" % (n + 1) for n in range(5)]  # Row1..Row5

# ---- 2U ENTER: the one cell with no switch ---------------------------------
# Mirrors calcumaker-core keys.rs ENTER_SPAN_CELL = (3, 5) (0-based row, col).
# Schematic rows/cols are 1-based, so that is Row4 / COL6. Row4 is the only row
# that differs, so it gets a dedicated 9-key sheet; the rest share the 10-key one.
ENTER_SPAN_ROW  = 4          # 1-based schematic row with the missing switch
ENTER_SPAN_COL  = 6          # 1-based column of the missing switch
ROW9_INSTANCE   = ENTER_SPAN_ROW - 1        # index into ROW_INST (Row4 -> 3)
ROW10_INSTANCES = [i for i in range(5) if i != ROW9_INSTANCE]   # Row1,2,3,5
ANNUNC    = "ca1c0000-0000-4000-8000-00000000eb20"
KBD_MCU   = "ca1c0000-0000-4000-8000-00000000eb30"
RGB_POWER = "ca1c0000-0000-4000-8000-00000000eb40"
MAIN_IF   = "ca1c0000-0000-4000-8000-00000000eb50"

# ---- symbol libraries -------------------------------------------------------
K.register_stdlib("Device", "R", "C", "D", "LED")
K.register_stdlib("Switch", "SW_Push")
K.register_stdlib("Connector_Generic", "Conn_02x06_Odd_Even", "Conn_01x16")   # J1 DF40 stack + J3 FFC cable
K.register_stdlib("Connector", "Conn_ARM_SWD_TagConnect_TC2030-NL")   # G0 SWD (J2)
K.register_stdlib("MCU_ST_STM32G0", "STM32G031K8Ux")   # keyboard scanner MCU (UFQFPN-32)
K.register_stdlib("LED", "SK6812")                     # per-key addressable RGB (base sym; MINI-E extends
#                                                        it + kschgen doesn't resolve `extends`, so use
#                                                        the base + override value/fp to the MINI-E variant)
K.register_stdlib("74xGxx", "74LVC1G125")              # 3V3 -> VLED data level shifter
K.register_stdlib("Transistor_FET", "Q_PMOS_GSD", "Q_NMOS_GSD")   # LED-rail load switch

# ---- footprint shorthands ---------------------------------------------------
R0402 = "Resistor_SMD:R_0402_1005Metric"
C0402 = "Capacitor_SMD:C_0402_1005Metric"
C0603 = "Capacitor_SMD:C_0603_1608Metric"
SOD123 = "Diode_SMD:D_SOD-123"
LED0603 = "LED_SMD:LED_0603_1608Metric"
RGB_LED_FP = "LED_SMD:LED_SK6812MINI-E_3.2x2.8mm_P1.5mm_ReverseMount"  # REVERSE (bottom) mount ->
#            LED sits on the BOTTOM with the sockets (single-sided assembly), shining UP through the
#            PCB into the MX switch's north LED window. Stock KiCad fp + 3D.
SOT235 = "Package_TO_SOT_SMD:SOT-23-5"            # 74LVC1G125 level shifter
SOT23 = "Package_TO_SOT_SMD:SOT-23"               # AO3401A / 2N7002 load switch
MX_FP = "calcumaker:SW_MX_HS_CPG151101S11_1u"   # VENDORED (marbastlib, CERN-OHL-P): Kailh
#            CPG151101S11 HOT-SWAP socket footprint (LCSC C41430893). PLACE-ON-BACK: switch fps go on
#            the board's BACK copper layer so the socket (authored on F.Cu) lands on the bottom + the
#            keycaps face up on the front; needs a switch plate. **HOT-SWAP ONLY** -- the switch
#            thru-holes are 0.15mm-ring socket pass-throughs, NOT solder pads, so a switch-only /
#            solder-in build is NOT possible on this fp (that would be a separate board rev using the
#            solder-in-only SW_Cherry_MX_1.00u_PCB fp). See DESIGN.md "Hot-swap switches".
MX_2U_FP = "calcumaker:SW_MX_HS_CPG151101S11_2u_Vertical"   # VENDORED: 1u hot-swap fp + the
#            four PCB-mount 2U stabilizer holes (y = +/-11.90mm, 23.8mm spacing). UNUSED by default:
#            we use PLATE-mount stabilizers, which clip into the switch plate the hot-swap sockets
#            already require and need NO pcb holes -- so the 2U ENTER simply reuses MX_FP. Switching
#            to PCB-mount stabs means (a) this fp for the ENTER switch and (b) a Row5 VARIANT SHEET,
#            since ENTER's switch sits on the shared 10-key sheet and multi-channel instances must
#            share footprints. See DESIGN.md "The 2U ENTER".
G0_FP = "Package_DFN_QFN:UFQFPN-32-1EP_5x5mm_P0.5mm_EP3.5x3.5mm"
SWD_FP = "Connector:Tag-Connect_TC2030-IDC-NL_2x03_P1.27mm_Vertical"
MEZZ_HEADER_FP = "Connector_Hirose_DF40:Hirose_DF40C-12DP-0.4V_2x06-1MP_P0.4mm"
FFC16_FP = "Connector_FFC-FPC:Hirose_FH12-16S-0.5SH_1x16-1MP_P0.50mm_Horizontal"  # keyboard FFC-cable alt (J3)

RLCSC = {"470": "C25117", "10k": "C25744", "100k": "C25741"}
CLCSC = {"100nF": "C1525"}

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
    """Lay single-instance comps in a grid -> write_wired_child comps format.
    Each spec is a comp dict incl. 'ref'; the ref goes into the instance tuple."""
    comps = []
    for idx, sp in enumerate(specs):
        ref = sp["ref"]
        c = {k: v for k, v in sp.items() if k != "ref"}
        c["x"] = x0 + (idx % per) * dx
        c["y"] = y0 + (idx // per) * dy
        comps.append((c, [(path, ref)]))
    return comps


# ===================== reusable 10-key ROW sheet =============================
# 10 keys, each = MX switch + 1N4148W diode + SK6812MINI-E RGB. Instantiated x5.
#   MATRIX  SW.1 -> ROW (hier);  SW.2 -> node KN;  D.A -> KN;  D.K -> COLk (global)
#           (anode@switch, cathode->COL; per-key diode = n-key rollover)
#   RGB     DIN -> LED1..LED10 -> DOUT (hier, chained @root);  VDD->VLED, VSS->GND
def build_key_row(keys, instances, file_uuid, filename, page, title, note_header):
    """One row sheet. `keys` = 1-based columns present (10-key row: 1..10; the
    Row4 variant omits ENTER_SPAN_COL). `instances` = ROW_INST indices that share
    this sheet. Reference numbering stays SW/D = col + 10*row_index, so the
    omitted key simply leaves a hole (SW36 / D36 / D91) rather than renumbering.
    The RGB chain is stitched by *position in the row*, so it closes over the gap.
    """
    comps = []
    wiring = ""

    def pr(fn):   # per-instance (path, reference)
        return [(f"/{ROOT_UUID}/{ROW_INST[i]}", fn(i)) for i in instances]

    for pos, k in enumerate(keys, start=1):   # pos = index along the RGB chain
        # x follows the physical column so the sheet stays 1:1 with the panel —
        # the missing key leaves a visible gap.
        bx = g(6 + (k - 1) * 14)
        # --- switch ---------------------------------------------------------
        sw = dict(lib_id="Switch:SW_Push", value="MX", fp=MX_FP, x=bx, y=g(18))
        comps.append((sw, pr(lambda i, kk=k: f"SW{kk + 10 * i}")))
        wiring += K.net_pin(sw, 1, "ROW", kind="hlabel", shape="input")
        wiring += K.net_pin(sw, 2, f"KN{k}", kind="label")
        # --- matrix diode (anode at switch, cathode to column) --------------
        d = dict(lib_id="Device:D", value="1N4148W", fp=SOD123, lcsc="C81598",
                 mpn="1N4148W", mfr="onsemi", x=bx, y=g(32))
        comps.append((d, pr(lambda i, kk=k: f"D{kk + 10 * i}")))
        wiring += K.net_pin(d, "A", f"KN{k}", kind="label")
        wiring += K.net_pin(d, "K", f"COL{k}", kind="glabel")
        # --- per-key RGB LED (single-wire chain), reverse/bottom mount ------
        led = dict(lib_id="LED:SK6812", value="SK6812MINI-E", fp=RGB_LED_FP,
                   lcsc="C5149201", mpn="SK6812MINI-E", mfr="OPSCO", x=bx, y=g(48))
        comps.append((led, pr(lambda i, kk=k: f"D{55 + kk + 10 * i}")))  # D56-65,66-75,...
        wiring += K.net_pin(led, "VDD", "VLED", kind="glabel")
        wiring += K.net_pin(led, "VSS", "GND", kind="glabel")
        if pos == 1:
            wiring += K.net_pin(led, "DIN", "DIN", kind="hlabel", shape="input")
        else:
            wiring += K.net_pin(led, "DIN", f"CH{pos}", kind="label")
        if pos == len(keys):
            wiring += K.net_pin(led, "DOUT", "DOUT", kind="hlabel", shape="output")
        else:
            wiring += K.net_pin(led, "DOUT", f"CH{pos + 1}", kind="label")

    note = (g(6), g(80), K.note_block(
        *note_header,
        "",
        "Each key = MX switch + 1N4148W diode + SK6812MINI-E RGB:",
        "  MATRIX  SW.1 -> ROW (hier pin);  SW.2 -> node KNk;  D.A -> KNk;",
        "          D.K -> COLk (global)   [anode@switch, cathode->COL; NKRO]",
        "  RGB     DIN -> LED -> LED -> ... -> DOUT (hier, chained by row",
        "          @root);  LED VDD -> VLED (gated), VSS -> GND",
        "",
        "Shared globals: COL1..COL10, VLED, GND.",
        "Per-instance hier pins: ROW (-> KB_ROWn @root), DIN/DOUT (RGB chain).",
        "Wire-once: a fix here propagates to all 5 rows. Placement 1:1 w/ panel.",
        "LED = SK6812MINI-E (C5149201), REVERSE/BOTTOM mount: it sits on the BOTTOM",
        "with the Kailh sockets -> SINGLE-SIDED assembly (one stencil, no flip),",
        "shining UP through the PCB into the MX switch's north LED window.",
        "",
        "SWITCH FP = SW_MX_HS_CPG151101S11_1u -> HOT-SWAP (Kailh CPG151101S11,",
        "  LCSC C41430893). PLACE-ON-BACK: put switch fps on the BACK copper layer",
        "  so the socket lands on the bottom + keycaps face up on front; + a plate.",
        "  NOT solderable for a switch-only build (thru-holes are 0.15mm-ring socket",
        "  pass-throughs, not solder pads) -> solder-in = a separate board rev."))
    return dict(uuid=file_uuid, file=filename, page=page, title=title,
                comps=comps, wiring=wiring, notes=[note], _dir=PROJ_DIR)


# ===================== Annunciator LEDs sheet (single instance) ==============
def build_annunc():
    path = f"/{ROOT_UUID}/{ANNUNC}"

    def ALED(ref, val, lcsc, mpn, mfr):
        return dict(ref=ref, lib_id="Device:LED", value=val, fp=LED0603,
                    lcsc=lcsc, mpn=mpn, mfr=mfr)

    specs = [
        ALED("D51", "f (yellow)", "C72038", "19-213/Y2C-CQ2R2L/3T(CY)", "Everlight"),
        ALED("D52", "g (blue)", "C965807", "XL-1608UBC-04", "XINGLIGHT"),
        ALED("D53", "C (red)", "C2286", "KT-0603R", "KENTO"),
        ALED("D54", "G (red)", "C2286", "KT-0603R", "KENTO"),
        ALED("D55", "LOWBAT (red)", "C2286", "KT-0603R", "KENTO"),
        R("R1", "470"), R("R2", "470"), R("R3", "470"), R("R4", "470"), R("R5", "470"),
    ]
    note = (15, 100, K.note_block(
        "ANNUNCIATOR LEDs   (DESIGN.md status-line mapping)  -  PLACED, not wired",
        "5 drive lines from the on-board STM32G0 (active high), each -> Rn 470R",
        "-> LED -> GND.  +3V3 feeds the resistors; GND returns.",
        "",
        "  D51 f  yellow  beside f key       D53 C red  carry    ] top edge,",
        "  D52 g  blue    beside g key       D54 G red  overflow ] under the",
        "                                    D55 LOWBAT red        display bezel",
        "",
        "470R = ~1.3-2.8mA (tune per color). FW: U575 App drives f/g from",
        "keys::Shift, C/G from Calc carry/overflow, LOWBAT from the batt ADC,",
        "pushing the 5-bit state to the G0 over I2C."))
    return dict(uuid=ANNUNC, file="annunc.kicad_sch", page="3",
                title="Annunciator LEDs (f g C G low-batt)",
                comps=place1(path, specs), wiring="", notes=[note], _dir=PROJ_DIR)


# ===================== Keyboard scanner MCU sheet (single instance) ==========
def build_kbd_mcu():
    path = f"/{ROOT_UUID}/{KBD_MCU}"
    specs = [
        dict(ref="U1", lib_id="MCU_ST_STM32G0:STM32G031K8Ux", value="STM32G031K8U6",
             fp=G0_FP, lcsc="C432207", mpn="STM32G031K8U6", mfr="STMicroelectronics"),
        C("C1", "100nF"), C("C2", "100nF"), C("C3", "100nF"), C("C4", "4.7uF", C0603),
        C("C5", "100nF"), R("R6", "10k"),
        dict(ref="J2", lib_id="Connector:Conn_ARM_SWD_TagConnect_TC2030-NL",
             value="SWD TC2030-NL", fp=SWD_FP),
    ]
    note = (15, 150, K.note_block(
        "SCANNER MCU  -  U1  STM32G031K8U6  (LCSC C432207, UFQFPN-32)  -  PLACED",
        "",
        "POWER  VDD -> +3V3 (C1/C2 100nF + C4 4.7uF); VDDA -> C3 100nF; VSS/EP GND",
        "RESET  NRST -> C5 100nF (also KB_NRST on J1); BOOT0 -> R6 10k to GND",
        "       (also KB_BOOT0 on J1: ROM UART/DFU reflash).  CLOCK internal HSI.",
        "",
        "MATRIX  5 ROW out -> KB_ROW1..5 (global);  10 COL in (int. pull-ups) ->",
        "        COL1..10 (global); hold ROWs low + COL EXTI for wake-from-Stop.",
        "ANNUN   5 GPIO -> the LED resistors (Annunciators sheet).",
        "RGB     LED_DATA -> RGBPower U2.A;  LED_EN -> RGBPower gate (Q2).",
        "LINK    I2C1 SDA/SCL + USART TX/RX + KB_IRQ out -> J1 mezzanine to U575.",
        "PROG    J2 SWD Tag-Connect (bare pads) or UART/DFU over J1.",
        "See DESIGN.md Low-power & wake."))
    return dict(uuid=KBD_MCU, file="kbd_mcu.kicad_sch", page="4",
                title="Keyboard scanner MCU (STM32G031K8U6)",
                comps=place1(path, specs), wiring="", notes=[note], _dir=PROJ_DIR)


# ===================== RGB power + data gate sheet (single instance) =========
# Level shifter + high-side load switch. The 49 SK6812MINI-E live on the Row sheets;
# this drives + gates the whole chain.
def build_rgb_power():
    path = f"/{ROOT_UUID}/{RGB_POWER}"
    specs = [
        dict(ref="U2", lib_id="74xGxx:74LVC1G125", value="74LVC1G125", fp=SOT235,
             lcsc="C23654", mpn="SN74LVC1G125DBVR", mfr="Texas Instruments"),
        dict(ref="Q1", lib_id="Transistor_FET:Q_PMOS_GSD", value="AO3401A",
             fp=SOT23, lcsc="C15127", mpn="AO3401A", mfr="AOS"),
        dict(ref="Q2", lib_id="Transistor_FET:Q_NMOS_GSD", value="2N7002",
             fp=SOT23, lcsc="C8545", mpn="2N7002", mfr="onsemi"),
        R("R7", "100k"), R("R8", "10k"), R("R10", "100k"), R("R9", "330"),
        C("C6", "100nF"), C("C7", "22uF", C0603),
    ]
    note = (15, 120, K.note_block(
        "RGB POWER + DATA GATE  -  drives the per-key SK6812MINI-E chain on the rows",
        "PLACED, not wired.  (The 49 LEDs are on Row1..Row5; Row4 has 9.)",
        "",
        "DATA   G0 LED_DATA -> U2.A;  U2.Y -> R9 330R -> KB_LED_DATA (-> Row1.DIN,",
        "       chained Row1..Row5).  U2 74LVC1G125: /OE -> GND; VCC -> VLED.",
        "GATE   VSYS (mezz pin 11, ~3.7-4.7V) -> Q1 AO3401A P-FET -> VLED (C7 22uF)",
        "       Q1 gate: R7 100k pull-up to VSYS = OFF default; Q2 2N7002 pulls low",
        "       G0 LED_EN -> R8 10k -> Q2 gate (R10 100k pulldown = OFF at boot)",
        "       -> in Stop, LED_EN low -> LEDs + U2 fully OFF (near-zero leakage)",
        "",
        "CURRENT  firmware MUST cap total brightness: 49x full-white ~0.74A would",
        "  exceed the DF40 contact + VSYS budget -- hint use lights a few keys."))
    return dict(uuid=RGB_POWER, file="rgb_power.kicad_sch", page="6",
                title="RGB power + data gate (level shift + load switch)",
                comps=place1(path, specs), wiring="", notes=[note], _dir=PROJ_DIR)


# ===================== Main-board mezzanine sheet (single instance) ==========
def build_main_if():
    path = f"/{ROOT_UUID}/{MAIN_IF}"
    J1 = dict(lib_id="Connector_Generic:Conn_02x06_Odd_Even", value="TO MCU (stack)",
              fp=MEZZ_HEADER_FP, lcsc="C6224952", mpn="DF40C-12DP-0.4V(51)",
              mfr="Hirose", x=g(18), y=g(18))
    J3 = dict(lib_id="Connector_Generic:Conn_01x16", value="TO MCU (FFC)",
              fp=FFC16_FP, lcsc="C262665", mpn="AFC01-S16FCA-00", mfr="JUSHUO",
              x=g(40), y=g(18))
    note = (15, 95, K.note_block(
        "MCU LINK  -  TWO options on the SAME nets; POPULATE ONE (mirrors mcu",
        "KeyboardIF J5/J6):",
        "  J1 STACK = DF40C-12DP 2x6 0.4mm HEADER (C6224952) -> mates DOWN to the",
        "     MCU receptacle (mcu J5 DF40B-12DS). ~1.5mm rigid stack, compact.",
        "  J3 CABLE = 16-pin 0.5mm FFC (AFC01-S16FCA-00, C262665) -> the MCU-board",
        "     FFC (mcu J6). Lets the MCU board mount freely -> more room under the",
        "     keys. 16-pin != the 12-pin display FFC -> can't cross-plug.",
        "PLACED, not wired.  Pinouts MUST match mcu J5 (DF40) / J6 (FFC):",
        "",
        "J1 DF40 (12-pin):",
        K.pin_table([(1, "+3V3"), (2, "GND"), (3, "SDA"), (4, "SCL"), (5, "UART_TX"),
                     (6, "UART_RX"), (7, "KB_IRQ"), (8, "KB_NRST"), (9, "KB_BOOT0"),
                     (10, "GND"), (11, "VSYS"), (12, "GND")]),
        "J3 FFC (16-pin: VSYS x2, GND x3, 2 spare):",
        K.pin_table([(1, "+3V3"), (2, "GND"), (3, "SDA"), (4, "SCL"), (5, "UART_TX"),
                     (6, "UART_RX"), (7, "KB_IRQ"), (8, "KB_NRST"), (9, "KB_BOOT0"),
                     (10, "GND"), (11, "VSYS"), (12, "VSYS"), (13, "GND"), (14, "GND"),
                     (15, "NC"), (16, "NC")]),
        "",
        "SDA/SCL <-> G0 I2C1; UART <-> G0 USART; KB_IRQ = G0 -> U575 wake;",
        "NRST/BOOT0 = U575 reflashes the G0. VSYS -> the local RGB load switch",
        "(RGBPower Q1) -> VLED. FFC cable (non-BOM): GCT 05-16-A-<len>-A-4-06-4-T."))
    return dict(uuid=MAIN_IF, file="main_if.kicad_sch", page="5",
                title="MCU link -- DF40 stack (J1) OR 16-pin FFC (J3), populate one",
                comps=[(J1, [(path, "J1")]), (J3, [(path, "J3")])], wiring="",
                notes=[note], _dir=PROJ_DIR)


# ============================ root ===========================================
def build_root_strings():
    sym = ""
    wiring = ""
    prev = "KB_LED_DATA"          # feeds Row1.DIN
    for i in range(5):
        name = f"Row{i+1}"
        x, y = g(10), g(10 + i * 11)
        w, h = g(26), g(8)
        rpy, dpy, opy = y + g(2), y + g(4), y + g(6)
        # Row4 has no switch at COL6 (2U ENTER) -> its own 9-key sheet.
        fname = "key_row_9.kicad_sch" if i == ROW9_INSTANCE else "key_row.kicad_sch"
        sym += K.w_sheet(name, fname, ROW_INST[i], x, y, w, h,
                         pins=[("ROW", "input", x, rpy, 180),
                               ("DIN", "input", x, dpy, 180),
                               ("DOUT", "output", x + w, opy, 0)])
        wiring += K.w_wire(x, rpy, x - g(3), rpy)
        wiring += K.w_glabel(f"KB_ROW{i+1}", x - g(3), rpy, 180, shape="output")
        wiring += K.w_wire(x, dpy, x - g(3), dpy)
        wiring += K.w_glabel(prev, x - g(3), dpy, 180, shape="input")
        nxt = f"RGB_CH{i+1}" if i < 4 else "RGB_END"
        wiring += K.w_wire(x + w, opy, x + w + g(3), opy)
        wiring += K.w_glabel(nxt, x + w + g(3), opy, 0, shape="output")
        prev = nxt
    for nm, fn, uu, yy in [("Annunciators", "annunc.kicad_sch", ANNUNC, 10),
                           ("KbdMCU", "kbd_mcu.kicad_sch", KBD_MCU, 22),
                           ("RGBPower", "rgb_power.kicad_sch", RGB_POWER, 34),
                           ("MainIF", "main_if.kicad_sch", MAIN_IF, 46)]:
        sym += K.w_sheet(nm, fn, uu, g(50), g(yy), g(22), g(8), pins=[])
    wiring += K.text_note(K.note_block(
        "Calcumaker 16 - Keyboard (MULTI-CHANNEL).",
        "Row1/2/3/5 = four instances of the reusable 10-key sheet key_row.kicad_sch.",
        "Row4 = key_row_9.kicad_sch: 9 keys, no switch/diode/RGB at COL6 -- that is",
        "the upper half of the 2U ENTER cap (switch lives in Row5/COL6). 49 keys.",
        "(MX switch + diode + SK6812MINI-E RGB per key). Shared buses on global nets",
        "COL1..COL10 / VLED / GND; each row's ROW line = a hier pin -> KB_ROWn,",
        "and the RGB DIN/DOUT are chained: KB_LED_DATA -> Row1 -> Row2 -> ... ->",
        "Row5 -> RGB_END. The G0 (KbdMCU) drives KB_ROW1-5 + COL1-10 + LED_DATA/EN;",
        "RGBPower gates VLED off in sleep. Wire the one-off sheets in eeschema."),
        g(50), g(60))
    pro_sheets = [[ROW_INST[i], f"Row{i+1}"] for i in range(5)] + \
                 [[ANNUNC, "Annunciators"], [KBD_MCU, "KbdMCU"],
                  [RGB_POWER, "RGBPower"], [MAIN_IF, "MainIF"]]
    return sym, wiring, pro_sheets


# ============================ generate =======================================
TITLE = dict(title="Calcumaker 16 — Keyboard", date="2026-07-06", rev="0.4",
             company="calcumaker authors",
             comments=["Programmer's/technical arbitrary-precision RPN calculator",
                       "Keyboard board: MULTI-CHANNEL 5x10 MX matrix + per-key RGB (row x5) + G0 scanner + mezzanine (DRAFT)"])

# Restructure to multi-channel: drop the obsolete flat sheets AND the one-off
# sheets that carry pre-restructure UUIDs, so all regenerate fresh with the
# constant UUIDs above (they were placed-not-wired -- nothing manual is lost).
if os.environ.get("KSCHGEN_FORCE") == "1":
    for _f in ("keypad.kicad_sch", "keylight.kicad_sch", "annunc.kicad_sch",
               "kbd_mcu.kicad_sch", "main_if.kicad_sch"):
        _p = os.path.join(PROJ_DIR, _f)
        if os.path.exists(_p):
            os.remove(_p)
            print(f"removed {_f} (regenerating fresh)")

print("child sheets:")
ALL_COLS = list(range(1, 11))
K.write_wired_child(build_key_row(
    ALL_COLS, ROW10_INSTANCES, ROW_FILE, "key_row.kicad_sch", "2",
    "Reusable 10-key row (MX + diode + SK6812MINI-E RGB)",
    ["REUSABLE 10-KEY ROW  (multi-channel: Row1, Row2, Row3, Row5)",
     "  Row1 -> SW1-10  / D1-10   / D56-65 (RGB)",
     "  Row2 -> SW11-20 / D11-20  / D66-75      ...",
     "  Row5 -> SW41-50 / D41-50  / D96-105",
     "  (Row4 is the 9-key variant -- sheet key_row_9.kicad_sch)"],
), PROJECT, ROOT_UUID, TITLE, PAPER_ROW)

K.write_wired_child(build_key_row(
    [k for k in ALL_COLS if k != ENTER_SPAN_COL], [ROW9_INSTANCE], ROW9_FILE,
    "key_row_9.kicad_sch", "7",
    "Row4 variant: 9 keys (2U ENTER spans COL6 - no switch there)",
    [f"ROW4 VARIANT - 9 KEYS (no switch at COL{ENTER_SPAN_COL})",
     "  The 2U ENTER keycap spans Row4+Row5 of COL6; its single switch is in",
     "  Row5/COL6, and a 2U stabilizer sits here. So this cell has NO switch,",
     "  NO diode and NO RGB LED -- the numbering keeps a hole:",
     "  Row4 -> SW31-35, SW37-40 / D31-35, D37-40 / D86-90, D92-95 (RGB)",
     "  RGB chain closes over the gap: DIN -> LED(COL1..5) -> LED(COL7..10) -> DOUT",
     "  Mirrors calcumaker-core keys.rs ENTER_SPAN_CELL = (3, 5) (0-based)."],
), PROJECT, ROOT_UUID, TITLE, PAPER_ROW)
K.write_wired_child(build_annunc(), PROJECT, ROOT_UUID, TITLE, PAPER_ROOT)
K.write_wired_child(build_kbd_mcu(), PROJECT, ROOT_UUID, TITLE, PAPER_ROOT)
K.write_wired_child(build_rgb_power(), PROJECT, ROOT_UUID, TITLE, PAPER_ROOT)
K.write_wired_child(build_main_if(), PROJECT, ROOT_UUID, TITLE, PAPER_ROOT)

sym, wiring, pro_sheets = build_root_strings()
K.write_root(PROJECT, PROJ_DIR, ROOT_UUID, TITLE, sym, wiring, pro_sheets,
             paper=PAPER_ROOT)
