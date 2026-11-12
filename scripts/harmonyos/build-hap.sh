#!/bin/sh
set -eu
build_mode=debug
unsigned=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --release) build_mode=release; shift ;;
    --unsigned) unsigned=1; shift ;;
    --help|-h)
      echo "Usage: $0 [--release] [--unsigned] [-- hvigor-options...]"
      echo "Default: debug build, using local signing when configured."
      exit 0 ;;
    --) shift; break ;;
    *) break ;;
  esac
done
export HATOBA_BUILD_MODE="$build_mode"
export HATOBA_UNSIGNED="$unsigned"
root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
: "${HATOBA_DEVECO_HOME:=/Applications/DevEco-Studio.app/Contents}"
export DEVECO_SDK_HOME="${DEVECO_SDK_HOME:-$HATOBA_DEVECO_HOME/sdk}"
export HATOBA_OHOS_NDK="${HATOBA_OHOS_NDK:-$DEVECO_SDK_HOME/default/openharmony/native}"
cd "$root"
node scripts/harmonyos/contracts.mjs --check
node scripts/harmonyos/check-boundaries.mjs
# Build the same offline deployment package embedded by the desktop shell.
pnpm worker:bundle
if [ "$build_mode" = release ]; then
  scripts/harmonyos/build-native.sh --locked --release
else
  scripts/harmonyos/build-native.sh --locked
fi
pnpm --filter @hatoba/desktop exec vite build --mode harmonyos --config vite.harmonyos.config.ts
cd platform/harmonyos
if [ ! -f local.properties ]; then
  printf 'sdk.dir=%s/sdk\n' "$HATOBA_DEVECO_HOME" > local.properties
fi
# The SDK tools use their bundled Node runtime. A newer host Node can remove APIs that
# this DevEco version still uses; keep the frontend's Node selection unchanged above.
deveco_node_home="$HATOBA_DEVECO_HOME/tools/node"
if [ ! -x "$deveco_node_home/bin/node" ]; then
  echo "DevEco bundled Node runtime not found: $deveco_node_home/bin/node" >&2
  exit 1
fi
run_deveco() {
  NODE_HOME="$deveco_node_home" PATH="$deveco_node_home/bin:$PATH" "$@"
}
run_deveco "$HATOBA_DEVECO_HOME/tools/ohpm/bin/ohpm" install
# Both artifacts must come from this build, especially when switching signing or build modes.
unsigned_hap=entry/build/default/outputs/default/entry-default-unsigned.hap
signed_hap=entry/build/default/outputs/default/entry-default-signed.hap
rm -f "$unsigned_hap" "$signed_hap"
run_deveco "$HATOBA_DEVECO_HOME/tools/hvigor/bin/hvigorw" --mode module \
  -p product=default -p module=entry@default -p buildMode="$build_mode" assembleHap --no-daemon "$@"
set -- "$root/scripts/harmonyos/check-hap.py" "$unsigned_hap" --require-unsigned
if [ "$build_mode" = release ]; then set -- "$@" --require-release; fi
python3 "$@"
if [ "$unsigned" = 1 ]; then
  if [ -f "$signed_hap" ]; then
    echo "Unsigned build unexpectedly produced a signed HAP" >&2
    exit 1
  fi
elif [ -f build-profile.local.json5 ]; then
  set -- "$root/scripts/harmonyos/check-hap.py" "$signed_hap"
  if [ "$build_mode" = release ]; then set -- "$@" --require-release; fi
  python3 "$@"
fi
