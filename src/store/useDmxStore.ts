import { create } from "zustand";
import * as dmx from "../lib/dmx";
import type { ConnectionStatus } from "../lib/dmx";

interface DmxStore {
  channels: Uint8Array;
  connection: ConnectionStatus;
  blackout: boolean;
  ports: string[];
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
  connection: { connected: false, port: null, error: null },
  blackout: false,
  ports: [],
  ready: false,

  init: async () => {
    try {
      const [universe, connection, blackout, ports] = await Promise.all([
        dmx.getUniverse(),
        dmx.getConnectionStatus(),
        dmx.getBlackout(),
        dmx.listSerialPorts(),
      ]);
      set({
        channels: Uint8Array.from(universe),
        connection,
        blackout,
        ports,
        ready: true,
      });
      dmx.onConnectionChanged((status) => set({ connection: status }));
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
    await dmx.connectSerialPort(portName);
  },

  disconnect: async () => {
    await dmx.disconnectSerialPort();
  },

  toggleBlackout: async () => {
    const next = !get().blackout;
    await dmx.setBlackout(next);
    set({ blackout: next });
  },

  applyFull: async (next) => {
    const channels = next instanceof Uint8Array ? next : Uint8Array.from(next);
    await dmx.updateUniverse(channels);
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
    await dmx.updateUniverse(channels);
    set({ channels });
  },
}));
