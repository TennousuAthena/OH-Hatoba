import { hvigor } from '@ohos/hvigor';
import { appTasks, OhosAppContext, OhosPluginId } from '@ohos/hvigor-ohos-plugin';
import fs from 'node:fs';
import path from 'node:path';

// Launcher branding follows the upstream desktop assets, including builds started from the IDE.
hvigor.getRootNode().afterNodeEvaluate(() => {
  const root = hvigor.getRootNode().getNodePath();
  const desktop = path.resolve(root, '../../apps/desktop/src-tauri');
  const upstream = JSON.parse(fs.readFileSync(path.join(desktop, 'tauri.conf.json'), 'utf8')) as { productName: string };
  if (!upstream.productName?.trim()) throw new Error('Upstream application name is missing');
  const stringsPath = path.join(root, 'AppScope/resources/base/element/string.json');
  const strings = JSON.parse(fs.readFileSync(stringsPath, 'utf8')) as { string: { name: string; value: string }[] };
  const label = strings.string.find((entry) => entry.name === 'app_name');
  if (!label) throw new Error('Application label resource is missing');
  if (label.value !== upstream.productName) {
    label.value = upstream.productName;
    fs.writeFileSync(stringsPath, JSON.stringify(strings, null, 2) + '\n');
  }
  const icon = fs.readFileSync(path.join(desktop, 'icons/icon.png'));
  const iconPath = path.join(root, 'AppScope/resources/base/media/app_icon.png');
  if (!fs.existsSync(iconPath) || !fs.readFileSync(iconPath).equals(icon)) fs.writeFileSync(iconPath, icon);
});

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
