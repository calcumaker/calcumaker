#!/usr/bin/env bash
# Flash the Calcumaker app over USB-C via the STM32 ROM USB-DFU bootloader, with
# NO ST-Link. Single flash slot (app owns 0x08000000+), no A/B.
#
# Flow: (1) if the app is running, send its `dfu` REPL command to enter the ROM
# DFU bootloader (it programs nSWBOOT0=0/nBOOT0=0 + OBL_LAUNCH); (2) dfu-util
# writes the new firmware to internal flash (alt 0); (3) dfu-util REWRITES the
# boot option bytes back to boot main flash (alt 1) — this is the "clear the
# flags in the DFU program phase" step — which resets straight into the new app.
#
# Usage: dfu-flash.sh [firmware.elf|firmware.bin]
#   OPTR_APP=0x........  override the "boot app" option-byte word (default: the
#                        U575 factory default nSWBOOT0=1/nBOOT0=1).
# dfu-util needs USB access — run with sudo or install a udev rule for 0483:df11.
set -euo pipefail

BIN="${1:?usage: dfu-flash.sh <firmware.elf|.bin>}"
OPTR_APP="${OPTR_APP:-0x1feff8aa}"          # nSWBOOT0=1,nBOOT0=1 -> boot main flash
APP_ID="1209:c160"                          # the running Calcumaker app
DFU_ID="0483:df11"                          # STM32 system ROM DFU
DFU=${DFU_UTIL:-dfu-util}
[ "$(id -u)" -ne 0 ] && command -v sudo >/dev/null && DFU="sudo $DFU"

# ELF -> raw binary if needed.
if file -b "$BIN" | grep -q ELF; then
    arm-none-eabi-objcopy -O binary "$BIN" /tmp/calcumaker-app.bin
    BIN=/tmp/calcumaker-app.bin
fi
echo "firmware: $BIN ($(stat -c%s "$BIN") bytes)"

# Option-byte restore image (little-endian OPTR word written to alt 1).
python3 -c "import struct;open('/tmp/calcumaker-optr.bin','wb').write(struct.pack('<I',int('$OPTR_APP',16)))"

# 1) Enter DFU: if the app is enumerated, drive its `dfu` REPL command.
if lsusb | grep -qi "$APP_ID"; then
    PORT=$(ls /dev/serial/by-id/*Calcumaker*if00 2>/dev/null | head -1 || true)
    if [ -n "$PORT" ]; then
        echo "entering DFU via the app REPL ($PORT)..."
        printf 'dfu\r' > "$PORT" || true
    fi
fi

echo "waiting for ROM DFU ($DFU_ID)..."
for _ in $(seq 1 30); do lsusb | grep -qi "$DFU_ID" && break; sleep 1; done
lsusb | grep -qi "$DFU_ID" || { echo "DFU device not found — enter DFU (REPL 'dfu' or BOOT0 high + reset) and retry"; exit 1; }

# 2) Flash the app.
echo "flashing app -> 0x08000000 (alt 0)"
$DFU -a 0 -d "$DFU_ID" -s 0x08000000 -D "$BIN"

# 3) Restore boot option bytes + leave (resets into the app). The device resets
# mid-transaction as the option bytes reload, so dfu-util's final get_status
# error here is expected/harmless.
echo "restoring boot option bytes (OPTR=$OPTR_APP) + reset to app (alt 1)"
$DFU -a 1 -d "$DFU_ID" -s 0x40022040:leave -D /tmp/calcumaker-optr.bin || true
echo "done — app should re-enumerate."
