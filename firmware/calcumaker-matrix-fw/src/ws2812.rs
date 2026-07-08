//! WS2812 (XL-1010) driver over the RP2040 PIO — 3 parallel data chains.
//!
//! SCAFFOLD. The RP2040's PIO is the reason this board uses an RP2040: a PIO
//! state machine clocks the 800 kHz WS2812 waveform with exact timing and near-
//! zero CPU, and up to 8 SMs can run independent chains. This board has **3
//! chains** (one per stack row, 768 px each), so 3 SMs on PIO0 + 3 DMA channels
//! feed them from the framebuffer.
//!
//! TODO(bring-up): load the ws2812 PIO program (24-bit GRB, T1/T2/T3 at the
//! 800 kHz bit period), claim SM0..2 + DMA, and stream each chain's slice of the
//! framebuffer (with the physical serpentine mapping applied). Until then
//! `flush` is a no-op so the crate builds and the render path is exercised.

use crate::font::{Frame, CHAINS, PER_CHAIN};

pub struct Driver {
    // TODO: pio: Pio<'static, PIO0>, sm: [StateMachine; CHAINS],
    //       dma: [Channel; CHAINS], and the loaded ws2812 program.
    _frames: u32,
}

impl Driver {
    /// TODO: take `PIO0` + the 3 data pins (PIN_3/4/5) + 3 DMA channels.
    pub fn new() -> Self {
        Self { _frames: 0 }
    }

    /// Map the logical framebuffer to the 3 physical chains and push them.
    ///
    /// Chain `k` = stack row `k` = framebuffer rows `k*8 .. k*8+8`, streamed as
    /// GRB. TODO: apply the per-cluster serpentine and hand each chain's byte
    /// buffer to its SM/DMA.
    pub fn flush(&mut self, fb: &Frame) {
        let _ = (fb, CHAINS, PER_CHAIN);
        self._frames = self._frames.wrapping_add(1);
        // no-op until the PIO/DMA path is wired.
    }
}

/// WS2812 wants GRB order; convert a logical (R,G,B) pixel. (Used by the PIO
/// flush path once wired.)
#[allow(dead_code)]
#[inline]
pub fn grb(px: [u8; 3]) -> [u8; 3] {
    [px[1], px[0], px[2]]
}
