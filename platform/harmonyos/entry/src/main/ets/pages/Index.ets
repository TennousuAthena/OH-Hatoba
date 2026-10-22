import { webview } from '@kit.ArkWeb';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { common } from '@kit.AbilityKit';
import { NativeHost } from '../host/NativeHost';
import { ClipboardAccess } from '../host/ClipboardAccess';
import { LOCAL_ENTRY, DIAGNOSTIC_ENTRY, isLocalNavigation, resolveLocalAsset } from '../host/LocalResources';

@Entry
@Component
struct Index {
  private controller: webview.WebviewController = new webview.WebviewController();
  private host?: NativeHost;
  @State private hostFailed: boolean = false;
  @StorageLink('OH_HATOBA_DIAGNOSTICS') private diagnostics: boolean = false;
  @StorageLink('OH_HATOBA_PASTE') private pasteRequested: boolean = false;

  aboutToAppear(): void {
    const context = this.getUIContext().getHostContext() as common.UIAbilityContext;
    this.host = new NativeHost(this.controller, context, (failed: boolean) => { this.hostFailed = failed; });
  }

  aboutToDisappear(): void { this.host?.begin(); }

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

  build() {
    Column() {
      if (this.hostFailed) {
        Text('原生宿主连接失败，请重新打开应用。').padding(16)
      }
      Web({ src: this.diagnostics ? DIAGNOSTIC_ENTRY : LOCAL_ENTRY, controller: this.controller })
        .javaScriptAccess(true)
        .keyboardAvoidMode(WebKeyboardAvoidMode.RESIZE_CONTENT)
        .domStorageAccess(false)
        .fileAccess(false)
        .onShowFileSelector((event) => {
          if (!event || !this.host) return false;
          const result = event.result;
          const mode = event.fileSelector.getMode();
          if (mode !== FileSelectorMode.FileOpenMode && mode !== FileSelectorMode.FileOpenMultipleMode) { result.handleFileList([]); return true; }
          this.host.selectForWeb(mode === FileSelectorMode.FileOpenMultipleMode).then((uris: string[]) => result.handleFileList(uris));
          return true;
        })
        .mixedMode(MixedMode.None)
        .onInterceptRequest((event) => this.resource(event?.request.getRequestUrl() ?? '', event?.request.getRequestMethod() ?? ''))
        .onPageBegin(() => {
          hilog.info(0x0000, 'OH-Hatoba', 'P0 page loading');
          this.host?.begin();
        })
        .onPageEnd(() => {
          hilog.info(0x0000, 'OH-Hatoba', 'P0 page loaded');
          this.host?.connect(this.diagnostics);
        })
        .onConsole((event) => {
          if (event?.message.getMessage() === 'OH-Hatoba application boot completed') hilog.info(0x0000, 'OH-Hatoba', 'Application boot completed');
          if (event?.message.getMessage() === 'OH-Hatoba application rendered') hilog.info(0x0000, 'OH-Hatoba', 'Application rendered');
          if (event?.message.getMessage() === 'OH-Hatoba host handshake received') {
            hilog.info(0x0000, 'OH-Hatoba', 'Web host handshake received');
          }
          // Never forward arbitrary Web console data, which could contain future user data.
          if (event?.message.getMessage() === 'OH-Hatoba P0 probe rendered') {
            hilog.info(0x0000, 'OH-Hatoba', 'P0 Web received and rendered native probe');
          }
          if (this.diagnostics && event?.message.getMessage() === 'OH-Hatoba shared business smoke rendered') {
            hilog.info(0x0000, 'OH-Hatoba', 'Shared business smoke passed and rendered');
          }
          if (this.diagnostics && event?.message.getMessage() === 'OH-Hatoba network probe rendered') {
            hilog.info(0x0000, 'OH-Hatoba', 'Network probe passed and rendered');
          }
          if (this.diagnostics && event?.message.getMessage() === 'OH-Hatoba Web SSH transport passed') hilog.info(0x0000, 'OH-Hatoba', 'Web SSH transport passed');
          if (this.diagnostics && event?.message.getMessage() === 'OH-Hatoba diagnostic failed') hilog.error(0x0000, 'OH-Hatoba', 'Diagnostic failed');
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
        .layoutWeight(1)
      if (this.pasteRequested) {
        Row({ space: 16 }) {
          Text('点击系统粘贴按钮继续').layoutWeight(1)
          PasteButton().onClick((_event: ClickEvent, result: PasteButtonOnClickResult) => {
            if (result === PasteButtonOnClickResult.SUCCESS) ClipboardAccess.paste();
            else ClipboardAccess.cancel();
          })
          Button('取消').onClick(() => ClipboardAccess.cancel())
        }.width('100%').padding(12)
      }
    }.width('100%').height('100%')
  }
}
