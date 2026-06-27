#!/usr/bin/env bash
# Cross-build static GMP + MPFR for the Calcumaker 16 MCU (STM32U575, Cortex-M33,
# hard-float) so the no_std engine (gmp-mpfr-nostd / calcumaker-core) can link
# against them on `thumbv8m.main-none-eabihf`.
#
# Outputs (gitignored): firmware/vendor/gmp-mpfr-arm/lib/{libgmp.a,libmpfr.a}
#                       firmware/vendor/gmp-mpfr-arm/include/{gmp.h,mpfr.h,...}
#
# Then build the firmware with:
#   GMP_MPFR_LIBDIR=firmware/vendor/gmp-mpfr-arm cargo build -p calcumaker-fw \
#       --target thumbv8m.main-none-eabihf
# (calcumaker-fw/build.rs links them when GMP_MPFR_LIBDIR is set.)
#
# Requires: arm-none-eabi-gcc/ar/ranlib, make, curl, tar (xz).
set -euo pipefail

GMP_VER="${GMP_VER:-6.3.0}"
MPFR_VER="${MPFR_VER:-4.2.1}"
TARGET="${TARGET:-arm-none-eabi}"

FW_DIR="$(cd "$(dirname "$0")/.." && pwd)"          # .../firmware
OUT="${OUT:-$FW_DIR/vendor/gmp-mpfr-arm}"           # install prefix (gitignored)
WORK="${WORK:-$FW_DIR/vendor/_build}"               # download/build scratch

# Cortex-M33 + FPU, hard-float ABI — MUST match thumbv8m.main-none-eabihf so the
# archives link with the Rust code + newlib.
ARCH_FLAGS="-mcpu=cortex-m33 -mthumb -mfloat-abi=hard -mfpu=fpv5-sp-d16"

export CC="$TARGET-gcc"
export AR="$TARGET-ar"
export RANLIB="$TARGET-ranlib"
# --specs=nosys.specs lets configure's link tests resolve syscalls (stubs);
# function/data-sections keep the final firmware small via --gc-sections.
# -std=gnu17: GCC 15 defaults to C23, where `void g(){}` means "no args" and
# breaks GMP 6.3.0's old-style configure probes (and some .c) — pin pre-C23.
export CFLAGS="$ARCH_FLAGS -O2 -ffunction-sections -fdata-sections -std=gnu17 --specs=nosys.specs"
JOBS="$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"

echo "==> toolchain: $($CC --version | head -1)"
echo "==> output:    $OUT"
mkdir -p "$OUT" "$WORK"
cd "$WORK"

fetch() { # url file
  [ -f "$2" ] || curl -fSL --retry 3 -o "$2" "$1"
}
fetch "https://ftp.gnu.org/gnu/gmp/gmp-$GMP_VER.tar.xz"   "gmp-$GMP_VER.tar.xz"
fetch "https://ftp.gnu.org/gnu/mpfr/mpfr-$MPFR_VER.tar.xz" "mpfr-$MPFR_VER.tar.xz"

# ---- GMP -------------------------------------------------------------------
echo "==> building GMP $GMP_VER"
rm -rf "gmp-$GMP_VER"
tar xf "gmp-$GMP_VER.tar.xz"
( cd "gmp-$GMP_VER"
  ./configure --host="$TARGET" --prefix="$OUT" \
      --disable-shared --enable-static --disable-assembly
  make -j"$JOBS"
  make install )

# ---- MPFR (against the cross GMP) ------------------------------------------
echo "==> building MPFR $MPFR_VER"
rm -rf "mpfr-$MPFR_VER"
tar xf "mpfr-$MPFR_VER.tar.xz"
( cd "mpfr-$MPFR_VER"
  ./configure --host="$TARGET" --prefix="$OUT" \
      --disable-shared --enable-static --with-gmp="$OUT"
  make -j"$JOBS"
  make install )

echo ""
echo "==> done:"
ls -la "$OUT/lib/"libgmp.a "$OUT/lib/"libmpfr.a
echo "==> symbol spot-check (should be > 0 each):"
echo "    __gmpz_init : $("$TARGET-nm" "$OUT/lib/libgmp.a"  2>/dev/null | grep -c 'T __gmpz_init')"
echo "    mpfr_init2  : $("$TARGET-nm" "$OUT/lib/libmpfr.a" 2>/dev/null | grep -c 'T mpfr_init2')"
echo "    mpfr_sqrt   : $("$TARGET-nm" "$OUT/lib/libmpfr.a" 2>/dev/null | grep -c 'T mpfr_sqrt')"
