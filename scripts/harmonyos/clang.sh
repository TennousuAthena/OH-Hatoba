#!/bin/sh
set -eu
: "${HATOBA_OHOS_NDK:?Set HATOBA_OHOS_NDK to the SDK native directory}"
case "${HATOBA_OHOS_CXX:-0}" in
  1) compiler=clang++ ;;
  *) compiler=clang ;;
esac
exec "$HATOBA_OHOS_NDK/llvm/bin/$compiler" --target=aarch64-linux-ohos \
  --sysroot="$HATOBA_OHOS_NDK/sysroot" -D__MUSL__ "$@"
