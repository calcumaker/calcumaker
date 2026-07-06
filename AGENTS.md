# Codex Assistant Guidelines — Calcumaker 16

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

## Hardware (KiCad 10) — split design, three boards (stacked)
- `hardware/calcumaker-mcu/` — the brain/PSU board: MCU + PSU + clock + SWD +
  display-IF + a **keyboard mezzanine** (J5). (Renamed from `calcumaker-main`
  2026-07-05 when the keyboard split off.)
- `hardware/calcumaker-keyboard/` — the front-panel board that **mezzanine-stacks
  above** the MCU board: the 50-key Cherry MX matrix + per-key diodes + the
  annunciator LEDs + the mating mezzanine header (J1). A dense LQFP-144 and 50
  through-hole keys don't share a PCB.
- `hardware/calcumaker-display/` — 7-seg stack (2–3 rows) + driver + interconnect
  (angled PCB, cabled to the MCU board; the display bus + power cross the
  connector). This one is **fully wired** as a KiCad multi-channel design (row ×3).
- Each board's schematic is **generated from its own data manifest**
  (`hardware/scripts/calcumaker-{mcu,keyboard,display}.schgen.py`), not
  hand-authored, then **placed, not wired** — wiring happens in eeschema using the
  per-sheet notes (the display board is the exception: its row is fully wired).
  - All manifests are **DRAFTs** with a guard (`CALCUMAKER_SCHGEN_DRAFT_OK=1`)
    pending the open part/layout items. Don't generate until those are resolved.
- Custom parts (Cherry MX footprints, display modules, MCU package if unbundled)
  go in `hardware/lib/{symbols,footprints.pretty,3dmodels}` and are registered in
  the per-board lib-tables (lib name `calcumaker`, shared).
- Run from `hardware/` (set `KICAD_CLI` if `kicad-cli` isn't on PATH):
  ```sh
  make gen-calcumaker-mcu        # regenerate a board's schematic from its manifest
  make check-calcumaker-keyboard # components / footprints / dup refs / ERC tally
  make docs                      # schematic + PCB SVGs + 3D renders + JLCPCB BOM (all boards)
  make jlc                       # full JLCPCB fab+assembly zips (all boards)
  ```

## Firmware (Rust)
- **The calculator: `firmware/calcumaker-core/`** — everything
  device-independent, a `no_std` **host-testable library**: the RPN +
  arbitrary-precision engine (`Calc`) over **GMP + MPFR** (single path, no
  fallback), the **keymap + f/g shift layers** (`keys` — design source of
  truth), key handling / entry editing (`App`), and the 7-seg encoding
  (`seg7`, TM1640 byte layout). Math is our own no_std FFI crate
  **`firmware/gmp-mpfr-nostd/`** (`Integer`/`Float`) — *like rug, but no_std*;
  host links system GMP/MPFR (`brew install gmp mpfr`), target links
  cross-built. `cargo test` / `cargo run --example repl`. **Do not add
  `rug`/`std` to the engine.** Logic lives here — not in the frontends.
- **Emulator: `firmware/calcumaker-emu/`** — the same `App` on a host terminal
  (crossterm): host keys → matrix cells, display = ASCII 7-seg from the real
  TM1640 segment bytes. `cargo run` (interactive) or
  `cargo run -- --press "2;3+"` (scripted; `;` = ENTER) for tests/demos.
- **Board crate: `firmware/calcumaker-fw/`** — **STM32U575ZGT6**, Cortex-M33,
  `no_std`, async via `embassy-stm32` (`stm32u575zg`), target
  `thumbv8m.main-none-eabihf`. Hardware only: matrix scan → `(row,col)`,
  TM1640 bus, heap via `embedded-alloc` (GMP → it via
  `mp_set_memory_functions`). On-target GMP/MPFR are **cross-built +
  link-verified** (`firmware/scripts/build-gmp-mpfr-arm.sh`); remaining
  bring-up: embassy clocks/GPIO + newlib at final link (see DESIGN.md).
- **Do not reintroduce a second numeric backend** (no pure-Rust fallback / no
  `numeric-*` features) — one GMP/MPFR path only. Same for the UI: emulator
  and firmware must stay thin I/O bindings around `calcumaker_core::App`.
- `common/` and `shared/` hold cross-target glue / protocol definitions.

## Licensing
- **Firmware** — repo `LICENSE` is **AGPL-3.0** (compatible with LGPLv3 GMP/MPFR).
- **Hardware** — `hardware/LICENSE` is **CERN-OHL-S v2** (strongly reciprocal;
  matches the AGPL posture). Keep the source-location notice (`hardware/README.md`).
