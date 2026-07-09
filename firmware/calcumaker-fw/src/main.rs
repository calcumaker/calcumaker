#![no_std]
#![no_main]

//! Calcumaker 16 firmware — MCU bring-up on **embassy-stm32** (STM32U575).
//!
//! Everything calculator lives in **`calcumaker-core`** (host-tested, emulated
//! by `calcumaker-emu`): the RPN engine over GMP + MPFR, the keymap + f/g shift
//! layers (`keys`), entry editing + dispatch (`App`), and the 7-seg encoding
//! (`seg7`). GMP/MPFR are cross-built for thumbv8m (firmware/scripts/) and
//! linked against the same FFI (`gmp-mpfr-nostd`); their C allocator
//! (malloc/free/realloc/calloc) is shimmed onto the Rust global heap below.
//!
//! Bring-up so far: 160 MHz clocks (`clock`), the arbitrary-precision engine
//! self-test (`selftest`), and USB (composite CDC-ACM REPL + HID keyboard,
//! `usb`). The keyboard matrix / TM1640 display are still stubs (`keypad`,
//! `display`) pending the board. See ../../DESIGN.md → firmware bring-up.

extern crate alloc;

use core::alloc::Layout;

use calcumaker_core::{App, Key};
use embassy_executor::Spawner;
use embassy_stm32::executor::Executor; // stm32 low-power executor (sleeps via low_power::sleep)
use embassy_time::Instant;
use static_cell::StaticCell;

// Panic handler + logger. Exactly one panic handler may be linked:
//   - production (default features): panic-halt (spin).
//   - `nucleo` validation target (--no-default-features --features nucleo):
//     panic-probe reports the panic over RTT, and defmt-rtt is the log sink.
#[cfg(feature = "panic-halt")]
use panic_halt as _;
#[cfg(feature = "nucleo")]
use {defmt_rtt as _, panic_probe as _};

// Logging shim: `log_info!` / `log_error!` map to defmt on the Nucleo target and
// compile to nothing otherwise, so the self-test payload (src/selftest.rs) is
// shared verbatim between the validation and production images. Defined before
// the modules that use them so they are in textual scope.
#[cfg(feature = "nucleo")]
macro_rules! log_info { ($($t:tt)*) => { ::defmt::info!($($t)*) }; }
#[cfg(not(feature = "nucleo"))]
macro_rules! log_info { ($($t:tt)*) => {{}}; }
#[cfg(feature = "nucleo")]
macro_rules! log_error { ($($t:tt)*) => { ::defmt::error!($($t)*) }; }
#[cfg(not(feature = "nucleo"))]
macro_rules! log_error { ($($t:tt)*) => {{}}; }

mod bootloader;
mod clock;
mod display;
mod keypad;
mod selftest;
mod usb;

// TLSF (vs LLFF) handles the variable-size bignum allocation churn with less
// fragmentation; GMP allocates here via the C shim below.
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

// ---------------------------------------------------------------------------
// C heap shim: GMP/MPFR call libc malloc/free/realloc/calloc. Back them with
// the Rust global allocator (embedded-alloc), so the bignum heap is the single
// heap. We stash the block's total size in a 16-byte header (>= max_align) so
// free()/realloc() can reconstruct the Layout.
// ---------------------------------------------------------------------------
const HDR: usize = 16;

unsafe fn c_alloc(size: usize) -> *mut u8 {
    let n = if size == 0 { 1 } else { size };
    let total = n + HDR;
    let layout = Layout::from_size_align_unchecked(total, HDR);
    let p = alloc::alloc::alloc(layout);
    if p.is_null() {
        return p;
    }
    (p as *mut usize).write(total);
    p.add(HDR)
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    c_alloc(size)
}

#[no_mangle]
pub unsafe extern "C" fn free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let base = ptr.sub(HDR);
    let total = (base as *mut usize).read();
    alloc::alloc::dealloc(base, Layout::from_size_align_unchecked(total, HDR));
}

#[no_mangle]
pub unsafe extern "C" fn realloc(ptr: *mut u8, size: usize) -> *mut u8 {
    if ptr.is_null() {
        return c_alloc(size);
    }
    if size == 0 {
        free(ptr);
        return core::ptr::null_mut();
    }
    let base = ptr.sub(HDR);
    let old_total = (base as *mut usize).read();
    let new_total = size + HDR;
    let np = alloc::alloc::realloc(
        base,
        Layout::from_size_align_unchecked(old_total, HDR),
        new_total,
    );
    if np.is_null() {
        return np;
    }
    (np as *mut usize).write(new_total);
    np.add(HDR)
}

#[no_mangle]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut u8 {
    let total = nmemb.checked_mul(size).unwrap_or(0);
    let p = c_alloc(total);
    if !p.is_null() {
        core::ptr::write_bytes(p, 0, total);
    }
    p
}

/// GMP calls `abort()` on internal errors (the engine guards against these).
#[no_mangle]
pub extern "C" fn abort() -> ! {
    loop {
        cortex_m::asm::udf();
    }
}

/// All allocation is routed through the Rust global heap (the malloc shim
/// above), so newlib's sbrk-based heap is never used. Overriding `_sbrk` here
/// keeps newlib's `sbrk.o` (which needs the linker's `end` heap marker) out of
/// the image. Never actually called; returns the sbrk failure value.
#[no_mangle]
pub extern "C" fn _sbrk(_incr: isize) -> *mut u8 {
    usize::MAX as *mut u8 // (void*)-1
}

// ---------------------------------------------------------------------------
// Every operation token the engine dispatches (calc.rs `command` / `input`),
// so the whole engine is referenced (linked) and the image exercises them all.
// Regenerate if operations are added (extracted from calc.rs).
// ---------------------------------------------------------------------------
const OPS: &[&str] = &[
    "clear", "drop", "+", "-", "*", "/", "chs", "swap", "dup", "sqrt", "sin", "cos", "tan",
    "ln", "exp", "inv", "sq", "asin", "acos", "atan", "sinh", "cosh", "tanh", "log", "exp10",
    "abs", "pow", "mod", "idiv", "pct", "e", "pi", "lastx", "enter", "over", "rolldn", "roll",
    "rollup", "and", "or", "xor", "not", "sl", "sr", "asr", "rl", "rr", "rlc", "rrc", "shl",
    "sln", "shr", "srn", "asrn", "rln", "rrn", "rlcn", "rrcn", "lj", "dbl*", "dbl/", "dblr",
    "bset", "bclr", "btest", "maskl", "maskr", "popcnt", "fact", "!", "float", "round", "trunc",
    "floor", "ceil", "frac", "hex", "dec", "oct", "bin", "wsize", "prec", "unsgn", "1s", "2s",
    "signmode", "anglemode", "rad", "deg", "grad", "lz", "suffix", "realmode", "intmode",
    "flexmode", "intentry", "stack4", "s+", "s-", "mean", "sdev", "lr", "yhat", "corr", "clstat",
    "ncr", "npr", ">n", ">i", ">pv", ">pmt", ">fv", "n?", "i?", "pv?", "pmt?", "fv?", "rcln",
    "rcli", "rclpv", "rclpmt", "rclfv", "beg", "end", "clfin", "12/", "12*", "pctchg", "pctt",
    "wmean", "cf0", "cfj", "nj", "clcf", "npv", "irr", "ddays", "dateadd", "dow", "depsl",
    "depsoyd", "depdb", "ran", "seed", "sf", "cf", "ftest", "clreg", "fix", "sci", "eng", "std",
    "sto0", "rcl0", "stof", "rclf",
];

/// Feed a few operands then the op, so binary/unary ops execute (errors are
/// ignored — the engine validates before popping, so a wrong-arity op just
/// returns Err without panicking, and its code still runs + links).
fn exercise_ops(app: &mut App) {
    for &op in OPS {
        let c = app.calc_mut();
        let _ = c.input("12345");
        let _ = c.input("678");
        let _ = c.input("2.5");
        let _ = c.input(op);
    }
    core::hint::black_box(app.calc().display());
    core::hint::black_box(app.calc().show_in(calcumaker_core::Radix::Hex));
}

/// Sweep every key on the base + f + g layers -> App dispatch, keymap, entry
/// editing, shift resolution, SETUP/STATUS paths.
fn exercise_keys(app: &mut App) {
    for shift in [None, Some(Key::ShiftF), Some(Key::ShiftG)] {
        if let Some(sk) = shift {
            app.press_key(sk);
        }
        for row in 0..5 {
            for col in 0..10 {
                app.press(row, col);
            }
        }
    }
}

#[cortex_m_rt::entry]
fn main() -> ! {
    init_heap();
    // Use embassy-stm32's low-power executor: when idle it calls low_power::sleep,
    // entering the deepest STOP mode the peripheral stop-refcounts allow. While
    // USB is enumerated its Stop1 refcount holds the part in Sleep (WFI); STOP2
    // is only reached once USB is gated. See DESIGN.md → Low-power & wake.
    static EXECUTOR: StaticCell<Executor> = StaticCell::new();
    EXECUTOR.init(Executor::new()).run(|spawner| {
        // `#[task]` returns Result<SpawnToken, _> in embassy-executor 0.10.
        spawner.spawn(amain(spawner).unwrap());
    });
}

#[embassy_executor::task]
async fn amain(spawner: Spawner) {
    // Bring up the U575: 160 MHz SYSCLK + 48 MHz USB clock + LSI/LPTIM (clock.rs).
    let p = embassy_stm32::init(clock::config());
    log_info!("calcumaker-fw boot @ 160 MHz; running engine self-test");

    // Validation payload: replay the golden RPN cases and check the on-target
    // GMP/MPFR reproduces the host results. On the Nucleo target this streams
    // PASS/FAIL over RTT; in the production image it runs silently (output
    // compiled out) but still forces the whole math stack to be linked + run.
    let t0 = Instant::now();
    let report = selftest::run_all();
    // Underscore-prefixed so it isn't flagged unused in the production build,
    // where log_info! compiles to nothing.
    let _elapsed_us = t0.elapsed().as_micros();
    log_info!(
        "self-test: {=u64} µs @ 160 MHz ({=u32} passed, {=u32} failed)",
        _elapsed_us,
        report.passed,
        report.failed,
    );
    // Keep the tally live so it (and its fields) aren't dead-stripped/warned in
    // the silent production build, where the log macros compile to nothing.
    core::hint::black_box((report.passed, report.failed, report.ok()));

    // Exercise the full engine so the linker keeps every operation + the GMP /
    // MPFR code they reach (this is the "does the whole stack link" image).
    let mut app = App::new(256);
    // Seed RAN# so it isn't the same sequence every unit/boot: the 96-bit device
    // UID (unique per chip) mixed with the boot tick. The hardware RNG is the
    // production upgrade for true per-boot entropy.
    {
        let uid = embassy_stm32::uid::uid();
        let mut s = t0.elapsed().as_ticks();
        for w in uid.chunks(8) {
            let mut b = [0u8; 8];
            b[..w.len()].copy_from_slice(w);
            s ^= u64::from_le_bytes(b);
        }
        app.calc_mut().reseed(s);
    }
    exercise_ops(&mut app);
    exercise_keys(&mut app);

    // Display / format / 7-seg encoding paths.
    core::hint::black_box(app.seg_rows());
    core::hint::black_box(app.text_rows());
    core::hint::black_box(app.x_full());
    core::hint::black_box(app.aux_lines());

    // STOP-mode demo (opt-in): bring up NO USB — the OTG peripheral's Stop1
    // refcount would otherwise pin the part in Sleep. Idle in 3 s bursts so the
    // low-power executor drops to STOP2 (RTC-woken) between them, and log the
    // uptime each wake: it must track real time across STOP, which is the
    // embassy#3504 / tick-hz-32_768 correctness check.
    #[cfg(feature = "stop-demo")]
    {
        let _ = (spawner, p.USB_OTG_FS, p.PA12, p.PA11);
        // Boot grace period: idle in short (< min_stop_pause) bursts for a few
        // seconds so the executor stays in *Sleep* (SWD/RTT alive) before it
        // starts entering deep STOP2 (which powers down the debug port). Without
        // this, a low-power image that STOPs within ~10 ms of boot locks out the
        // ST-Link — recovery then needs st-flash under reset. See DESIGN.md.
        for _ in 0..50 {
            embassy_time::Timer::after(embassy_time::Duration::from_millis(100)).await;
        }
        let mut n = 0u32;
        loop {
            log_info!("stop-demo tick {=u32} @ {=u64} ms uptime", n, Instant::now().as_millis());
            n += 1;
            embassy_time::Timer::after(embassy_time::Duration::from_secs(3)).await;
        }
    }

    // USB: composite CDC-ACM (engine REPL) + HID keyboard on OTG_FS. Owns the
    // device loop; the calculator engine lives inside it for now.
    #[cfg(not(feature = "stop-demo"))]
    usb::run(spawner, p.USB_OTG_FS, p.PA12, p.PA11).await;
}
