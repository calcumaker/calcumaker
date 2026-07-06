#![no_std]
#![no_main]

//! Calcumaker 16 firmware — **cross-compile / link smoke image**.
//!
//! Everything calculator lives in **`calcumaker-core`** (host-tested, emulated
//! by `calcumaker-emu`): the RPN engine over GMP + MPFR, the keymap + f/g shift
//! layers (`keys`), entry editing + dispatch (`App`), and the 7-seg encoding
//! (`seg7`). GMP/MPFR are cross-built for thumbv8m (firmware/scripts/) and
//! linked against the same FFI (`gmp-mpfr-nostd`); their C allocator
//! (malloc/free/realloc/calloc) is shimmed onto the Rust global heap below.
//!
//! `main` here does NOT wire real GPIO/peripherals yet — it **exercises every
//! engine operation** (all ~150 `Calc` tokens + a full key/shift sweep + the
//! display/format/7-seg paths) so the linker keeps the whole engine and the
//! image proves the full arbitrary-precision stack cross-compiles and links.
//! See ../../DESIGN.md → Numeric core.

extern crate alloc;

use core::alloc::Layout;
use cortex_m_rt::entry;
use panic_halt as _;

use calcumaker_core::{App, Key};

mod display;
mod keypad;

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

#[entry]
fn main() -> ! {
    init_heap();

    // Exercise the full engine so the linker keeps every operation + the GMP /
    // MPFR code they reach (this is the "does the whole stack link" image).
    let mut app = App::new(256);
    exercise_ops(&mut app);
    exercise_keys(&mut app);

    // Display / format / 7-seg encoding paths.
    core::hint::black_box(app.seg_rows());
    core::hint::black_box(app.text_rows());
    core::hint::black_box(app.x_full());
    core::hint::black_box(app.aux_lines());

    // Board skeleton (no real GPIO yet — see keypad.rs / display.rs TODOs).
    let mut display = display::Display::new();
    let mut keypad = keypad::Keypad::new();

    loop {
        if let Some((row, col)) = keypad.scan() {
            app.press(row, col);
            display.render(&[[0; display::DIGITS_PER_ROW]; display::ROWS]);
        }
        // TODO(mcu): enter low-power Stop mode, wake on KB_IRQ from the G0.
        cortex_m::asm::wfi();
    }
}
