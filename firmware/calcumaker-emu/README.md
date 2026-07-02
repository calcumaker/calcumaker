# calcumaker-emu

The **Calcumaker 16 emulator** — the real calculator on a host terminal.

This is not a look-alike: it hosts the same `calcumaker_core::App` the firmware
runs. Host keys map to the physical **50-key matrix**, presses resolve through
the **f/g shift layers**, and the display is drawn from the **same TM1640
segment bytes** the hardware receives, rendered as ASCII 7-segment art. If it
works here, the only difference on the device is GPIO.

```
+------------------------------------------------------------------+
|                  _       _   _   _   _   _   _   _   _   _   _   |
|   | |_|   | |_|  _|   |  _| |_  |_   _|  _|   |  _| | | |_|   |  |
|   |.  |   |   | |_    |  _|  _| |_| |_   _|   |  _| |_|  _|  _|  |
+------------------------------------------------------------------+
 DEC  prec 256  word unbounded
 X: 1.4142135623730950488016887242096980785696718753769480731766797379907324784621
```

## Run it

```sh
brew install gmp mpfr    # one-time host deps (apt: libgmp-dev libmpfr-dev)
cargo run                # interactive; ? shows the key map, Ctrl-C quits
```

Keys mirror the device layout (f = gold shift, g = blue shift):

| Host           | Calculator                                             |
| -------------- | ------------------------------------------------------ |
| `0-9` `a-f` `.` | digits (hex digits radix-permitting), decimal point   |
| `+ - * /`      | arithmetic                                             |
| `Enter` `Bksp` `X` `n` `E` | ENTER, backspace, CLx, CHS, EEX            |
| `S C T L Q P I` | sin cos tan ln √x yˣ 1/x                              |
| `& \| ^ ~ < >` | AND OR XOR NOT SHL SHR                                 |
| `H D O B`      | HEX DEC OCT BIN                                        |
| `W` `x` `v` `m` `r` | wsize (X bits, 0=∞), swap, roll-down, STO, RCL    |
| `F` `G`        | f / g shift (then e.g. `F`,`E` = π; `F`,`I` = prec)    |

## Scripted mode (tests, demos, CI)

`--press` feeds a key string and prints the final frame — `;` (or `\n`) is
ENTER:

```sh
cargo run -- --press "2;3+"          # 2 ENTER 3 +      -> 5
cargo run -- --press "2Q"            # sqrt(2) to 77 digits at prec 256
cargo run -- --press "8W;H0f~"       # 8 wsize, hex, NOT 0F -> F0
cargo run -- --prec 1024 --press "FE"  # pi at 1024 bits
```

## Relationship to the firmware

`calcumaker-core` owns everything you see here: the keymap + shift layers
(`keys`), entry editing + dispatch (`App`), the engine (`Calc`, GMP + MPFR),
and the 7-seg encoding (`seg7`). The firmware (`../calcumaker-fw`) contributes
only the matrix scan and the TM1640 bus; this crate contributes only the
terminal. One calculator, two I/O bindings.

## License

AGPL-3.0 (see repo root).
