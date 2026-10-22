import type { HatobaApi } from "../api";
import { createCommandApi, type ApiTransport } from "../tauri";
import { connectHost, type HostBridge } from "./bridge";
import { commandContract } from "./commands.generated";

/** Reuse upstream's complete command mapping; no parallel hand-written Harmony API table. */
export async function createHarmonyApi(host?: HostBridge): Promise<HatobaApi> {
  const bridge = host ?? await connectHost();

  class NativeChannel<T> {
    readonly id: number;
    onmessage: (payload: T) => void = () => {};
    constructor() {
      this.id = bridge.channel((payload) => this.onmessage(payload as T));
    }
  }

  const commands = new Proxy({}, {
    get(_target, method: string) {
      const contract = commandContract[method as keyof typeof commandContract];
      if (!contract) return undefined;
      return (...values: unknown[]) => {
        const args: Record<string, unknown> = {};
        const channels: number[] = [];
        const expectedChannels: readonly number[] = contract.channels;
        for (const [index, parameter] of contract.parameters.entries()) {
          const value = values[index];
          if (expectedChannels.includes(index) !== (value instanceof NativeChannel)) {
            throw { code: "internal", detail: "HarmonyOS channel contract mismatch" };
          }
          if (value instanceof NativeChannel) {
            channels.push(value.id);
            args[parameter] = { channel: value.id };
          } else args[parameter] = value;
        }
        return bridge.invoke(contract.command, args).catch((error: unknown) => {
          channels.forEach((id) => bridge.releaseChannel(id));
          throw error;
        });
      };
    },
  });

  // tauri-specta types name Tauri's nominal Channel and Event classes. This is the only
  // transport-boundary cast: createCommandApi uses just constructor/onmessage/payload.
  // Contract generation + the common adapter tests guard those assumptions on every upgrade.
  return createCommandApi({
    commands,
    Channel: NativeChannel,
    listen: async (event: string, handler: (event: { payload: unknown }) => void) =>
      bridge.listen(event as never, (payload) => handler({ payload })),
  } as unknown as ApiTransport);
}
