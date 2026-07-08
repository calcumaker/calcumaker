# calcumaker-display-fw

Firmware for the **7-segment display module** (`hardware/calcumaker-display`): an
**STM32G031K8U6** that is an SPI slave to the calculator's U575. It receives a
[`calcumaker-display-proto::DisplayFrame`](../calcumaker-display-proto) over the
unified connector, encodes the text rows to 7-seg bytes, and bit-bangs the 3
TM1640s (driven at 5 V through the on-board 74HCT125).

- Target: `thumbv6m-none-eabi` (STM32G031, Cortex-M0+). `embassy-stm32`.
- Build: `cargo build --release`. Flash: `cargo run --release` (probe-rs on the
  J3 SWD Tag-Connect). Dev logging over RTT: `--features dev`.

## Status — SCAFFOLD

Real: the TM1640 bit-bang driver (`tm1640.rs`) and the 7-seg render (`render.rs`).
Stubbed: **`recv_frame`** in `main.rs` — SPI1 slave + DMA of `WIRE_LEN` bytes;
currently returns a self-test frame. Pins in `main.rs` are placeholders — align
them with the wired `disp_mcu` schematic sheet. `render.rs` duplicates
`core::seg7`'s glyphs (so this crate needn't link the GMP-bearing engine); TODO:
split `core::seg7` into a shared crate.
