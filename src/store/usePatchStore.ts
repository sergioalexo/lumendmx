import { create } from "zustand";
import { persist } from "zustand/middleware";
import { useUndoStore } from "./useUndoStore";

/** One physical fixture in the user's rig: which library fixture it is, which of
 * its modes it's running in, and where it's addressed. */
export interface PatchedFixture {
  id: string;
  fixtureId: string;
  modeIndex: number;
  /** DMX start address, 1-512. */
  address: number;
  name: string;
}

interface PatchState {
  fixtures: PatchedFixture[];
  addFixture: (f: Omit<PatchedFixture, "id">) => PatchedFixture;
  removeFixture: (id: string) => void;
  /** Replaces the whole patch list (e.g. loading a show file), bypassing undo. */
  replaceAll: (fixtures: PatchedFixture[]) => void;
}

export const usePatchStore = create<PatchState>()(
  persist(
    (set, get) => ({
      fixtures: [],
      addFixture: (f) => {
        const fixture: PatchedFixture = { ...f, id: crypto.randomUUID() };
        useUndoStore.getState().push({
          label: `Patch ${fixture.name}`,
          redo: () => set((s) => ({ fixtures: [...s.fixtures, fixture] })),
          undo: () => set((s) => ({ fixtures: s.fixtures.filter((x) => x.id !== fixture.id) })),
        });
        return fixture;
      },
      removeFixture: (id) => {
        const removed = get().fixtures.find((f) => f.id === id);
        if (!removed) return;
        useUndoStore.getState().push({
          label: `Unpatch ${removed.name}`,
          redo: () => set((s) => ({ fixtures: s.fixtures.filter((f) => f.id !== id) })),
          undo: () => set((s) => ({ fixtures: [...s.fixtures, removed] })),
        });
      },
      replaceAll: (fixtures) => set({ fixtures }),
    }),
    { name: "lumendmx-patch" },
  ),
);
