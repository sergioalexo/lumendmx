import { create } from "zustand";
import * as dmx from "../lib/dmx";
import type { UniverseStatus } from "../lib/showfile/generated";
import { useShowStore } from "./useShowStore";

/** The show's first universe, which the top bar's quick connect/blackout
 * controls operate on. Additional universes are managed from Setup. */
export const PRIMARY_UNIVERSE_ID = 1;

function idleStatus(): UniverseStatus {
  return { universeId: PRIMARY_UNIVERSE_ID, connected: false, error: null, framesPerSec: 0 };
}

interface DmxStore {
  channels: Uint8Array;
  status: UniverseStatus;
  blackout: boolean;
  ports: dmx.SerialPortDescriptor[];
  ready: boolean;

  init: () => Promise<void>;
  refreshPorts: () => Promise<void>;
  connect: (portName: string) => Promise<void>;
  disconnect: () => Promise<void>;
  toggleBlackout: () => Promise<void>;
  /** Overwrites every channel (e.g. a Scene trigger or a full AI rewrite). */
  applyFull: (next: Uint8Array | number[]) => Promise<void>;
  /** Merges a sparse channel -> value patch into the current state (e.g. an AI tweak
   * that should only touch "the wash lights" and leave everything else running). */
  applyPatch: (patch: Record<number, number>) => Promise<void>;
}

export const useDmxStore = create<DmxStore>((set, get) => ({
  channels: new Uint8Array(dmx.UNIVERSE_SIZE),
  status: idleStatus(),
  blackout: false,
  ports: [],
  ready: false,

  init: async () => {
    try {
      const [universe, status, blackout, ports] = await Promise.all([
        dmx.getUniverseData(PRIMARY_UNIVERSE_ID).catch(() => Array(dmx.UNIVERSE_SIZE).fill(0)),
        dmx.getUniverseStatus(PRIMARY_UNIVERSE_ID),
        dmx.getBlackout(),
        dmx.listSerialPorts(),
      ]);
      set({
        channels: Uint8Array.from(universe),
        status: status ?? idleStatus(),
        blackout,
        ports,
        ready: true,
      });
      void dmx.onUniverseStatusChanged((s) => {
        if (s.universeId === PRIMARY_UNIVERSE_ID) set({ status: s });
      });
    } catch (err) {
      // Expected when the UI is opened outside the Tauri runtime (e.g. a plain
      // browser preview during frontend development) -- the Rust backend simply
      // isn't reachable, so leave the store at its disconnected defaults.
      console.warn("DMX backend unavailable:", err);
    }
  },

  refreshPorts: async () => {
    const ports = await dmx.listSerialPorts();
    set({ ports });
  },

  connect: async (portName: string) => {
    await useShowStore.getState().setUniverseDriver(PRIMARY_UNIVERSE_ID, {
      kind: "ftdi",
      port: portName,
      rateHz: 30,
    });
  },

  disconnect: async () => {
    await useShowStore.getState().setUniverseDriver(PRIMARY_UNIVERSE_ID, { kind: "null" });
  },

  toggleBlackout: async () => {
    const next = !get().blackout;
    await dmx.setBlackout(next);
    set({ blackout: next });
  },

  applyFull: async (next) => {
    const channels = next instanceof Uint8Array ? next : Uint8Array.from(next);
    await dmx.updateUniverseData(PRIMARY_UNIVERSE_ID, channels);
    set({ channels });
  },

  applyPatch: async (patch) => {
    const channels = new Uint8Array(get().channels);
    for (const [ch, value] of Object.entries(patch)) {
      const idx = Number(ch) - 1;
      if (idx >= 0 && idx < dmx.UNIVERSE_SIZE) {
        channels[idx] = Math.max(0, Math.min(255, value));
      }
    }
    await dmx.updateUniverseData(PRIMARY_UNIVERSE_ID, channels);
    set({ channels });
  },
}));
