#![no_std]
#![no_main]

//! Calcumaker 16 firmware — hardware skeleton.
//!
//! The calculator logic + arbitrary-precision math live in the
//! **`calcumaker-core`** library (RPN engine over GMP + MPFR, single path,
//! host-tested). This crate is the board bring-up: clocks, the Cherry MX matrix
//! scan, the 7-segment display driver, and (eventually) hosting the engine.
//!
//! Engine integration is the open task: `calcumaker-core` is `std`/`rug` for
//! host testing; on the STM32 we link the SAME GMP/MPFR cross-built for
//! thumbv8m and route GMP's allocator to the global heap below
//! (`mp_set_memory_functions`). See ../../DESIGN.md → Numeric core.

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

    // TODO(mcu): clocks + GPIO/SPI init via embassy-stm32 once pinned.
    // TODO(engine): construct a calcumaker_core::Calc here once the engine is
    // linked for the target (point GMP's allocator at the heap above).
    let mut display = display::Display::new();
    let mut keypad = keypad::Keypad::new();

    loop {
        if let Some(_key) = keypad.scan() {
            // TODO(engine): feed `_key` into the Calc engine, then render its
            // formatted stack rows:
            display.render(&[]);
        }
        // TODO(mcu): enter low-power Stop mode, wake on a key-matrix interrupt.
        cortex_m::asm::wfi();
    }
}
