import { create } from "zustand";
import type { LightAsset } from "../lib/types";
import { AssetRunner } from "../lib/chaseEngine";
import { useDmxStore } from "./useDmxStore";

const STORAGE_KEY = "lumendmx.library.v1";

function loadAssets(): LightAsset[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? (JSON.parse(raw) as LightAsset[]) : [];
  } catch {
    return [];
  }
}

function persist(assets: LightAsset[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(assets));
}

interface LibraryStore {
  assets: LightAsset[];
  activeAssetIds: Set<string>;

  addAsset: (asset: LightAsset) => void;
  removeAsset: (id: string) => void;
  trigger: (id: string) => void;
  stop: (id: string) => void;
  /** Replaces the whole library (e.g. loading a show file's legacy assets). */
  replaceAll: (assets: LightAsset[]) => void;
}

const runners = new Map<string, AssetRunner>();

export const useLibraryStore = create<LibraryStore>((set, get) => ({
  assets: loadAssets(),
  activeAssetIds: new Set(),

  addAsset: (asset) => {
    const assets = [...get().assets, asset];
    persist(assets);
    set({ assets });
  },

  removeAsset: (id) => {
    get().stop(id);
    const assets = get().assets.filter((a) => a.id !== id);
    persist(assets);
    set({ assets });
  },

  trigger: (id) => {
    const asset = get().assets.find((a) => a.id === id);
    if (!asset) return;

    get().stop(id);

    const dmx = useDmxStore.getState();
    const runner = new AssetRunner(
      asset,
      (channel) => dmx.channels[channel - 1] ?? 0,
      (patch) => {
        void useDmxStore.getState().applyPatch(patch);
      },
    );
    runners.set(id, runner);
    runner.start();

    const activeAssetIds = new Set(get().activeAssetIds);
    activeAssetIds.add(id);
    set({ activeAssetIds });
  },

  stop: (id) => {
    const runner = runners.get(id);
    if (runner) {
      runner.stop();
      runners.delete(id);
    }
    if (get().activeAssetIds.has(id)) {
      const activeAssetIds = new Set(get().activeAssetIds);
      activeAssetIds.delete(id);
      set({ activeAssetIds });
    }
  },

  replaceAll: (assets) => {
    for (const id of get().activeAssetIds) get().stop(id);
    persist(assets);
    set({ assets });
  },
}));
