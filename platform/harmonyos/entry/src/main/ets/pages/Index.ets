import { webview } from '@kit.ArkWeb';
import { hilog } from '@kit.PerformanceAnalysisKit';
import hatoba from 'libhatoba.so';
import { LOCAL_ORIGIN, LOCAL_ENTRY, isLocalNavigation, resolveLocalAsset } from '../host/LocalResources';

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

  private resource(url: string, method: string): WebResourceResponse {
    const response = new WebResourceResponse();
    response.setResponseCode(403);
    response.setReasonMessage('Forbidden');
    response.setResponseData('');
    const asset = resolveLocalAsset(url, method);
    if (!asset) return response;
    try {
      const manager = this.getUIContext().getHostContext()?.resourceManager;
      if (!manager) return response;
      const bytes = manager.getRawFileContentSync(asset.path);
      response.setResponseData(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
      response.setResponseMimeType(asset.mime);
      response.setResponseEncoding('UTF-8');
      response.setResponseHeader([{ headerKey: 'X-Content-Type-Options', headerValue: 'nosniff' }]);
      response.setResponseCode(200);
      response.setReasonMessage('OK');
    } catch (_) {
      response.setResponseCode(404);
      response.setReasonMessage('Not Found');
    }
    return response;
  }

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
        // This P0 report contains only build metadata and checks over generated test data.
        hilog.info(0x0000, 'OH-Hatoba', 'P0 native probe: %{public}s', json);
        const reply: HostReply = { v: 1, type: 'reply', id: request.id, ok: true, result: JSON.parse(json) as Object };
        this.reply(reply, generation);
      }).catch(() => {
        const error: HostFailure = { code: 'internal', detail: 'Native probe failed' };
        const reply: HostReply = { v: 1, type: 'reply', id: request.id, ok: false, error: error };
        this.reply(reply, generation);
      });
      });
      // Navigation is restricted to packaged resources below. There are no remote frames.
      this.controller.postMessage('oh-hatoba-host/v1', [this.ports[1]], LOCAL_ORIGIN);
      hilog.info(0x0000, 'OH-Hatoba', 'P0 host port offered');
      this.hostFailed = false;
    } catch (_) {
      hilog.error(0x0000, 'OH-Hatoba', 'P0 host port connection failed');
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
      Web({ src: LOCAL_ENTRY, controller: this.controller })
        .javaScriptAccess(true)
        .domStorageAccess(false)
        .fileAccess(false)
        .mixedMode(MixedMode.None)
        .onInterceptRequest((event) => this.resource(event?.request.getRequestUrl() ?? '', event?.request.getRequestMethod() ?? ''))
        .onPageBegin(() => {
          hilog.info(0x0000, 'OH-Hatoba', 'P0 page loading');
          this.closePorts();
        })
        .onPageEnd(() => {
          hilog.info(0x0000, 'OH-Hatoba', 'P0 page loaded');
          this.connect();
        })
        .onConsole((event) => {
          // Never forward arbitrary Web console data, which could contain future user data.
          if (event?.message.getMessage() === 'OH-Hatoba P0 probe rendered') {
            hilog.info(0x0000, 'OH-Hatoba', 'P0 Web received and rendered native probe');
          }
          if (event?.message.getMessageLevel() === MessageLevel.Error) {
            hilog.error(0x0000, 'OH-Hatoba', 'P0 Web console error at line %{public}d', event.message.getLineNumber());
          }
          return false;
        })
        .onLoadIntercept((event) => {
          if (!event) return true;
          const url: string = event.data.getRequestUrl();
          if (!isLocalNavigation(url)) {
            hilog.warn(0x0000, 'OH-Hatoba', 'P0 blocked navigation scheme: %{public}s', url.split(':')[0]);
          }
          return !isLocalNavigation(url);
        })
        .width('100%')
        .height('100%')
    }.width('100%').height('100%')
  }
}
