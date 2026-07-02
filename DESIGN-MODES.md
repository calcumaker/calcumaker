# Calcumaker 16 — Personalities & Modes (design plan)

> **Status: PLAN, not implemented.** This documents how we intend to support
> HP-variant "personalities" and additional display/interaction modes on the
> same hardware and engine. Nothing here changes current behavior; the
> calculator today is one personality — the HP-16C-lineage programmer model
> described in `DESIGN.md`. Implementation is phased at the end.

## 1. Motivation

The hardware — 50 Cherry MX keys with f/g shifts, 3×16 seven-segment rows —
and the engine — arbitrary-precision GMP/MPFR under an RPN stack — are far
more general than the 16C keymap currently bolted onto them. The same device
could plausibly host a scientific personality (HP-15C lineage), a financial
one (HP-12C lineage), or user-tuned variants, and users may want interaction
options (classic 4-level stack discipline) independent of any personality.

## 2. Vocabulary

Three distinct concepts, kept orthogonal:

| Term | What it is | Examples | Lives in |
|------|-----------|----------|----------|
| **Personality** | A coherent keymap + exposed function set + display conventions | `16C` (default), `SCI` (15C-flavored), `FIN` (12C-flavored), `NATIVE` (superset) | `keys::Keymap` + App |
| **Mode** | An orthogonal setting *within* a personality | radix, word size, sign mode, angle unit, FIX/SCI/ENG, lz — all already exist | `Calc` |
| **Interaction mode** | How the stack/entry behaves, independent of keymap | unbounded stack (current) vs **classic 4-level** X/Y/Z/T | `Calc` |

The engine (`Calc`) stays personality-agnostic: it is a superset of functions,
and a personality merely selects which ones are reachable from keys and how
results are displayed. This preserves the single-code-path principle — no
per-personality math.

## 3. The hardware constraint that shapes everything

**Keycap legends are printed.** A physical unit cannot relabel its keys, but
that constrains *legends*, not *switching*:

- **Runtime switching works on hardware too** — a personality is firmware
  state, selected from the **SETUP menu** (see §5.6; the menu itself is
  implemented today for the display/interaction tunables, and the
  personality selector becomes one more entry, `PErS`, in P1). Legends of
  the printed personality then simply don't match — acceptable for field
  debugging and for HP-hobbyist users, and the emulator relabels for free.
- A physical device still ships with **one printed personality** (a keycap
  SKU decision at ordering time — same PCB, different caps). Cherry MX makes
  this cheap: relegendable or swapped keycap sets, and the hot-swap socket
  option already in the design allows switch/cap experimentation.
- The **NATIVE** personality is the escape hatch: a superset keymap where the
  printed 16C legends stay truthful and extras live on f/g layers — this is
  approximately what exists today and remains the default.

## 4. Personalities planned

### 4.1 `16C` / `NATIVE` (current — formalize only)
What is implemented now: the full 16C programmer model plus our extensions
(arbitrary precision `prec`, angle modes, FIX/SCI/ENG, exact-integer
contract). Formalizing means naming it and routing it through the `Keymap`
struct rather than file-level consts. **No behavior change.**

### 4.2 `SCI` — HP-15C-flavored scientific (✅ implemented)
- **Keymap** (`keys::SCI`): digits/ENTER/shifts/arithmetic/STATUS/SETUP at
  the identical physical positions as 16C; the hex-digit row becomes inverse
  trig + log10/eˣ/10ˣ, the bitops row becomes Σ+/Σ−/mean/sdev/x!/%, the
  radix row becomes FIX/SCI/ENG/auto/angle-mode. f = hyperbolics +
  L.R./ŷ/r/CLΣ; g = nCr/nPr/RAN#/seed (+ the shared window keys).
- **Engine additions** (all in, engine superset as designed): Σ registers at
  working precision (`s+ s- mean sdev lr yhat corr clstat`), **exact** GMP
  `ncr`/`npr` (mpz binomial; size-guarded), `ran`/`seed` (xorshift64,
  deterministic until seeded; firmware seeds from hardware entropy).
- **Explicitly out of scope for SCI v1**: 15C complex numbers and matrices
  (MPC — own design round if ever).
- **Display defaults on switch** (`Keymap::apply_defaults`): DEG, FIX 4,
  decimal; 16C's bundle restores RAD + AUTO. Data (stack/registers/prec) is
  never touched by a switch.

### 4.3 `FIN` — HP-12C-flavored financial (✅ implemented; bonds deferred)
Product call: being a useful desk calculator IS the identity, so FIN is in.
- **Engine** (all at working precision, non-destructive errors): TVM
  registers n/i/PV/PMT/FV + BEG/END (`>reg` stores, `reg?` solves — the
  solved value is stored back like the 12C; `i?` via bracketed bisection);
  grouped cash flows `cf0/cfj/nj` with `npv` (discounts at the `i` register)
  and `irr` (stored into `i`); dates as M.DYYYY floats (`ddays` actual +
  30/360, `dateadd`, `dow`; Gregorian 1583–9999); depreciation
  `depsl/depsoyd/depdb` over cost=PV/salvage=FV/life=n (DB factor from `i`,
  salvage-floored); percent family `pctchg`/`pctt`/`wmean`; `12*`/`12/`.
- **Keymap** (`keys::FIN`): the 12C's TVM row lands on the hex-digit row —
  **a keyed number stores, a bare press solves** (the 12C trick, driven by
  the App's pending-entry state); cash-flow row below; f = 12×/12÷ + dates +
  depreciation; g = BEG/END + CLCF. Defaults: FIX 2, decimal.
- **Deferred to FIN v2**: bonds (PRICE/YTM — semiannual actual/actual day
  counting; the classic off-by-a-penny territory; wants golden vectors from
  the 12C handbook first), AMORT (amortization schedules), and odd-period
  (fractional-n) TVM.

### 4.4 Not planned
10C/11C (subsets of SCI, no distinct value), 41/42S (alphanumeric display
required — our 7-seg can't do a soft-menu machine), anything programmable
(keystroke programming is its own open product decision in `DESIGN.md`,
orthogonal to personalities).

## 5. Architecture changes (when implemented)

### 5.1 `keys.rs` → keymap values, not consts
```rust
pub struct Keymap {
    pub name: &'static str,
    pub base: [[Key; COLS]; ROWS],
    pub f:    [[Key; COLS]; ROWS],
    pub g:    [[Key; COLS]; ROWS],
}
pub static HP16C: Keymap = Keymap { /* today's three tables */ };
```
`App` holds `keymap: &'static Keymap` (default `&HP16C`);
`Shift::resolve` takes the keymap. The `Key` enum stays the global superset
and grows as personalities need (e.g. `SigmaPlus`, `Tvm(N|I|Pv|Pmt|Fv)`).
Firmware cost: keymaps are `const` tables in flash — negligible.

### 5.2 Classic 4-level stack as a `Calc` interaction mode (✅ implemented)
```rust
pub enum StackModel { Unbounded, Classic4 }
```
Implemented semantics for `Classic4` (SETUP item `StAC`, tokens
`stack4`/`stackfree`):
- Fixed X/Y/Z/T; **T replicates** on every consuming op via a post-op
  normalization (shortfall refills from the bottom, growth drops T) — the
  "constant in T" idiom works (`5 ENTER ENTER ENTER 2 * * *` → 250).
- **Stack lift discipline**: `Calc` carries a lift flag — entry after
  ENTER/CLx/CLEAR *overwrites* X, entry after anything else lifts; STO
  re-enables lift. In classic mode the App's keyed-number-then-ENTER also
  duplicates (`3 ENTER +` doubles), matching the real HP entry model; the
  unbounded model keeps the simpler push-once behavior.
- CLx (`drop`) zeroes X in place, keeping Y/Z/T; R↓/R↑ rotate exactly 4;
  `over` still works (treated as a lift-push — documented non-HP extra).
- **Switching**: Unbounded → Classic4 keeps the top 4 (zero-padded beneath)
  with a "top 4 kept" warning from the SETUP menu when values are dropped;
  Classic4 → Unbounded is lossless.

### 5.3 Personality selection & persistence
- `App::set_personality(&'static Keymap)` + a `Calc` defaults bundle per
  personality (angle default, FIX default, stack model suggestion).
- **Selected from the SETUP menu** (§5.6) on hardware and emulator alike — a
  `PErS` entry cycling `16C → SCI → FIN → 16C`. No dedicated key, so it
  can't be hit accidentally, and no boot chord is needed. The emulator
  additionally takes `--personality <name>` for scripting.
- Personality + all modes join the continuous-memory state (the existing
  planned `Calc` serialization for flash) so the device wakes as configured.
- Mode carryover on switch: `Calc` state (stack, registers, prec, radix…)
  is **kept** — only the keymap and display defaults change. Nothing about a
  personality switch should destroy data (except the documented Classic4
  truncation, which is the stack-model switch, not the personality switch).

### 5.4 Display conventions per personality
Small table-driven differences, all in App/format:
- radix letter suffix: 16C/NATIVE only (SCI/FIN are decimal machines);
- default FloatFmt and angle mode;
- STATUS view rows adapt (FIN shows TVM register summary instead of
  word/sign line).

### 5.5 Emulator
- `--personality <name>` flag; help overlay renders from the active keymap
  tables instead of the current static text (this is worth doing anyway —
  it removes a hand-maintained duplicate of `keys.rs`).
- Keymap-diff test: every personality's tables contain only keys the App can
  dispatch (no `Nop` regressions on printed faces).

### 5.6 The SETUP menu (✅ implemented — the runtime-configuration surface)
`g`-shift CLx opens an interactive settings menu on the glass:
`SEtUP` / `<n> <name>` / `<value>`; R↓/R↑ move between items, ENTER cycles
the value, CLx/Backspace/SETUP exits; all other keys are swallowed with a
hint. Items today: `SUFF` (radix letter), `LEAd 0` (leading zeros), `AnGLE`
(rad/deg/grad), `SIGn` (2's/1's/unsigned). Ground rules, which future items
follow:
- **Toggles and cycles only** — numeric settings (prec, wsize, FIX digits)
  stay RPN-postfix, because the keypad already does number entry well.
- Names and values must be **7-seg renderable** (enforced by test).
- The menu mutates `Calc` through the same setters the tokens use — no
  parallel state.
- `StAC` (stack model, ✅) and `PErS` (personality selector, ✅ — cycles the
  `PERSONALITIES` registry; a single entry today, so it reports
  "only 16C installed") are in. Future items (e.g. MPFR rounding mode) land
  here rather than growing new keys.

## 6. Phasing

| Phase | Scope | Risk |
|-------|-------|------|
| **P1** ✅ | `Keymap` struct + `HP16C` static + `PERSONALITIES` registry + App plumbing + `PErS` menu entry (single personality; emulator `--personality` deferred until a second exists) | done |
| **P2** ✅ | `StackModel::Classic4` in the engine + stack-lift entry discipline + `StAC` menu entry + tests | done |
| **P3** ✅ | `SCI` personality: keymap + statistics/PRNG/comb-perm engine additions; DEG/FIX-4 defaults; emulator `--personality`; keymap-dispatchability test across all personalities | done |
| **P4** ✅ | `FIN` personality: TVM/NPV/IRR/dates/depreciation engine pack + keymap (bonds/AMORT/odd-period → FIN v2) | done |
| — | Complex numbers via MPC (SCI v2) | own design round |

Each phase follows the established loop: tests at every stage, subagent
validation, per-stage commits.

## 7. Open questions (decide before P1/P2)

1. **Default angle mode per personality** — NATIVE/16C stays RAD (current,
   documented); SCI would ship DEG (HP convention). Is a per-personality
   default acceptable, or should angle mode be global-sticky?
2. **Classic4 truncation UX** — confirm-on-switch vs silent-with-message
   when the unbounded stack holds > 4 values.
3. **Keycap SKUs** — do we ever actually print non-16C caps, or are SCI/FIN
   emulator-first personalities indefinitely? (Affects how much P3/P4 UI
   polish matters.)
4. **Does `FIN` belong in this product at all**, or is it a separate
   firmware image for the same hardware? (Flash is 2 MB — space is not the
   constraint; identity is.)

## 8. Non-goals

- No per-personality math paths — one engine, superset functions.
- No dynamic/user-defined keymaps in firmware v1 (emulator experimentation
  may come first; a user keymap editor is out of scope).
- No alphanumeric display emulation (41C/42S class) on 7-seg hardware.
