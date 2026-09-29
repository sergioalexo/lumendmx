import { create } from "zustand";
import { persist } from "zustand/middleware";

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
}

export const usePatchStore = create<PatchState>()(
  persist(
    (set) => ({
      fixtures: [],
      addFixture: (f) => {
        const fixture: PatchedFixture = { ...f, id: crypto.randomUUID() };
        set((s) => ({ fixtures: [...s.fixtures, fixture] }));
        return fixture;
      },
      removeFixture: (id) => set((s) => ({ fixtures: s.fixtures.filter((f) => f.id !== id) })),
    }),
    { name: "lumendmx-patch" },
  ),
);
