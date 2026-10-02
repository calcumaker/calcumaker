# Mainboard and keypad wiring review — 2026-10-02

The MCU and keyboard schematics are now connected KiCad 10 hierarchies. Edit the
committed `.kicad_sch` files in KiCad; the guarded `*.schgen.py` manifests describe
an earlier placement draft and must not be forced over this wiring.

## What is connected

- Mainboard: MCU supplies and VCAP, LSE, reset/boot/SWD, display interface,
  six QSPI signals, keyboard interfaces, USB data/CC protection, charger,
  fuel gauge, battery/VBUS measurement and 3.3 V regulator.
- Keypad: scanner MCU, 49 switch/diode pairs, five annunciators, RGB power
  switch/buffer, 49 RGB LEDs in one chain and 49 local bypass capacitors.
- Parent sheets carry explicit signal ports and wires. Repeated key rows share
  the column and supply globals; each row's scan and RGB ports stay hierarchical.
  Row4 uses the nine-key variant. SW36/D36/D91/C136 are intentionally absent
  above the lower-cell 2U ENTER switch (SW46).
- Existing real component identities were retained; repeated-sheet annotation
  and project root paths were repaired for KiCad 10. Added components have
  distinct references and all 302 components have assigned footprints.

## Checks

From `hardware/`:

```sh
make check-wiring check-calcumaker-mcu check-calcumaker-keyboard
```

`check-wiring.py` exports fresh KiCad XML netlists and ERC JSON. It rejects
annotation errors, duplicate references, missing footprints, unexpected ERC
violations and failed electrical assertions. It checks package-pin assignments,
connector parity, every matrix series connection, diode polarity, the complete
49-LED chain, every RGB bypass, supply separation, reset/debug, QSPI, charger
controls and regulator configuration. To retain reports:

```sh
uv run python scripts/check-wiring.py --output-dir /tmp/calcumaker-check
```

Verified with KiCad 10.0.6:

| Board | Components | ERC errors | ERC warnings |
|---|---:|---:|---:|
| MCU | 75 | 0 | 1 reviewed |
| Keyboard | 227 | 0 | 0 |

The sole mainboard warning is U1 PC6/pad37 versus PSU `#FLG06` on `TCPP01_EN`.
PC6 supplies TCPP01 VCC as specified by its GPIO-powered application circuit;
KiCad models the GPIO as bidirectional. The power flag declares this supply
intent. The checker accepts only that particular pair of pin UUIDs, and also
asserts the net's exact membership. The ERC rule remains enabled project-wide.
This is a documented electrical-model warning, not a claim of measured hardware.

## Connector contract

UART names are from the **mainboard's perspective**: `UART_TX` is main PA2 to
keyboard PA3, and `UART_RX` is keyboard PA2 to main PA3. Populate either the stack
connector or the FFC pair. Verify physical contact orientation before layout.

| Pin | Stack: main J5 / keyboard J1 | FFC: main J6 / keyboard J3 |
|---:|---|---|
| 1 | +3V3 | +3V3 |
| 2 | GND | GND |
| 3 | I2C_SDA | I2C_SDA |
| 4 | I2C_SCL | I2C_SCL |
| 5 | UART_TX | UART_TX |
| 6 | UART_RX | UART_RX |
| 7 | KB_IRQ | KB_IRQ |
| 8 | KB_NRST | KB_NRST |
| 9 | KB_BOOT0 | KB_BOOT0 |
| 10 | GND | GND |
| 11 | VSYS | VSYS |
| 12 | GND | VSYS |
| 13–14 | — | GND |
| 15–16 | — | deliberate NC |

## Pin assignments added or corrected

The full U575 QSPI/SPI/UART/I2C map remains in [DESIGN.md](../DESIGN.md).
**U575 PB15 is LQFP64 pad36; pad63 is VSS.**

| Mainboard function | GPIO | Pad |
|---|---|---:|
| Keyboard interrupt / reset / boot | PA0 / PC10 / PC11 | 14 / 51 / 52 |
| Charger INT / PG / CE | PC2 / PC4 / PC5 | 10 / 24 / 25 |
| TCPP VCC-enable / DB / fault | PC6 / PC7 / PC3 | 37 / 38 / 11 |
| Gauge alert | PC13 | 2 |
| VBUS ADC / battery ADC | PC0 / PC1 | 8 / 9 |
| Display IRQ / reset / boot | PB12 / PC8 / PC9 | 33 / 39 / 40 |

| Keyboard function | G031 GPIOs, in signal order | UFQFPN32 pads |
|---|---|---|
| ROW1–5 | PA8, PA9, PA10, PA11, PA12 | 18, 19, 21, 22, 23 |
| COL1–10 | PA0, PA1, PB2, PB3, PA4, PA5, PA6, PA7, PC14, PC15 | 7, 8, 17, 27, 11, 12, 13, 14, 2, 3 |
| F / G / CARRY / OVERFLOW / LOWBAT | PB0, PB1, PB4, PB8, PB9 | 15, 16, 28, 32, 1 |
| USART2 TX / RX (AF1) | PA2 / PA3 | 9 / 10 |
| I2C1 SCL / SDA (AF6) | PB6 / PB7 | 30 / 31 |
| IRQ to main | PC6 | 20 |
| RGB TIM2_CH1 (AF2) / enable | PA15 / PB5 | 26 / 29 |
| SWDIO / SWCLK+BOOT0 / reset | PA13 / PA14 / PF2 | 24 / 25 / 6 |

The project-local `calcumaker:STM32G031K8U6` corrects the shared KiCad family
symbol's NC/PA9 and NC/PA10 pins: this package has dedicated PA9/pad19 and
PA10/pad21. Do not remap them onto PA11/PA12. PC14/15 are column inputs, avoiding
their output-current limitations. Column EXTI indices are all distinct.

Rows are active-low open-drain (unselected rows high impedance); column inputs
use pull-ups. Current flows **column → diode anode → cathode → switch → row**.
All rows can be held low for column EXTI wake. Annunciators are active-high through
470 Ω resistors. KB_BOOT0 reaches shared PA14/SWCLK through 1 kΩ R11, with a
10 kΩ local pull-down. Main firmware must tri-state KB_BOOT0 during SWD; hardware
BOOT0 requires the G0's `nBOOT_SEL=0` option setting.

## Power corrections and bring-up requirements

- U575RGT6 uses the LDO supply arrangement. VCAP/pad30 has C22+C23 (2×2.2 µF,
  ≥10 V, combined ESR <20 mΩ at 3 MHz), isolated from 3V3. Check effective
  capacitance and ESR for the ordered parts. VBAT is on regulated 3V3, not the
  raw cell. Main R13/R14 provide the shared I2C pull-ups.
- BQ25601 VAC/pad1 and VBUS/pad24 both use protected VBUS. PSEL has a 100 kΩ
  pull-up to REGN, staying below its 7 V absolute maximum even at 12 V USB-PD.
  CE has a 100 kΩ pull-up, so **charging stays disabled until firmware configures
  the selected cell and enables it**. INT and PG have 10 kΩ pull-ups.
- J8 accepts the pack's **10 kΩ 103AT-2 NTC** to ground, in parallel with R3
  (30.1 kΩ), with R2 (5.23 kΩ) to REGN. Without the sensor, TS inhibits charge;
  this is not a fixed-temperature bypass. J9 momentarily grounds QON to exit
  ship mode, using the charger's internal pull-up.
- Program the charger's **14 V OVP setting before requesting a 12 V PD contract**,
  set safe cell charge current and input-current limit, and service or disable
  the watchdog deliberately. Follow TCPP's VCC → UCPD Rd → DB release sequence.
- USBLC6 pin5 is tied to 3V3 for the USB data clamp. It must never see the 12 V
  VBUS rail. The TCPP/FET circuit handles VBUS and CC protection.
- TPS63900 SEL is low; CFG3=16.2 kΩ selects 3.3 V. CFG1=36.5 kΩ and CFG2=GND
  configure the alternate 3.3 V setting and unlimited input-current mode.
  Select L1 with ≥2 A saturation rating, and check capacitor bias derating.
- BAT_SENSE uses a 1 MΩ / 1 MΩ divider and 100 nF reservoir/filter. Allow adequate
  ADC settling time; do not treat this as a low-impedance ADC source.

## Still required before fabrication

This completes schematic connectivity, not board layout or hardware qualification.
The existing VSYS-fed SK6812MINI-E chain remains subject to its minimum supply
voltage and the 74LVC1G125's guaranteed input threshold over the full cell/charger
range. Resolve that power/logic-level choice with measurements or a revised rail
and buffer before fabrication. Hold LED_RAW low before shutting VLED off.

Also select the actual cell/NTC and charge limits; verify inductor current ratings,
capacitor effective values, connector current limits/FFC contact orientation,
stack height, reverse-mount RGB pin-to-footprint mapping, and placement of each
LED's bypass. Confirm clock load caps against the purchased crystal. Route and
review the charger/buck/USB loops and thermal pads. These boards have no finished
PCB layout in this change. Display/matrix board designs and firmware were not
changed; firmware must adopt the pin map and sequencing above.

## Primary references used

- [ST STM32U575 datasheet, DS13737](https://www.st.com/resource/en/datasheet/stm32u575rg.pdf): LQFP64 pinout and LDO/VCAP supply scheme.
- [ST STM32G031 datasheet, DS12992](https://www.st.com/resource/en/datasheet/stm32g031k8.pdf): UFQFPN32 pinout, alternate functions and PC14/PC15 constraints.
- [TI BQ25601 datasheet](https://www.ti.com/lit/ds/symlink/bq25601.pdf): PSEL/VAC/TS, charger controls, QON, OVP and watchdog.
- [TI TPS63900 datasheet](https://www.ti.com/lit/ds/symlink/tps63900.pdf): configuration resistor tables and power-stage requirements.
- [ST TCPP01-M12 datasheet](https://www.st.com/resource/en/datasheet/tcpp01-m12.pdf): protected-port pinout and GPIO power/DB sequencing.
- [ST USBLC6-2 datasheet](https://www.st.com/resource/en/datasheet/usblc6-2.pdf): data-line protection and VBUS clamp limits.
- [TI SN74LVC1G125 datasheet](https://www.ti.com/lit/ds/symlink/sn74lvc1g125.pdf): voltage-dependent input thresholds.
