import { UIAbility, Want } from '@kit.AbilityKit';
import { window } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { commonEventManager } from '@kit.BasicServicesKit';
import hatoba from 'libhatoba.so';
import { ClipboardAccess } from '../host/ClipboardAccess';
import { BackgroundLease } from '../host/BackgroundLease';

export default class EntryAbility extends UIAbility {
  private destroyed: boolean = false;
  private screen?: commonEventManager.CommonEventSubscriber;
  onCreate(want: Want): void {
    AppStorage.setOrCreate('OH_HATOBA_DIAGNOSTICS', want.parameters?.['ohhatoba.diagnostics'] === true);
    AppStorage.setOrCreate('OH_HATOBA_NETWORK_TESTS', want.parameters?.['ohhatoba.networkTests'] === true);
    hilog.info(0x0000, 'OH-Hatoba', 'Diagnostic mode: %{public}s', String(want.parameters?.['ohhatoba.diagnostics'] === true));
    AppStorage.setOrCreate('OH_HATOBA_FOCUSED', true);
    AppStorage.setOrCreate('OH_HATOBA_PASTE', false);
    AppStorage.setOrCreate('OH_HATOBA_NOTIFICATIONS_READY', false);
    this.notification(want);
    commonEventManager.createSubscriber({ events: [commonEventManager.Support.COMMON_EVENT_SCREEN_LOCKED] }).then((subscriber: commonEventManager.CommonEventSubscriber) => {
      if (this.destroyed) { try { commonEventManager.unsubscribe(subscriber); } catch (_) {} return; }
      this.screen = subscriber;
      commonEventManager.subscribe(subscriber, () => { ClipboardAccess.cancel(); hatoba.lifecycle('screen_locked'); });
    }).catch(() => hilog.warn(0x0000, 'OH-Hatoba', 'Screen lock subscription unavailable'));
  }
  private notification(want: Want): void {
    const target = want.parameters?.['ohhatoba.notification'];
    if (typeof target === 'string' && target.length <= 1024) AppStorage.setOrCreate('OH_HATOBA_NOTIFICATION', target);
  }
  onNewWant(want: Want): void { this.notification(want); }
  onForeground(): void { BackgroundLease.foreground(); hatoba.lifecycle('foreground'); }
  onBackground(): void { AppStorage.set('OH_HATOBA_FOCUSED', false); ClipboardAccess.cancel(); }
  onDestroy(): void {
    this.destroyed = true;
    BackgroundLease.close(this.context);
    if (this.screen) { try { commonEventManager.unsubscribe(this.screen); } catch (_) {} }
    hatoba.detach();
  }
  onWindowStageCreate(windowStage: window.WindowStage): void {
    try {
      windowStage.on('windowStageEvent', (event: window.WindowStageEventType) => {
        if (event === window.WindowStageEventType.ACTIVE) AppStorage.set('OH_HATOBA_FOCUSED', true);
        if (event === window.WindowStageEventType.INACTIVE) AppStorage.set('OH_HATOBA_FOCUSED', false);
      });
      windowStage.loadContent('pages/Index', (error) => {
        if (error.code) hilog.error(0x0000, 'OH-Hatoba', 'Could not load application page');
      });
    } catch (_) {
      hilog.error(0x0000, 'OH-Hatoba', 'Could not initialize application window');
    }
  }
}
