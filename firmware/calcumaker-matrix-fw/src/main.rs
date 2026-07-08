//! calcumaker RGB dot-matrix display MODULE firmware (RP2040, Cortex-M0+).
//!
//! Role: **SPI slave**. The calculator's U575 (`calcumaker-fw`) writes a
//! `calcumaker_display_proto::DisplayFrame` over the unified connector; this fw
//! renders the text rows into a 96×24 pixel framebuffer and streams it to the
//! 2304× XL-1010 (WS2812) array via the PIO. No calculator engine here.
//!
//! Status: SCAFFOLD. The framebuffer + font render path + the frame decode are
//! real; the WS2812 PIO output (`ws2812`) and the SPI-slave DMA receive
//! (`recv_frame`) are stubbed. Firmware MUST cap total brightness (2304 LEDs at
//! full white is many amps) — the demo below uses very dim colours. Pins are
//! placeholders that must match the wired `rp2040`/`rgb_power` schematic sheets.

#![no_std]
#![no_main]

use calcumaker_display_proto::{DisplayFrame, MAX_ROWS};
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, Timer};

#[cfg(feature = "panic-halt")]
use panic_halt as _;
#[cfg(feature = "dev")]
use {defmt_rtt as _, panic_probe as _};

mod font;
mod ws2812;

use font::Frame;

/// Receive one frame from the U575. TODO(spi): SPI0 slave (SCLK/MOSI/CS) + DMA of
/// exactly `WIRE_LEN` bytes, then `DisplayFrame::decode`. For now, a self-test.
async fn recv_frame() -> DisplayFrame {
    Timer::after(Duration::from_millis(200)).await;
    let mut f = DisplayFrame::new();
    f.set_row(0, "CALC 16");
    f.set_row(1, "DEADBEEF");
    f.set_row(2, "3.14159");
    f
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // LED_EN → the VLED high-side gate (rgb_power sheet). Start LOW (LEDs off)
    // until we have a frame; the gate cuts VLED + the shifter in sleep.
    let mut led_en = Output::new(p.PIN_2, Level::Low);

    // WS2812 chains over the PIO (3 data pins, one per stack row).
    // TODO: ws2812::Driver::new(p.PIO0, p.PIN_3, p.PIN_4, p.PIN_5, dma…).
    let mut leds = ws2812::Driver::new();
    let mut fb = Frame::new();

    // Very dim per-row tints (R,G,B) — brightness cap is mandatory here.
    let tint = [[0u8, 24, 0], [24, 12, 0], [0, 12, 24]];

    led_en.set_high(); // enable VLED

    loop {
        let frame = recv_frame().await;
        fb.clear();
        for r in 0..MAX_ROWS {
            fb.text(0, r, frame.row(r), tint[r]);
        }
        leds.flush(&fb);
        Timer::after(Duration::from_millis(33)).await; // ~30 fps
    }
}
