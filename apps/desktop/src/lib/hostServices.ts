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
}
