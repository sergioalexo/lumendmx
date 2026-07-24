import { create } from "zustand";
import * as ollama from "../lib/ollama";
import type { PullProgress } from "../lib/ollama";

interface OllamaStore {
  reachable: boolean | null; // null = not checked yet
  installedModels: string[];
  pulling: boolean;
  progress: PullProgress | null;
  error: string | null;

  refresh: (baseUrl: string) => Promise<void>;
  pull: (baseUrl: string, model: string) => Promise<void>;
}

export const useOllamaStore = create<OllamaStore>((set) => ({
  reachable: null,
  installedModels: [],
  pulling: false,
  progress: null,
  error: null,

  refresh: async (baseUrl) => {
    try {
      const models = await ollama.listModels(baseUrl);
      set({ reachable: true, installedModels: models.map((m) => m.name), error: null });
    } catch (err) {
      set({ reachable: false, error: err instanceof Error ? err.message : String(err) });
    }
  },

  pull: async (baseUrl, model) => {
    set({ pulling: true, error: null, progress: null });
    try {
      await ollama.pullModel(baseUrl, model, (progress) => set({ progress }));
      const models = await ollama.listModels(baseUrl);
      set({ reachable: true, installedModels: models.map((m) => m.name) });
    } catch (err) {
      set({ error: err instanceof Error ? err.message : String(err) });
    } finally {
      set({ pulling: false, progress: null });
    }
  },
}));
