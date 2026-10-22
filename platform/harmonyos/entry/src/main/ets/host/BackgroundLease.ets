import { common, wantAgent } from '@kit.AbilityKit';
import { backgroundTaskManager } from '@kit.BackgroundTasksKit';

/** Scoped OS tasks: OAuth gets a transient delay; only active file transfers request DATA_TRANSFER. */
export class BackgroundLease {
  private static transient: number = -1;
  private static transfers: Set<string> = new Set<string>();
  private static active: boolean = false;
  private static serial: Promise<void> = Promise.resolve();
  static authentication(): void {
    BackgroundLease.foreground();
    try {
      const request = backgroundTaskManager.requestSuspendDelay('完成浏览器身份认证', () => BackgroundLease.foreground());
      BackgroundLease.transient = request.requestId;
    } catch (_) {}
  }
  static foreground(): void {
    if (BackgroundLease.transient !== -1) {
      try { backgroundTaskManager.cancelSuspendDelay(BackgroundLease.transient); } catch (_) {}
      BackgroundLease.transient = -1;
    }
  }
  static transfer(context: common.UIAbilityContext, id: string, active: boolean): Promise<void> {
    if (active) BackgroundLease.transfers.add(id); else BackgroundLease.transfers.delete(id);
    BackgroundLease.serial = BackgroundLease.serial.then(async () => {
      if (BackgroundLease.transfers.size && !BackgroundLease.active) {
        try {
          const agent = await wantAgent.getWantAgent({ wants: [{ bundleName: 'dev.ohhatoba.app', abilityName: 'EntryAbility' }], actionType: wantAgent.OperationType.START_ABILITY, requestCode: 1, actionFlags: [wantAgent.WantAgentFlags.UPDATE_PRESENT_FLAG] });
          await backgroundTaskManager.startBackgroundRunning(context, backgroundTaskManager.BackgroundMode.DATA_TRANSFER, agent);
          BackgroundLease.active = true;
        } catch (_) { /* Foreground transfers still run when the OS declines a background lease. */ }
      }
      if (!BackgroundLease.transfers.size && BackgroundLease.active) {
        try { await backgroundTaskManager.stopBackgroundRunning(context); } catch (_) {}
        BackgroundLease.active = false;
      }
    });
    return BackgroundLease.serial;
  }
  static close(context: common.UIAbilityContext): void {
    BackgroundLease.foreground(); BackgroundLease.transfers.clear(); BackgroundLease.transfer(context, '', false);
  }
}
