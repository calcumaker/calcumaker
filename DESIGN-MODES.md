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

### 4.2 `SCI` — HP-15C-flavored scientific
- **Keymap**: promote the scientific set to primary faces (trig unshifted,
  `y^x`, `1/x`, `%`, statistics), demote the programmer row (bitops/radix
  move to f/g or disappear).
- **Engine additions needed**: statistics registers (Σ+, mean, s, linear
  regression) — straightforward on MPFR; `RAN#` (PRNG — decide determinism
  policy); combinations/permutations (GMP exact — cheap).
- **Explicitly out of scope for SCI v1**: 15C complex numbers and matrices.
  Complex wants **MPC** (the MPFR-based complex library) — a real dependency
  decision (cross-build, memory) that gets its own design round if ever.
- **Display**: FIX 4 default (HP convention), DEG default angle mode — note
  this personality flips the angle default; see §7.

### 4.3 `FIN` — HP-12C-flavored financial
- **Engine additions needed**: TVM solver (n, i, PV, PMT, FV — an iterative
  root-find on MPFR; well-understood), amortization, NPV/IRR (iterative),
  date arithmetic (calendar math, no MPFR involvement).
- **Keymap**: top row becomes n/i/PV/PMT/FV like the 12C.
- **This is the largest work item** and the least aligned with the
  "programmer's calculator" identity — phase it last, decide separately
  whether it's wanted at all.

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

### 5.2 Classic 4-level stack as a `Calc` interaction mode
```rust
pub enum StackModel { Unbounded, Classic4 }
```
Semantics to implement for `Classic4` (the part that needs care):
- Fixed X/Y/Z/T; **T replicates** on every two-operand op (`T→Z→Y`, T kept).
- **Stack lift discipline**: entry after ENTER *overwrites* X (lift
  disabled); entry after an operation *lifts*. CLx disables lift. This is
  the real HP entry model our simplified flush-based entry currently skips —
  implementing it properly means App entry and Calc coordinate on a lift
  flag.
- R↓/R↑ become 4-element rotations; `over` disappears (not an HP key).
- **Switching semantics**: Unbounded → Classic4 keeps the top 4, warns via
  message if depth > 4 (the rest is *dropped* — destructive, so the switch
  itself should confirm or be documented loudly); Classic4 → Unbounded is
  lossless.
- Engine tests to write: T-replication chains (the "constant in T" idiom),
  lift-flag transitions for every op class, CLx behavior.

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
- Planned additions land here rather than growing new keys: `PErS`
  (personality, P1), `StAC` (stack model, P2), MPFR rounding mode if added.

## 6. Phasing

| Phase | Scope | Risk |
|-------|-------|------|
| **P1** | `Keymap` struct + `HP16C` static + App plumbing + emulator `--personality` (only one personality exists; pure refactor, zero behavior change) | low |
| **P2** | `StackModel::Classic4` in the engine + proper stack-lift entry discipline + tests | medium — touches the entry model |
| **P3** | `SCI` personality: keymap + statistics/PRNG/comb-perm engine additions; DEG/FIX-4 defaults | medium |
| **P4** | `FIN` personality: TVM/NPV/IRR/date engine pack | high effort; separate go/no-go |
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
