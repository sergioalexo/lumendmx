import { create } from "zustand";
import { useLibraryStore } from "./useLibraryStore";

const STORAGE_KEY = "lumendmx.midiMap.v1";

function midiKey(channel: number, note: number) {
  return `${channel}:${note}`;
}

function loadMap(): Record<string, string> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? (JSON.parse(raw) as Record<string, string>) : {};
  } catch {
    return {};
  }
}

interface MidiInputInfo {
  id: string;
  name: string;
}

interface MidiStore {
  supported: boolean;
  inputs: MidiInputInfo[];
  /** `${midiChannel}:${note}` -> asset id */
  midiMap: Record<string, string>;
  learnTargetAssetId: string | null;

  init: () => Promise<void>;
  startLearn: (assetId: string) => void;
  cancelLearn: () => void;
  unbindAsset: (assetId: string) => void;
}

let midiAccess: MIDIAccess | null = null;

function bindInputListeners(get: () => MidiStore) {
  if (!midiAccess) return;
  for (const input of midiAccess.inputs.values()) {
    input.onmidimessage = (event: MIDIMessageEvent) => {
      const data = event.data;
      if (!data || data.length < 3) return;
      const [status, note, velocity] = data;
      const isNoteOn = status >= 0x90 && status <= 0x9f && velocity > 0;
      if (!isNoteOn) return;

      const channel = (status & 0x0f) + 1;
      const key = midiKey(channel, note);
      const state = get();

      if (state.learnTargetAssetId) {
        const midiMap = { ...state.midiMap, [key]: state.learnTargetAssetId };
        localStorage.setItem(STORAGE_KEY, JSON.stringify(midiMap));
        useMidiStore.setState({ midiMap, learnTargetAssetId: null });
        return;
      }

      const assetId = state.midiMap[key];
      if (assetId) {
        useLibraryStore.getState().trigger(assetId);
      }
    };
  }
}

function snapshotInputs(): MidiInputInfo[] {
  if (!midiAccess) return [];
  return Array.from(midiAccess.inputs.values()).map((input) => ({
    id: input.id,
    name: input.name ?? "Unknown MIDI Device",
  }));
}

export const useMidiStore = create<MidiStore>((set, get) => ({
  supported: typeof navigator !== "undefined" && "requestMIDIAccess" in navigator,
  inputs: [],
  midiMap: loadMap(),
  learnTargetAssetId: null,

  init: async () => {
    if (!get().supported) return;
    try {
      midiAccess = await navigator.requestMIDIAccess();
      bindInputListeners(get);
      set({ inputs: snapshotInputs() });

      // Hot-swap support (SRS 2.4): re-bind listeners whenever a controller is
      // plugged or unplugged mid-set, without crashing or requiring a restart.
      midiAccess.onstatechange = () => {
        bindInputListeners(get);
        set({ inputs: snapshotInputs() });
      };
    } catch (err) {
      console.error("MIDI access denied or unavailable:", err);
    }
  },

  startLearn: (assetId) => set({ learnTargetAssetId: assetId }),
  cancelLearn: () => set({ learnTargetAssetId: null }),

  unbindAsset: (assetId) => {
    const midiMap = Object.fromEntries(
      Object.entries(get().midiMap).filter(([, id]) => id !== assetId),
    );
    localStorage.setItem(STORAGE_KEY, JSON.stringify(midiMap));
    set({ midiMap });
  },
}));
