//! calcumaker 7-seg display MODULE firmware (STM32G031K8U6, Cortex-M0+).
//!
//! Role: **SPI slave**. The calculator's U575 (`calcumaker-fw`) writes a
//! `calcumaker_display_proto::DisplayFrame` over the unified connector; this fw
//! decodes it and renders the text rows to the 3 TM1640s. It carries NO
//! calculator engine (no GMP/alloc) — just receive-decode-render.
//!
//! Status: SCAFFOLD. GPIO + the TM1640 driver + the render path are real; the
//! SPI-slave DMA receive is stubbed (`recv_frame`) and renders a self-test frame
//! until wired up. Pins below are placeholders that must match the wired
//! `disp_mcu` schematic sheet.

#![no_std]
#![no_main]

use calcumaker_display_proto::{DisplayFrame, MAX_ROWS};
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_time::{Duration, Timer};

#[cfg(feature = "panic-halt")]
use panic_halt as _;
#[cfg(feature = "dev")]
use {defmt_rtt as _, panic_probe as _};

mod render;
mod tm1640;

use tm1640::Display;

/// Receive one frame from the U575. TODO(spi): configure SPI1 as a slave
/// (PA5=SCK, PA7=MOSI, PA4=NSS) with a circular DMA of exactly `WIRE_LEN` bytes,
/// then `DisplayFrame::decode(&buf)`. For now, return a fixed self-test frame.
async fn recv_frame() -> DisplayFrame {
    Timer::after(Duration::from_millis(50)).await;
    let mut f = DisplayFrame::new();
    f.set_row(0, "CALC");
    f.set_row(1, "16");
    f.set_row(2, "rEAdY");
    f
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    // DISP_IRQ → U575 (module ready / frame-ack), idle high.
    let mut irq = Output::new(p.PA1, Level::High, Speed::Low);

    // 3× TM1640: shared CLK + per-row DIN, driven at 5 V via the on-board
    // 74HCT125 (DispPower sheet). Pins must match the wired disp_mcu sheet.
    let mut disp = Display::new(
        Output::new(p.PB0, Level::High, Speed::Low), // DISP_CLK
        Output::new(p.PB1, Level::High, Speed::Low), // DIN1 → Row1 (U1)
        Output::new(p.PB2, Level::High, Speed::Low), // DIN2 → Row2 (U2)
        Output::new(p.PB3, Level::High, Speed::Low), // DIN3 → Row3 (U3)
    );
    for r in 0..MAX_ROWS {
        disp.set_brightness(r, 4);
    }

    loop {
        let frame = recv_frame().await;
        for r in 0..MAX_ROWS {
            let cells = render::encode_row(frame.row(r));
            disp.write_row(r, &cells);
        }
        // Pulse IRQ low → an EXTI edge on the U575 ("rendered / ready for next").
        irq.set_low();
        Timer::after(Duration::from_micros(5)).await;
        irq.set_high();
    }
}
