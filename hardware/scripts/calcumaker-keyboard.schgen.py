#!/usr/bin/env python3
"""Regenerate the Calcumaker 16 **keyboard board** hierarchical schematic.

    CALCUMAKER_SCHGEN_DRAFT_OK=1 python3 scripts/calcumaker-keyboard.schgen.py
    (or: CALCUMAKER_SCHGEN_DRAFT_OK=1 make gen-calcumaker-keyboard)

*** DRAFT ***
The keyboard board is the **top board** of a three-board split (see DESIGN.md →
Board Partition). It stacks ABOVE the MCU board on a fine-pitch mezzanine and
carries everything front-panel: the **50-key Cherry MX matrix** (5x10 + per-key
diode), its **own STM32G031K8U6 scanner** (U1, UFQFPN-32), the **annunciator
LEDs** (f/g beside the shift keys, C/G/low-batt along the top edge), and the
mating **mezzanine header** (J1) back down to the MCU board. Splitting it off the
MCU board keeps a dense LQFP-144 away from 50 through-hole keyswitches — each PCB
gets an easy layout.

Keyscanning lives HERE (on the G0), not on the main board: only an **I2C + UART
link + a KB_IRQ wake line + power** cross the mezzanine (NOT the raw matrix). The
G0 scans + debounces + Stop/EXTI-wakes + drives the LEDs and reports (row,col)
to the U575 over I2C (see DESIGN.md Low-power & wake). PLACED not wired — wire it
in eeschema from the per-sheet notes. The MX matrix could later become a KiCad
multi-channel design (one reusable 10-key row x5), but it's placed flat for now.

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
ROOT_UUID = "ca1c0000-0000-4000-8000-00000000eb01"   # keep stable across regens

# ---- symbol libraries -------------------------------------------------------
K.register_stdlib("Device", "R", "C", "D", "LED")
K.register_stdlib("Switch", "SW_Push")
K.register_stdlib("Connector_Generic", "Conn_02x05_Odd_Even")   # DF40 mezzanine to MCU (J1)
K.register_stdlib("Connector", "Conn_ARM_SWD_TagConnect_TC2030-NL")   # G0 SWD (J2)
K.register_stdlib("MCU_ST_STM32G0", "STM32G031K8Ux")   # keyboard scanner MCU (UFQFPN-32)

# ---- footprint shorthands ---------------------------------------------------
R0402 = "Resistor_SMD:R_0402_1005Metric"
C0402 = "Capacitor_SMD:C_0402_1005Metric"
C0603 = "Capacitor_SMD:C_0603_1608Metric"
SOD123 = "Diode_SMD:D_SOD-123"
LED0603 = "LED_SMD:LED_0603_1608Metric"
MX_FP = "Button_Switch_Keyboard:SW_Cherry_MX_1.00u_PCB"   # plate/PCB-mount 1u; Kailh hot-swap variant optional
G0_FP = "Package_DFN_QFN:UFQFPN-32-1EP_5x5mm_P0.5mm_EP3.5x3.5mm"
SWD_FP = "Connector:Tag-Connect_TC2030-IDC-NL_2x03_P1.27mm_Vertical"
MEZZ_HEADER_FP = "Connector_Hirose_DF40:Hirose_DF40C-10DP-0.4V_2x05-1MP_P0.4mm"


def R(ref, val):
    return dict(ref=ref, lib_id="Device:R", value=val, fp=R0402,
                lcsc={"470": "C25117", "10k": "C25744"}.get(val, ""))


def C(ref, val, fp=C0402):
    return dict(ref=ref, lib_id="Device:C", value=val, fp=fp,
                lcsc={"100nF": "C1525"}.get(val, ""))


# ============================ Keypad sheet ===================================
# Wide HP-16C-style layout: 5 rows x 10 cols = 50 full-size Cherry MX keys, with
# f/g shifts (3 functions per key). See DESIGN.md "Keypad" for the keymap.
# Matrix: ROWr (G0 GPIO out) -> SW -> 1N4148W (anode@SW, cathode@COLc) -> COLc
# (G0 GPIO in, internal pull-up). Scanned LOCALLY by the on-board STM32G0 (U1) —
# the matrix does NOT cross the mezzanine. Key (row r, col c): index = (r-1)*10 +
# c => SW1..SW50, D1..D50 (this board has no PSU, so diodes = D1..).
KEY_SW = [dict(ref="SW%d" % i, lib_id="Switch:SW_Push", value="MX", fp=MX_FP)
          for i in range(1, 51)]
KEY_D = [dict(ref="D%d" % i, lib_id="Device:D", value="1N4148W", fp=SOD123,
              lcsc="C81598", mpn="1N4148W", mfr="onsemi") for i in range(1, 51)]
KEYPAD = dict(name="Keypad", file="keypad.kicad_sch",
    title="Cherry MX key matrix (5x10, 50 keys)", page="2",
    big=KEY_SW, small=KEY_D,
    note=(15, 130, "Calcumaker 16 keyboard — Keypad: 50 Cherry MX keys in a 5x10 "
          "scanned matrix (wide HP-16C layout + f/g shifts; keymap in DESIGN.md). "
          "PLACED, not wired. WIRING: ROW1..ROW5 + COL1..COL10 go to the on-board "
          "STM32G0 (U1, KbdMCU sheet) — ROWs = G0 GPIO outputs; COLs = G0 GPIO "
          "inputs with INTERNAL pull-ups (no external R, kept in Stop). Each key "
          "SWn in series with Dn (anode at switch, cathode to its COL) for n-key "
          "rollover. Key (row r, col c): SW#=(r-1)*10+c, D#=(r-1)*10+c. Scan: the "
          "G0 drives one ROW low, reads COLs; reports (row,col) to the U575 over "
          "I2C. WAKE: the G0 holds all ROWs low + EXTI on the COLs -> any keypress "
          "wakes the G0 from Stop, which then asserts KB_IRQ to wake the U575 (see "
          "DESIGN.md Low-power & wake). Optional Kailh hot-swap sockets."))

# ======================= Annunciator LEDs sheet ==============================
# Front-panel status lamps (DESIGN.md Open Q6): f (gold) + g (blue) BESIDE the
# shift keys, C (carry) + G (overflow) + low-battery along the top edge under the
# display bezel. Each = an on-board G0 GPIO (active high) -> Rn 470R -> LED ->
# GND. They live here (not on the hidden MCU board) because they're visible
# indicators next to the keys; the U575 pushes their state to the G0 over I2C.
# 470R @3V3 = ~1.3mA (blue) .. ~2.8mA (red/yellow).
def ALED(ref, val, lcsc, mpn, mfr):
    return dict(ref=ref, lib_id="Device:LED", value=val, fp=LED0603,
                lcsc=lcsc, mpn=mpn, mfr=mfr)


ANNUNC = dict(name="Annunciators", file="annunc.kicad_sch",
    title="Annunciator LEDs (f g C G low-batt)", page="3",
    big=[],
    small=[
        ALED("D51", "f (yellow)", "C72038", "19-213/Y2C-CQ2R2L/3T(CY)", "Everlight"),
        ALED("D52", "g (blue)", "C965807", "XL-1608UBC-04", "XINGLIGHT"),
        ALED("D53", "C (red)", "C2286", "KT-0603R", "KENTO"),
        ALED("D54", "G (red)", "C2286", "KT-0603R", "KENTO"),
        ALED("D55", "LOWBAT (red)", "C2286", "KT-0603R", "KENTO"),
        R("R1", "470"), R("R2", "470"), R("R3", "470"),
        R("R4", "470"), R("R5", "470"),
    ],
    note=(15, 105, "Calcumaker 16 keyboard — Annunciator LEDs (DESIGN.md "
          "status-line mapping). PLACED, not wired. Five drive lines from the "
          "ON-BOARD STM32G0 (U1, active high), each -> Rn 470R -> LED -> GND: D51 "
          "'f' yellow + D52 'g' blue mounted BESIDE the f/g keys; D53 'C' carry, "
          "D54 'G' overflow, D55 low-battery along the top edge under the display "
          "bezel. +3V3 feeds the resistors; GND is the return. 470R = ~1.3-2.8mA; "
          "adjust per color at bring-up. Firmware: the U575's calcumaker-core App "
          "drives f/g from keys::Shift, C/G from Calc::carry()/overflow(), LOWBAT "
          "from the battery ADC, and pushes the 5-bit state to the G0 over I2C; the "
          "G0 sets the pins. Radix/STATUS/errors render in the 7-seg digits."))

# ======================= Keyboard scanner MCU sheet ==========================
# STM32G031K8U6 (LCSC C432207, UFQFPN-32) scans the 5x10 matrix, drives the 5
# annunciator LEDs, and talks to the U575 over I2C (+ UART) across the mezzanine.
# It Stop-sleeps between keystrokes and wakes on a column EXTI, then asserts
# KB_IRQ to wake the U575 (see DESIGN.md Low-power & wake). Internal HSI clock —
# no crystal. Reflash via SWD (J2 Tag-Connect) or the ROM UART/DFU bootloader
# over the mezzanine (KB_BOOT0/KB_NRST).
KBD_MCU = dict(name="KbdMCU", file="kbd_mcu.kicad_sch",
    title="Keyboard scanner MCU (STM32G031K8U6)", page="4",
    big=[
        dict(ref="U1", lib_id="MCU_ST_STM32G0:STM32G031K8Ux", value="STM32G031K8U6",
             fp=G0_FP, lcsc="C432207", mpn="STM32G031K8U6", mfr="STMicroelectronics"),
    ],
    small=[
        C("C1", "100nF"), C("C2", "100nF"),         # VDD decoupling
        C("C3", "100nF"),                           # VDDA
        C("C4", "4.7uF", C0603),                    # bulk
        C("C5", "100nF"),                           # NRST
        R("R6", "10k"),                             # BOOT0 pulldown (boot to app)
        dict(ref="J2", lib_id="Connector:Conn_ARM_SWD_TagConnect_TC2030-NL",
             value="SWD TC2030-NL", fp=SWD_FP),
    ],
    note=(15, 150, "Calcumaker 16 keyboard — Scanner MCU U1 STM32G031K8U6 (LCSC "
          "C432207, UFQFPN-32). PLACED, not wired. POWER: VDD -> +3V3 (C1/C2 100nF "
          "+ C4 4.7uF bulk); VDDA -> C3 100nF; VSS/EP -> GND. RESET: NRST + C5 "
          "100nF (also -> KB_NRST on J1 so the MCU can reset it). BOOT0 -> R6 10k "
          "to GND (also -> KB_BOOT0 on J1 for ROM UART/DFU reflash). CLOCK: "
          "internal HSI, no crystal. MATRIX: 5 ROW (out) + 10 COL (in, internal "
          "pull-ups) -> Keypad sheet; hold ROWs low + COL EXTI for wake-from-Stop. "
          "ANNUNCIATORS: 5 GPIO -> the LED resistors (Annunciators sheet). LINK to "
          "the U575 (via J1 mezzanine): I2C1 SDA/SCL (reports keys, receives "
          "annunciator state) + USART TX/RX (alt/expansion + bootloader) + KB_IRQ "
          "out (wakes the U575). PROGRAMMING: J2 SWD Tag-Connect (bare pads) or the "
          "UART/DFU bootloader over J1. See DESIGN.md Low-power & wake."))

# ===================== Main-board mezzanine sheet ============================
# The mating half of the LOW-PROFILE board-to-board stack. J1 = Hirose DF40 2x5
# (10-pin) 0.4mm HEADER (DF40C-10DP, LCSC C424635); the MCU board carries the
# receptacle (DF40C-10DS C424636). DF40C = 1.5mm stack height. Only a serial link
# + power cross it (the matrix stays on this board's G0). Pin-for-pin identical
# assignment to J5 (mated pin N <-> N).
MAIN_IF = dict(name="MainIF", file="main_if.kicad_sch",
    title="MCU mezzanine (I2C + UART link down to the MCU board)", page="5",
    big=[
        dict(ref="J1", lib_id="Connector_Generic:Conn_02x05_Odd_Even",
             value="TO MCU", fp=MEZZ_HEADER_FP,
             lcsc="C424635", mpn="DF40C-10DP-0.4V(51)", mfr="Hirose"),
    ],
    small=[],
    note=(15, 105, "Calcumaker 16 keyboard — MCU mezzanine (J1 = Hirose DF40 2x5 "
          "0.4mm HEADER DF40C-10DP-0.4V, LCSC C424635). Mates DOWN to the MCU "
          "board's receptacle (calcumaker-mcu J5 DF40C-10DS-0.4V C424636); "
          "LOW-PROFILE 1.5mm stack. PLACED, not wired. PINOUT (MUST match "
          "calcumaker-mcu J5 exactly): 1=+3V3, 2=GND, 3=I2C_SDA, 4=I2C_SCL, "
          "5=KB_UART_TX, 6=KB_UART_RX, 7=KB_IRQ, 8=KB_NRST, 9=KB_BOOT0, 10=GND. "
          "SDA/SCL <-> the G0's I2C1; UART_TX/RX <-> the G0's USART; KB_IRQ = G0 -> "
          "U575 wake (keypress); KB_NRST/KB_BOOT0 = U575 -> G0 (reset + bootloader "
          "reflash). MECH: at 1.5mm stack keep the MX pins off the MCU-board area "
          "(trim, or MCU board under a keyless region). Verify DF40C-10DP land vs "
          "the KiCad DF40 2x5 footprint + the 3D stack at layout. See DESIGN.md "
          "Board Partition + Low-power & wake."))

# ============================ generate =======================================
K.build(
    project="calcumaker-keyboard", proj_dir=PROJ_DIR, root_uuid=ROOT_UUID,
    title=dict(title="Calcumaker 16 — Keyboard", date="2026-07-05", rev="0.2",
               company="calcumaker authors",
               comments=["Programmer's/technical arbitrary-precision RPN calculator",
                         "Keyboard board: Cherry MX matrix + STM32G0 scanner + annunciators + MCU mezzanine (DRAFT)"]),
    sheets=[KEYPAD, ANNUNC, KBD_MCU, MAIN_IF],
)
