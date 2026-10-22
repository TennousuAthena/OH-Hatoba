import { hvigor } from '@ohos/hvigor';
import { appTasks, OhosAppContext, OhosPluginId } from '@ohos/hvigor-ohos-plugin';
import fs from 'node:fs';
import path from 'node:path';

// Only local signing material is merged, in memory. SDK/device policy stays in tracked config.
hvigor.nodesEvaluated(() => {
  const localPath = path.join(__dirname, 'build-profile.local.json5');
  if (!fs.existsSync(localPath)) return;
  const context = hvigor.getRootNode().getContext(OhosPluginId.OHOS_APP_PLUGIN) as OhosAppContext;
  const profile = context.getBuildProfileOpt();
  const local = JSON.parse(fs.readFileSync(localPath, 'utf8')) as {
    app: Pick<typeof profile.app, 'signingConfigs' | 'products'>;
  };
  if (!local.app.signingConfigs?.length) throw new Error('Local signing configuration is empty');
  profile.app.signingConfigs = local.app.signingConfigs;
  for (const product of profile.app.products ?? []) {
    const signing = local.app.products?.find((entry) => entry.name === product.name)?.signingConfig;
    if (signing) product.signingConfig = signing;
  }
  context.setBuildProfileOpt(profile);
});
export default { system: appTasks, plugins: [] };
