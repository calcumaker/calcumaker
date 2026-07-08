# calcumaker-matrix-fw

Firmware for the **RGB dot-matrix display module** (`hardware/calcumaker-matrix`):
an **RP2040** that is an SPI slave to the calculator's U575. It receives a
[`calcumaker-display-proto::DisplayFrame`](../calcumaker-display-proto) over the
unified connector, renders the text rows into a 96×24 pixel framebuffer, and
streams it to the 2304× XL-1010 (WS2812) array over 3 PIO-driven data chains.

- Target: `thumbv6m-none-eabi` (RP2040, Cortex-M0+). `embassy-rp`.
- Build: `cargo build --release`. Flash: `cargo run --release` (probe-rs/SWD) or
  drag-drop the UF2 over USB **BOOTSEL**.
- Dev logging over RTT: `--features dev`.

## Status — SCAFFOLD

Real: the framebuffer + 5×7 font blitter (`font.rs`) and the frame decode path.
Stubbed (the bring-up work):

- **`ws2812.rs`** — the PIO WS2812 driver (3 SMs + DMA, GRB, serpentine mapping)
  is a no-op `flush`. This is the headline task; the RP2040 was chosen *for* its
  PIO.
- **`recv_frame`** in `main.rs` — SPI0 slave + DMA of `WIRE_LEN` bytes; currently
  returns a self-test frame.

⚠ **Brightness cap is mandatory** — 2304 WS2812 at full white is many amps. VLED
comes from the dedicated VSYS inlet (J2) via the `LED_EN` gate; keep colours dim
and enforce a global current budget. Pins in `main.rs` are placeholders — align
them with the wired `rp2040`/`rgb_power` schematic sheets.
