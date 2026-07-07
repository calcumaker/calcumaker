//! RCC / clock configuration, shared by both boards (Nucleo-U575ZI-Q and the
//! production RGT6 — same U575 die, same clock tree).
//!
//! - **SYSCLK = 160 MHz** (the U575 max) from **PLL1-R**, fed by the 16 MHz
//!   **HSI** (÷1 → ×10 → ÷1 = 160 MHz). RANGE1 voltage scaling is required above
//!   110 MHz. No external HSE needed, so this is board-independent.
//! - **USB 48 MHz** from the **HSI48** RC oscillator, selected as the `ICLK`
//!   kernel clock, and **CRS-trimmed against the USB host SOF** (`sync_from_usb`)
//!   — accurate enough for USB FS with no crystal. This is exactly how the
//!   production board sources its USB clock (DESIGN.md → USB FS).

use embassy_stm32::rcc::{
    Hsi48Config, LsConfig, Pll, PllDiv, PllMul, PllPreDiv, PllSource, RtcClockSource, Sysclk,
    VoltageScale, mux,
};
use embassy_stm32::Config;

pub fn config() -> Config {
    let mut config = Config::default();
    let rcc = &mut config.rcc;

    // 16 MHz HSI as the PLL1 reference.
    rcc.hsi = true;
    rcc.pll1 = Some(Pll {
        source: PllSource::HSI,   // 16 MHz
        prediv: PllPreDiv::DIV1,  // PLL input 16 MHz
        mul: PllMul::MUL10,       // VCO 160 MHz
        divp: None,
        divq: None,
        divr: Some(PllDiv::DIV1), // PLL1-R = 160 MHz → SYSCLK
    });
    rcc.sys = Sysclk::PLL1_R;
    rcc.voltage_range = VoltageScale::RANGE1; // required for SYSCLK > 110 MHz

    // 48 MHz USB clock: HSI48, CRS-synced to the USB frame start.
    rcc.hsi48 = Some(Hsi48Config { sync_from_usb: true });
    rcc.mux.iclksel = mux::Iclksel::HSI48;

    // Low-power timing substrate: enable LSI (~32 kHz) and clock the RTC from it.
    // LSI (not LSE) avoids any dependency on a 32.768 kHz crystal being fitted —
    // safe on the Nucleo; the production board can move to LSE. This is the wake/
    // sleep-timing clock; once embassy-stm32 gains a U5 RTC time driver it also
    // becomes the STOP-surviving time base (see Cargo.toml note).
    rcc.ls = LsConfig {
        rtc: RtcClockSource::LSI,
        lsi: true,
        lse: None,
    };

    config
}
