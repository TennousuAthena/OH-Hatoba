import { hvigor } from '@ohos/hvigor';
import { appTasks, OhosAppContext, OhosPluginId } from '@ohos/hvigor-ohos-plugin';
import fs from 'node:fs';
import path from 'node:path';

// Only local signing material is merged, in memory. SDK/device policy stays in tracked config.
// Apply before child modules construct their signing tasks and capture the product.
hvigor.getRootNode().afterNodeEvaluate(() => {
  // Hvigor bundles this script into its cache; __dirname is not the project directory.
  const localPath = path.join(hvigor.getRootNode().getNodePath(), 'build-profile.local.json5');
  if (!fs.existsSync(localPath)) return;
  const context = hvigor.getRootNode().getContext(OhosPluginId.OHOS_APP_PLUGIN) as OhosAppContext;
  const profile = context.getBuildProfileOpt();
  const local = JSON.parse(fs.readFileSync(localPath, 'utf8')) as {
    app: Pick<typeof profile.app, 'signingConfigs' | 'products'>;
  };
  if (!local.app.signingConfigs?.length) throw new Error('Local signing configuration is empty');
  profile.app.signingConfigs = local.app.signingConfigs;
  const currentProduct = context.getCurrentProduct().getProductName();
  for (const product of profile.app.products ?? []) {
    const signing = local.app.products?.find((entry) => entry.name === product.name)?.signingConfig;
    if (!signing && product.name !== currentProduct) continue;
    if (!signing || !local.app.signingConfigs.some((entry) => entry.name === signing)) {
      throw new Error('Assign local signing configuration to the current product before building');
    }
    product.signingConfig = signing;
  }
  context.setBuildProfileOpt(profile);
  console.log('OH-Hatoba: loaded local signing configuration');
});
export default { system: appTasks, plugins: [] };
