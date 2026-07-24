import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";

export interface UpdateProgress {
  downloaded: number;
  total: number;
}

/** Single source of truth for update state (unlike MediaFetch, which keeps a
 * separate GitHub-polling command purely for the version badge alongside the
 * plugin's own check() -- here the one `Update` object drives both). */
export async function checkForUpdate(): Promise<Update | null> {
  return check();
}

export async function getCurrentVersion(): Promise<string> {
  return getVersion();
}

export async function installUpdate(
  update: Update,
  onProgress: (progress: UpdateProgress) => void,
): Promise<void> {
  let downloaded = 0;
  let total = 0;
  await update.downloadAndInstall((event) => {
    if (event.event === "Started") {
      total = event.data.contentLength ?? 0;
      onProgress({ downloaded: 0, total });
    } else if (event.event === "Progress") {
      downloaded += event.data.chunkLength;
      onProgress({ downloaded, total });
    }
  });
  await relaunch();
}
