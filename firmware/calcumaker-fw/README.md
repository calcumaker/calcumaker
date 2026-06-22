# calcumaker-fw

The calcumaker application crate — Rust **`no_std`**, Cortex-M.

## Modules

| Module | Role |
|--------|------|
| `main.rs` | Heap init, peripheral bring-up (TODO: embassy), the super-loop. |
| `rpn.rs` | RPN stack + evaluation (programmer's model, HP-16C lineage). Host-testable. |
| `keypad.rs` | Cherry MX matrix scan → decoded `Key` events. |
| `display.rs` | Multi-row 7-segment driver — renders the X/Y/Z/T stack. |
| `numeric/` | Arbitrary-precision core; `Number` over a swappable backend. |

## Numeric backend (Cargo feature — pick one)

- `numeric-pure` *(default)* — pure-Rust (dashu / astro-float). Fully `no_std`,
  always buildable.
- `numeric-gmp` — **GNU MP + MPFR via FFI** (preferred). Requires a cross-built
  `libgmp`/`libmpfr`; see `build.rs` and `../../DESIGN.md`.

```bash
cargo build --release                                                   # pure-Rust
cargo build --release --no-default-features --features numeric-gmp      # GMP/MPFR
```

## Not buildable yet

The MCU/HAL is not pinned, so the target, `embassy-stm32` chip feature,
`memory.x`, and the probe-rs chip name in `.cargo/config.toml` are placeholders.
See `../README.md` and `../../DESIGN.md` for the wiring checklist.
