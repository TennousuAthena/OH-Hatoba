import { webview } from '@kit.ArkWeb';
import hatoba from 'libhatoba.so';

interface HostRequest {
  v: number;
  type: string;
  id: number;
  command: string;
}

interface HostFailure {
  code: string;
  detail: string;
}

interface HostReply {
  v: number;
  type: string;
  id: number;
  ok: boolean;
  result?: Object;
  error?: HostFailure;
}

@Entry
@Component
struct Index {
  private controller: webview.WebviewController = new webview.WebviewController();
  private ports: webview.WebMessagePort[] = [];
  private generation: number = 0;
  private pending: Set<number> = new Set<number>();
  @State private hostFailed: boolean = false;

  private closePorts(): void {
    this.generation++;
    this.pending.clear();
    this.ports.forEach((port: webview.WebMessagePort) => {
      try { port.close(); } catch (_) {}
    });
    this.ports = [];
  }

  aboutToDisappear(): void {
    this.closePorts();
  }

  private connect(): void {
    this.closePorts();
    try {
      this.ports = this.controller.createWebMessagePorts();
      const generation: number = this.generation;
      this.ports[0].onMessageEvent((message: webview.WebMessage) => {
      if (typeof message !== 'string' || message.length > 1048576 || this.generation !== generation) return;
      let request: HostRequest;
      try {
        request = JSON.parse(message) as HostRequest;
      } catch (_) {
        this.closePorts();
        return;
      }
      if (!request || request.v !== 1) { this.closePorts(); return; }
      if (request.type !== 'call' && request.type !== 'cancel') return;
      if (!Number.isSafeInteger(request.id) || request.id < 1) { this.closePorts(); return; }
      if (request.type === 'cancel') { this.pending.delete(request.id); return; }
      if (request.type !== 'call' || this.pending.has(request.id)) return;
      if (this.pending.size >= 64) { this.closePorts(); return; }
      this.pending.add(request.id);
      // P0 exposes a real dependency probe only. Business commands must be wired to the shared
      // runtime before switching the packaged page to the full upstream UI.
      if (request.command !== 'host_probe') {
        const error: HostFailure = { code: 'invalid_input', detail: 'Command is not available in the P0 host' };
        const reply: HostReply = { v: 1, type: 'reply', id: request.id, ok: false, error: error };
        this.reply(reply, generation);
        return;
      }
      hatoba.probe().then((json: string) => {
        const reply: HostReply = { v: 1, type: 'reply', id: request.id, ok: true, result: JSON.parse(json) as Object };
        this.reply(reply, generation);
      }).catch(() => {
        const error: HostFailure = { code: 'internal', detail: 'Native probe failed' };
        const reply: HostReply = { v: 1, type: 'reply', id: request.id, ok: false, error: error };
        this.reply(reply, generation);
      });
      });
      // Navigation is restricted to packaged resources below. There are no remote frames.
      this.controller.postMessage('oh-hatoba-host/v1', [this.ports[1]], '*');
      this.hostFailed = false;
    } catch (_) {
      this.hostFailed = true;
      this.closePorts();
    }
  }

  private reply(reply: HostReply, generation: number): void {
    if (generation !== this.generation || !this.pending.delete(reply.id) || this.ports.length !== 2) return;
    try {
      this.ports[0].postMessageEvent(JSON.stringify(reply));
    } catch (_) {
      this.hostFailed = true;
      this.closePorts();
    }
  }

  build() {
    Column() {
      if (this.hostFailed) {
        Text('原生宿主连接失败，请重新打开应用。').padding(16)
      }
      Web({ src: $rawfile('web/harmonyos-probe.html'), controller: this.controller })
        .javaScriptAccess(true)
        .domStorageAccess(false)
        .fileAccess(false)
        .mixedMode(MixedMode.None)
        .onPageBegin(() => this.closePorts())
        .onPageEnd(() => this.connect())
        .onLoadIntercept((event) => {
          if (!event) return true;
          const url: string = event.data.getRequestUrl();
          return !url.startsWith('resource://');
        })
        .width('100%')
        .height('100%')
    }.width('100%').height('100%')
  }
}
