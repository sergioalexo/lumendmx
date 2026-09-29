import { create } from "zustand";
import { showfileAutosave, showfileNew, showfileOpen, showfileRecent, showfileSave } from "../lib/showfile/api";
import type { RecentShowEntry, ShowFile, ShowSettings, UniverseConfig, Workspace } from "../lib/showfile/generated";
import { assetsToLegacy, legacyToAssets } from "../lib/showfile/legacyAssets";
import { useLibraryStore } from "./useLibraryStore";
import { usePatchStore } from "./usePatchStore";

const AUTOSAVE_INTERVAL_MS = 60_000;

/** Everything in a `ShowFile` except `patch` and `legacyAssets`, which live in
 * (and are read live from) `usePatchStore`/`useLibraryStore` instead of being
 * duplicated in this store's state. */
export interface ShowMeta {
  schemaVersion: number;
  id: string;
  name: string;
  createdAt: string;
  modifiedAt: string;
  universes: UniverseConfig[];
  workspaces: Workspace[];
  settings: ShowSettings;
}

function toMeta(show: ShowFile): ShowMeta {
  return {
    schemaVersion: show.schemaVersion,
    id: show.id,
    name: show.name,
    createdAt: show.createdAt,
    modifiedAt: show.modifiedAt,
    universes: show.universes,
    workspaces: show.workspaces,
    settings: show.settings,
  };
}

function assembleShowFile(meta: ShowMeta): ShowFile {
  return {
    ...meta,
    patch: usePatchStore.getState().fixtures,
    legacyAssets: assetsToLegacy(useLibraryStore.getState().assets),
  };
}

/** True while a show is being loaded into `usePatchStore`/`useLibraryStore`,
 * so their change subscriptions below don't mark the freshly-opened show dirty. */
let applyingShow = false;

function applyPatchAndAssets(show: ShowFile): void {
  applyingShow = true;
  try {
    usePatchStore.getState().replaceAll(show.patch);
    useLibraryStore.getState().replaceAll(legacyToAssets(show.legacyAssets));
  } finally {
    applyingShow = false;
  }
}

interface ShowState {
  meta: ShowMeta | null;
  path: string | null;
  dirty: boolean;
  saving: boolean;
  recent: RecentShowEntry[];

  init: () => Promise<void>;
  newShow: () => Promise<void>;
  openShow: (path?: string) => Promise<void>;
  save: () => Promise<void>;
  saveAs: () => Promise<void>;
  refreshRecent: () => Promise<void>;
}

let autosaveTimer: ReturnType<typeof setInterval> | null = null;

export const useShowStore = create<ShowState>((set, get) => ({
  meta: null,
  path: null,
  dirty: false,
  saving: false,
  recent: [],

  init: async () => {
    await get().newShow();
    void get().refreshRecent();

    if (!autosaveTimer) {
      autosaveTimer = setInterval(() => {
        const { meta, dirty } = get();
        if (!meta || !dirty) return;
        void showfileAutosave(assembleShowFile(meta)).catch((err) => {
          console.warn("Autosave failed:", err);
        });
      }, AUTOSAVE_INTERVAL_MS);
    }
  },

  newShow: async () => {
    try {
      const show = await showfileNew();
      applyPatchAndAssets(show);
      set({ meta: toMeta(show), path: null, dirty: false });
    } catch (err) {
      // Expected outside the Tauri runtime (plain browser preview).
      console.warn("Show backend unavailable:", err);
    }
  },

  openShow: async (path) => {
    const [show, usedPath] = await showfileOpen(path);
    applyPatchAndAssets(show);
    set({ meta: toMeta(show), path: usedPath, dirty: false });
    void get().refreshRecent();
  },

  save: async () => {
    const { meta, path } = get();
    if (!meta) return;
    set({ saving: true });
    try {
      const [usedPath, saved] = await showfileSave(assembleShowFile(meta), path ?? undefined);
      set({ meta: toMeta(saved), path: usedPath, dirty: false });
      void get().refreshRecent();
    } finally {
      set({ saving: false });
    }
  },

  saveAs: async () => {
    const { meta } = get();
    if (!meta) return;
    set({ saving: true });
    try {
      const [usedPath, saved] = await showfileSave(assembleShowFile(meta), undefined);
      set({ meta: toMeta(saved), path: usedPath, dirty: false });
      void get().refreshRecent();
    } finally {
      set({ saving: false });
    }
  },

  refreshRecent: async () => {
    try {
      const recent = await showfileRecent();
      set({ recent });
    } catch (err) {
      console.warn("Could not load recent shows:", err);
    }
  },
}));

// `usePatchStore` rehydrates its persisted state asynchronously on startup,
// which fires this subscription once before any real edit happens; skip
// marking dirty until that initial hydration has actually finished.
let patchHydrated = usePatchStore.persist.hasHydrated();
usePatchStore.persist.onFinishHydration(() => {
  patchHydrated = true;
});

function markDirtyUnlessApplyingShow() {
  if (!applyingShow) {
    useShowStore.setState({ dirty: true });
  }
}

usePatchStore.subscribe(() => {
  if (patchHydrated) markDirtyUnlessApplyingShow();
});
useLibraryStore.subscribe(markDirtyUnlessApplyingShow);
