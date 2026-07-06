/* calcumaker-fw linker memory map — PLACEHOLDER.
 *
 * Values match the selected STM32U575RGT6 (1 MB flash / 768 KB SRAM, LQFP-64).
 * NOTE: the "G" flash-size code = 1 MB (the "I" parts are 2 MB) — the firmware
 * links at ~323 KB, so 1 MB is ample. Notes:
 *   - STM32U5 SRAM is banked: SRAM1 (192K) + SRAM2 (64K) + SRAM3 (512K) are
 *     CONTIGUOUS from 0x20000000 = 768K usable as one `RAM` region; SRAM4 (16K)
 *     lives in the backup domain (0x28000000) and is omitted here.
 *   - The external 4 MB quad-SPI NOR (U7, on OCTOSPI1) is memory-mapped at
 *     0x90000000 when enabled — a separate region, not part of this internal map.
 *   - If using `embassy-stm32` with the `memory-x` feature, that crate supplies
 *     memory.x for you and this file can be deleted.
 */
MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 1024K   /* STM32U575RG: 1 MB */
  RAM   : ORIGIN = 0x20000000, LENGTH = 768K    /* SRAM1+SRAM2+SRAM3 (contiguous); SRAM4 16K separate */
}

/* cortex-m-rt places the stack at the end of RAM by default. The bignum heap
 * (embedded-alloc) is carved out of a static buffer in main.rs, not here. */
