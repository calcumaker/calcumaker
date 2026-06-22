# Calcumaker 16 — Design Document

> Repo: `calcumaker` · Product: **Calcumaker 16** (see `NAMING.md`).

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
rows**) carried on its **own angled PCB** — a **split design** (main board +
display board) where only power and the display serial bus cross the
interconnect. The device is **battery + USB-C** powered, on the **low-power
STM32U575**. The firmware main loop is **Rust (`no_std`)**.

This document is the source of truth for the hardware and firmware design. It
follows the structure of the sibling BenchBits repos (ephemerkey / notchdeck /
tsumikoro). The repo's firmware departs from those (which are C / Zephyr) by
being Rust.

---

## Resolved Decisions (from project kickoff)

| # | Decision | Choice |
|---|----------|--------|
| 1 | Math stack | **Prefer GMP + MPFR (via FFI); pure-Rust fallback** (dashu/astro-float) if no_std cross-compile proves impractical. |
| 2 | Power | **1S Li-ion + USB-C charging + buck-boost**; "low power" = long battery life via aggressive sleep between keystrokes. |
| 3 | Display | **Stacked RPN registers, 2–3 rows (option)** — multi-row 7-segment showing the top of the stack. Driver + digit parts chosen by LCSC price/availability. |
| 4 | Keypad | **Wide HP-16C-style layout** for programming / technical / engineering use; full-size Cherry MX. |
| 5 | Firmware language | **Rust, `no_std`** main loop (async via embassy once MCU is pinned). |
| 6 | Firmware license | **AGPL-3.0** (repo LICENSE) — compatible with LGPLv3 GMP/MPFR. |
| 7 | MCU | **STM32U575ZGT6** (Cortex-M33, 2 MB / 786 KB, LQFP-144, ULP). Chosen on LCSC/JLCPCB availability: the L4R5 is ~unstocked (5 pcs), U575ZGT6 is in stock (230 pcs, ~$4.90, JLCPCB Extended) and keeps GMP/MPFR open. Target `thumbv8m.main-none-eabihf`. |
| 8 | Board partition | **Split: `calcumaker-main` + `calcumaker-display`.** The display board angles up; only +3V3/GND + the display serial bus cross the interconnect (simplifies wiring). |
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

### Board partition: split main + display boards

Calcumaker 16 is **two PCBs**:

- **`calcumaker-main`** — MCU (U575), PSU, the Cherry MX key matrix, and the
  interconnect to the display.
- **`calcumaker-display`** — the multi-row 7-segment stack + its driver IC(s) +
  the interconnect back to main. It **mounts at an upward angle** for readability.

Putting the driver *on the display board* means only **+3V3, GND, and the
display serial bus** (a handful of signals) cross the connector — instead of
dozens of segment/digit lines — which is what **simplifies the wiring**.

- **Interconnect:** a **2.54 mm 1×8 header** (PZ254V-11-08P, LCSC C492407 —
  well-stocked, cheap, mechanically supports the angled board; FFC was rejected,
  ~2 pcs LCSC stock). Pinout `1=+3V3, 2=GND, 3=CLK(shared), 4=DIN1, 5=DIN2,
  6=DIN3, 7=GND, 8=spare` — the TM1640 driver uses a **2-wire** bus, so it's a
  shared clock + one data line per row driver (not SPI). Keep +3V3/GND wide for
  the display LED current. `calcumaker-main:J3` ↔ `calcumaker-display:J1` pinouts
  must match; join with a short ribbon/cable for the upward angle.

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
  Display-only (keys live on the main board). TM1638 (C19187) is the drop-in if
  on-chip key-scan is ever wanted.
- **Digits: FJ5161AH** (LCSC C8093, 0.56" **4-digit common-cathode**, ~$0.19) —
  4 per row → **12 modules**. Common-cathode matches the TM1640. 0.36" FJ3461AH
  (C10708) is the option if board space is tight.
- **⚠ Through-hole digits.** No SMD multi-digit 7-segment displays are stocked on
  LCSC — the well-stocked parts are THT. So `calcumaker-display` needs **THT
  assembly** (JLCPCB through-hole add-on, or hand/wave solder); the TM1640s are
  SMT. See `hardware/PARTS.md`.
- **Power note:** LED 7-segment is the dominant active current draw, *not* the
  MCU — and it's drawn from +3V3 **across the interconnect**, so it gates the
  main board's buck-boost sizing (the TPS63900 placeholder likely needs
  upsizing). Use TM1640 brightness/dimming + blank-on-idle + display-off in
  sleep to honor the battery goal.

KiCad symbols: digits use the **stock** `Display_Character:CC56-12EWA` (0.56"
4-digit common-cathode); the **TM1640** symbol is authored from the datasheet in
`hardware/lib/symbols/calcumaker.kicad_sym`. The display board **generates and
passes the structure check** (placed, not wired). See `hardware/PARTS.md`.

### Keypad: full-size Cherry MX, wide HP-16C-style layout

A wide landscape layout in the HP-16C / Voyager tradition (with **f / g** shift
keys to reach the programmer + scientific function set). Full-size Cherry MX
switches on a scanned matrix; **one diode per key** for n-key rollover.

- ~**40–45 keys** → a matrix on GPIO (e.g. 6 rows × 8 cols = 48, or 5 × 9 = 45).
- Optional **Kailh hot-swap sockets** for switch choice without soldering.
- The exact key map (digit/nibble keys, ENTER, arithmetic, AND/OR/XOR/NOT/
  shifts/rotates, base select, word-size select, stack ops, precision) is
  finalized with the front-panel layout.

### Power: 1S Li-ion + USB-C charge + buck-boost

Mirrors the ephemerkey power path (proven in the sibling repo):
USB-C (sink) → ESD → Li-ion charger → load-share → buck-boost → 3V3. Sized up
for the larger active load (display + MCU running MPFR). See **Power Tree**.

### Numeric core: GMP/MPFR (preferred) with a pure-Rust fallback

The firmware's numeric core is abstracted behind `firmware/calcumaker-fw/
src/numeric/` so the RPN engine is backend-agnostic. Two Cargo-feature-selected
backends — see **Numeric Core (firmware)** and **GMP/MPFR on no_std** below.

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
| Numeric (pref) | **GMP + MPFR via FFI** | cross-built `.a`, allocator routed to the heap |
| Numeric (fallback) | **`dashu`/`ibig` + `astro-float`** | pure-Rust, fully no_std + alloc |

---

## GMP/MPFR on no_std (the hard part)

Verdict from research (see also the recall note `ref-gmp-mpfr-no-std`):

- **`rug` / `gmp-mpfr-sys` are impractical on bare metal.** `gmp-mpfr-sys`
  compiles vendored GMP/MPFR C with a *host* C compiler and explicitly does not
  support cross-compilation; `rug`'s `no_std` mode only drops std-dependent Rust
  glue, the core types still pull `gmp-mpfr-sys` (and `libc`). Don't go this way.
- **The path that works = manual FFI to a separately cross-built static lib.**
  Recipe:
  1. Build **GMP** with
     `./configure --host=arm-none-eabi --disable-assembly`
     `CC=arm-none-eabi-gcc CFLAGS="-mcpu=cortex-m33 -mthumb --specs=nosys.specs -nostartfiles"`.
     `--disable-assembly` is **mandatory** (no `mpn` asm backend for M-profile).
  2. Build **MPFR** against that GMP via `--with-gmp-include` / `--with-gmp-lib`
     (versions must match).
  3. Link **picolibc** (successor to newlib-nano) for the libc + libm symbols
     MPFR's transcendentals need.
  4. Route GMP's allocator to the Rust global heap via
     `mp_set_memory_functions(malloc, realloc, free)` at init (`numeric::init()`).
  5. Link the `.a`s from `build.rs` under the `numeric-gmp` feature.
- **Footprint:** ~0.5–1 MB flash for both libs; heap scales with precision —
  this is the main reason for the large-flash/large-RAM MCU.
- **Licensing:** GMP is LGPLv3/GPLv2, MPFR is LGPLv3 — compatible with the
  AGPL-3.0 firmware; honor LGPL relinking terms for any shipped product.

This is gated behind the `numeric-gmp` Cargo feature; `build.rs` currently warns
that the cross-built libs aren't wired yet.

## Pure-Rust Fallback

Fully `no_std` + `alloc`, MIT/Apache, no C toolchain — the `numeric-pure`
feature (default until GMP/MPFR FFI is wired):

- **Integers (GMP analog):** `dashu-int` (or `ibig`) — arbitrary-precision
  integers; covers HP-16C hex/bin/bitwise/word-size needs.
- **Floats + transcendentals (MPFR analog):** **`astro-float`** — arbitrary-
  precision floats with **correctly-rounded** sin/cos/exp/ln, the closest MPFR
  analog. (`dashu-float` exists but has narrower transcendental coverage.)
- Both allocate from `embedded-alloc` (TLSF) as the global allocator.

---

## Power Tree

```
USB-C (J?) ──VBUS──┬── ESD (USBLC6) ──► D+/D- ──► STM32 USB FS
                   │
                   ├── Li-ion charger (e.g. MCP73831 / BQ-class) ──► BAT+
                   │       (charge current sized for the chosen cell)
                   │
   BAT+ ──┬── load-share (P-FET + Schottky) ──► VSYS ──► buck-boost ──► +3V3
          │                                       (TPS63xxx-class, ULP)
   1S Li-ion (JST-PH)                              │
                                                   ├──► MCU 3V3
                                                   └──► display 3V3 (LED current
                                                        is the dominant load —
                                                        budget separately)
```

- Battery feeds VSYS only when USB is absent (load-share). USB present → system
  runs from USB and the charger tops up the cell.
- **Display current budget** must be computed once the digit count + drive
  current are chosen — it dominates the active power and the charger/buck-boost
  sizing. (Open Question.)

---

## Pin Budget

MCU is **STM32U575ZGT6** (LQFP-144). Fill a pin table (package pin → function →
AF) here once the panel layout fixes the matrix dimensions. Expected peripheral
use (all on the main board):

- **SPI** (or I²C) → display driver chain, **out via the interconnect** to the
  display board (MAX7219 cascade / HT16K33 / TM-series).
- **GPIO matrix** → Cherry MX rows × cols (+ EXTI wake on a column for
  wake-from-Stop on keypress).
- **USB FS** (PA11/PA12) → CDC console / provisioning.
- **SWD** (PA13/PA14) → Tag-Connect programming header.
- **LSE 32.768 kHz** crystal → RTC (sleep timing).
- **ADC** → battery voltage sense.

---

## Schematic Sheet Plan

Two boards, each generated from its own manifest
(`hardware/scripts/calcumaker-{main,display}.schgen.py`), then placed-not-wired,
then wired in eeschema.

**`calcumaker-main`:**

| Sheet | File | Contents |
|-------|------|----------|
| Root | `calcumaker-main.kicad_sch` | sheet symbols + title block |
| MCU | `mcu.kicad_sch` | STM32U575 + decoupling + LSE + SWD + USB + BOOT0 |
| PSU | `psu.kicad_sch` | USB-C + ESD + charger + load-share + buck-boost + battery conn |
| Keypad | `keypad.kicad_sch` | Cherry MX matrix + per-key diodes + wake line |
| Interconnect | `interconnect.kicad_sch` | J3 → display board (+3V3/GND + SPI bus) |

**`calcumaker-display`:**

| Sheet | File | Contents |
|-------|------|----------|
| Root | `calcumaker-display.kicad_sch` | sheet symbols + title block |
| Display | `display.kicad_sch` | 7-seg array (2–3 rows) + driver chain + brightness/blank |
| Interconnect | `interconnect.kicad_sch` | J1 ← main board (pinout matches main J3) |

---

## Numeric Core (firmware)

`firmware/calcumaker-fw/src/numeric/mod.rs` exposes a backend-agnostic
`Number` (an arbitrary-precision `Int` or `Real`) and a `Radix` for the
programmer display modes. The RPN engine (`rpn.rs`) only uses `Number`, so the
backend swaps via Cargo feature with no engine changes:

- `numeric-gmp` → `Int` = `mpz_t`, `Real` = `mpfr_t` (FFI). `init()` calls
  `mp_set_memory_functions` to use the global heap.
- `numeric-pure` → `Int` = dashu/ibig, `Real` = astro-float.

Keep `rpn.rs` and `numeric/` **host-testable** (no HAL) so the calculator logic
can be unit-tested on a desktop (and against the pure-Rust backend) before
silicon.

---

## Open Questions

Resolved: ✅ MCU (Q7) · ✅ board partition = split (Q8) · ✅ hardware license =
CERN-OHL-S (Q9) · ✅ product name = Calcumaker 16 (Q10) · ✅ display driver+digits
(TM1640 + FJ5161AH) · ✅ interconnect (1×8 2.54 mm header). Remaining:

1. ✅ **KiCad symbols done** — digits use stock `CC56-12EWA`; TM1640 authored
   (`lib/symbols/calcumaker.kicad_sym`); display board generates + checks OK.
   Remaining: confirm THT-assembly route (JLCPCB THT add-on vs hand-solder), and
   verify the FJ5161AH pinout vs CC56-12 + the TM1640 SOP-28 footprint at layout.
2. **Buck-boost upsizing + display rail voltage.** The TM1640's VDD is **5 V
   nominal** (VIH = 0.7·VDD = 3.5 V > the STM32's 3.3 V). Either run the display
   at **3.3 V** (out of TM1640 spec but common; dimmer; needs low-Vf red/green/
   yellow digits — single rail, no level-shift) or add a **5 V boost + level
   shifters** on CLK/DIN. Then resize the buck-boost for the display LED current
   (TPS63900's ~hundreds-of-mA ceiling is likely too low). **← next task.**
3. **Numeric backend for first bring-up.** Start on `numeric-pure` (always
   builds) and bring up GMP/MPFR FFI in parallel, or commit to GMP/MPFR from the
   outset? (Recommendation: bring the product up on pure-Rust, port to GMP/MPFR
   once the cross-build is proven — both behind the same `Number` API.)
4. **Keypad map + count.** Final key set and matrix dimensions for the wide
   HP-16C-style layout; f/g shift scheme; Kailh hot-swap or soldered.
5. **Battery cell + capacity.** Drives charger current (PROG resistor) and
   runtime target.

---

## Parts List (preliminary)

Anchored where known; `TBD` pending the Open Questions. LCSC/MPN are filled into
KiCad symbol fields as parts are placed (so `make bom` emits a JLCPCB BOM). The
per-board BOM source-of-truth is **`hardware/PARTS.md`**.

| Block | Part | Status |
|-------|------|--------|
| MCU (main) | **STM32U575ZGT6** (2MB/786KB, M33, LQFP-144) | ✅ selected — LCSC C5271004, JLCPCB Extended |
| Display driver (display) ×3 | **TM1640** (16-dig CC, 2-wire) | ✅ LCSC C5337152, ~$0.12 — 1/row |
| 7-seg digits (display) ×12 | **FJ5161AH** 0.56" 4-digit CC (**THT**) | ✅ LCSC C8093, ~$0.19 — 4/row |
| Interconnect | **PZ254V-11-08P** 1×8 2.54mm header | ✅ LCSC C492407; main J3 ↔ display J1 |
| Keyswitches (main) | Cherry MX (full size) + optional Kailh hot-swap sockets | TBD count |
| Key diodes (main) | 1N4148W (SOD-123) ×N | per key |
| USB-C (main) | receptacle + CC 5.1k + USBLC6 ESD | as ephemerkey PSU |
| Charger (main) | MCP73831 / BQ-class | sized to cell |
| Buck-boost (main) | TPS63xxx-class (ULP) | sized to display LED load |
| Battery (main) | 1S Li-ion (JST-PH) | capacity TBD |
| RTC crystal (main) | 32.768 kHz | LSE |
| Programming (main) | SWD Tag-Connect TC2030-NL | as sibling repos |

---

## Firmware Dependencies

See `reference/README.md` and the **Software Stack** table above. Cross-built
GMP/MPFR (for `numeric-gmp`) are produced out-of-tree and linked via `build.rs`;
they are **not** vendored into the repo (gitignored under `firmware/vendor/`).
