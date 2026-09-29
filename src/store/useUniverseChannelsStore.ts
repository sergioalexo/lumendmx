import { create } from "zustand";
import * as engineApi from "../lib/engine";

interface UniverseChannelsState {
  /** Keyed by universe id. Populated from the engine's ~20Hz frame event, so
   * this works for any universe, not just the primary one `useDmxStore`
   * tracks for the top bar's quick-connect UI. */
  channels: Record<number, Uint8Array>;
  init: () => Promise<void>;
}

export const useUniverseChannelsStore = create<UniverseChannelsState>((set) => ({
  channels: {},

  init: async () => {
    try {
      // Must be awaited, not fire-and-forget: `listen()` rejects
      // asynchronously outside the Tauri runtime, so an un-awaited call here
      // would surface as an unhandled promise rejection instead of being
      // caught below.
      await engineApi.onUniverseFrame((frame) => {
        set((s) => ({
          channels: { ...s.channels, [frame.universeId]: Uint8Array.from(frame.channels) },
        }));
      });
    } catch (err) {
      // Expected outside the Tauri runtime (plain browser preview).
      console.warn("Could not subscribe to universe frames:", err);
    }
  },
}));

export function getUniverseChannel(universe: number, channel: number): number {
  return useUniverseChannelsStore.getState().channels[universe]?.[channel - 1] ?? 0;
}
