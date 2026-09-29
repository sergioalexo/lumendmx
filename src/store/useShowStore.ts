import { create } from "zustand";
import * as dmx from "../lib/dmx";
import { showfileAutosave, showfileNew, showfileOpen, showfileRecent, showfileSave } from "../lib/showfile/api";
import type {
  DriverConfig,
  RecentShowEntry,
  ShowFile,
  ShowSettings,
  UniverseConfig,
  Workspace,
} from "../lib/showfile/generated";
import { assetsToLegacy, legacyToAssets } from "../lib/showfile/legacyAssets";
import { useLibraryStore } from "./useLibraryStore";
import { usePatchStore } from "./usePatchStore";

/** Stable reference for "no show open yet" selectors (`s.meta?.universes ??
 * EMPTY_UNIVERSES`). A fresh `[]` literal in the selector itself would give
 * useSyncExternalStore a new snapshot identity every render and spin into an
 * infinite re-render loop. */
export const EMPTY_UNIVERSES: UniverseConfig[] = [];

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

/** Spins up (or re-spins-up) each of the show's universes' output drivers to
 * match what's on disk. Best-effort: a universe whose driver fails to start
 * (e.g. its serial port isn't plugged in) is logged, not fatal to loading the
 * show — the Setup screen will show it as disconnected. */
async function activateUniverses(show: ShowFile): Promise<void> {
  await Promise.all(
    show.universes.map(async (universe) => {
      try {
        await dmx.configureUniverse(universe.id, universe.driver);
      } catch (err) {
        console.warn(`Failed to start universe ${universe.id} (${universe.name}):`, err);
      }
    }),
  );
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

  /** Reconfigures a universe's driver, live and in the show's saved state. */
  setUniverseDriver: (universeId: number, driver: DriverConfig) => Promise<void>;
  /** Adds a new universe (starts with no driver) and returns its id. */
  addUniverse: (name: string) => number;
  /** Stops a universe's driver and drops it from the show entirely. */
  removeUniverseEntirely: (universeId: number) => void;
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
      void activateUniverses(show);
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
    void activateUniverses(show);
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

  setUniverseDriver: async (universeId, driver) => {
    await dmx.configureUniverse(universeId, driver);
    const { meta } = get();
    if (!meta) return;
    const universes = meta.universes.map((u) => (u.id === universeId ? { ...u, driver } : u));
    set({ meta: { ...meta, universes }, dirty: true });
  },

  addUniverse: (name) => {
    const { meta } = get();
    if (!meta) throw new Error("No show open");
    const nextId = meta.universes.reduce((max, u) => Math.max(max, u.id), 0) + 1;
    const universe: UniverseConfig = { id: nextId, name, driver: { kind: "null" } };
    set({ meta: { ...meta, universes: [...meta.universes, universe] }, dirty: true });
    return nextId;
  },

  removeUniverseEntirely: (universeId) => {
    void dmx.removeUniverse(universeId);
    const { meta } = get();
    if (!meta) return;
    const universes = meta.universes.filter((u) => u.id !== universeId);
    set({ meta: { ...meta, universes }, dirty: true });
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
