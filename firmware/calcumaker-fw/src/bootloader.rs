//! Firmware update via the **STM32 ROM USB-DFU bootloader** — no custom
//! bootloader, no A/B slots (the app owns all of flash).
//!
//! Entry: the `dfu` command programs the **boot option bytes** so the next reset
//! boots system memory, then sets **OBL_LAUNCH**, which reloads the option bytes
//! and resets straight into the ROM USB-DFU device (`0483:df11`). `dfu-util` then
//! flashes the app *and* rewrites the option bytes back to boot the app — see the
//! `make dfu` target. Physical **BOOT0** high at reset reaches the same ROM code
//! as a hardware backup.
//!
//! Why option bytes and not a jump to `0x0BF90000`: on the U5 the naive jump
//! HardFaults (system memory reads as 0 even with TrustZone off); the RM-defined
//! entry is the boot pattern (nSWBOOT0 / nBOOT0), which is verified working here.

use embassy_stm32::pac::FLASH;

// Flash unlock key sequences (RM0456).
const KEY1: u32 = 0x4567_0123;
const KEY2: u32 = 0xCDEF_89AB;
const OPTKEY1: u32 = 0x0819_2A3B;
const OPTKEY2: u32 = 0x4C5D_6E7F;

/// Reboot into the ROM USB-DFU bootloader by programming the boot option bytes
/// (nSWBOOT0 = 0, nBOOT0 = 0 → boot system memory) and launching an option-byte
/// reload. `OBL_LAUNCH` triggers an immediate system reset, so this never
/// returns. The host must restore nBOOT0 = 1 after flashing (the `make dfu`
/// target does this via `dfu-util` alt 1) or the part stays in DFU.
pub fn enter_rom_dfu() -> ! {
    cortex_m::interrupt::disable();
    // Standard flash option-byte program sequence (RM0456 §7).
    while FLASH.nssr().read().bsy() {}
    // Unlock the flash control and option registers.
    FLASH.nskeyr().write_value(KEY1);
    FLASH.nskeyr().write_value(KEY2);
    FLASH.optkeyr().write_value(OPTKEY1);
    FLASH.optkeyr().write_value(OPTKEY2);
    // Select "boot from system memory" for the next reset.
    FLASH.optr().modify(|w| {
        w.set_n_swboot0(false);
        w.set_n_boot0(false);
    });
    FLASH.nscr().modify(|w| w.set_optstrt(true));
    while FLASH.nssr().read().bsy() {}
    // Reload option bytes → immediate system reset into the ROM bootloader.
    FLASH.nscr().modify(|w| w.set_obl_launch(true));
    loop {
        cortex_m::asm::nop();
    }
}
