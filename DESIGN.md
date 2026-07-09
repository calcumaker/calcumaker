# Calcumaker 16 — Design Document

> Repo: `calcumaker` · Product: **Calcumaker 16** (see `NAMING.md`).
> Future personalities / HP-variant modes are **planned** (not implemented)
> in [`DESIGN-MODES.md`](DESIGN-MODES.md).

## Overview

Calcumaker 16 is a wide-format, full-size **Cherry MX** **programmer's /
technical RPN calculator**. It follows the **HP-16C** lineage — hexadecimal /
octal / binary / decimal entry, bitwise and shift/rotate operators, and
selectable word sizes — and extends it with **arbitrary-precision** math:

- **GNU MP (libgmp)** for unbounded integers (the programmer side: huge
  hex/decimal values, exact bitwise);
- **MPFR (libmpfr)** for correctly-rounded floating point and transcendental
  functions (the scientific side), at user-selectable precision.

The top of the RPN stack is shown on a **multi-row 7-segment** display (**2–3
rows**) carried on its **own angled PCB** — part of a **three-board split**
(MCU board + keyboard board stacked on a mezzanine, + the angled display board);
only power and the display serial bus cross that interconnect. The device is
**battery + USB-C** powered, on the **low-power STM32U575**. The firmware main
loop is **Rust (`no_std`)**.

This document is the source of truth for the hardware and firmware design. It
follows the structure of the sibling BenchBits repos (ephemerkey / notchdeck /
tsumikoro). The repo's firmware departs from those (which are C / Zephyr) by
being Rust.

---

## Resolved Decisions (from project kickoff)

| # | Decision | Choice |
|---|----------|--------|
| 1 | Math stack | **GMP + MPFR only — single path, no fallback.** Engine = `calcumaker-core` over our own no_std bindings `gmp-mpfr-nostd` (host: links system GMP/MPFR; target: cross-built). One `no_std` crate, host-tested + REPL. |
| 2 | Power | **1S Li-ion + USB-C charging + buck-boost**; "low power" = long battery life via aggressive sleep between keystrokes. |
| 3 | Display | **Stacked RPN registers, 2–3 rows (option)** — multi-row 7-segment showing the top of the stack. Driver + digit parts chosen by LCSC price/availability. |
| 4 | Keypad | **Wide HP-16C-style layout** for programming / technical / engineering use; full-size Cherry MX. |
| 5 | Firmware language | **Rust, `no_std`** main loop (async via embassy once MCU is pinned). |
| 6 | Firmware license | **AGPL-3.0** (repo LICENSE) — compatible with LGPLv3 GMP/MPFR. |
| 7 | MCU | **STM32U575RGT6** (Cortex-M33, **1 MB** / 768 KB, **LQFP-64**, ULP) + **4 MB quad-SPI NOR on OCTOSPI1**. Firmware measured ~323 KB → 1 MB is ample; the matrix scans off-board so LQFP-64 has enough GPIO (smaller/cheaper). Same U575 die (USB FS, OCTOSPI, GMP/MPFR-capable). LCSC C5270980. Target `thumbv8m.main-none-eabihf`. |
| 8 | Board partition | **Three boards: `calcumaker-mcu` + `calcumaker-keyboard` (DF40 mezzanine-stacked above it) + `calcumaker-display` (angled, 0.5 mm FFC).** Keeps a dense LQFP-64 off the 49-key through-hole matrix. The keyboard has its **own STM32G0 scanner**, so only an **I²C+UART link + power** cross the mezzanine (not the raw matrix); the display bus + power cross the FFC. |
| 9 | Hardware license | **CERN-OHL-S v2** (`hardware/LICENSE`) — strongly reciprocal, matches the AGPL copyleft posture. |
| 10 | Product name | **Calcumaker 16** (see `NAMING.md`). |

---

## Architecture Decisions

### MCU: large-flash / low-power STM32 (Cortex-M4F or M33)

The dominant sizing driver is the arbitrary-precision math. GMP + MPFR together
are on the order of **~0.5–1 MB of flash**, but their heap needs are **modest**
for a calculator — even very high working precision is a few KB per value, so a
4-level stack plus registers and MPFR scratch is comfortably in the **tens of
KB**. So the real constraint is **flash ≥ ~1–2 MB** (to keep the real GMP/MPFR
option open); **~128–320 KB RAM is plenty**. An FPU helps the (mostly software)
multi-precision arithmetic at the margins and is standard on the candidates.
Low power and a USB FS device (console / provisioning) round out the list.

> **Cost matters — this is a calculator, not a workstation.** The 4 MB / 3 MB
> top-end U5 (U5G9) is far more part than this needs and is expensive; it has
> been dropped as the headline pick.

**SELECTED: STM32U575RGT6 (Cortex-M33, 1 MB flash / 768 KB SRAM, LQFP-64, ULP),
plus a 4 MB quad-SPI NOR on OCTOSPI1.** Same U575 die (FPU + USB FS + OCTOSPI +
TrustZone, ST's current ULP line), chosen on **availability + fit**: the L4R5 is
effectively unavailable (~5 pcs), while the U575 family is well stocked and
GMP/MPFR-capable. The **firmware links at ~323 KB** (full engine + GMP + MPFR,
*measured* — see Numeric core), so **1 MB flash is ample (~3× headroom)** — no
need for the pricier/scarcer 2 MB `I` parts. With the **key matrix scanned
off-board** (the keyboard G0), the MCU needs far fewer GPIO, so the smaller
**LQFP-64 (10×10 mm)** fits — cheaper and suited to the low-profile stack. A
**4 MB (32 Mbit) quad-SPI NOR** (W25Q32JVSSIQ, LCSC C179173) on **OCTOSPI1** adds
memory-mapped (XIP) storage for constant tables + state persistence / future
keystroke programs. Target `thumbv8m.main-none-eabihf`; HAL `embassy-stm32`
(`stm32u575rg`).

> **Flash-code gotcha:** the STM32 `G` size code = **1 MB** (the `I` parts are
> 2 MB). Earlier notes mislabeled the ZGT6 as "2 MB"; the ZGT6/VGT6/RGT6 are all
> **1 MB** — which the measured 323 KB firmware fits with room to spare.

**Availability ladder (live LCSC/JLCPCB via jlcsearch; all JLCPCB "Extended"):**

| Part | LCSC# | Pkg | Flash / RAM | Stock | Fit |
|------|-------|-----|-------------|-------|-----|
| **STM32U575RGT6** | C5270980 | LQFP-64 (10×10) | 1 MB / 768 KB | 345 | ✅ selected — smaller pkg, matrix off-board |
| STM32U575VGT6 | C5270988 | LQFP-100 (14×14) | 1 MB / 768 KB | 17k | best-stocked U575 — drop-in if more GPIO wanted |
| STM32U575ZGT6 | C5271004 | LQFP-144 (20×20) | 1 MB / 768 KB | 230 | prior pick (oversized once the matrix left) |
| STM32L4R5ZIT6 | C1339786 | LQFP-144 | 2 MB / 640 KB | 5 | GMP-capable but ~unstocked → no-go |
| STM32U575RIT6 | C5270992 | LQFP-64 | 2 MB / 768 KB | 26 | the `I`/2 MB variant if ever needed — thin stock |

> Other L4R5 packages (LQFP-100 VIT6, UFBGA QIY6) returned **no LCSC listing**.
> Stock/price are point-in-time (fetched during scaffolding); re-check at order.

Considerations once pinned:
- **SRAM may be banked** (esp. on U5). For a single contiguous heap, use the
  largest contiguous span or place the heap section explicitly.
- **External memory:** internal RAM is ample, but a **4 MB quad-SPI NOR is now
  fitted on OCTOSPI1** (U7, `QSPIFlash` sheet) for memory-mapped constants +
  state/program storage (not for RAM).
- **USB FS** for a CDC console / provisioning + firmware update.

### Board partition: three boards (MCU + keyboard stacked, display cabled)

Calcumaker 16 is **three PCBs** (revised 2026-07-05 — the keyboard split off the
MCU board):

- **`calcumaker-mcu`** — the brain/PSU board: MCU (U575), PSU, clock, SWD, the
  display 5 V rail + level shifter + interconnect, and a **keyboard mezzanine**
  (J5). This is the dense fine-pitch SMT board (LQFP-64). *Bottom of the stack.*
- **`calcumaker-keyboard`** — the front-panel board: the 49-key Cherry MX matrix (2U ENTER)
  + per-key diodes + the annunciator LEDs + the mating **mezzanine header** (J1).
  A simple 2-layer through-hole board. *Stacks directly above the MCU board.*

  **The 2U ENTER.** ENTER is a **double-height (2U) keycap**, like every HP
  Voyager. It occupies two cells of the 5×10 grid — rows 3 and 4 of column 5
  (0-based) — but has a **single switch**, wired to the **lower cell's row line**
  (`KB_ROW5` × `COL6`), so ENTER keeps its matrix position and the firmware scan
  is unchanged. The other cell gets **no switch, diode, or RGB LED**: the grid is
  50 cells but the board is **49 keys / 49 LEDs**.

  *Physical placement (≠ the logical cell).* A 2U keycap's stem is **centered on
  the key**, so the switch body — and its RGB LED, which shines up through the
  switch's north window — sits on the **boundary between the two rows**, i.e.
  9.525 mm (½U) from either 1U cell center, at the column-5 x. The row/col
  assignment above is a *net* assignment, not a coordinate; the layout must place
  this one switch off the 1U grid.

  **Stabilizer.** A 2U key wants a stabilizer or it rocks. Two options, and the
  choice decides whether the PCB changes at all:
  - **Plate-mount (recommended)** — the stab clips into the switch plate we
    already need for the hot-swap sockets. KiCad's `SW_Cherry_MX_2.00u_Vertical_Plate`
    has *exactly the same pads as the 1u plate footprint* — **no PCB holes**. So
    the existing `calcumaker:SW_MX_HS_CPG151101S11_1u` footprint is reused as-is
    for ENTER; only the plate cutout and the keycap change. Keep the area under
    the stab wire clear (our SK6812s are reverse-mounted on the bottom, so it is).
  - **PCB-mount** — needs four NPTH holes, symmetric about the switch center at
    **y = ±11.90 mm** (23.8 mm stab spacing), each wing a **3.05 mm** hole at
    x = −7.00 mm and a **4.00 mm** hole at x = +8.24 mm (15.24 mm apart). Stock
    `Button_Switch_Keyboard:SW_Cherry_MX_2.00u_Vertical_PCB` has this pattern but
    is **solder-in**, so the hot-swap 2U variant is **vendored** as
    `calcumaker:SW_MX_HS_CPG151101S11_2u_Vertical` (our 1u hot-swap footprint plus
    those four holes; its origin is already the switch center). Place-on-back is
    safe: mirroring X maps the stab-hole set onto itself rotated 180°, and a 2U key
    is symmetric. Choosing PCB-mount would **also** force a **Row5 variant sheet**,
    since ENTER's switch sits on the shared 10-key sheet and multi-channel instances
    must share footprints — another reason to prefer plate-mount.

  The stabilizer itself is **mechanical hardware** (Cherry/Durock 2U, plate- or
  PCB-mount), not a schematic symbol — it carries no net and appears only in the
  layout + the mechanical BOM.
  - Firmware: `keys.rs` marks that cell `Key::Absent` (≠ `Key::Nop`, a real key
    with no function) in every layer of every personality; `ENTER_SWITCH_CELL` /
    `ENTER_SPAN_CELL` / `cell_has_switch()` are the source of truth, pinned by the
    `enter_is_2u_in_every_personality` golden test.
  - Displaced functions (identical in all four personalities): **x⇄y** moves onto
    the base layer at (3,4) — it is core RPN and stays unshifted; the mode key it
    displaced there moves to **f+ENTER** (16C `WSIZE`, SCI/15C `ANGLE`, FIN `%T`);
    and **FLOAT** moves to **g+ENTER**.
  - PCB: the matrix is no longer five *identical* rows, so the multi-channel
    design instantiates the reusable 10-key sheet **four times** (Row1/2/3/5) and a
    dedicated **9-key variant sheet** (`key_row_9.kicad_sch`) once for **Row4**.
    Reference numbering keeps a hole (no `SW36` / `D36` / `D91`) rather than
    renumbering, and Row4's RGB daisy-chain closes over the gap (LED@COL5 → LED@COL7).
- **`calcumaker-display`** — the multi-row 7-segment stack + its driver ICs + the
  interconnect back to the MCU board. It **mounts at an upward angle** for
  readability, cabled (not stacked).

**Why split the keyboard off the MCU board:** laying a dense LQFP-64 (0.5 mm
pitch, needs careful fan-out / multiple layers) into the same board as 50
full-size through-hole keyswitches is a routing and mechanical fight. Splitting
gives each PCB an easy job — the MCU board is a compact SMT brain, the keyboard
is a plain switch matrix — and lets the keyboard be the physical top surface the
user types on while the MCU board hides underneath.

**Keyboard link — stack *or* cable (populate one).** Because the keyboard has its
**own STM32G0 scanner**, only a **serial link + power** cross to the MCU board,
not the raw matrix. The same 12 signals map to **two footprints on each board —
populate whichever the build needs**:

- **Stacked:** a **Hirose DF40, 0.4 mm, 2×6 (12-pin), ~1.5 mm** mezzanine —
  receptacle `J5` **DF40B-12DS-0.4V** (C3641147) on the MCU board, header `J1`
  **DF40C-12DP-0.4V** (C6224952) on the keyboard. Compact rigid sandwich.
- **Cabled:** a **16-pin 0.5 mm FFC** — `J6` (MCU) + `J3` (keyboard), both
  **AFC01-S16FCA-00** (C262665, same family as the display FFC). The flat cable
  lets the **MCU board mount anywhere in the case**, off the keyboard footprint.

**Why the cable option exists:** the DF40 stack is only ~1.5 mm (tallest DF40 is
4 mm), but the keyboard's **bottom is now crowded** — Kailh hot-swap sockets
(~1.8 mm), reverse-mount LEDs, switch bodies — so the MCU board (STM32U5 QFP + PSU)
has nowhere to sit stacked directly under the keys. The **FFC decouples its
position** and fixes that; the DF40 stays for anyone who wants the compact
sandwich. **The FFC is 16-pin so its cable can't cross-plug the 12-pin display
FFC** (and the extra pins give **VSYS ×2 + GND ×3** for the LED current, + 2 spare).

The 12 signals: `+3V3 · GND · I²C_SDA · I²C_SCL · UART_TX · UART_RX · KB_IRQ ·
KB_NRST · KB_BOOT0 · GND · VSYS · GND` — VSYS is the always-on battery/USB rail
feeding the keyboard's per-key RGB. I²C is the primary key/annunciator channel;
UART is spare/expansion + the G0's ROM bootloader; `KB_IRQ` is the keypress wake.

*Mechanical:* the **cabled** build mounts both boards to the enclosure
independently (like the display) — no clearance conflict, and the recommended
default given the crowded keyboard bottom. The **stacked** build needs the MCU
board under a keyless region (or trimmed MX/socket clearance), tall connectors
(USB-C ~3.2 mm, battery JST ~5 mm) at the board edge, and 4 corner standoffs.
(DF40C-12DS isn't LCSC-stocked so the receptacle is DF40B-12DS — still 1.5 mm: the
DF40 *suffix* sets stack height, not the B/C letter; taller DF40HC variants swap
on the same land. Verify all lands + 3D clearance at layout.)

### Per-key RGB accent lighting (keyboard)

Fifty **SK6812MINI-E** (LCSC **C5149201**, single-wire addressable RGB) —
**per-key backlight** that hints key positions / presses. It's a **reverse /
bottom-mount** part (KiCad `LED_SK6812MINI-E_3.2x2.8mm_P1.5mm_ReverseMount`): it
sits on the **bottom** of the PCB and shines **up through** a cutout into each MX
switch's north LED window. That puts the LEDs on the **same side as the Kailh
sockets → single-sided assembly** (one stencil, one reflow pass, no board flip —
cheaper JLC, and it's why we didn't use a top-emit part). All 50 daisy-chain
(DIN→DOUT) off **one G0 pin** (WS2812 protocol, ~800 kHz); refs `D56–D105`.

**Power = gated VSYS.** The LEDs are ~5 V parts and the keyboard rail is only
3.3 V, so they run off **VSYS** — the MCU board's load-shared battery/USB rail
(~3.7–4.7 V, "more volts") — brought up the **widened 12-pin mezzanine**. A
**high-side P-FET load switch** (Q1 AO3401A + Q2 2N7002) on the keyboard **gates
the whole LED rail off in sleep** (G0 `LED_EN` low → LEDs + level shifter dead,
near-zero leakage) to protect battery life. A **74LVC1G125** buffer powered from
the *gated* rail level-shifts the 3.3 V data up to the LED V_IH (0.7·V_DD —
marginal at VSYS ≈ 4.7 V on USB otherwise).

**Current budget:** 50× full-white ≈ 0.75 A would exceed the DF40 contact + VSYS
budget, so the firmware **must cap total brightness** — "hint" use lights a few
keys at a time (pressed key + neighbours), an all-keys idle glow stays dim.

### Hot-swap switches (place-on-back) — this is a hot-swap board

The keyswitches use marbastlib's **`SW_MX_HS_CPG151101S11_1u`** footprint (vendored,
CERN-OHL-P): the switch mount + the Kailh **CPG151101S11** hot-swap socket. It is a
**place-on-back** footprint — the switch footprints go on the board's **back copper
layer**, so the socket (authored on `F.Cu`) lands on the physical **bottom** and
the keycaps face up on the front.

**This is a hot-swap board.** The socket (LCSC **C41430893**, ~93k stock,
JLCPCB-assemblable) is populated on the bottom; switches **plug in**,
field-swappable. A **plate** holds the unsoldered switches (a separate mechanical
part). The socket protrudes ~1.8 mm below the PCB (enclosure/standoff detail).

> **Not solderable for a switch-only build.** The switch thru-holes are minimal
> **0.15 mm-ring pass-throughs** for the pin into the socket — **not** solder
> pads. So a switch-only / **solder-in build is not supported on this footprint**.
> A solder-in board would use the dedicated solder-in footprint
> (`SW_Cherry_MX_1.00u_PCB`, also vendored) — a **separate board revision**, not a
> BOM toggle on this one. (Earlier notes framed this as a one-board "combo" — that
> was wrong; the pass-throughs don't make a reliable solder joint.)

Wider keys (2u Enter, etc.) need **stabilizers** — marbastlib's `STAB_MX_2u` and
KiCad's own `SW_Cherry_MX_2.00u_PCB` already carry the stabilizer mounts — a
layout option for later.

Keeping the display driver *on the display board* means only **+5V, GND, and the
display serial bus** (a handful of signals) cross that connector — instead of
dozens of segment/digit lines — which is what **simplifies the display wiring**.

- **Display interconnect:** a **0.5 mm 12-conductor FFC** — connector
  **AFC01-S12FCA-00** (LCSC C262661) on both `calcumaker-mcu:J3` and every display
  board's `J1`; the flat-flex **cable is a non-assembled DigiKey accessory** —
  **GCT FFC05-TIN `05-12-A-<length>-A-4-06-4-T`**. It is now a **unified,
  technology-agnostic SPI "display-module" bus** (see the next section), pinout
  `1=VSYS, 2=VSYS, 3=GND, 4=GND, 5=+3V3, 6=SPI_SCLK, 7=SPI_MOSI, 8=SPI_CS,
  9=DISP_IRQ, 10=DISP_NRST, 11=DISP_BOOT, 12=GND` — **identical on both the 7-seg
  and RGB-matrix display boards, so they are interchangeable.** `mcu:J3` ↔
  `<display>:J1` must match; verify FFC contact orientation at layout. LED current
  for the RGB matrix does **not** cross this FFC — it takes a dedicated 2-pin VSYS
  lead (`mcu:J7` → matrix `J2`).

### Aux display: optional 128×32 OLED (the "DNP-optional aux" pattern)

**Errors and rich status on a digits-only machine** are handled in two layers:

1. **The glass is primary.** Errors show HP-16C-style codes — a transient
   `Error N` on the X row (`CalcError::code()`: 0 math domain · 1 register/
   flag · 2 bits/shift/word · 3 mode ranges · 4 too large · 5 no solution ·
   6 stack/entry · 7 dates · 8 statistics · 9 reserved for crash recovery).
   The calculator is **fully usable with no OLED**.
2. **The aux OLED is optional detail.** A 0.91″ SSD1306 128×32 I2C module on
   the **display board** (`AuxDisplay` sheet, J2 1×4 socket, **DNP by
   default**), hand-placed alongside the THT digits. Content is
   **`App::aux_lines()`** — 4 lines × 21 chars (6×8 font), ONE code path for
   the firmware panel and the emulator's mock panel: an optional
   **status-flags header** (personality/radix/angle/number-mode, then
   prec/word/sign/C/G/format/shift/pending-register — SETUP > `OLEd`
   FLAG/oFF, default on), followed by the error text (`CalcError::text()`)
   or the **full-precision X** when idle (the windowing helper). I2C runs at
   3V3 straight from the MCU across interconnect pins 8–10; pull-ups R14/R15
   (4.7 kΩ) sit on the MCU board, DNP with the OLED. Firmware sleeps the
   panel in idle (~10 µA).

**The pattern** (reusable): optional capability = a cheap socket/footprint on
the board, DNP by default, module hand-placed by builders who want it — the
base build's cost, power budget, and identity stay untouched, and the firmware
degrades gracefully (glass codes) when the part is absent.

### Display: multi-row 7-segment (RPN stack), 2–3 rows

A multi-row 7-segment array shows the **top of the RPN stack**, **3 rows × 16
digits**. The board is laid out for 3 rows with the **top row optionally
populated**, so it builds as a **2- or 3-row** display (firmware-configurable).
A 16-digit row holds a full 64-bit hex word, or a signed mantissa + exponent;
arbitrary-precision values that exceed the row width are **windowed / scrolled**.

**Selected by LCSC price/availability (research):**

- **Driver: TM1640** (LCSC C5337152, SOP-28, ~$0.12, deep stock). A 2-wire bus
  drives **16 common-cathode digits per chip = one full row**, so **3 chips**
  cover 3 rows (vs ~6 MAX7219 at ~20× the cost). Shared CLK + one DIN per chip.
  Display-only (keys live on the keyboard board). TM1638 (C19187) is the drop-in if
  on-chip key-scan is ever wanted.
- **Digits: FJ5161AH** (LCSC C8093, 0.56" **single-digit common-cathode**, ~$0.10)
  — **16 per row → 48 digits** total. FJ5161AH is a *one-digit* part (confirmed on
  LCSC), so a row is 16 discrete digits, not a 4-up module — which gives even,
  continuous digit spacing across the 16-digit number. Common-cathode matches the
  TM1640: segments a–g,dp tie to the shared **SEG1–8** bus; each digit's cathode
  goes to one of **GRID1–16**.
- **⚠ Through-hole digits.** No SMD multi-digit 7-segment displays are stocked on
  LCSC — the well-stocked parts are THT. So `calcumaker-display` needs **THT
  assembly** (JLCPCB through-hole add-on, or hand/wave solder); the TM1640s are
  SMT. See `hardware/PARTS.md`.
- **Power note:** LED 7-segment is the dominant active current draw, *not* the
  MCU — and it's drawn from +3V3 **across the interconnect**, so it gates the
  MCU board's buck-boost sizing (the TPS63900 placeholder likely needs
  upsizing). Use TM1640 brightness/dimming + blank-on-idle + display-off in
  sleep to honor the battery goal.

**Schematic = KiCad multi-channel.** Every row is electrically identical (1
TM1640 + 16 digits over the shared SEG bus), so the row is authored **once** as a
reusable, fully-wired child sheet (`display_row.kicad_sch`) and instantiated
**three times** at the root (Row1/Row2/Row3), each annotating to its own refs
(U1/DS1–16, U2/DS17–32, U3/DS33–48). The shared bus rides global nets
(**+5V / GND / DISP_CLK**); each row's serial data **DIN** is a hierarchical pin
fed by **DIN1/DIN2/DIN3** from the interconnect. This replaced the old flat sheet
that drew all three rows redundantly and ran off the page.

KiCad symbols: the **TM1640** and the **single-digit `FJ5161AH`** are both
authored in `hardware/lib/symbols/calcumaker.kicad_sym` (registered in the display
board's `sym-lib-table`); the digit land is the 0.56" single-digit
`Display_7Segment:7SegmentLED_LTS6760_LTS6780`. ⚠ An earlier scaffold wrongly
mapped FJ5161AH to the **4-digit** `Display_Character:CC56-12EWA` /
`CC56-12GWA` — that 4-digit symbol/footprint/3D is where a phantom "clock colon"
came from; the real single-digit part has none. The display board **generates,
passes the structure check, and is fully wired** (ERC clean apart from the
expected connector-fed `power_pin_not_driven` on +5V/GND). See `hardware/PARTS.md`.

### Unified display-module interface (SPI; two interchangeable modules)

The display is a **swappable module**. The MCU board exposes ONE technology-
agnostic connector (`J3`, the 0.5 mm 12-pin FFC above) and speaks ONE protocol, so
it neither knows nor cares which display is attached. Each display board carries
its **own MCU** that receives semantic *display intent* over **SPI** and renders it
locally — mirroring the keyboard's "own-STM32G0-over-a-serial-link" pattern.

Two modules exist today, both on the identical `J1` connector/pinout:

- **`calcumaker-display` (7-seg):** an **STM32G031** (SPI slave) drives the 3
  TM1640s locally; it makes its own 5 V (TPS61022 boost) and level-shifts
  CLK+DIN1/2/3 3V3→5V (74HCT125) **on the module** — which is why those parts left
  the MCU board. Powered by VSYS/+3V3 from the FFC.
- **`calcumaker-matrix` (RGB dot-matrix):** an **RP2040** (PIO = the ideal WS2812
  engine) drives **96×24 = 2304× 1 mm** addressable RGB (XL-1010RGBC-2812B-S) in 3
  chains, built as a **nested multi-channel** design — an 8×8 `led_cluster` ×12 →
  `led_row` (768 px, one chain) ×3 → board. LED current (amps) comes on a
  **dedicated VSYS lead** (mcu `J7` → matrix `J2`), never the FFC. It also delivers
  the full alphanumerics / scrolling / color the 7-seg can't (so the 14-seg idea
  was dropped — genuine single-digit 14-seg is poorly stocked on LCSC and the
  matrix covers that need). 4-layer board at ~1.5 mm pitch; firmware caps brightness.

**The SPI "display intent" contract** (firmware, deferred): the U575 is SPI master
and writes a compact frame — the `App` display rows (text + dp/marker attributes),
annunciator/flag/mode state, and aux-OLED content. Each module MCU decodes it and
renders natively (7-seg glyphs via `core::seg7`; a pixel framebuffer + font on the
RP2040), keeping ONE glyph source-of-truth. `DISP_IRQ` = module→MCU ready/attention;
`DISP_NRST`/`DISP_BOOT` let the U575 reflash the module MCU (STM32 NRST/BOOT0;
RP2040 RUN/BOOTSEL). Any future display is a new module — no MCU-board change. See
`PARTS.md` for the per-board BOMs.

### Keypad: full-size Cherry MX, wide HP-16C-style layout

**5 rows × 10 columns = 50 full-size Cherry MX keys** (≈190 × 95 mm — authentically
wide), in the HP-16C / Voyager tradition with **f (gold) / g (blue)** shifts
(3 functions per key). Power is a **slide switch** (not in the matrix); any
keypress wakes the MCU from Stop, so no dedicated ON key is needed.

> **Width is deliberate.** Tighter packings were considered — 8×6 (≈152 mm, A–F
> as a 3×2 block) and 7×6 (≈133 mm) — but the full **10-wide** Voyager face was
> chosen for authenticity and one-function-per-key clarity, accepting the ~190 mm
> board width that full-size keys imply.

**Base (unshifted) faces:**

```
 SIN   COS   TAN   LN    √x    yˣ    1/x   EEX   ⌫     CLx
  A     B     C     D     E     F     7     8     9     ÷
 AND   OR    XOR   NOT   SL    SR    4     5     6     ×
 HEX   DEC   OCT   BIN   WSIZE x⇄y   1     2     3     −
  f     g    STO   RCL   R↓    ENTER 0     .     CHS   +
```

- Right 4 columns = numeric keypad + operators; **A–F** sit above 7-8-9 as the
  hex extension.
- **f (gold)** → inverse / advanced: ASIN/ACOS/ATAN, eˣ, x², **PREC** (set
  arbitrary-precision working digits), π, LASTx; over A–F → bit set/clear/test,
  MASKL/MASKR, bit-count; over AND…SR → RL/RR/ASR/RMD; over HEX/DEC/OCT/BIN →
  FLOAT; WSIZE → sign mode (unsigned / 1's / 2's); R↓ → R↑.
- **g (blue)** → secondary: SINH/COSH/TANH, LOG, 10ˣ, n!, %, RND (real → int);
  over HEX/DEC/OCT/BIN → **FIX/SCI/ENG/auto** (display format over display
  base; digit count from X). OFF sits on the f layer.
- The full keymap is the source of truth in
  **`firmware/calcumaker-core/src/keys.rs`** (`BASE` / `LAYER_F` / `LAYER_G`) —
  keep the two in sync. It lives in the core (not the board crate) because the
  emulator and the firmware share it; the f/g shift resolution
  (`keys::Shift`) and all key handling (`App`) sit beside it. Shift assignments
  marked `Nop` are open for refinement.
- **Visual diagrams:** `doc/keymap-16c.txt` / `doc/keymap-sci.txt` — ASCII
  key grids **generated from keys.rs** (`cargo run --example keymaps`), one
  per personality, freshness-enforced by a golden test so they can't drift.
  These are the reference for keycap-legend planning.

**Electrical:** 5-row × 10-col scanned matrix, scanned **on the keyboard board by
its own STM32G031K8U6** (LCSC C432207, UFQFPN-32 — not the main MCU; see Board
Partition). ROWr = G0 GPIO outputs, COLc = G0 GPIO inputs on **internal pull-ups**
(no external resistors — lower idle current; the G0 retains pull-ups in Stop).
**One 1N4148W per key** (anode at switch, cathode to its column) for n-key
rollover. 15 GPIO (5 rows + 10 cols) on the G0. Refs: `SW1..SW50` (key `(r,c)` =
`SW(r-1)*10+c`), diodes `D1..D50`. Optional **Kailh hot-swap sockets** (same
footprint family). The G0 reports `(row,col)` events to the main MCU over the
mezzanine I²C bus (see Low-power & wake).

### Low-power & wake

Splitting the scanner onto the keyboard G0 gives **two independently-sleeping
domains**, both wake-capable; between keystrokes both sit in **Stop** at a few µA,
so the aggressive-sleep battery goal is preserved (the display + active MPFR
still dominate the budget).

- **Idle:** the **U575** is in **Stop 2** (~1–3 µA); the keyboard **G0** is in
  **Stop** (~1–5 µA), holding **all matrix rows low** with **columns on EXTI/wake
  pins** (internal pull-ups) — the classic keypress-wake trick, now on the G0.
  The G0 must Stop-and-wake, **never poll-scan continuously** (that would burn mA).
- **Key-on-wake is a two-stage chain:** keypress → a column pulls low → **G0
  wakes** (EXTI) → G0 scans + debounces → asserts **`KB_IRQ`** (mezzanine line)
  into a U575 **WKUP/EXTI** pin → **U575 wakes from Stop** → U575 reads the
  `(row,col)` event(s) from the G0 over **I²C**. Latency is sub-millisecond — the
  mechanical key is still down when the G0 scans, so nothing is missed.
- **Reverse direction is free:** the G0 also **wakes from Stop on I²C
  address-match**, so the U575 can push annunciator-LED states to the G0 without a
  dedicated wake line. During active typing both stay awake and I²C runs normally.
- **Power switch:** a **slide switch** gates power; there is no dedicated ON key
  (any keypress wakes via the chain above). *Optional robustness:* a wired-OR
  "any-key-down" line straight to a U575 WKUP pin would let the U575 wake
  independently of the G0 (one extra signal + a small diode-OR); `KB_IRQ` alone is
  the default.
- **Firmware split:** matrix scan + debounce + Stop/EXTI-wake + annunciator drive
  live in a **new keyboard-G0 firmware** (`embassy-stm32`, `stm32g031`,
  `thumbv6m`); the U575 firmware becomes an **I²C reader of `(row,col)` events**
  instead of a direct scanner. **`calcumaker-core::App` is unchanged** — it
  consumes `(row,col)` either way — so the engine, keymap, and emulator are
  unaffected.

### Power: 1S Li-ion + USB-C charge + buck-boost

Mirrors the ephemerkey power path (proven in the sibling repo):
USB-C (sink) → ESD → Li-ion charger → load-share → buck-boost → 3V3. Sized up
for the larger active load (display + MCU running MPFR). See **Power Tree**.

### Numeric core: GMP + MPFR, single path (no fallback)

The calculator engine lives in **`firmware/calcumaker-core/`** — a plain library
(RPN stack + the `Value` = arbitrary-precision int/real) over **GNU MP + MPFR**.
There is **one** numeric path: the pure-Rust fallback was dropped. On the host
the engine talks to GMP/MPFR through our own **`gmp-mpfr-nostd`** crate (thin
`no_std` FFI — *like `rug`, but for a `no_std` world*). On the host it links the
system GMP/MPFR and is fully unit-tested + runnable (`cargo run --example repl`);
the **same `no_std` crate** also compiles for the MCU, where the firmware links
the cross-built GMP/MPFR. See **Numeric Core**, **Host development & testing**,
and **GMP/MPFR on the target** below.

---

## Software Stack (Rust, no_std)

| Layer | Choice | Notes |
|-------|--------|-------|
| Toolchain | stable Rust, target `thumbv8m.main-none-eabihf` (M33 / U575) | `thumbv7em-none-eabihf` if an L4+ (M4F) is used |
| Runtime | `cortex-m`, `cortex-m-rt` | super-loop now; → embassy executor later |
| HAL | **`embassy-stm32`** (async), feature `stm32u575zg` | `stm32u5` PAC underneath |
| Heap | **`embedded-alloc` (TLSF)** | TLSF handles variable-size bignum churn with less fragmentation than LLFF |
| Flash/debug | **`probe-rs`** (`cargo run`/`cargo embed`) | set the chip name in `.cargo/config.toml` |
| Logging | `defmt` + RTT (optional) | |
| Math bindings | **`gmp-mpfr-nostd`** (own no_std FFI) | host links system GMP/MPFR; target links cross-built |
| Engine + app | **`calcumaker-core`** (RPN, no_std) | one path; engine (`Calc`), keymap + shifts (`keys`), key handling + entry editing (`App`), 7-seg encoding (`seg7`) |
| Emulator | **`calcumaker-emu`** (host, crossterm) | the same `App` on a terminal — ASCII 7-seg from the real TM1640 segment bytes |

---

## Host development & testing (works today)

The engine's math goes through our own **`gmp-mpfr-nostd`** crate — thin `no_std`
FFI bindings to GMP/MPFR (`Integer` = `mpz`, `Float` = `mpfr`), *like `rug` but
for a `no_std` world*. On the host its `build.rs` links the **system / Homebrew**
GMP + MPFR (no build-from-source), so the engine is a normal library you develop
and test on the desktop against the **real** C libraries — and it builds in
under a second:

```sh
brew install gmp mpfr                 # one-time host deps (apt: libgmp-dev libmpfr-dev)
cd firmware/calcumaker-core
cargo test                            # engine + app + seg7 tests vs real GMP/MPFR
cargo run --example repl              # interactive RPN (token REPL)
```

Crucially this is **one crate, `no_std`** — it also compiles for the MCU target
(`cargo build --target thumbv8m.main-none-eabihf` succeeds today); only the final
link to the C libraries differs. This is the single source of truth for the
calculator logic and math.

## Emulator (host CLI) — the device UI without the device

**`firmware/calcumaker-emu/`** runs the calculator on a standard terminal. It
is not a mock: the whole device-independent calculator lives in
`calcumaker-core` and the emulator hosts it exactly as the firmware will —

- **`keys`** — the 49-key matrix keymap + f/g shift layers + resolution
  (`Shift`), the design source of truth;
- **`App`** — key handling on top of the engine: HP-style digit-by-digit entry
  (live `_` cursor, backspace, EEX, CHS-in-entry flips the mantissa/exponent
  sign), flush-on-operation, CLx/ENTER semantics, error → status message;
- **`seg7`** — text → per-digit **TM1640 segment bytes** (a..g+dp bit layout,
  `.` folds into the previous digit's dp, right-aligned, `]`-shaped overflow
  marker in the last cell when a value exceeds 16 digits).

Both frontends are thin I/O bindings around `App::press(row, col)` +
`App::seg_rows()`: the firmware path contributes keyboard-board events and the
TM1640 bus; the emulator maps host keys to matrix cells and renders **the same
segment bytes** as LED-style 7-seg art — Unicode block elements by default
(`▄`/`█`, dp = `▗` in its own column + an inter-digit gap, ~100 columns),
`--ascii` for a plain `_`/`|` fallback — plus
annunciators and the untruncated X, where the arbitrary precision is visible.
If it works in the emulator, the only difference on the device is GPIO.

```sh
cd firmware/calcumaker-emu
cargo run                            # interactive; ? = key map, Ctrl-C = quit
cargo run -- --press "2;3+"          # scripted: 2 ENTER 3 + → prints the frame
cargo run -- --prec 1024 --press "FE"  # f-shift E = pi, at 1024 bits
```

Display policy (initial, revisit with real glass): X (or the live entry) on the
bottom row, Y/Z above; values right-aligned. **AUTO-mode reals are
display-rounded to the 16-digit window** (correctly, by MPFR — HP behaviour:
the glass rounds, the register keeps full precision; a value a hair under
382.1 shows `382.1`, not `382.09999…`; exponent-bound values go scientific
with maximal digits). The emulator's `X:` line is the SHOW view — X at full
precision. Integers and explicit FIX/SCI/ENG wider than the row truncate with
the overflow marker, and the **window keys** (16C `<`/`>`, g-shift SL/SR)
scroll X through the rest: window 0 = 15 cells + marker, window k ≥ 1 picks up
exactly where the marker cut off (every digit reachable — tested by
reassembly); any other key resets the view; `win k/N` annunciator. Engine modes are RPN
postfix like the HP-16C: `<bits> W` (WSIZE, 0 = unbounded), f-shift `I`
(= `prec`, pops X as the MPFR working precision), f-shift WSIZE cycles the sign
mode, g-shift over the radix keys sets FIX/SCI/ENG/auto (digits from X). The
annunciator line shows radix, prec, word size + sign mode, the **C**/**G**
flags, the format, a pending f/g shift or STO/RCL, and error blips.
Runtime configuration lives in the **SETUP menu** (g-shift CLx): suffix /
leading zeros / angle / sign mode today, personalities later
(`DESIGN-MODES.md` §5.6); numeric settings stay RPN-postfix.

## GMP/MPFR on the target (✅ cross-built + link-verified)

The Rust is already `no_std` and target-compiling; the C libraries are now
cross-built too. **`firmware/scripts/build-gmp-mpfr-arm.sh`** builds static
`libgmp.a` + `libmpfr.a` for Cortex-M33 hard-float and installs them to
`firmware/vendor/gmp-mpfr-arm/` (gitignored — reproducible, not committed):

```sh
firmware/scripts/build-gmp-mpfr-arm.sh        # GMP 6.3.0 + MPFR 4.2.1, ~5 min
GMP_MPFR_LIBDIR=firmware/vendor/gmp-mpfr-arm \
  cargo build -p calcumaker-fw --target thumbv8m.main-none-eabihf
```

Key build details (see recall note `ref-gmp-mpfr-no-std`):
- `./configure --host=arm-none-eabi --disable-assembly` (the `--disable-assembly`
  is **mandatory** — no `mpn` asm backend for M-profile); MPFR `--with-gmp=`.
- `CFLAGS=-mcpu=cortex-m33 -mthumb -mfloat-abi=hard -mfpu=fpv5-sp-d16
  -std=gnu17 --specs=nosys.specs` — **`-std=gnu17`** is required (GCC 15 defaults
  to C23, which breaks GMP's old-style configure probes), and the **hard-float**
  flags make the ABI match `thumbv8m.main-none-eabihf`.
- `calcumaker-fw/build.rs` links them when `GMP_MPFR_LIBDIR` is set;
  `gmp-mpfr-nostd`'s own `build.rs` no-ops on `-none-eabi` — same FFI, just a
  different linker input.

**Verified:** a Cortex-M33 ELF links cleanly against the libs with
`Tag_ABI_VFP_args: VFP registers` (hard-float) and `FPv5/FP-D16`; a GMP+MPFR
program is ~127 KB text. **Remaining (firmware bring-up, not math):** at final
link, route GMP's allocator to the heap (`mp_set_memory_functions`) and resolve
newlib's `memcpy`/libm for GMP/MPFR (link the toolchain libc/libm) — folded into
the MCU/HAL bring-up.

- **Footprint:** ~0.5–1 MB flash for both libs; heap scales with precision —
  the reason for the large-flash MCU.
- **Licensing:** GMP is LGPLv3/GPLv2, MPFR is LGPLv3 — compatible with the
  AGPL-3.0 firmware; honor LGPL relinking terms for a shipped product.

---

## Power Tree

```
USB-C ──VBUS──┬── ESD (USBLC6) ──► D+/D- ──► STM32 USB FS
              │
              ├── Li-ion charger (MCP73831) ──► BAT+   (charge I sized to cell)
              │
 BAT+ ──┬── load-share (P-FET + Schottky) ──► VSYS ──┬── buck-boost ──► +3V3 ──► MCU
        │                                            │   (TPS63900, ULP, low-Iq,
 1S Li-ion (JST-PH)                                  │    always on — light load)
                                                     │
                                                     ├──► J3 FFC pins 1–2 ──► display module
                                                     │      (+3V3 on pin 5)    (boosts its own 5V)
                                                     │
                                                     └──► J7 2-pin JST-PH ──► RGB-matrix LED inlet
                                                            (LED amps only, off the signal FFC)
```

- **The MCU board generates one rail: +3V3.** The MCU runs on the ultra-low-Iq
  3V3 buck-boost (TPS63900, always on, so sleep current stays tiny). **VSYS is
  passed through raw** to the display module — on the FFC (`J3` pins 1–2) and,
  for the RGB matrix's LED current, over the dedicated 2-pin `J7` lead.
- **5V and level shifting live on the display module, not here.** The 7-seg
  module boosts VSYS→5V (TPS61022) and level-shifts CLK+DIN1/2/3 3V3→5V
  (74HCT125) for its own TM1640s. The MCU board only speaks the 3V3 SPI
  display-module bus. There is **no 5V boost, no 74HCT125 and no `DISP_PWR_EN`
  on the MCU board** — display power-down is a module-side concern, commanded
  over the SPI bus (or via `DISP_NRST`).
- Battery feeds VSYS only when USB is absent (load-share); USB present → run from
  USB, charger tops up the cell. VSYS (3.0–4.7V) is always < 5V → a boost works.
- **Display current budget** (3× TM1640 × 16 CC digits) sizes the **module's**
  boost and the `J7` VSYS lead, not the MCU board's 3V3 rail.

---

## Pin Budget

MCU is **STM32U575RGT6** (LQFP-64). Expected peripheral use (all on the MCU board
— the key matrix + annunciators moved to the keyboard board):

- **Display bus** → **SPI1** master (SCK + MOSI + CS) to the display module's own
  MCU, plus **DISP_IRQ** / **DISP_NRST** / **DISP_BOOT**. No MISO — the 12-pin FFC
  doesn't carry one. (Was a bit-banged TM1640 2-wire bus; those parts moved to the
  module.)
- **Keyboard link** (mezzanine to the keyboard G0) → **I²C** (SDA/SCL) + **UART**
  (TX/RX) + **KB_IRQ** (on a WKUP pin — keypress wake from Stop) + KB_NRST +
  KB_BOOT0.
- **OCTOSPI1** → 6 GPIOs: CLK, NCS, IO0–IO3 → the 4 MB quad-SPI NOR (U7).
- **USB FS** (PA11/PA12) → CDC console / provisioning.
- **SWD** (PA13/PA14) → Tag-Connect programming header.
- **LSE 32.768 kHz** crystal → RTC (sleep timing).
- **ADC** → battery voltage sense.

Rough count ~26 signal GPIO — comfortably inside LQFP-64 (~50 I/O).

### Committed pin map

**OCTOSPI1 does have a valid LQFP-64 mapping** (open question closed). On U5 the
OCTOSPI GPIOs belong to the **OCTOSPI I/O manager**, so the AF signals are named
`OCTOSPIM_P1_*`; OCTOSPIM routes OCTOSPI1 → Port 1 (straight-through). **LQFP-64
bonds out no Port-2 bus** (ports E/F/G are absent — the only P2 signal present is
`P2_NCS`), so Port 1 is mandatory. On this package **IO0–IO3 have exactly one pin
each**; only CLK (PA3 | PB10) and NCS (PA2 | PA4 | PC11) offered a choice.

| Function | Signal | Pin | AF | Pkg | Note |
|---|---|---|---|---|---|
| OCTOSPI1 → U7 | CLK | PB10 | AF10 | 29 | PA3 (p17) is the only alt — too far from the IOs |
| | NCS | PA4 | AF3 | 20 | costs SPI1_NSS + WKUP2 |
| | IO0 | PB1 | AF10 | 27 | forced |
| | IO1 | PB0 | AF10 | 26 | forced |
| | IO2 | PA7 | AF10 | 23 | forced |
| | IO3 | PA6 | AF10 | 22 | forced |
| SPI1 → display J3 | SCK | PA5 | AF5 | 21 | PA6/PA7 are QSPI now |
| | MOSI | PB5 | AF5 | 57 | |
| | CS | PA15 | AF5 | 50 | or any GPIO |
| USART2 → keyboard | TX | PA2 | AF7 | 16 | pair kept intact by choosing PA4 for NCS |
| | RX | PA3 | AF7 | 17 | |
| I²C1 → keyboard | SCL | PB6 | AF4 | 58 | |
| | SDA | PB7 | AF4 | 59 | PB3 is SWO — don't use it for SDA |
| USB FS | DM/DP | PA11/PA12 | | 44/45 | |
| SWD | SWDIO/SWCLK/SWO | PA13/PA14/PB3 | | 46/49/55 | |
| LSE | OSC32_IN/OUT | PC14/PC15 | | 3/4 | |

Choosing **CLK=PB10 + NCS=PA4** keeps the whole quad bus in a contiguous
**pin 20–29** cluster (short, length-matchable at ≥50 MHz) *and* leaves PA2/PA3
whole as the USART2 TX/RX pair for the keyboard link. Verified against the ST
CubeMX pin database for `STM32U575RGTx` / LQFP-64.

Still open: **KB_IRQ must land on a WKUP pin** (keypress wake from Stop) — e.g.
PA0/PB2 (WKUP1), PC13 (WKUP2), PB6 (WKUP3, conflicts with I²C1 SCL above).
Battery-sense ADC pin also unassigned.

---

## Schematic Sheet Plan

Three boards, each generated from its own manifest
(`hardware/scripts/calcumaker-{mcu,keyboard,display}.schgen.py`); the display and
the keyboard matrix are **multi-channel** (reusable row instantiated N×, fully
wired), the remaining sheets are placed-not-wired (wired in eeschema).

**`calcumaker-mcu`:**

| Sheet | File | Contents |
|-------|------|----------|
| Root | `calcumaker-mcu.kicad_sch` | sheet symbols + title block |
| MCU | `mcu.kicad_sch` | STM32U575RGTx (U1) + VDD/VDDA/VDDUSB decoupling + VCORE + NRST/BOOT0 + the **committed pin map**. Also absorbs the three former one-off sheets: LSE crystal (Y1 + C24/C25), SWD Tag-Connect TC2030-NL (J4), and the unified SPI display-module interface — connector **J3** (0.5 mm 12-pin FFC) + **J7** VSYS outlet (5V + level shifting live on the module) |
| PSU | `psu.kicad_sch` | USB-C + ESD + charger + load-share + 3V3 buck-boost (MCU) + battery conn |
| KeyboardIF | `keyboard_if.kicad_sch` | Keyboard link, **populate one**: DF40 2×6 stack (J5, DF40B-12DS) **or** 16-pin FFC cable (J6, AFC01-S16FCA-00) — I²C+UART+**VSYS** |
| QSPIFlash | `qspi_flash.kicad_sch` | 4 MB quad-SPI NOR (U7, W25Q32JVSSIQ) on OCTOSPI1 + CS# pull-up (R9) + decoupling (C26) |

**`calcumaker-keyboard`:**

| Sheet | File | Contents |
|-------|------|----------|
| Root | `calcumaker-keyboard.kicad_sch` | 5× `key_row` instances + 4 one-off sheets; per-row `ROW`→`KB_ROWn` + the RGB DIN→DOUT chain wired here |
| **key_row ×5** | `key_row.kicad_sch` | **Reusable 10-key row (MULTI-CHANNEL, fully wired): each key = MX switch + 1N4148W diode + SK6812MINI-E RGB (reverse-mount).** Instantiated ×5: Row1–5 → SW1–50 / D1–50 (matrix) / D56–105 (RGB). Shared COL1–10/VLED/GND global; ROW + RGB DIN/DOUT hierarchical |
| Annunciators | `annunc.kicad_sch` | 5 status LEDs (f g C G low-batt, D51–55 + R1–5) ← the on-board G0 |
| KbdMCU | `kbd_mcu.kicad_sch` | **STM32G031K8U6 (U1, UFQFPN-32)** scanner + decoupling + BOOT0 + SWD (J2) |
| RGBPower | `rgb_power.kicad_sch` | RGB **level shifter (U2)** + **gated high-side load switch (Q1/Q2 + R7–10/C6–7)** — drives + sleep-gates the per-key chain off VSYS |
| MainIF | `main_if.kicad_sch` | MCU link, **populate one**: DF40 2×6 header (J1, DF40C-12DP) **or** 16-pin FFC (J3, AFC01-S16FCA-00) → the MCU board (+VSYS for the RGB) |

All three boards **generate from their manifests and pass the structure check**:
`calcumaker-mcu` = 56 components (placed-not-wired), `calcumaker-keyboard` = 179
components (**5×10 matrix + per-key RGB wired multi-channel** as `key_row` ×5; the
G0/annunciator/RGB-power/mezzanine sheets placed-not-wired), `calcumaker-display`
= 60 components (fully wired, multi-channel).
Symbols are stock KiCad except the authored `TM1640` and single-digit `FJ5161AH`.

**`calcumaker-display`:**

| Sheet | File | Contents |
|-------|------|----------|
| Root | `calcumaker-display.kicad_sch` | Row1/Row2/Row3 + Interconnect + Aux + **DispMCU** (STM32G031) + **DispPower** (5V boost + shifter) sheet symbols |
| Row1/2/3 | `display_row.kicad_sch` (reused ×3) | **multi-channel** — 1 TM1640 + 16 single-digit FJ5161AH, fully wired (shared SEG bus + GRID1–16) |
| Interconnect | `interconnect.kicad_sch` | J1 ← MCU board (unified SPI; pinout matches mcu J3 + matrix J1) |
| AuxDisplay | `aux.kicad_sch` | DNP-optional SSD1306 OLED socket (J2) |

---

## Numeric Core

`firmware/calcumaker-core/` is the engine, with **one** numeric path:

- `Value` = `Int(gmp_mpfr_nostd::Integer)` (GMP) **or**
  `Real(gmp_mpfr_nostd::Float)` (MPFR).
- `Calc` = the RPN stack + token input; integers stay integers through
  `+ - * /` and the bitwise/shift ops; the scientific functions promote to MPFR
  reals. `float` / `round` / `trunc` / `floor` / `ceil` / `frac` convert
  between the kinds explicitly.
- **Exactness contract:** when the mathematical result of integer operands is
  an integer, it stays an exact GMP integer — `pow` (non-negative exponent,
  mpz_pow_ui, ~1 Mbit result cap, 0/±1 bases uncapped), `sq`, `exp10`, `fact`,
  and **division when it divides evenly** (`6 2 /` = exact 3). An inexact
  quotient **promotes to a real** (`3 2 /` = 1.5) — division never truncates
  silently; truncation lives only where it's expected and visible: under a
  word size (16C programmer division, annunciators lit) or the explicit
  `idiv`. **Number-type mode** (`tYPE` in SETUP; tokens `flexmode` /
  `intmode` / `realmode`): **FLE** (flexible, default) = the safe model above;
  **Int** = proper 16C integer mode as a setting — division truncates and
  sets Carry on an inexact quotient, unbounded included; **rEAL** = the
  float-machine model (plain decimal digits parse as reals) — SCI/FIN
  default. The FLOAT key enters rEAL (converting X, 16C-faithful); a radix
  key exits rEAL back to FLE (Int persists). INT/REAL annunciators show the
  non-default modes; counts/indexes accept integral reals everywhere.
  `sqrt` on an integer is the 16C-style **integer root** (⌊√x⌋, carry = the
  root was inexact; negative errors) — enter `2.0` or `float` for the real
  root. Negative exponents (fractional results) promote to MPFR.
- **HP-16C programmer model** under a word size (`<bits> wsize`; 0 = unbounded):
  - **Sign modes** `2s` / `1s` / `unsgn` (2's default). Values are stored as
    canonical signed integers; bitwise/shift/rotate ops act on the n-bit
    pattern; hex/oct/bin display the **pattern** (−15 @16b 2's = `FFF1`),
    decimal displays the signed value; non-decimal entry is a pattern, decimal
    entry a signed value. Mode / word-size changes reinterpret the stack
    **bit-pattern-preserving** (16C behaviour). 1's complement folds −0 onto 0.
  - **Flags:** **C** carry (add carry-out, subtract borrow, the bit shifted or
    rotated out, an inexact integer √) and **G** out-of-range (result wrapped).
  - **Leading zeros** (`lz`, 16C flag 3): pad hex/oct/bin display to the word
    width (`0F` @8 bits, `000F` @16).
  - `sl sr asr rl rr` act on X by one bit (the panel keys); `shl shr rln rrn`
    shift/rotate Y by X bits; `rlc rrc` (+`rlcn rrcn`) rotate through the
    carry — an (n+1)-bit rotation. `bset bclr btest maskl maskr popcnt` cover
    the bit ops (`btest` leaves the value in Y and pushes 0/1); `lj`
    left-justifies (Y = value, X = count); `dbl* dbl/ dblr` are the 16C
    double-word ops (2's comp / unsigned only — 1's-comp −0 makes the double
    word ambiguous).
  - **Flags 0–5** (`sf`/`cf`/`ftest`, index from X): 0–2 user bits, 3/4/5 alias
    leading-zeros / carry / overflow. `clreg` wipes the STO registers.
    SHOW (f-shifted radix keys) displays X in another base transiently.
- **16 STO/RCL registers** (`sto0`…`stof` / `rcl0`…`rclf` — one per hex digit
  key; on the keypad STO/RCL wait for the next digit key).
- **Real display formats:** AUTO (`%g`-style) / `FIX n` / `SCI n` / `ENG n`
  (digit count from X; `std` = back to AUTO). Inf/NaN display as `inf`/`nan`.
- **Angle modes** `rad` (default) / `deg` / `grad` for the circular trig
  (hyperbolics unaffected; g-shift WSIZE cycles). Conversions run through MPFR
  π with 32 guard bits; DEG/GRAD reduce mod the full circle **exactly** (fmod)
  and special-case exactly-representable angles — `deg 180 sin` = 0 (not a
  2^-prec residue), `30 sin` = 0.5, `45 tan` = 1, `0.5 asin` = 30; tan at
  90°/270° shows `inf`.
- **Errors never consume operands** — every op validates stack depth, types,
  and domain before popping (and LASTx updates only on success), so a failed
  op leaves the calculator exactly as it was (HP behaviour).
- HAL-free and fully **host-testable** (`cargo test`) + runnable
  (`cargo run --example repl`, or the full UI in `calcumaker-emu`).

The firmware consumes this crate; on the target the only thing that changes is
where GMP/MPFR come from (cross-built, linked at the FFI layer) — the engine code
is identical.

---

## Open Questions

Resolved: ✅ MCU (Q7) · ✅ board partition = split (Q8) · ✅ hardware license =
CERN-OHL-S (Q9) · ✅ product name = Calcumaker 16 (Q10) · ✅ display driver+digits
(TM1640 + FJ5161AH) · ✅ interconnect (12-position 0.5 mm FFC) · ✅ aux OLED
(DNP-optional, display board). Remaining:

1. ✅ **KiCad symbols done** — the single-digit `FJ5161AH` and the `TM1640` are
   both authored in `lib/symbols/calcumaker.kicad_sym` (registered in the display
   `sym-lib-table`); digit land = 0.56" `LTS6760`. Display board generates, checks
   OK, and is a **fully-wired multi-channel** design (reusable `display_row` ×3).
   Remaining: confirm THT-assembly route (JLCPCB THT add-on vs hand-solder), and
   verify FJ5161AH pad map vs the LTS6760 land + the TM1640 SOP-28 footprint at
   layout.
2. ✅ **Display rail = 5 V + level shifter** (decided + parts chosen). EN-gated
   **TPS61022** boost (C915088) + 1µH FTC201610 (C5832342) + 0603 caps; FB
   divider R6 732k/R7 100k → 5V. **SN74HCT125** level shifter (C352957, KiCad
   symbol `74AHCT125`). Remaining: verify boost Isat/FB and the downsized 3V3
   inductor Isat at layout. (TPS61022 + STM32U575 symbols turned out stock in
   KiCad, so the MCU board generates with no custom authoring.)
3. ✅ **Numeric engine = single GMP/MPFR path** (`gmp-mpfr-nostd` + `calcumaker-core`),
   host-tested + REPL, compiles for `thumbv8m`. ✅ **GMP/MPFR cross-built +
   link-verified** for Cortex-M33 hard-float (build script + `build.rs` wired).
   ✅ **Emulator target** (`calcumaker-emu`): the full device UI (keymap/App/
   seg7, now in the core) on a host terminal. ✅ Display windowing (16C
   `<`/`>`) implemented. Remaining is firmware bring-up: route GMP's
   allocator to the heap + resolve newlib at final link (folded into the
   MCU/HAL work).
4. ✅ **Annunciators (status line → hardware) — decided + implemented.**
   16C precedent: lamps ONLY for what must be visible mid-keystroke; the rest
   lives in the digits. **(a) Five keyboard-board LEDs** (`Annunc` sheet,
   D51–D55 + R1–R5 470R, driven by the keyboard G0): f yellow C72038 + g blue
   C965807 beside the shift keys; C / G / low-batt red C2286 along the top edge
   under the display bezel. (The display-board alternative was rejected: it
   needs a 4th TM1640 + DIN4 *and* a 5th level-shifter channel.) **(b)** ✅
   **f-STATUS momentary view** in the App
   (f-CLx): `bASE 16 2S rAd` / `P256 b8` / `AUtO 010000` (fmt + flags 543210)
   as 7-seg text until the next key — emulator shows it on the glass.
   **(c)** errors + SHOW already render as transient text. **(d)** ✅ radix
   as a 16C-style suffix letter on the X row — `h`/`o`/`b` for non-decimal
   integers, decimal unmarked (deviation from the 16C's `d`; absence =
   decimal) — a **display tunable** (`suffix` token toggles; on by default;
   emulator `--no-suffix`). Remaining: wire the LED GPIOs on the keyboard board;
   LOWBAT needs the battery ADC/status path.
5. ✅ **Keypad designed + boards generated (three-board split).** 5×10 grid, 49 keys (2U ENTER spans two cells),
   f/g scheme, internal-pull-up matrix + two-stage EXTI wake. The keypad +
   annunciators + their **STM32G0 scanner** now live on **`calcumaker-keyboard`**
   (**key_row ×5 multi-channel** / Annunciators / KbdMCU / RGBPower / MainIF, 178
   comp — matrix + 50 per-key RGB wired as one reusable 10-key row), which
   mezzanine-stacks (I²C+UART+VSYS) above **`calcumaker-mcu`**
   (MCU / PSU / KeyboardIF / QSPIFlash, 46 comp — Clock, Programming and DisplayIF
   were merged into the MCU sheet). All symbols stock except the authored
   TM1640 / FJ5161AH; both **generate + pass the structure check**. Remaining:
   refine `Nop` shift assignments; confirm Cherry MX vs Kailh hot-swap; verify the
   STM32U5 VCORE LDO-vs-SMPS choice (SMPS needs an inductor); verify the DF40
   stack height vs MX pin clearance; then **wire the boards in eeschema**.
6. **Battery cell + capacity.** Drives charger current (PROG resistor) and
   runtime target.

---

## Parts List (preliminary)

Anchored where known; `TBD` pending the Open Questions. LCSC/MPN are filled into
KiCad symbol fields as parts are placed (so `make bom` emits a JLCPCB BOM). The
per-board BOM source-of-truth is **`hardware/PARTS.md`**.

| Block | Part | Status |
|-------|------|--------|
| MCU (mcu) | **STM32U575RGT6** (1MB/768KB, M33, LQFP-64) | ✅ selected — LCSC C5270980, JLCPCB Extended |
| QSPI flash (mcu) | **W25Q32JVSSIQ** 4MB (32Mbit) quad-SPI NOR (SOIC-8) on OCTOSPI1 | ✅ LCSC C179173, ~$0.30 — XIP constants + state/program storage |
| Display driver (display) ×3 | **TM1640** (16-dig CC, 2-wire) | ✅ LCSC C5337152, ~$0.12 — 1/row |
| 7-seg digits (display) ×48 | **FJ5161AH** 0.56" **single-digit** CC (**THT**) | ✅ LCSC C8093, ~$0.10 — **16/row** (one digit each) |
| Display interconnect | **AFC01-S12FCA-00** 0.5mm 12P FFC (mcu J3 ↔ any display J1) | ✅ C262661; **unified SPI** — the 7-seg + RGB-matrix modules are interchangeable; cable = DigiKey accessory |
| Aux display | **0.91″ SSD1306 128×32 I2C OLED module** on a 1×4 socket (PZ254V-11-04P, C2691448) | ✅ DNP-optional; display board `AuxDisplay` sheet |
| Keyboard scanner MCU | **STM32G031K8U6** (UFQFPN-32) on the keyboard board | ✅ LCSC C432207, ~$0.60 — scans matrix + drives LEDs + I²C/UART to U575 |
| Keyboard link — stack option ×2 | **Hirose DF40 0.4mm 2×6 (12-pin)** 1.5mm: DF40B-12DS (mcu J5) + DF40C-12DP (kbd J1) | ✅ LCSC C3641147 / C6224952; compact rigid stack; DNP if using the FFC |
| Keyboard link — cable option ×2 | **AFC01-S16FCA-00** 16-pin 0.5mm FFC: mcu J6 + kbd J3 (+ GCT FFC cable, non-BOM) | ✅ LCSC C262665; MCU board mounts freely; 16-pin ≠ 12-pin display FFC (no cross-plug); DNP if stacking |
| Keyswitches (keyboard) ×50 | Cherry MX (full size) — **hot-swap** footprint `SW_MX_HS_CPG151101S11_1u` (place-on-back; Kailh socket) | 5×10 matrix; socket CPG151101S11 (C41430893); **not** solder-in-capable |
| Key diodes (keyboard) ×50 | 1N4148W (SOD-123) | C81598; one per key (NKRO) |
| Annunciator LEDs (keyboard) ×5 | f yellow (C72038), g blue (C965807), C·G·low-batt red (C2286) + 5× 470Ω | ✅ front-panel, beside the keys |
| Per-key RGB (keyboard) ×50 | **SK6812MINI-E** reverse/bottom-mount RGB — under each key, backlight (D56–D105); **single-sided w/ the sockets** | ✅ LCSC C5149201, ~161k; daisy-chained off the G0 |
| RGB level shift + gate (keyboard) | **SN74LVC1G125** (3V3→VLED data) + **AO3401A**/**2N7002** high-side load switch | ✅ LCSC C23654 / C15127 / C8545 — LEDs on **gated VSYS**, off in sleep |
| USB-C (mcu) | receptacle + CC 5.1k + USBLC6 ESD | as ephemerkey PSU |
| Charger (mcu) | MCP73831 / BQ-class | sized to cell |
| Buck-boost 3V3 (mcu) | TPS63900 (ULP, low-Iq) — **MCU only** | ✅ stays as-is (light load); L→0805 |
| 5V boost (mcu) | **TPS61022RWUR** (EN-gated) + 1µH (FTC201610) + 0603 caps | ✅ LCSC C915088 / C5832342 |
| Level shifter (mcu) | **SN74HCT125DR** quad buffer @5V (CLK+DIN×3) | ✅ LCSC C352957 (symbol `74AHCT125`) |
| Battery (mcu) | 1S Li-ion (JST-PH) | capacity TBD |
| RTC crystal (mcu) | 32.768 kHz | LSE |
| Programming (mcu) | SWD Tag-Connect TC2030-NL | as sibling repos |

---

## Firmware Dependencies

See `reference/README.md` and the **Software Stack** table above. The engine
(`calcumaker-core`) depends on **`gmp-mpfr-nostd`** (our no_std FFI). On the host
it links the **system** GMP/MPFR (`brew install gmp mpfr`); for the target the
**cross-built** GMP/MPFR are produced out-of-tree and linked via
`calcumaker-fw/build.rs` — **not** vendored into the repo (gitignored under
`firmware/vendor/`).
