//! USB (OTG_FS) — a **composite device**: a CDC-ACM serial console wired to an
//! interactive RPN REPL over the real `calcumaker_core` engine, plus a HID
//! keyboard that can type the current X value into the focused host app.
//!
//! This is the design's "USB FS → CDC console / provisioning" port (DESIGN.md).
//! On the Nucleo-U575ZI-Q the USER USB-C is wired to PA11/PA12 (the same OTG_FS
//! pins as the production board), so this module is board-independent.
//!
//! Layout: one spawned task drives the USB device stack (`usb_task`); `run`
//! then owns the ACM REPL loop and the HID writer. The engine lives here for
//! now — once the matrix/display are wired it moves up to the shared `App`.

use calcumaker_core::{seg7, Calc};
use embassy_executor::Spawner;
use embassy_stm32::usb::{Driver, InterruptHandler};
use embassy_stm32::{bind_interrupts, peripherals, Peri};
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{CdcAcmClass, State as CdcState};
use embassy_usb::class::hid::{
    Config as HidConfig, HidBootProtocol, HidSubclass, HidWriter, State as HidState,
};
use embassy_usb::driver::EndpointError;
use embassy_usb::{Builder, Config, UsbDevice};
use static_cell::StaticCell;
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};

bind_interrupts!(struct Irqs {
    OTG_FS => InterruptHandler<peripherals::USB_OTG_FS>;
});

type MyDriver = Driver<'static, peripherals::USB_OTG_FS>;

/// Engine working precision for the USB REPL (bits).
const PREC: u32 = 256;
/// CDC bulk max packet size.
const PKT: usize = 64;

#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, MyDriver>) -> ! {
    device.run().await
}

/// Bring up the composite USB device and run the ACM REPL forever.
pub async fn run(
    spawner: Spawner,
    otg: Peri<'static, peripherals::USB_OTG_FS>,
    dp: Peri<'static, peripherals::PA12>,
    dm: Peri<'static, peripherals::PA11>,
) -> ! {
    // --- OTG_FS driver -------------------------------------------------------
    static EP_OUT: StaticCell<[u8; 256]> = StaticCell::new();
    let mut otg_cfg = embassy_stm32::usb::Config::default();
    // Bus-powered dev board: no VBUS sense pin wired, so assume always-powered.
    otg_cfg.vbus_detection = false;
    let driver = Driver::new_fs(otg, Irqs, dp, dm, EP_OUT.init([0; 256]), otg_cfg);

    // --- Device descriptors --------------------------------------------------
    let mut config = Config::new(0x1209, 0xC160); // pid.codes VID + a bring-up PID
    config.manufacturer = Some("Calcumaker");
    config.product = Some("Calcumaker 16 (bring-up)");
    config.serial_number = Some("CM16-NUCLEO-U575");
    config.max_power = 100;
    config.max_packet_size_0 = 64;
    // Composite device (IAD) so CDC + HID coexist under one configuration.
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;

    static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
    static BOS_DESC: StaticCell<[u8; 64]> = StaticCell::new();
    static MSOS_DESC: StaticCell<[u8; 32]> = StaticCell::new();
    static CONTROL_BUF: StaticCell<[u8; 128]> = StaticCell::new();
    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 64]),
        MSOS_DESC.init([0; 32]),
        CONTROL_BUF.init([0; 128]),
    );

    // --- CDC-ACM (REPL console) ---------------------------------------------
    static CDC_STATE: StaticCell<CdcState> = StaticCell::new();
    let mut acm = CdcAcmClass::new(&mut builder, CDC_STATE.init(CdcState::new()), PKT as u16);

    // --- HID keyboard (type X to host) --------------------------------------
    static HID_STATE: StaticCell<HidState> = StaticCell::new();
    let hid_config = HidConfig {
        report_descriptor: KeyboardReport::desc(),
        request_handler: None,
        poll_ms: 20,
        max_packet_size: 8,
        hid_subclass: HidSubclass::No,
        hid_boot_protocol: HidBootProtocol::None,
    };
    let mut hid: HidWriter<'static, MyDriver, 8> =
        HidWriter::new(&mut builder, HID_STATE.init(HidState::new()), hid_config);

    // --- Run -----------------------------------------------------------------
    spawner.spawn(usb_task(builder.build()).unwrap());
    log_info!("USB composite up: CDC-ACM REPL + HID keyboard");

    let mut calc = Calc::new(PREC);
    loop {
        acm.wait_connection().await;
        log_info!("USB host connected");
        let _ = repl(&mut acm, &mut hid, &mut calc).await;
        log_info!("USB host disconnected");
    }
}

/// One connected REPL session; returns when the host disconnects.
async fn repl(
    acm: &mut CdcAcmClass<'static, MyDriver>,
    hid: &mut HidWriter<'static, MyDriver, 8>,
    calc: &mut Calc,
) -> Result<(), EndpointError> {
    write(
        acm,
        b"\r\nCalcumaker 16 RPN - space-separated tokens; ENTER evaluates.\r\n",
    )
    .await?;
    write(
        acm,
        b"  e.g. `2 3 +`  |  `float 2 sqrt`  |  `type` sends X to host over HID\r\n> ",
    )
    .await?;

    let mut line = LineBuf::new();
    let mut buf = [0u8; PKT];
    loop {
        let n = acm.read_packet(&mut buf).await?;
        for &b in &buf[..n] {
            match b {
                b'\r' | b'\n' => {
                    write(acm, b"\r\n").await?;
                    process(acm, hid, calc, line.as_str()).await?;
                    line.clear();
                    write(acm, b"> ").await?;
                }
                0x08 | 0x7f => {
                    if line.pop() {
                        write(acm, b"\x08 \x08").await?; // erase on the terminal
                    }
                }
                _ => {
                    if line.push(b) {
                        write(acm, &[b]).await?; // echo
                    }
                }
            }
        }
    }
}

/// Evaluate one entered line: either the `type` command (HID) or RPN tokens.
async fn process(
    acm: &mut CdcAcmClass<'static, MyDriver>,
    hid: &mut HidWriter<'static, MyDriver, 8>,
    calc: &mut Calc,
    line: &str,
) -> Result<(), EndpointError> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    if line == "type" {
        let x = calc.show_fit(seg7::DIGITS_PER_ROW);
        type_string(hid, x.as_bytes()).await;
        write(acm, b"(typed X over HID)\r\n").await?;
        return Ok(());
    }
    if line == "dfu" {
        // Reboot into the STM32 ROM USB-DFU bootloader (dfu-util over USB-C).
        write(
            acm,
            b"entering ROM DFU bootloader (run `make dfu` to reflash)...\r\n",
        )
        .await?;
        Timer::after(embassy_time::Duration::from_millis(50)).await; // flush the ACM
        crate::bootloader::enter_rom_dfu();
    }
    for tok in line.split_whitespace() {
        let _ = calc.input(tok); // engine validates arity; errors are non-fatal
    }
    // Show X as the hardware display renders it — the digit-window precision
    // limit + rounding (show_fit), not the full working-precision value.
    write(acm, b"= ").await?;
    write(acm, calc.show_fit(seg7::DIGITS_PER_ROW).as_bytes()).await?;
    write(acm, b"\r\n").await
}

/// Write `data` to the ACM endpoint, split into max-packet chunks.
async fn write(acm: &mut CdcAcmClass<'static, MyDriver>, data: &[u8]) -> Result<(), EndpointError> {
    for chunk in data.chunks(PKT) {
        acm.write_packet(chunk).await?;
    }
    Ok(())
}

/// Type an ASCII string over the HID keyboard (press/release per char, then
/// ENTER). Unmappable bytes are skipped.
async fn type_string(hid: &mut HidWriter<'static, MyDriver, 8>, s: &[u8]) {
    for &ch in s {
        if let Some((modifier, key)) = ascii_to_key(ch) {
            tap(hid, modifier, key).await;
        }
    }
    tap(hid, 0, 0x28).await; // ENTER
}

/// Send one key press followed by an all-released report.
async fn tap(hid: &mut HidWriter<'static, MyDriver, 8>, modifier: u8, key: u8) {
    let press = KeyboardReport {
        modifier,
        reserved: 0,
        leds: 0,
        keycodes: [key, 0, 0, 0, 0, 0],
    };
    let _ = hid.write_serialize(&press).await;
    Timer::after_millis(5).await;
    let _ = hid.write_serialize(&KeyboardReport::default()).await;
    Timer::after_millis(5).await;
}

/// ASCII → (modifier, USB HID usage id). Covers what the engine can display:
/// digits, letters (hex/exponent), sign, dot, plus, space.
fn ascii_to_key(c: u8) -> Option<(u8, u8)> {
    const SHIFT: u8 = 0x02;
    Some(match c {
        b'1'..=b'9' => (0, 0x1E + (c - b'1')),
        b'0' => (0, 0x27),
        b'a'..=b'z' => (0, 0x04 + (c - b'a')),
        b'A'..=b'Z' => (SHIFT, 0x04 + (c - b'A')),
        b'.' => (0, 0x37),
        b'-' => (0, 0x2D),
        b'+' => (SHIFT, 0x2E), // shift + '=' key
        b' ' => (0, 0x2C),
        _ => return None,
    })
}

/// Fixed-capacity line buffer for the REPL (no alloc needed for input editing).
struct LineBuf {
    buf: [u8; 256],
    len: usize,
}

impl LineBuf {
    fn new() -> Self {
        Self {
            buf: [0; 256],
            len: 0,
        }
    }
    fn push(&mut self, b: u8) -> bool {
        if self.len < self.buf.len() {
            self.buf[self.len] = b;
            self.len += 1;
            true
        } else {
            false
        }
    }
    fn pop(&mut self) -> bool {
        if self.len > 0 {
            self.len -= 1;
            true
        } else {
            false
        }
    }
    fn clear(&mut self) {
        self.len = 0;
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}
