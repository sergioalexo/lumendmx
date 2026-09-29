import type { FixtureChannel, FixtureDefinition, FixtureMode } from "./types";

/** Structural validation for a fixture JSON file -- bundled or user-imported. */
export function validateFixtureDefinition(json: unknown): string[] {
  const errors: string[] = [];
  if (typeof json !== "object" || json === null) {
    return ["not a JSON object"];
  }

  const def = json as Partial<FixtureDefinition>;
  if (!def.id || typeof def.id !== "string") errors.push('missing "id" (string)');
  if (!def.manufacturer || typeof def.manufacturer !== "string") {
    errors.push('missing "manufacturer" (string)');
  }
  if (!def.model || typeof def.model !== "string") errors.push('missing "model" (string)');

  if (!Array.isArray(def.modes) || def.modes.length === 0) {
    errors.push('missing "modes" (non-empty array)');
    return errors;
  }

  def.modes.forEach((mode: FixtureMode, i) => {
    if (!mode || typeof mode !== "object") {
      errors.push(`modes[${i}] is not an object`);
      return;
    }
    if (!mode.name || typeof mode.name !== "string") errors.push(`modes[${i}].name missing`);
    if (typeof mode.channelCount !== "number") errors.push(`modes[${i}].channelCount missing`);
    if (!Array.isArray(mode.channels) || mode.channels.length === 0) {
      errors.push(`modes[${i}].channels missing/empty`);
      return;
    }
    mode.channels.forEach((ch: FixtureChannel, j) => {
      if (typeof ch.offset !== "number" || ch.offset < 1) {
        errors.push(`modes[${i}].channels[${j}].offset invalid`);
      }
      if (!ch.type || typeof ch.type !== "string") {
        errors.push(`modes[${i}].channels[${j}].type invalid`);
      }
    });
  });

  return errors;
}
