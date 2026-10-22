import { pasteboard } from '@kit.BasicServicesKit';

/** A visible system PasteButton grants temporary access. No restricted clipboard permission. */
export class ClipboardAccess {
  private static resolve?: (value: string) => void;
  private static reject?: (error: Error) => void;
  private static current?: () => boolean;
  private static timer: number = -1;

  static request(current: () => boolean): Promise<string> {
    if (ClipboardAccess.resolve) return Promise.reject(new Error('Paste request is already open'));
    return new Promise<string>((resolve, reject) => {
      ClipboardAccess.resolve = resolve; ClipboardAccess.reject = reject; ClipboardAccess.current = current;
      AppStorage.setOrCreate('OH_HATOBA_PASTE', true);
      ClipboardAccess.timer = setTimeout(() => ClipboardAccess.cancel(), 90000);
    });
  }
  static async paste(): Promise<void> {
    try {
      if (!ClipboardAccess.current?.()) throw new Error('Page changed');
      const data = await pasteboard.getSystemPasteboard().getData();
      const text = data.getPrimaryText();
      if (text.length > 1048576 || !ClipboardAccess.current?.()) throw new Error('Invalid paste result');
      ClipboardAccess.resolve?.(text);
    } catch (_) { ClipboardAccess.reject?.(new Error('Paste unavailable')); }
    finally { ClipboardAccess.clear(); }
  }
  static cancel(): void { ClipboardAccess.reject?.(new Error('Paste cancelled')); ClipboardAccess.clear(); }
  private static clear(): void {
    if (ClipboardAccess.timer !== -1) clearTimeout(ClipboardAccess.timer);
    ClipboardAccess.timer = -1; ClipboardAccess.resolve = undefined; ClipboardAccess.reject = undefined; ClipboardAccess.current = undefined;
    AppStorage.setOrCreate('OH_HATOBA_PASTE', false);
  }
}
