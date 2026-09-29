import { create } from "zustand";
import * as dmx from "../lib/dmx";
import type { UniverseStatus } from "../lib/showfile/generated";

interface UniverseStatusesState {
  /** Keyed by universe id. */
  statuses: Record<number, UniverseStatus>;
  init: () => Promise<void>;
}

/** Live status (connected/error/fps) for every active universe, for the top
 * bar's per-universe indicators and the Setup screen's universe list. */
export const useUniverseStatusesStore = create<UniverseStatusesState>((set) => ({
  statuses: {},

  init: async () => {
    try {
      const all = await dmx.getAllStatuses();
      set({ statuses: Object.fromEntries(all.map((s) => [s.universeId, s])) });
      void dmx.onUniverseStatusChanged((status) => {
        set((state) => ({ statuses: { ...state.statuses, [status.universeId]: status } }));
      });
    } catch (err) {
      // Expected outside the Tauri runtime (plain browser preview).
      console.warn("Could not load universe statuses:", err);
    }
  },
}));
