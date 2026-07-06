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
| 7 | MCU | **STM32U575ZGT6** (Cortex-M33, 2 MB / 786 KB, LQFP-144, ULP). Chosen on LCSC/JLCPCB availability: the L4R5 is ~unstocked (5 pcs), U575ZGT6 is in stock (230 pcs, ~$4.90, JLCPCB Extended) and keeps GMP/MPFR open. Target `thumbv8m.main-none-eabihf`. |
| 8 | Board partition | **Three boards: `calcumaker-mcu` + `calcumaker-keyboard` (DF40 mezzanine-stacked above it) + `calcumaker-display` (angled, 0.5 mm FFC).** Keeps a dense LQFP-144 off the 50-key through-hole matrix. The keyboard has its **own STM32G0 scanner**, so only an **I²C+UART link + power** cross the mezzanine (not the raw matrix); the display bus + power cross the FFC. |
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

**SELECTED: STM32U575ZGT6 (Cortex-M33, 2 MB flash / 786 KB SRAM, LQFP-144, ULP).**
Decided on **LCSC / JLCPCB availability** — the deciding factor. Live jlcsearch
data shows the L4R5 is effectively unavailable (~5 pcs, ~$8.98, and *no other
L4R5 package is stocked at all*), whereas the **U575ZGT6
([C5271004](https://www.lcsc.com/product-detail/C5271004.html)) is in stock
(~230 pcs, ~$4.90, JLCPCB "Extended" = assemblable)** — and it has *more* SRAM
than the L4R5. It keeps the GMP/MPFR FFI path fully open (2 MB flash), has an
FPU + USB FS + OCTOSPI, and is on ST's current ULP line (TrustZone). Target
`thumbv8m.main-none-eabihf`; HAL `embassy-stm32` (`stm32u575zg`). The LQFP-144
gives ample GPIO for the key matrix.

> Bonus: at ~$4.90 the GMP-capable U575 costs about the **same** as the 1 MB
> pure-Rust fallback parts — so keeping GMP open is essentially free here.

**Availability ladder (live LCSC/JLCPCB via jlcsearch; all JLCPCB "Extended"):**

| Part | LCSC# | Flash / RAM | Core | Stock | ~Unit $ | Fit |
|------|-------|-------------|------|-------|---------|-----|
| **STM32U575ZGT6** | C5271004 | 2 MB / 786 KB | M33 | **230** | ~$4.90 | ✅ selected — buyable + GMP-capable |
| STM32L4R5ZIT6 | C1339786 | 2 MB / 640 KB | M4F | 5 | ~$8.98 | GMP-capable but ~unstocked → no-go |
| STM32L496RGT6 | C124720 | 1 MB / 320 KB | M4F | 95 | ~$4.72 | pure-Rust (GMP tight in 1 MB) |
| STM32L476RGT6 | C74797 | 1 MB / 128 KB | M4F | 1408 | ~$4.88 | cheapest/best-stocked pure-Rust pick |

> Other L4R5 packages (LQFP-100 VIT6, UFBGA QIY6) returned **no LCSC listing**.
> Stock/price are point-in-time (fetched during scaffolding); re-check at order.

Considerations once pinned:
- **SRAM may be banked** (esp. on U5). For a single contiguous heap, use the
  largest contiguous span or place the heap section explicitly.
- **External memory:** likely unnecessary — internal RAM is ample for calculator
  precisions. (L4+/U5 do offer OCTOSPI for PSRAM/flash if ever needed.)
- **USB FS** for a CDC console / provisioning + firmware update.

### Board partition: three boards (MCU + keyboard stacked, display cabled)

Calcumaker 16 is **three PCBs** (revised 2026-07-05 — the keyboard split off the
MCU board):

- **`calcumaker-mcu`** — the brain/PSU board: MCU (U575), PSU, clock, SWD, the
  display 5 V rail + level shifter + interconnect, and a **keyboard mezzanine**
  (J5). This is the dense fine-pitch SMT board (LQFP-144). *Bottom of the stack.*
- **`calcumaker-keyboard`** — the front-panel board: the 50-key Cherry MX matrix
  + per-key diodes + the annunciator LEDs + the mating **mezzanine header** (J1).
  A simple 2-layer through-hole board. *Stacks directly above the MCU board.*
- **`calcumaker-display`** — the multi-row 7-segment stack + its driver ICs + the
  interconnect back to the MCU board. It **mounts at an upward angle** for
  readability, cabled (not stacked).

**Why split the keyboard off the MCU board:** laying a dense LQFP-144 (0.5 mm
pitch, needs careful fan-out / multiple layers) into the same board as 50
full-size through-hole keyswitches is a routing and mechanical fight. Splitting
gives each PCB an easy job — the MCU board is a compact SMT brain, the keyboard
is a plain switch matrix — and lets the keyboard be the physical top surface the
user types on while the MCU board hides underneath.

**Stacking — the mezzanine:** because the keyboard has its **own STM32G0 scanner**
(below), only a **serial link + power** cross the stack, not the raw matrix. A
**low-profile fine-pitch mezzanine** — **Hirose DF40, 0.4 mm pitch, 2×5 (10-pin),
1.5 mm stack height** — receptacle `J5` **DF40C-10DS-0.4V** (LCSC C424636) on the
MCU board, header `J1` **DF40C-10DP-0.4V** (LCSC C424635) on the keyboard board.
It carries 10 pins: `+3V3 · GND · I²C_SDA · I²C_SCL · UART_TX · UART_RX · KB_IRQ ·
KB_NRST · KB_BOOT0 · GND`. Both an **I²C and a UART** bus are exposed (I²C is the
primary key/annunciator channel; UART is spare/expansion + the G0's ROM
bootloader); `KB_IRQ` is the keypress wake line (see Low-power & wake). The two
halves are pin-for-pin identical (mated pin N ↔ pin N). DF40 was chosen over a
generic 1.27 mm header/socket because it's a **documented Hirose mating pair with
a dedicated KiCad footprint + 3D model and real LCSC stock** — the cheap generic
ZX parts had no datasheet and unverifiable gender/land.

*Mechanical:* the DF40C mates at only **1.5 mm** — genuinely low-profile, but too
thin to clear the **MX switch pins (~2–3 mm)** protruding below the keyboard PCB.
So the MCU board must sit under a **keyless region** (top / display-bezel area)
or the pins get trimmed; and tall connectors (**USB-C ~3.2 mm, battery JST
~5 mm**) go at the **MCU board edge, overhanging beyond the keyboard footprint**.
4 corner standoffs (matched to the 1.5 mm stack) take the load. All new footprints
carry KiCad **3D (STEP) models** — verify the stack in the 3D viewer, and the
DF40C land vs the KiCad DF40 2×5 footprint, at layout. (Taller DF40 variants —
DF40HC(2.0)/(2.5)/… — swap in on the same land if more clearance is wanted.)

Keeping the display driver *on the display board* means only **+5V, GND, and the
display serial bus** (a handful of signals) cross that connector — instead of
dozens of segment/digit lines — which is what **simplifies the display wiring**.

- **Display interconnect:** a **0.5 mm 12-conductor FFC** — connector
  **AFC01-S12FCA-00** (LCSC C262661) on both `calcumaker-mcu:J3` and
  `calcumaker-display:J1`; the flat-flex **cable is a non-assembled DigiKey
  accessory** — **GCT FFC05-TIN `05-12-A-<length>-A-4-06-4-T`** (12-position,
  0.5 mm; **length + contact orientation set at layout**). Pinout `1=+5V, 2=+5V,
  3=GND, 4=CLK(shared), 5=DIN1, 6=DIN2,
  7=DIN3, 8=GND, 9=+3V3, 10=SDA, 11=SCL, 12=GND` — the TM1640 driver uses a
  **2-wire** bus (shared clock + one data line per row driver, not SPI); pins
  9–11 feed the **DNP-optional aux OLED** (3V3 I²C, unused when unpopulated).
  **+5V and GND are doubled/tripled** because a 0.5 mm FFC conductor is only
  ~0.4 A and the 3 multiplexed TM1640s peak ~0.3–0.5 A on +5V (less with
  brightness capping). `calcumaker-mcu:J3` ↔ `calcumaker-display:J1` must match;
  verify the FFC contact orientation (top/bottom, same/opposite-end) at layout.

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

- **`keys`** — the 50-key matrix keymap + f/g shift layers + resolution
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
                                                     └── 5V boost (EN-gated) ──► +5V
                                                         (TBD part)         │
                                                              ▲ DISP_PWR_EN │ ──► display
                                                                (MCU GPIO,  │     (TM1640
                                                                 off=sleep) │      + LEDs)
   MCU 3V3 ─► 74HCT125 (VCC=+5V) ─► CLK/DIN1/2/3 at 5V logic ─────────────► display
```

- **Two rails.** The MCU runs on the ultra-low-Iq 3V3 buck-boost (always on, so
  sleep current stays tiny). The display's LED load lives on a **separate 5V
  boost gated by `DISP_PWR_EN`** — fully off in sleep. This keeps the low-power
  story intact while giving the TM1640 its 5V (and the LEDs more Vf headroom).
- The 4 control lines (CLK + DIN1/2/3) are translated 3V3→5V by a **74HCT125**
  (VIH=2V at VCC=5V) so the 3.3V MCU drives the 5V TM1640 inputs in spec.
- Battery feeds VSYS only when USB is absent (load-share); USB present → run from
  USB, charger tops up the cell. VSYS (3.0–4.7V) is always < 5V → a boost works.
- **Display current budget** (3× TM1640 × 16 CC digits) sizes the **5V boost**,
  not the 3V3 rail. Boost part + level shifter chosen by availability (research).

---

## Pin Budget

MCU is **STM32U575ZGT6** (LQFP-144). Fill a pin table (package pin → function →
AF) here once the panel layout fixes the matrix dimensions. Expected peripheral
use (all on the MCU board):

- **Display bus** → 4 GPIOs: TM1640 2-wire (shared CLK + DIN1/DIN2/DIN3),
  bit-banged at 3V3 → 74HCT125 → 5V → interconnect. Plus **DISP_PWR_EN** GPIO →
  5V-boost EN (display off in sleep).
- **GPIO matrix** → Cherry MX rows × cols (+ EXTI wake on a column for
  wake-from-Stop on keypress).
- **USB FS** (PA11/PA12) → CDC console / provisioning.
- **SWD** (PA13/PA14) → Tag-Connect programming header.
- **LSE 32.768 kHz** crystal → RTC (sleep timing).
- **ADC** → battery voltage sense.
- **Annunciator LEDs** → 5 GPIOs (active high): f, g, C, G, low-batt
  (Annunciators sheet, D61–D65 + R9–R13).

---

## Schematic Sheet Plan

Two boards, each generated from its own manifest
(`hardware/scripts/calcumaker-{main,display}.schgen.py`), then placed-not-wired,
then wired in eeschema.

**`calcumaker-mcu`:**

| Sheet | File | Contents |
|-------|------|----------|
| Root | `calcumaker-mcu.kicad_sch` | sheet symbols + title block |
| MCU | `mcu.kicad_sch` | STM32U575ZGTx (U1) + VDD/VDDA/VDDUSB decoupling + VCORE + NRST/BOOT0 |
| Clock | `clock.kicad_sch` | LSE 32.768 kHz crystal (Y1) + load caps (RTC) |
| Programming | `prog.kicad_sch` | SWD Tag-Connect TC2030-NL (J4) |
| PSU | `psu.kicad_sch` | USB-C + ESD + charger + load-share + 3V3 buck-boost (MCU) + battery conn |
| DisplayIF | `display_if.kicad_sch` | EN-gated 5V boost (TPS61022) + 74HCT125 level shifter + **J3 (0.5 mm FFC)** → display |
| KeyboardIF | `keyboard_if.kicad_sch` | Hirose DF40 2×5 0.4 mm mezzanine receptacle (J5, DF40C-10DS, 1.5 mm stack) — I²C+UART link to the keyboard board |

**`calcumaker-keyboard`:**

| Sheet | File | Contents |
|-------|------|----------|
| Root | `calcumaker-keyboard.kicad_sch` | sheet symbols + title block |
| Keypad | `keypad.kicad_sch` | 5×10 Cherry MX matrix (SW1–50 + diodes D1–50) → the on-board G0 |
| Annunciators | `annunc.kicad_sch` | 5 status LEDs (f g C G low-batt, D51–55 + R1–5) ← the on-board G0 |
| KbdMCU | `kbd_mcu.kicad_sch` | **STM32G031K8U6 (U1, UFQFPN-32)** scanner + decoupling + BOOT0 + SWD (J2) |
| MainIF | `main_if.kicad_sch` | Hirose DF40 2×5 0.4 mm mezzanine header (J1, DF40C-10DP, 1.5 mm stack) → down to the MCU board |

All three boards **generate from their manifests and pass the structure check**:
`calcumaker-mcu` = 52 components, `calcumaker-keyboard` = 119 components (both
placed-not-wired), `calcumaker-display` = 60 components (fully wired, multi-channel).
Symbols are stock KiCad except the authored `TM1640` and single-digit `FJ5161AH`.

**`calcumaker-display`:**

| Sheet | File | Contents |
|-------|------|----------|
| Root | `calcumaker-display.kicad_sch` | Row1/Row2/Row3 + Interconnect + Aux sheet symbols; per-row DIN1/2/3 routing |
| Row1/2/3 | `display_row.kicad_sch` (reused ×3) | **multi-channel** — 1 TM1640 + 16 single-digit FJ5161AH, fully wired (shared SEG bus + GRID1–16) |
| Interconnect | `interconnect.kicad_sch` | J1 ← MCU board (pinout matches mcu J3) |
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
5. ✅ **Keypad designed + boards generated (three-board split).** 5×10 (50 keys),
   f/g scheme, internal-pull-up matrix + two-stage EXTI wake. The keypad +
   annunciators + their **STM32G0 scanner** now live on **`calcumaker-keyboard`**
   (Keypad / Annunciators / KbdMCU / MainIF, 119 comp), which mezzanine-stacks
   (I²C+UART) above **`calcumaker-mcu`** (MCU / Clock / Programming / PSU /
   DisplayIF / KeyboardIF, 52 comp). All symbols stock except the authored
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
| MCU (mcu) | **STM32U575ZGT6** (2MB/786KB, M33, LQFP-144) | ✅ selected — LCSC C5271004, JLCPCB Extended |
| Display driver (display) ×3 | **TM1640** (16-dig CC, 2-wire) | ✅ LCSC C5337152, ~$0.12 — 1/row |
| 7-seg digits (display) ×48 | **FJ5161AH** 0.56" **single-digit** CC (**THT**) | ✅ LCSC C8093, ~$0.10 — **16/row** (one digit each) |
| Display interconnect | **AFC01-S12FCA-00** 0.5mm 12P FFC (MCU J3 ↔ display J1) | ✅ LCSC C262661; +5V/GND doubled; **cable = DigiKey accessory** |
| Aux display | **0.91″ SSD1306 128×32 I2C OLED module** on a 1×4 socket (PZ254V-11-04P, C2691448) | ✅ DNP-optional; display board `AuxDisplay` sheet |
| Keyboard scanner MCU | **STM32G031K8U6** (UFQFPN-32) on the keyboard board | ✅ LCSC C432207, ~$0.60 — scans matrix + drives LEDs + I²C/UART to U575 |
| Keyboard mezzanine ×2 | **Hirose DF40C-10** 0.4mm 2×5 low-profile (1.5mm stack): DF40C-10DS (mcu J5) + DF40C-10DP (keyboard J1) | ✅ LCSC C424636 / C424635; KiCad fp + 3D model; verify MX-pin clearance |
| Keyswitches (keyboard) ×50 | Cherry MX (full size) + optional Kailh hot-swap sockets | 5×10 matrix |
| Key diodes (keyboard) ×50 | 1N4148W (SOD-123) | C81598; one per key (NKRO) |
| Annunciator LEDs (keyboard) ×5 | f yellow (C72038), g blue (C965807), C·G·low-batt red (C2286) + 5× 470Ω | ✅ front-panel, beside the keys |
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
