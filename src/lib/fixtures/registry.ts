import type { FixtureDefinition } from "./types";
import { validateFixtureDefinition } from "./validate";

const modules = import.meta.glob("../../fixtures/library/*.json", { eager: true }) as Record<
  string,
  { default: FixtureDefinition }
>;

export const builtInFixtures: FixtureDefinition[] = Object.entries(modules)
  .map(([path, mod]) => {
    const errors = validateFixtureDefinition(mod.default);
    if (errors.length > 0) {
      console.error(`Invalid bundled fixture ${path}: ${errors.join("; ")}`);
      return null;
    }
    return mod.default;
  })
  .filter((def): def is FixtureDefinition => def !== null);
