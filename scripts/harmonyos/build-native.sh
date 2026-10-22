#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
if [ -z "${HATOBA_OHOS_NDK:-}" ]; then
  HATOBA_OHOS_NDK="/Applications/DevEco-Studio.app/Contents/sdk/default/openharmony/native"
fi
export HATOBA_OHOS_NDK
test -x "$HATOBA_OHOS_NDK/llvm/bin/clang" || { echo "OHOS native SDK not found" >&2; exit 1; }
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_OHOS_LINKER="$root/scripts/harmonyos/clang.sh"
export CC_aarch64_unknown_linux_ohos="$root/scripts/harmonyos/clang.sh"
export CXX_aarch64_unknown_linux_ohos="$root/scripts/harmonyos/clangxx.sh"
export AR_aarch64_unknown_linux_ohos="$HATOBA_OHOS_NDK/llvm/bin/llvm-ar"
cd "$root"
exec cargo build -p hatoba-ohos --lib --target aarch64-unknown-linux-ohos "$@"
