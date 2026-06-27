# gmp-mpfr-nostd

Minimal **`no_std`** FFI bindings to **GNU MP** (`mpz` → `Integer`) and **MPFR**
(`mpfr` → `Float`) — the subset a calculator needs. *Like `rug`, but for a
`no_std` world.*

Unlike `rug`/`gmp-mpfr-sys` (which are `std` and build the C libraries from
source with the host compiler), this crate is just declarations + thin wrappers:

- **`no_std` + `alloc`** — one crate that builds for the host *and* for a
  bare-metal target (`thumbv8m.main-none-eabihf`, …).
- **Links, doesn't build.** `build.rs` links the **system / Homebrew** GMP+MPFR
  on the host (so `cargo test` "just works" in <1 s); on the target the firmware
  links GMP/MPFR **cross-built** for the MCU. Same Rust code both places.
- **Owned types with `Drop`** — `Integer` (`mpz`) and `Float` (`mpfr`) clear
  themselves; `Clone`, the arithmetic/bitwise/shift operators, radix
  parsing/formatting, factorial, and the MPFR transcendentals are implemented.

## Use

```rust
use gmp_mpfr_nostd::{Integer, Float};

let n = Integer::from_str_radix("ff", 16).unwrap() & Integer::from_str_radix("0f", 16).unwrap();
assert_eq!(n.to_string_radix(16), "f");

let root2 = Float::from_i64(256, 2).sqrt();           // 256-bit MPFR
assert!(root2.to_string_radix(10, 20).starts_with("1.4142135623730950488"));
```

## Host requirements

GMP + MPFR present (e.g. `brew install gmp mpfr`, or `apt install libgmp-dev
libmpfr-dev`). `build.rs` finds Homebrew kegs automatically.

## Target

For `*-none-eabi*`, `build.rs` links nothing — the firmware supplies GMP/MPFR
cross-built for the MCU (`--host=arm-none-eabi --disable-assembly`, picolibc),
with GMP's allocator routed to the firmware heap. See the repo `DESIGN.md`.

## Scope / caveats

Hand-written FFI for the documented `__mpz_struct` / `__mpfr_struct` ABIs; only
the functions the engine uses are bound. Round-to-nearest throughout. Not a
general-purpose `rug` replacement — just enough for Calcumaker 16.

## License

AGPL-3.0. GMP/MPFR are LGPLv3 — compatible.
