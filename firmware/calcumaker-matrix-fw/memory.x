/* RP2040 memory map — boots from the on-board W25Q32 (4 MB) QSPI flash via XIP.
   BOOT2 (first 256 B) is the second-stage bootloader (embassy-rp supplies it). */
MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    FLASH : ORIGIN = 0x10000100, LENGTH = 4096K - 0x100
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K
}
