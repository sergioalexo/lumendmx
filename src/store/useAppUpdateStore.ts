import { create } from "zustand";
import type { Update } from "@tauri-apps/plugin-updater";
import * as appUpdate from "../lib/appUpdate";
import type { UpdateProgress } from "../lib/appUpdate";

interface AppUpdateStore {
  currentVersion: string;
  update: Update | null;
  checking: boolean;
  installing: boolean;
  progress: UpdateProgress | null;
  error: string | null;
  checked: boolean;

  checkNow: () => Promise<void>;
  install: () => Promise<void>;
}

export const useAppUpdateStore = create<AppUpdateStore>((set, get) => ({
  currentVersion: "",
  update: null,
  checking: false,
  installing: false,
  progress: null,
  error: null,
  checked: false,

  checkNow: async () => {
    set({ checking: true, error: null });
    try {
      const [update, currentVersion] = await Promise.all([
        appUpdate.checkForUpdate(),
        appUpdate.getCurrentVersion(),
      ]);
      set({ update, currentVersion, checked: true });
    } catch (err) {
      // Expected outside the Tauri runtime, or offline -- try again later.
      set({ error: err instanceof Error ? err.message : String(err) });
    } finally {
      set({ checking: false });
    }
  },

  install: async () => {
    const update = get().update;
    if (!update) return;
    set({ installing: true, error: null });
    try {
      await appUpdate.installUpdate(update, (progress) => set({ progress }));
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
    } finally {
      set({ installing: false, progress: null });
    }
  },
}));
