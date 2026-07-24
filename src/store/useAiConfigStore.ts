import { create } from "zustand";
import type { AiConfig } from "../lib/ai/types";

const STORAGE_KEY = "lumendmx.aiConfig.v1";

const DEFAULT_CONFIG: AiConfig = {
  backend: "ollama",
  model: "llama3.1",
  baseUrl: "http://localhost:11434",
};

function loadConfig(): AiConfig {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? { ...DEFAULT_CONFIG, ...(JSON.parse(raw) as AiConfig) } : DEFAULT_CONFIG;
  } catch {
    return DEFAULT_CONFIG;
  }
}

interface AiConfigStore {
  config: AiConfig;
  setConfig: (config: AiConfig) => void;
}

// NOTE (PRD review): API keys are persisted in localStorage for MVP simplicity.
// Before shipping, move these to the OS keychain via a Tauri secret-storage plugin
// instead of plain browser storage.
export const useAiConfigStore = create<AiConfigStore>((set) => ({
  config: loadConfig(),
  setConfig: (config) => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
    set({ config });
  },
}));
