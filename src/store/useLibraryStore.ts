import { create } from "zustand";
import type { LightAsset } from "../lib/types";
import * as engineApi from "../lib/engine";
import { PRIMARY_UNIVERSE_ID } from "../lib/constants";

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

  /** Runs a Scene/Chase/FX as a frame-accurate engine playback layer (see
   * engine::legacy_playback), not a JS timer — two triggered assets touching
   * the same channel now go through the same HTP/LTP merge as everything
   * else instead of racing to call `applyPatch` last. */
  trigger: (id) => {
    const asset = get().assets.find((a) => a.id === id);
    if (!asset) return;

    void engineApi.triggerAsset(id, PRIMARY_UNIVERSE_ID, engineApi.legacyAssetToSteps(asset), asset.loop);

    const activeAssetIds = new Set(get().activeAssetIds);
    activeAssetIds.add(id);
    set({ activeAssetIds });
  },

  stop: (id) => {
    void engineApi.stopAsset(id);
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
