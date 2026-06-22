#![no_std]
#![no_main]

//! calcumaker — programmer's / technical arbitrary-precision RPN calculator.
//!
//! Chip-agnostic `no_std` skeleton: a cortex-m-rt super-loop that scans the
//! Cherry MX matrix, feeds the RPN engine, and refreshes the 7-segment stack
//! display. It will move to an `embassy-stm32` async executor once the MCU is
//! pinned (see ../../DESIGN.md). Does not build until the HAL + a numeric
//! backend are wired.

extern crate alloc;

use cortex_m_rt::entry;
use panic_halt as _;

mod display;
mod keypad;
mod numeric;
mod rpn;

// TLSF (vs LLFF) handles the variable-size bignum allocation churn with less
// fragmentation — see ../../DESIGN.md → Numeric core.
use embedded_alloc::TlsfHeap as Heap;

#[global_allocator]
static HEAP: Heap = Heap::empty();

/// Heap backing the arbitrary-precision allocator. Provisional — size against
/// the chosen MCU's RAM and the working precision (DESIGN.md → Numeric core).
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
    numeric::init(); // route GMP's allocator to the global heap (no-op for pure-Rust)

    // TODO(mcu): clocks + GPIO/SPI/I2C init via embassy-stm32 once pinned.
    let mut stack = rpn::Stack::new();
    let mut display = display::Display::new();
    let mut keypad = keypad::Keypad::new();

    display.render(&stack);

    loop {
        if let Some(key) = keypad.scan() {
            stack.handle(key);
            display.render(&stack);
        }
        // TODO(mcu): enter low-power Stop mode, wake on a key-matrix interrupt.
        cortex_m::asm::wfi();
    }
}
