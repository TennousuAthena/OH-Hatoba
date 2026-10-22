import { common } from '@kit.AbilityKit';
import { webview } from '@kit.ArkWeb';
import { certificateManager } from '@kit.DeviceCertificateKit';
import { i18n } from '@kit.LocalizationKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import deviceInfo from '@ohos.deviceInfo';
import hatoba from 'libhatoba.so';
import { LOCAL_ORIGIN } from './LocalResources';
import { SystemServices } from './SystemServices';
import { ClipboardAccess } from './ClipboardAccess';
import { BackgroundLease } from './BackgroundLease';

interface NativeConfig {
  filesDir: string;
  cacheDir: string;
  deviceName: string;
  locale: string;
  systemCaPath: string;
  biometricAvailable: boolean;
  diagnostics: boolean;
  networkTests: boolean;
}
interface NativeStatus { ok: boolean; }
interface ProbeReply { type: string; id: number; ok: boolean; result?: Object; }
interface RequestInfo { command?: string; id?: number; }

export class NativeHost {
  private controller: webview.WebviewController;
  private context: common.UIAbilityContext;
  private failure: (failed: boolean) => void;
  private generation: number = 0;
  private ports: webview.WebMessagePort[] = [];
  private timer: number = -1;
  private probes: Set<number> = new Set<number>();
  private networkProbes: Set<number> = new Set<number>();
  private services?: SystemServices;

  constructor(controller: webview.WebviewController, context: common.UIAbilityContext,
    failure: (failed: boolean) => void) {
    this.controller = controller;
    this.context = context;
    this.failure = failure;
  }

  begin(): void {
    ClipboardAccess.cancel();
    BackgroundLease.close(this.context);
    AppStorage.setOrCreate('OH_HATOBA_NOTIFICATIONS_READY', false);
    this.generation++;
    if (this.timer !== -1) { clearInterval(this.timer); this.timer = -1; }
    this.ports.forEach((port: webview.WebMessagePort) => { try { port.close(); } catch (_) {} });
    this.ports = [];
    this.probes.clear();
    this.networkProbes.clear();
    hatoba.detach();
  }

  async connect(diagnostics: boolean): Promise<void> {
    const generation = this.generation;
    try {
      const config: NativeConfig = {
        filesDir: diagnostics ? `${this.context.filesDir}/diagnostics` : this.context.filesDir,
        cacheDir: this.context.cacheDir,
        deviceName: deviceInfo.marketName || deviceInfo.productModel || 'HarmonyOS device',
        locale: i18n.System.getSystemLanguage(),
        systemCaPath: certificateManager.getCertificateStorePath({ certType: certificateManager.CertType.CA_CERT_SYSTEM }),
        biometricAvailable: !diagnostics && SystemServices.authTypes().length > 0,
        diagnostics: diagnostics,
        networkTests: diagnostics && (AppStorage.get<boolean>('OH_HATOBA_NETWORK_TESTS') ?? false)
      };
      const initialized: NativeStatus = JSON.parse(await hatoba.initialize(JSON.stringify(config))) as NativeStatus;
      if (config.networkTests) hilog.info(0x0000, 'OH-Hatoba', 'Network diagnostics enabled');
      if (generation !== this.generation) return;
      if (!initialized.ok) throw new Error('Native initialization failed');
      const services = new SystemServices(this.context, () => generation === this.generation);
      this.services = services;
      this.ports = this.controller.createWebMessagePorts();
      this.ports[0].onMessageEvent((message: webview.WebMessage) => {
        if (generation !== this.generation || typeof message !== 'string' || message.length > 1048576) return;
        try {
          const info: RequestInfo = JSON.parse(message) as RequestInfo;
          if (info.command === 'host_probe' && info.id) this.probes.add(info.id);
          if (diagnostics && config.networkTests && info.command === 'host_network_probe' && info.id) {
            this.networkProbes.add(info.id);
            hilog.info(0x0000, 'OH-Hatoba', 'Network diagnostics started');
          }
          const result: NativeStatus = JSON.parse(hatoba.submit(message)) as NativeStatus;
          if (!result.ok) throw new Error('Native message rejected');
        } catch (_) { this.failure(true); this.begin(); }
      });
      this.timer = setInterval(() => {
        if (generation !== this.generation || this.ports.length !== 2) return;
        try {
          hatoba.drain().forEach((message: string | ArrayBuffer) => {
            if (typeof message === 'string' && (this.probes.size || this.networkProbes.size)) {
              const reply: ProbeReply = JSON.parse(message) as ProbeReply;
              if (reply.type === 'reply' && this.probes.delete(reply.id) && reply.ok) {
                // Only the generated-data dependency probe is logged, never business replies.
                hilog.info(0x0000, 'OH-Hatoba', 'P0 native probe: %{public}s', JSON.stringify(reply.result));
              }
              if (reply.type === 'reply' && this.networkProbes.delete(reply.id) && reply.ok) {
                hilog.info(0x0000, 'OH-Hatoba', 'Native network probe: %{public}s', JSON.stringify(reply.result));
              }
            }
            this.ports[0].postMessageEvent(message);
          });
          hatoba.drainSystem().forEach((request: string) => {
            services.process(request).then((reply: string) => {
              if (generation === this.generation) hatoba.systemReply(reply);
            });
          });
          const notification = AppStorage.get<string>('OH_HATOBA_NOTIFICATION');
          if (!diagnostics && notification && AppStorage.get<boolean>('OH_HATOBA_NOTIFICATIONS_READY')) {
            const delivered: NativeStatus = JSON.parse(hatoba.lifecycle(notification)) as NativeStatus;
            if (delivered.ok) AppStorage.set('OH_HATOBA_NOTIFICATION', '');
          }
        } catch (_) { this.failure(true); this.begin(); }
      }, 20);
      this.controller.postMessage('oh-hatoba-host/v1', [this.ports[1]], LOCAL_ORIGIN);
      this.failure(false);
      hilog.info(0x0000, 'OH-Hatoba', 'Shared native runtime connected');
    } catch (_) {
      if (generation === this.generation) {
        this.failure(true);
        hilog.error(0x0000, 'OH-Hatoba', 'Native host initialization failed');
      }
    }
  }

  async selectForWeb(multiple: boolean): Promise<string[]> { return this.services ? this.services.selectForWeb(multiple) : []; }
}
