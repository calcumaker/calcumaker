# calcumaker-display-proto

The **display-module wire protocol** — the "display intent" frame the calculator's
U575 (`calcumaker-fw`) sends over SPI, and that each display module renders
locally:

- the 7-seg module (`calcumaker-display-fw`, STM32G031), and
- the RGB-matrix module (`calcumaker-matrix-fw`, RP2040).

`DisplayFrame` carries text rows + annunciator/mode flags + aux-OLED content — a
display-agnostic *intent*, not raw driver bytes — so one frame serves any display
and a new display is just a new module (no MCU-board change). Fixed-length wire
form (`WIRE_LEN`, magic + version + XOR checksum) so a slave can DMA a constant
transfer.

Pure `no_std`, **no dependencies**, host-testable: `cargo test`.
