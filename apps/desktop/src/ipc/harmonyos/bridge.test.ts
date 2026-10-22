import { afterEach, describe, expect, it, vi } from "vitest";
import { createHarmonyApi } from "./api";
import { HostBridge } from "./bridge";

class Port {
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onmessageerror: (() => void) | null = null;
  messages: Record<string, unknown>[] = [];
  start() {}
  close() {}
  postMessage(value: string) { this.messages.push(JSON.parse(value) as Record<string, unknown>); }
  receive(value: unknown) { this.onmessage?.({ data: typeof value === "object" && !(value instanceof ArrayBuffer) ? JSON.stringify(value) : value }); }
  reply(result: unknown, id = this.messages.at(-1)?.id) { this.receive({ v: 1, type: "reply", id, ok: true, result }); }
}

afterEach(() => vi.useRealTimers());

describe("HarmonyOS transport", () => {
  it("uses upstream's parameter mapping and passes real results and failures", async () => {
    const port = new Port();
    const bridge = new HostBridge(port as unknown as MessagePort);
    const api = await createHarmonyApi(bridge);
    const result = api.host_set_favorite("h1", true);
    expect(port.messages[0]).toMatchObject({ command: "host_set_favorite", args: { id: "h1", favorite: true } });
    port.reply(null);
    await expect(result).resolves.toBeUndefined();
    const missing = api.host_get("gone");
    port.receive({ v: 1, type: "reply", id: port.messages.at(-1)?.id, ok: false, error: { code: "not_found", detail: "host" } });
    await expect(missing).rejects.toMatchObject({ code: "not_found" });
    bridge.close();
  });

  it("delivers early session frames before connect resolves and releases failed channels", async () => {
    const port = new Port();
    const bridge = new HostBridge(port as unknown as MessagePort);
    const api = await createHarmonyApi(bridge);
    const frames: Uint8Array[] = [];
    const result = api.ssh_connect("h1", { cols: 80, rows: 24 } as never, (frame) => frames.push(frame));
    const call = port.messages[0];
    const id = ((call.args as Record<string, unknown>).channel as { channel: number }).channel;
    const frame = new Uint8Array(7);
    new DataView(frame.buffer).setUint32(0, id, true);
    frame.set([3, 0, 255], 4);
    port.receive(frame.buffer);
    expect(frames).toEqual([new Uint8Array([3, 0, 255])]);
    await vi.waitFor(() => expect(port.messages.at(-1)?.type).toBe("ack"));
    expect(port.messages.at(-1)).toMatchObject({ type: "ack", channel: id, bytes: 3 });
    port.receive({ v: 1, type: "reply", id: call.id, ok: false, error: { code: "ssh", detail: "rejected" } });
    await expect(result).rejects.toMatchObject({ code: "ssh" });
    port.receive(frame.buffer);
    expect(frames).toHaveLength(1);
    bridge.close();
  });

  it("withholds terminal credits until parsing ends and preserves ACK order", async () => {
    const port = new Port(); const bridge = new HostBridge(port as unknown as MessagePort);
    const consumed: (() => void)[] = [];
    const id = bridge.channel(() => new Promise<void>((resolve) => consumed.push(resolve)));
    for (const length of [2, 3]) {
      const frame = new Uint8Array(length + 4); new DataView(frame.buffer).setUint32(0, id, true); port.receive(frame.buffer);
    }
    consumed[1](); await Promise.resolve(); await Promise.resolve();
    expect(port.messages).toEqual([]);
    consumed[0]();
    await vi.waitFor(() => expect(port.messages).toHaveLength(2));
    expect(port.messages.map((m) => m.bytes)).toEqual([2, 3]); bridge.close();
  });

  it("does not confuse timed-out replies with later calls", async () => {
    vi.useFakeTimers();
    const port = new Port();
    const bridge = new HostBridge(port as unknown as MessagePort, 10);
    const first = bridge.invoke("app_info");
    const rejected = expect(first).rejects.toMatchObject({ detail: "HarmonyOS request timed out" });
    await vi.advanceTimersByTimeAsync(11);
    await rejected;
    const second = bridge.invoke("app_info");
    port.reply("stale", 1);
    port.reply("current", 2);
    await expect(second).resolves.toBe("current");
    bridge.close();
  });

  it("unsubscribes events and rejects all requests when the host disconnects", async () => {
    const port = new Port();
    const bridge = new HostBridge(port as unknown as MessagePort);
    const api = await createHarmonyApi(bridge);
    const receive = vi.fn();
    const unlisten = await api.listen("vault://locked", receive);
    port.receive({ v: 1, type: "event", event: "vault://locked", payload: { reason: "manual" } });
    unlisten();
    port.receive({ v: 1, type: "event", event: "vault://locked", payload: {} });
    expect(receive).toHaveBeenCalledTimes(1);
    const pending = api.app_info();
    bridge.close();
    await expect(pending).rejects.toMatchObject({ code: "internal" });
    await expect(api.app_info()).rejects.toMatchObject({ detail: "HarmonyOS host disconnected" });
  });

  it("fails pending calls on protocol mismatch instead of accepting stale host messages", async () => {
    const port = new Port();
    const bridge = new HostBridge(port as unknown as MessagePort);
    const pending = bridge.invoke("app_info");
    port.receive({ v: 2, type: "reply", id: 1, ok: true, result: {} });
    await expect(pending).rejects.toMatchObject({ detail: "HarmonyOS protocol mismatch" });
  });

  it("keeps the request queue bounded and rejects work after malformed binary input", async () => {
    const port = new Port();
    const bridge = new HostBridge(port as unknown as MessagePort);
    const pending = Array.from({ length: 64 }, () => bridge.invoke("app_info").catch((error: unknown) => error));
    await expect(bridge.invoke("app_info")).rejects.toMatchObject({ detail: "HarmonyOS request queue is full" });
    port.receive(new Uint8Array([0, 1]).buffer);
    expect(await Promise.all(pending)).toHaveLength(64);
    await expect(bridge.invoke("app_info")).rejects.toMatchObject({ detail: "HarmonyOS host disconnected" });
  });

  it("settles a timeout even when posting cancellation fails", async () => {
    vi.useFakeTimers();
    const port = new Port();
    const bridge = new HostBridge(port as unknown as MessagePort, 10);
    const pending = bridge.invoke("app_info");
    vi.spyOn(port, "postMessage").mockImplementation(() => { throw new Error("port closed"); });
    const rejected = expect(pending).rejects.toMatchObject({ detail: "HarmonyOS request timed out" });
    await vi.advanceTimersByTimeAsync(11);
    await rejected;
  });
});
