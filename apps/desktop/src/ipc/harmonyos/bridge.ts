import type { AppError, EventMap } from "../types";

export const HOST_PROTOCOL = 1;
export const HANDSHAKE = "oh-hatoba-host/v1";
const MAX_PENDING = 64;
const MAX_CHANNELS = 128;
const MAX_FRAME = 256 * 1024;
const MAX_JSON = 1024 * 1024;

const failure = (detail: string): AppError => ({ code: "internal", detail });

/** Native owns credentials and business state. This port carries only whitelisted commands and
 * existing DTOs. It is independent of Tauri globals and never falls back to the demo backend. */
export class HostBridge {
  private sequence = 0;
  private channelSequence = 0;
  private closed = false;
  private pending = new Map<number, { resolve: (value: unknown) => void; reject: (error: AppError) => void; timer: ReturnType<typeof setTimeout> }>();
  private channels = new Map<number, (payload: unknown) => unknown>();
  private acknowledgements = new Map<number, Promise<void>>();
  private listeners = new Map<string, Set<(payload: unknown) => void>>();

  constructor(private readonly port: MessagePort, private readonly timeoutMs = 60_000) {
    port.onmessage = (event) => {
      try { this.receive(event.data); }
      catch { this.close("Native message handler failed"); }
    };
    port.onmessageerror = () => this.close("Native message could not be decoded");
    port.start();
  }

  invoke(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
    if (this.closed) return Promise.reject(failure("HarmonyOS host disconnected"));
    if (this.pending.size >= MAX_PENDING) return Promise.reject(failure("HarmonyOS request queue is full"));
    const id = ++this.sequence;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        try { this.send({ v: HOST_PROTOCOL, type: "cancel", id }); }
        catch { this.close("HarmonyOS host disconnected"); }
        reject(failure("HarmonyOS request timed out"));
      }, this.timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      try {
        this.send({ v: HOST_PROTOCOL, type: "call", id, command, args });
      } catch {
        clearTimeout(timer);
        this.pending.delete(id);
        reject(failure("Could not send HarmonyOS request"));
      }
    });
  }

  channel(callback: (payload: unknown) => unknown): number {
    if (this.closed || this.channels.size >= MAX_CHANNELS) throw failure("HarmonyOS channel limit reached");
    const id = ++this.channelSequence;
    this.channels.set(id, callback);
    return id;
  }

  releaseChannel(id: number) {
    this.channels.delete(id);
    this.acknowledgements.delete(id);
  }

  listen<K extends keyof EventMap>(event: K, callback: (payload: EventMap[K]) => void): () => void {
    const callbacks = this.listeners.get(event) ?? new Set();
    const handler = callback as (payload: unknown) => void;
    callbacks.add(handler);
    this.listeners.set(event, callbacks);
    return () => {
      callbacks.delete(handler);
      if (!callbacks.size) this.listeners.delete(event);
    };
  }

  close(reason = "HarmonyOS page closed") {
    if (this.closed) return;
    this.closed = true;
    try { this.port.close(); } catch { /* Reject pending work even if the port already died. */ }
    for (const request of this.pending.values()) {
      clearTimeout(request.timer);
      request.reject(failure(reason));
    }
    this.pending.clear();
    this.channels.clear();
    this.acknowledgements.clear();
    this.listeners.clear();
  }

  private send(message: object) {
    const encoded = JSON.stringify(message);
    if (encoded.length > MAX_JSON) throw failure("HarmonyOS request exceeds message limit");
    this.port.postMessage(encoded);
  }

  private receive(data: unknown) {
    if (this.closed) return;
    if (data instanceof ArrayBuffer) {
      if (data.byteLength < 5 || data.byteLength > MAX_FRAME + 4) return this.close("Invalid native terminal frame");
      const id = new DataView(data).getUint32(0, true);
      const handler = this.channels.get(id);
      if (handler) {
        const consumed = handler(data.slice(4));
        // xterm resolves after parsing the bytes. A slow renderer must hold native credits.
        // Keep ACK order even if consumers resolve their promises out of order.
        const tail = Promise.all([this.acknowledgements.get(id), consumed]).then(() => {
          if (!this.closed && this.channels.has(id)) this.send({ v: HOST_PROTOCOL, type: "ack", channel: id, bytes: data.byteLength - 4 });
        }).catch(() => { this.close("Terminal consumer failed"); });
        this.acknowledgements.set(id, tail);
        void tail.then(() => { if (this.acknowledgements.get(id) === tail) this.acknowledgements.delete(id); });
      }
      return;
    }
    if (typeof data !== "string" || data.length > MAX_JSON) return this.close("Invalid native message");
    let message: Record<string, unknown>;
    try {
      const parsed: unknown = JSON.parse(data);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error();
      message = parsed as Record<string, unknown>;
    } catch {
      return this.close("Invalid native JSON");
    }
    if (message.v !== HOST_PROTOCOL) return this.close("HarmonyOS protocol mismatch");
    if (message.type === "reply" && typeof message.id === "number") {
      const request = this.pending.get(message.id);
      if (!request) return; // A late reply to a cancelled/timed-out call cannot complete a new call.
      this.pending.delete(message.id);
      clearTimeout(request.timer);
      if (message.ok === true) request.resolve(message.result);
      else {
        const error = message.error;
        request.reject(error && typeof error === "object" && "code" in error && "detail" in error ? error as AppError : failure("Native command failed"));
      }
    } else if (message.type === "event" && typeof message.event === "string") {
      this.listeners.get(message.event)?.forEach((handler) => handler(message.payload));
    } else if (message.type === "channel" && typeof message.channel === "number") {
      this.channels.get(message.channel)?.(message.payload);
    } else if (message.type === "channel_close" && typeof message.channel === "number") {
      this.releaseChannel(message.channel);
    }
  }
}

/** The native shell sends exactly one port after loading the packaged local page. */
export function connectHost(timeoutMs = 10_000): Promise<HostBridge> {
  return new Promise((resolve, reject) => {
    const receive = (event: MessageEvent) => {
      if (event.data !== HANDSHAKE || event.ports.length !== 1) return;
      console.info('OH-Hatoba host handshake received');
      clearTimeout(timer);
      window.removeEventListener("message", receive);
      const bridge = new HostBridge(event.ports[0]);
      window.addEventListener("pagehide", () => bridge.close(), { once: true });
      resolve(bridge);
    };
    const timer = setTimeout(() => {
      window.removeEventListener("message", receive);
      reject(failure("HarmonyOS native host did not connect"));
    }, timeoutMs);
    window.addEventListener("message", receive);
  });
}

let sharedBridge: Promise<HostBridge> | undefined;
/** Business API and system services share one native port and one handshake. */
export function harmonyBridge(): Promise<HostBridge> {
  return sharedBridge ??= connectHost();
}
