#!/bin/sh
set -eu
HATOBA_OHOS_CXX=1 exec "$(dirname "$0")/clang.sh" "$@"
