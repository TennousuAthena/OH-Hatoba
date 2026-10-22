import { common, wantAgent } from '@kit.AbilityKit';
import { notificationManager } from '@kit.NotificationKit';
import { userAuth } from '@kit.UserAuthenticationKit';
import { pasteboard } from '@kit.BasicServicesKit';
import { picker, fileIo as fs, fileUri } from '@kit.CoreFileKit';
import { window } from '@kit.ArkUI';
import hatoba from 'libhatoba.so';
import { ClipboardAccess } from './ClipboardAccess';
import { BackgroundLease } from './BackgroundLease';

interface NotificationTarget { tab?: string | null; conversation_id?: string | null; host_id?: string | null; }

interface HostArgs {
  background?: string;
  text?: string;
  url?: string;
  directory?: boolean;
  defaultName?: string;
  extensions?: string[];
  title?: string;
  body?: string;
  interactive?: boolean;
  challenge?: number[];
  purpose?: string;
  id?: string;
  active?: boolean;
  ready?: boolean;
  target?: NotificationTarget;
}
interface SystemCall { id: number; method: string; args: HostArgs; }
interface NativeStatus { ok: boolean; result?: string; }
interface SystemReply { id: number; ok: boolean; result?: Object | null; }
interface Capabilities {
  localTerminal: boolean;
  sshAgent: boolean;
  stdioMcp: boolean;
  desktopCli: boolean;
  tray: boolean;
  updater: boolean;
  folderPicker: boolean;
}

/** Runs only in ArkTS. Native system requests use a separate queue from Web messages. */
export class SystemServices {
  private static notificationId: number = 10;
  static authTypes(): userAuth.UserAuthType[] {
    const supported: userAuth.UserAuthType[] = [];
    for (const type of [userAuth.UserAuthType.FINGERPRINT, userAuth.UserAuthType.FACE]) {
      try { userAuth.getAvailableStatus(type, userAuth.AuthTrustLevel.ATL2); supported.push(type); } catch (_) {}
    }
    return supported;
  }
  private context: common.UIAbilityContext;
  private current: () => boolean;
  private pickerBusy: boolean = false;
  constructor(context: common.UIAbilityContext, current: () => boolean) { this.context = context; this.current = current; }

  async process(encoded: string): Promise<string> {
    let id: number = 0;
    try {
      const call: SystemCall = JSON.parse(encoded) as SystemCall;
      id = call.id;
      if (!this.current()) throw new Error('Native page changed');
      const result = await this.invoke(call.method, call.args);
      const reply: SystemReply = { id: id, ok: true, result: result };
      return JSON.stringify(reply);
    } catch (_) {
      const reply: SystemReply = { id: id, ok: false };
      return JSON.stringify(reply);
    }
  }

  private async invoke(method: string, args: HostArgs): Promise<Object | null> {
    switch (method) {
      case 'capabilities': {
        const result: Capabilities = {
          localTerminal: false, sshAgent: false, stdioMcp: false, desktopCli: false,
          tray: false, updater: false,
          folderPicker: canIUse('SystemCapability.FileManagement.UserFileService.FolderSelection')
        };
        return result;
      }
      case 'window_surface': {
        const background = args.background ?? '';
        if (!/^#[0-9A-F]{6}$/i.test(background)) throw new Error('Invalid window background');
        const mainWindow = await window.getLastWindow(this.context);
        if (!this.current()) throw new Error('Native page changed');
        const value = parseInt(background.slice(1), 16);
        const luminance = (value >> 16) * 299 + ((value >> 8) & 255) * 587 + (value & 255) * 114;
        const foreground = luminance >= 128000 ? '#000000' : '#FFFFFF';
        mainWindow.setWindowBackgroundColor(background);
        AppStorage.setOrCreate('OH_HATOBA_BACKGROUND', background);
        AppStorage.setOrCreate('OH_HATOBA_FOREGROUND', foreground);
        // Gesture navigation uses the window background; this also covers three-button mode.
        try { await mainWindow.setWindowSystemBarProperties({ navigationBarColor: background, navigationBarContentColor: foreground }); } catch (_) {}
        return background;
      }
      case 'clipboard_write': {
        if (typeof args.text !== 'string' || args.text.length > 1048576) throw new Error('Invalid clipboard text');
        await pasteboard.getSystemPasteboard().setData(pasteboard.createData(pasteboard.MIMETYPE_TEXT_PLAIN, args.text));
        return null;
      }
      case 'clipboard_read': {
        if (args?.interactive) return ClipboardAccess.request(this.current);
        const data = await pasteboard.getSystemPasteboard().getData();
        const text = data.getPrimaryText();
        if (text.length > 1048576) throw new Error('Clipboard is too large');
        return text;
      }
      case 'biometric_auth': return this.authenticate(args);
      case 'notification_permission': await notificationManager.requestEnableNotification(this.context); return null;
      case 'notifications_ready': AppStorage.setOrCreate('OH_HATOBA_NOTIFICATIONS_READY', args.ready === true); return null;
      case 'transfer_active': {
        if (!args.id || args.id.length > 128) throw new Error('Invalid transfer');
        await BackgroundLease.transfer(this.context, args.id, args.active === true); return null;
      }
      case 'notification_show': {
        if (!(await notificationManager.isNotificationEnabled())) return null;
        const target = JSON.stringify(args.target ?? {});
        if (target.length > 1024) throw new Error('Invalid notification target');
        const id = ++SystemServices.notificationId;
        const agent = await wantAgent.getWantAgent({ wants: [{ bundleName: 'dev.ohhatoba.app', abilityName: 'EntryAbility', parameters: { 'ohhatoba.notification': target } }], actionType: wantAgent.OperationType.START_ABILITY, requestCode: id, actionFlags: [wantAgent.WantAgentFlags.UPDATE_PRESENT_FLAG] });
        await notificationManager.publish({ id: id, wantAgent: agent, content: { notificationContentType: notificationManager.ContentType.NOTIFICATION_CONTENT_BASIC_TEXT, normal: { title: (args.title ?? '').slice(0, 200), text: (args.body ?? '').slice(0, 1000) } } });
        return null;
      }
      case 'open_url': {
        const url = args.url ?? '';
        if (url.length > 8192 || !/^https?:\/\/[^\/?#\s@]+([\/?#]|$)/i.test(url)) throw new Error('Invalid external URL');
        if (args.purpose === 'authentication') BackgroundLease.authentication();
        await this.context.openLink(url);
        return null;
      }
      case 'window_focused': return AppStorage.get<boolean>('OH_HATOBA_FOCUSED') ?? true;
      case 'pick_open_path': {
        const selected = await this.open(args, false);
        return selected.length ? selected[0] : null;
      }
      case 'pick_open_files': return this.open(args, true);
      case 'pick_save_path': return this.save(args);
      case 'app_quit': await this.context.terminateSelf(); return null;
      default: throw new Error('Unsupported native service');
    }
  }

  private async authenticate(args: HostArgs): Promise<number[]> {
    if (!args.challenge || args.challenge.length !== 32 || !args.challenge.every((n: number) => Number.isInteger(n) && n >= 0 && n <= 255)) throw new Error('Invalid authentication challenge');
    const types = SystemServices.authTypes();
    if (!types.length) throw new Error('Biometrics unavailable');
    const auth = userAuth.getUserAuthInstance({ challenge: new Uint8Array(args.challenge), authType: types, authTrustLevel: userAuth.AuthTrustLevel.ATL2 }, { title: '解锁 OH-Hatoba 保险库' });
    return new Promise<number[]>((resolve, reject) => {
      const timer = setTimeout(() => { try { auth.cancel(); } catch (_) {} reject(new Error('Authentication timed out')); }, 90000);
      auth.on('result', { onResult: (result: userAuth.UserAuthResult) => {
        clearTimeout(timer);
        if (this.current() && result.result === userAuth.UserAuthResultCode.SUCCESS && result.token) {
          const token = Array.from(result.token); result.token.fill(0); resolve(token);
        } else { reject(new Error('Authentication cancelled or unavailable')); }
      } });
      try { auth.start(); } catch (_) { clearTimeout(timer); reject(new Error('Authentication unavailable')); }
    });
  }

  // Web File API receives only URIs explicitly selected by the user, never sandbox paths.
  async selectForWeb(multiple: boolean): Promise<string[]> {
    if (!this.current() || this.pickerBusy) return [];
    this.pickerBusy = true;
    try {
      const options = new picker.DocumentSelectOptions(); options.maxSelectNumber = multiple ? 64 : 1;
      const uris = await new picker.DocumentViewPicker(this.context).select(options);
      return this.current() ? uris : [];
    } catch (_) { return []; } finally { this.pickerBusy = false; }
  }

  private filters(extensions?: string[]): string[] {
    if (!extensions?.length) return [];
    const valid = extensions.filter((extension: string) => /^[A-Za-z0-9]{1,16}$/.test(extension)).slice(0, 5);
    return valid.length ? [`Files|${valid.map((extension: string) => `.${extension}`).join(',')}`] : [];
  }

  private register(uri: string, write: boolean): string {
    if (!this.current()) throw new Error('Native page changed');
    const name = new fileUri.FileUri(uri).name;
    const file = fs.openSync(uri, write ? fs.OpenMode.WRITE_ONLY : fs.OpenMode.READ_ONLY);
    try {
      const result: NativeStatus = JSON.parse(hatoba.grant(file.fd, name, write)) as NativeStatus;
      if (!result.ok || !result.result) throw new Error('Document grant failed');
      return result.result;
    } finally { fs.closeSync(file.fd); }
  }

  private async open(args: HostArgs, multiple: boolean): Promise<string[]> {
    if (this.pickerBusy) throw new Error('Document picker is already open');
    this.pickerBusy = true;
    try {
    if (args.directory && !canIUse('SystemCapability.FileManagement.UserFileService.FolderSelection')) throw new Error('Folder selection unavailable');
    const options = new picker.DocumentSelectOptions();
    options.maxSelectNumber = multiple ? 64 : 1;
    if (args.directory) options.selectMode = picker.DocumentSelectMode.FOLDER;
    const filters = this.filters(args.extensions);
    if (filters.length) options.fileSuffixFilters = filters;
    const uris = await new picker.DocumentViewPicker(this.context).select(options);
    return uris.map((uri: string) => this.register(uri, false));
    } finally { this.pickerBusy = false; }
  }

  private async save(args: HostArgs): Promise<string | null> {
    if (this.pickerBusy) throw new Error('Document picker is already open');
    this.pickerBusy = true;
    try {
    const name = (args.defaultName ?? 'export.txt').split(/[\\/]/).pop() ?? 'export.txt';
    if (!name || name.length > 255 || name === '.' || name === '..') throw new Error('Invalid suggested filename');
    const options = new picker.DocumentSaveOptions();
    options.newFileNames = [name];
    const filters = this.filters(args.extensions);
    if (filters.length) options.fileSuffixChoices = filters;
    const uris = await new picker.DocumentViewPicker(this.context).save(options);
    return uris.length ? this.register(uris[0], true) : null;
    } finally { this.pickerBusy = false; }
  }
}
