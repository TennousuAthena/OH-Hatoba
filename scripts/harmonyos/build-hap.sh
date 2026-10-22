#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)
: "${HATOBA_DEVECO_HOME:=/Applications/DevEco-Studio.app/Contents}"
export DEVECO_SDK_HOME="${DEVECO_SDK_HOME:-$HATOBA_DEVECO_HOME/sdk}"
export HATOBA_OHOS_NDK="${HATOBA_OHOS_NDK:-$DEVECO_SDK_HOME/default/openharmony/native}"
cd "$root"
node scripts/harmonyos/contracts.mjs --check
node scripts/harmonyos/check-boundaries.mjs
scripts/harmonyos/build-native.sh --locked
pnpm --filter @hatoba/desktop exec vite build --mode harmonyos --config vite.harmonyos.config.ts
cd platform/harmonyos
if [ ! -f local.properties ]; then
  printf 'sdk.dir=%s/sdk\n' "$HATOBA_DEVECO_HOME" > local.properties
fi
"$HATOBA_DEVECO_HOME/tools/ohpm/bin/ohpm" install
# Do not mistake a stale signed artifact for the output of this build.
rm -f entry/build/default/outputs/default/entry-default-signed.hap
"$HATOBA_DEVECO_HOME/tools/hvigor/bin/hvigorw" --mode module \
  -p product=default -p module=entry@default -p buildMode=debug assembleHap --no-daemon "$@"
python3 "$root/scripts/harmonyos/check-hap.py"
if [ -f build-profile.local.json5 ]; then
  python3 "$root/scripts/harmonyos/check-hap.py" entry/build/default/outputs/default/entry-default-signed.hap
fi
