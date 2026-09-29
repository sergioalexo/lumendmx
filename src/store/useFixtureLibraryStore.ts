import { create } from "zustand";
import { persist } from "zustand/middleware";
import { builtInFixtures } from "../lib/fixtures/registry";
import type { FixtureDefinition } from "../lib/fixtures/types";

interface FixtureLibraryState {
  /** User-imported fixtures (via the "Import fixture JSON" button), persisted
   * locally. Bundled fixtures live in `src/fixtures/library` and ship with the app. */
  customFixtures: FixtureDefinition[];
  addCustomFixture: (def: FixtureDefinition) => void;
  removeCustomFixture: (id: string) => void;
}

export const useFixtureLibraryStore = create<FixtureLibraryState>()(
  persist(
    (set) => ({
      customFixtures: [],
      addCustomFixture: (def) =>
        set((s) => ({
          customFixtures: [...s.customFixtures.filter((f) => f.id !== def.id), def],
        })),
      removeCustomFixture: (id) =>
        set((s) => ({ customFixtures: s.customFixtures.filter((f) => f.id !== id) })),
    }),
    { name: "lumendmx-custom-fixtures" },
  ),
);

/** Bundled + imported fixtures combined (imported ones win on id collision).
 * A plain selector hook, not a store method, so components actually re-render
 * when `customFixtures` changes (e.g. right after an import). */
export function useAllFixtures(): FixtureDefinition[] {
  const customFixtures = useFixtureLibraryStore((s) => s.customFixtures);
  const customIds = new Set(customFixtures.map((f) => f.id));
  return [...builtInFixtures.filter((f) => !customIds.has(f.id)), ...customFixtures];
}
