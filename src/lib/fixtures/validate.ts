import type { ChannelCapability, FixtureChannel, FixtureDefinition, FixtureMode } from "./types";

function validateCapability(cap: ChannelCapability, path: string): string[] {
  const errors: string[] = [];
  if (
    !Array.isArray(cap.dmxRange) ||
    cap.dmxRange.length !== 2 ||
    cap.dmxRange.some((n) => typeof n !== "number" || n < 0 || n > 255)
  ) {
    errors.push(`${path}.dmxRange must be [min, max] within 0-255`);
  } else if (cap.dmxRange[0] > cap.dmxRange[1]) {
    errors.push(`${path}.dmxRange min must be <= max`);
  }
  if (!cap.type || typeof cap.type !== "string") errors.push(`${path}.type invalid`);
  return errors;
}

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
      const path = `modes[${i}].channels[${j}]`;
      if (typeof ch.offset !== "number" || ch.offset < 1) {
        errors.push(`${path}.offset invalid`);
      }
      if (!ch.type || typeof ch.type !== "string") {
        errors.push(`${path}.type invalid`);
      }
      if (ch.fineOffset !== undefined && (typeof ch.fineOffset !== "number" || ch.fineOffset < 1)) {
        errors.push(`${path}.fineOffset invalid`);
      }
      if (ch.capabilities !== undefined) {
        if (!Array.isArray(ch.capabilities) || ch.capabilities.length === 0) {
          errors.push(`${path}.capabilities must be a non-empty array when present`);
        } else {
          ch.capabilities.forEach((cap, k) => errors.push(...validateCapability(cap, `${path}.capabilities[${k}]`)));
        }
      }
    });
  });

  return errors;
}
