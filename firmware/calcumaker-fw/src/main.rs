#![no_std]
#![no_main]

//! Calcumaker 16 firmware — hardware skeleton.
//!
//! Everything calculator lives in **`calcumaker-core`** (host-tested, and
//! emulated on a terminal by `calcumaker-emu`): the RPN engine over GMP + MPFR,
//! the keymap + f/g shift layers (`keys`), entry editing + dispatch (`App`),
//! and the 7-seg segment encoding (`seg7`). This crate is only the board:
//! clocks, the Cherry MX matrix scan → `(row, col)`, and the TM1640 bus that
//! pushes `App::seg_rows()` bytes to the glass.
//!
//! GMP/MPFR are cross-built for thumbv8m (firmware/scripts/, link-verified);
//! remaining bring-up: embassy clocks/GPIO, newlib libc/libm at final link, and
//! routing GMP's allocator to the heap below via `mp_set_memory_functions`.
//! See ../../DESIGN.md → Numeric core.

extern crate alloc;

use cortex_m_rt::entry;
use panic_halt as _;

mod display;
mod keypad;

// TLSF (vs LLFF) handles the variable-size bignum allocation churn with less
// fragmentation; GMP allocates here once wired up.
use embedded_alloc::TlsfHeap as Heap;

#[global_allocator]
static HEAP: Heap = Heap::empty();

/// Heap backing the arbitrary-precision allocator. Provisional — size against
/// the chosen working precision (DESIGN.md → Numeric core).
const HEAP_SIZE: usize = 64 * 1024;

fn init_heap() {
    use core::mem::MaybeUninit;
    static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
    // SAFETY: called exactly once, before any allocation.
    unsafe { HEAP.init(core::ptr::addr_of_mut!(HEAP_MEM) as usize, HEAP_SIZE) }
}

#[entry]
fn main() -> ! {
    init_heap();

    // TODO(mcu): clocks + GPIO init via embassy-stm32 once pinned.
    // TODO(engine): once GMP's allocator is routed to the heap, this loop is
    //   let mut app = calcumaker_core::App::new(256);
    //   ... app.press(row, col); display.render(&app.seg_rows());
    // (exactly what calcumaker-emu runs on the host today).
    let mut display = display::Display::new();
    let mut keypad = keypad::Keypad::new();

    loop {
        if let Some((_row, _col)) = keypad.scan() {
            display.render(&[[0; display::DIGITS_PER_ROW]; display::ROWS]);
        }
        // TODO(mcu): enter low-power Stop mode, wake on a key-matrix interrupt.
        cortex_m::asm::wfi();
    }
}
