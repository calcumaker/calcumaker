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
| 1 | Math stack | **GMP + MPFR only — single path, no fallback.** Engine = `calcumaker-core` over our own no_std bindings `gmp-mpfr-nostd` (host: links system GMP/MPFR; target: cross-built). One `no_std` crate, host-tested + REPL. |
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
- **g (blue)** → secondary: SINH/COSH/TANH, LOG, 10ˣ, n!, %, RND, OFF.
- The full keymap is the source of truth in
  `firmware/calcumaker-fw/src/keypad.rs` (`BASE` / `LAYER_F` / `LAYER_G`) — keep
  the two in sync. Shift assignments marked `Nop` are open for refinement.

**Electrical:** 5-row × 10-col scanned matrix. ROWr = GPIO outputs, COLc = GPIO
inputs on **internal pull-ups** (no external resistors — lower idle current;
STM32U5 retains pull-ups in Stop). **One 1N4148W per key** (anode at switch,
cathode to its column) for n-key rollover. One column also drives an EXTI line:
in Stop all rows are held low, so any keypress pulls a column → wake. 15 GPIO
(5 rows + 10 cols). Refs: `SW1..SW50` (key `(r,c)` = `SW(r-1)*10+c`), diodes
`D11..D60`. Optional **Kailh hot-swap sockets** (same footprint family).

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
| Engine | **`calcumaker-core`** (RPN, no_std) | one path; host-tested + REPL |

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
cargo test                            # 12 engine tests vs real GMP/MPFR
cargo run --example repl              # interactive RPN
```

Crucially this is **one crate, `no_std`** — it also compiles for the MCU target
(`cargo build --target thumbv8m.main-none-eabihf` succeeds today); only the final
link to the C libraries differs. This is the single source of truth for the
calculator logic and math.

## GMP/MPFR on the target (the remaining step)

The Rust is already `no_std` and target-compiling; the only thing left is to
provide GMP/MPFR as static libs for the MCU and link them. Recipe (see recall
note `ref-gmp-mpfr-no-std`):

1. Build **GMP**: `./configure --host=arm-none-eabi --disable-assembly`
   `CC=arm-none-eabi-gcc CFLAGS="-mcpu=cortex-m33 -mthumb --specs=nosys.specs -nostartfiles"`.
   `--disable-assembly` is **mandatory** (no `mpn` asm backend for M-profile).
2. Build **MPFR** against that GMP (`--with-gmp-include` / `--with-gmp-lib`;
   versions must match).
3. Link **picolibc** for the libc + libm symbols MPFR needs.
4. Route GMP's allocator to the firmware heap via
   `mp_set_memory_functions(malloc, realloc, free)` at init.
5. Point `calcumaker-fw/build.rs` at the `.a`s (`rustc-link-lib=static=mpfr`,
   `=gmp`). `gmp-mpfr-nostd`'s own `build.rs` already no-ops on `-none-eabi`, so
   nothing else changes — same FFI, just a different linker input.

- **Footprint:** ~0.5–1 MB flash for both libs; heap scales with precision —
  the reason for the large-flash MCU.
- **Licensing:** GMP is LGPLv3/GPLv2, MPFR is LGPLv3 — compatible with the
  AGPL-3.0 firmware; honor LGPL relinking terms for a shipped product.
- **Risk / open:** the cross-build + picolibc is the one finicky part. If it ever
  proves impractical, the fallback is *not* a second math library (we removed
  that) but a hardware reconsideration (a Linux-capable SoM, where the same
  bindings link the on-device system GMP/MPFR).

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
use (all on the main board):

- **Display bus** → 4 GPIOs: TM1640 2-wire (shared CLK + DIN1/DIN2/DIN3),
  bit-banged at 3V3 → 74HCT125 → 5V → interconnect. Plus **DISP_PWR_EN** GPIO →
  5V-boost EN (display off in sleep).
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
| MCU | `mcu.kicad_sch` | STM32U575ZGTx (U1) + VDD/VDDA/VDDUSB decoupling + VCORE + NRST/BOOT0 |
| Clock | `clock.kicad_sch` | LSE 32.768 kHz crystal (Y1) + load caps (RTC) |
| Programming | `prog.kicad_sch` | SWD Tag-Connect TC2030-NL (J4) |
| PSU | `psu.kicad_sch` | USB-C + ESD + charger + load-share + 3V3 buck-boost (MCU) + battery conn |
| Keypad | `keypad.kicad_sch` | 5×10 Cherry MX matrix (50 SW + 50 diodes) + wake line |
| DisplayIF | `display_if.kicad_sch` | EN-gated 5V boost (TPS61022) + 74HCT125 level shifter + J3 → display |

Both boards **generate from their manifests and pass the structure check**
(placed-not-wired): `calcumaker-main` = 149 components across the 6 subsheets
above; `calcumaker-display` = 21 components. All symbols are stock KiCad except
the authored TM1640.

**`calcumaker-display`:**

| Sheet | File | Contents |
|-------|------|----------|
| Root | `calcumaker-display.kicad_sch` | sheet symbols + title block |
| Display | `display.kicad_sch` | 7-seg array (2–3 rows) + driver chain + brightness/blank |
| Interconnect | `interconnect.kicad_sch` | J1 ← main board (pinout matches main J3) |

---

## Numeric Core

`firmware/calcumaker-core/` is the engine, with **one** numeric path:

- `Value` = `Int(gmp_mpfr_nostd::Integer)` (GMP) **or**
  `Real(gmp_mpfr_nostd::Float)` (MPFR).
- `Calc` = the RPN stack + token input; integers stay integers through
  `+ - * /` and the bitwise/shift ops (HP-16C model, masked to the word size);
  the scientific functions promote to MPFR reals.
- HAL-free and fully **host-testable** (`cargo test`) + runnable
  (`cargo run --example repl`).

The firmware consumes this crate; on the target the only thing that changes is
where GMP/MPFR come from (cross-built, linked at the FFI layer) — the engine code
is identical.

---

## Open Questions

Resolved: ✅ MCU (Q7) · ✅ board partition = split (Q8) · ✅ hardware license =
CERN-OHL-S (Q9) · ✅ product name = Calcumaker 16 (Q10) · ✅ display driver+digits
(TM1640 + FJ5161AH) · ✅ interconnect (1×8 2.54 mm header). Remaining:

1. ✅ **KiCad symbols done** — digits use stock `CC56-12EWA`; TM1640 authored
   (`lib/symbols/calcumaker.kicad_sym`); display board generates + checks OK.
   Remaining: confirm THT-assembly route (JLCPCB THT add-on vs hand-solder), and
   verify the FJ5161AH pinout vs CC56-12 + the TM1640 SOP-28 footprint at layout.
2. ✅ **Display rail = 5 V + level shifter** (decided + parts chosen). EN-gated
   **TPS61022** boost (C915088) + 1µH FTC201610 (C5832342) + 0603 caps; FB
   divider R6 732k/R7 100k → 5V. **SN74HCT125** level shifter (C352957, KiCad
   symbol `74AHCT125`). Remaining: verify boost Isat/FB and the downsized 3V3
   inductor Isat at layout. (TPS61022 + STM32U575 symbols turned out stock in
   KiCad, so the main board generates with no custom authoring.)
3. ✅ **Numeric engine = single GMP/MPFR path**, our own `no_std` bindings
   (`gmp-mpfr-nostd`) — host-tested + REPL, and the crate already compiles for
   `thumbv8m`. **Remaining: cross-build GMP/MPFR for the MCU + link them** (see
   "GMP/MPFR on the target"). The Rust side is done; this is just the C libs.
4. ✅ **Keypad designed + main board generated.** 5×10 (50 keys), f/g scheme,
   internal-pull-up matrix + EXTI wake. The main board is decomposed into 6
   subsheets (MCU / Clock / Programming / PSU / Keypad / DisplayIF), all symbols
   stock (TPS61022 + STM32U575 were both in KiCad), and it **generates + passes
   the structure check** (149 comp). Remaining: refine `Nop` shift assignments;
   confirm Cherry MX vs Kailh hot-swap; verify the STM32U5 VCORE LDO-vs-SMPS
   choice (SMPS needs an inductor); then **wire both boards in eeschema**.
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
| Interconnect | **PZ254V-11-08P** 1×8 2.54mm header (carries +5V) | ✅ LCSC C492407; main J3 ↔ display J1 |
| Keyswitches (main) ×50 | Cherry MX (full size) + optional Kailh hot-swap sockets | 5×10 matrix |
| Key diodes (main) ×50 | 1N4148W (SOD-123) | C81598; one per key (NKRO) |
| USB-C (main) | receptacle + CC 5.1k + USBLC6 ESD | as ephemerkey PSU |
| Charger (main) | MCP73831 / BQ-class | sized to cell |
| Buck-boost 3V3 (main) | TPS63900 (ULP, low-Iq) — **MCU only** | ✅ stays as-is (light load); L→0805 |
| 5V boost (main) | **TPS61022RWUR** (EN-gated) + 1µH (FTC201610) + 0603 caps | ✅ LCSC C915088 / C5832342 |
| Level shifter (main) | **SN74HCT125DR** quad buffer @5V (CLK+DIN×3) | ✅ LCSC C352957 (symbol `74AHCT125`) |
| Battery (main) | 1S Li-ion (JST-PH) | capacity TBD |
| RTC crystal (main) | 32.768 kHz | LSE |
| Programming (main) | SWD Tag-Connect TC2030-NL | as sibling repos |

---

## Firmware Dependencies

See `reference/README.md` and the **Software Stack** table above. The engine
(`calcumaker-core`) depends on **`gmp-mpfr-nostd`** (our no_std FFI). On the host
it links the **system** GMP/MPFR (`brew install gmp mpfr`); for the target the
**cross-built** GMP/MPFR are produced out-of-tree and linked via
`calcumaker-fw/build.rs` — **not** vendored into the repo (gitignored under
`firmware/vendor/`).
