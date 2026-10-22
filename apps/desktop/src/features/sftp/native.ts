import { isTauri, isHarmonyOS } from "@/ipc/api";
import { hostService } from "@/lib/hostServices";

/** Native file pickers. In a plain browser (mock backend) they fall back to the file input / file name. */

/** Upload picker (SFTP-02). Resolves to local paths, or file names in the browser. */
export async function pickUploadFiles(): Promise<string[]> {
  if (isHarmonyOS()) return hostService("pick_open_files");
  if (isTauri()) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ multiple: true, directory: false });
    return picked == null ? [] : Array.isArray(picked) ? picked : [picked];
  }
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = true;
    input.onchange = () => resolve(Array.from(input.files ?? [], (f) => f.name));
    input.oncancel = () => resolve([]);
    input.click();
  });
}

/** Save dialog for a download. `null` when the user cancels. */
export async function pickSavePath(fileName: string): Promise<string | null> {
  if (isHarmonyOS()) return hostService("pick_save_path", { defaultName: fileName });
  if (isTauri()) {
    const { save } = await import("@tauri-apps/plugin-dialog");
    return save({ defaultPath: fileName });
  }
  return fileName;
}
