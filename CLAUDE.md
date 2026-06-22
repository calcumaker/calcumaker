# Claude Code Assistant Guidelines — Calcumaker 16

Repo `calcumaker`; product **Calcumaker 16** (see `NAMING.md`). A wide-format,
full-size-Cherry-MX **programmer's / technical RPN calculator**. It follows the
HP-16C lineage (hex / oct / bin / dec, bitwise ops, selectable word sizes) and
extends it with **arbitrary-precision** math: **GNU MP (libgmp)** for big
integers and **MPFR (libmpfr)** for correctly-rounded floating point and
transcendentals. A multi-row 7-segment display (2–3 rows) shows the top of the
RPN stack. **Split design:** the display sits on its own angled PCB. Battery +
USB-C powered. The firmware main loop is **Rust (no_std)**. This repo holds the
KiCad hardware (`hardware/`), firmware (`firmware/`), the design doc
(`DESIGN.md`), naming (`NAMING.md`), and references (`reference/`).

## General Rules
- Do not run `cat` or `stty` commands.
- Do not read debug messages from the serial port — ask the user to do that.
- Use Makefile / Cargo targets when possible.
- This is a **KiCad 10** project — use `kicad-cli` for schematic/PCB operations.
- Use `uv` for all Python operations (never bare `pip`).
- **Don't assume** final part choices that are still open (display driver/digits,
  interconnect connector, keypad layout, battery). Source of truth is `DESIGN.md`.

## Source of truth
- **`DESIGN.md`** — full hardware + firmware design, decisions, the part list,
  the schematic sheet plan, and the **Open Questions** (display/interconnect
  parts, keypad layout, numeric-backend bring-up).
- `reference/README.md` — dependency + datasheet pointers.
- Per-sheet **on-canvas notes** in the schematic carry the wiring/pin spec.

## Hardware (KiCad 10) — split design, two boards
- `hardware/calcumaker-main/` — MCU + PSU + keypad + interconnect.
- `hardware/calcumaker-display/` — 7-seg stack (2–3 rows) + driver + interconnect
  (angled PCB; only +3V3/GND + the display SPI bus cross the connector).
- Each board's schematic is **generated from its own data manifest**
  (`hardware/scripts/calcumaker-{main,display}.schgen.py`), not hand-authored,
  then **placed, not wired** — wiring happens in eeschema using the per-sheet
  notes.
  - Both manifests are **DRAFTs** with a guard (`CALCUMAKER_SCHGEN_DRAFT_OK=1`)
    pending the open part/layout items. Don't generate until those are resolved.
- Custom parts (Cherry MX footprints, display modules, MCU package if unbundled)
  go in `hardware/lib/{symbols,footprints.pretty,3dmodels}` and are registered in
  the per-board lib-tables (lib name `calcumaker`, shared).
- Run from `hardware/` (set `KICAD_CLI` if `kicad-cli` isn't on PATH):
  ```sh
  make gen-calcumaker-main       # regenerate a board's schematic from its manifest
  make check-calcumaker-display  # components / footprints / dup refs / ERC tally
  make docs                      # schematic + PCB SVGs + 3D renders + JLCPCB BOM (all boards)
  make jlc                       # full JLCPCB fab+assembly zips (all boards)
  ```

## Firmware (Rust, no_std)
- App crate: `firmware/calcumaker-fw/` (**STM32U575ZGT6**, Cortex-M33, `no_std`,
  async via `embassy-stm32` feature `stm32u575zg`; target
  `thumbv8m.main-none-eabihf`).
- **Numeric core** is abstracted behind `firmware/calcumaker-fw/src/numeric/` so
  the engine can sit on **GMP/MPFR via FFI** (preferred) **or** a **pure-Rust**
  arbitrary-precision backend (fallback) — selected by Cargo feature.
- Heap: bignum allocators need a `no_std` global allocator (`embedded-alloc`);
  GMP is pointed at it via `mp_set_memory_functions`.
- `common/` and `shared/` hold cross-target glue / protocol definitions.

## Licensing
- **Firmware** — repo `LICENSE` is **AGPL-3.0** (compatible with LGPLv3 GMP/MPFR).
- **Hardware** — `hardware/LICENSE` is **CERN-OHL-S v2** (strongly reciprocal;
  matches the AGPL posture). Keep the source-location notice (`hardware/README.md`).
