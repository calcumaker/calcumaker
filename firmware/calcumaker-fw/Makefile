# calcumaker-fw — build/flash helpers.
#
# Two images from ONE codebase (see Cargo.toml [features]):
#   * production link-smoke  — STM32U575RGT6, spin-on-panic, no logging.
#   * `nucleo` validation    — Nucleo-U575ZI-Q (STM32U575ZIT6Q), defmt/RTT
#                              self-test output, panic-probe.
# Both link the cross-built GMP/MPFR archives (GMP_MPFR_LIBDIR).

TARGET       := thumbv8m.main-none-eabihf
# Cross-built GMP + MPFR install prefix (firmware/scripts/build-gmp-mpfr-arm.sh).
GMP_MPFR_LIBDIR ?= $(abspath ../vendor/gmp-mpfr-arm)
export GMP_MPFR_LIBDIR

# probe-rs chip ids. Production part vs. the Nucleo-U575ZI-Q die (2 MB, SMPS).
CHIP_PROD    := STM32U575RGTx
CHIP_NUCLEO  := STM32U575ZITxQ

# `cargo run` on an embedded target invokes the runner from .cargo/config.toml.
# For the Nucleo we override it (env var beats config) to point probe-rs at the
# ZI die and stream defmt/RTT.
NUCLEO_RUNNER := probe-rs run --chip $(CHIP_NUCLEO)
RUNNER_ENV    := CARGO_TARGET_THUMBV8M_MAIN_NONE_EABIHF_RUNNER="$(NUCLEO_RUNNER)"

.PHONY: build build-nucleo run-nucleo check check-nucleo clean size probe

## Production link-smoke image (default features).
build:
	cargo build --release

## Nucleo validation image (defmt/RTT self-test). Builds only; does not flash.
build-nucleo:
	cargo build --release --no-default-features --features nucleo

## Flash the Nucleo-U575ZI-Q over its on-board ST-LINK and stream the RTT
## self-test output (probe-rs decodes defmt). Ctrl-C to detach.
run-nucleo:
	$(RUNNER_ENV) cargo run --release --no-default-features --features nucleo

## Type-check both configurations.
check:
	cargo check --release
check-nucleo:
	cargo check --release --no-default-features --features nucleo

## Show the section sizes of the last-built Nucleo image.
size:
	arm-none-eabi-size target/$(TARGET)/release/calcumaker-fw

## List attached debug probes.
probe:
	probe-rs list

clean:
	cargo clean
