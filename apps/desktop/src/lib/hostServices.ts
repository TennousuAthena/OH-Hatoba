import { isHarmonyOS } from "@/ipc/api";

export interface HostCapabilities {
  localTerminal: boolean;
  sshAgent: boolean;
  stdioMcp: boolean;
  desktopCli: boolean;
  tray: boolean;
  updater: boolean;
  folderPicker: boolean;
}
const desktop: HostCapabilities = { localTerminal: true, sshAgent: true, stdioMcp: true, desktopCli: true, tray: true, updater: true, folderPicker: true };
const restricted: HostCapabilities = { localTerminal: false, sshAgent: false, stdioMcp: false, desktopCli: false, tray: false, updater: false, folderPicker: false };
let capabilities: HostCapabilities | undefined;
let surfaceObserver: MutationObserver | undefined;
let surfaceChanges = Promise.resolve();
let requestedBackground: string | undefined;

/** Keep native safe-area backgrounds in step with the shared app palette. */
function observeWindowSurface(): void {
  if (surfaceObserver) return;
  const root = document.documentElement;
  const update = () => {
    // The production CSS minifier abbreviates colors such as #ffffff to #fff.
    const background = getComputedStyle(root).getPropertyValue("--win").trim().toUpperCase()
      .replace(/^#([0-9A-F])([0-9A-F])([0-9A-F])$/, "#$1$1$2$2$3$3");
    if (!/^#[0-9A-F]{6}$/.test(background) || background === requestedBackground) return;
    requestedBackground = background;
    surfaceChanges = surfaceChanges.then(async () => {
      if (background !== requestedBackground) return;
      const applied = await hostService<string>("window_surface", { background });
      if (applied !== background) throw new Error("Native window background mismatch");
      if (background === requestedBackground) console.info("OH-Hatoba window surface synchronized");
    }).catch(() => { console.warn("OH-Hatoba native window background update failed"); });
  };
  surfaceObserver = new MutationObserver(update);
  surfaceObserver.observe(root, { attributes: true, attributeFilter: ["data-theme"] });
  update();
}

export function hostCapabilities(): Readonly<HostCapabilities> { return capabilities ?? (isHarmonyOS() ? restricted : desktop); }
export async function hostService<T>(method: string, args: Record<string, unknown> = {}): Promise<T> {
  if (!isHarmonyOS()) throw new Error("HarmonyOS host service requested on another host");
  const { harmonyBridge } = await import("@/ipc/harmonyos/bridge");
  return await (await harmonyBridge()).invoke("host_service", { method, ...args }) as T;
}
export async function initializeHostServices(): Promise<void> {
  if (!isHarmonyOS()) return;
  const result = await hostService<HostCapabilities>("capabilities");
  if (!result || Object.keys(desktop).some((key) => typeof result[key as keyof HostCapabilities] !== "boolean")) throw new Error("Invalid native capabilities");
  capabilities = Object.freeze({ ...result });
  observeWindowSurface();
}
