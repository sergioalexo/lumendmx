/** Imports an Open Fixture Library (OFL, MIT-licensed) fixture JSON file into
 * our `FixtureDefinition` schema.
 *
 * Scope: single (non-matrix) fixtures — the vast majority of pars, moving
 * heads, wash/spot/beam fixtures, etc. OFL's `matrix`/`templateChannels`
 * pixel-group fixtures are rejected with a clear error rather than guessed
 * at: full per-pixel addressing is Phase 7's Pixel Map feature, not this
 * importer's job. Built against OFL's documented fixture format and a real
 * fixture (Cameo Auro Spot 300) fetched from the OFL repository.
 */
import type { ChannelCapability, ChannelType, FixtureChannel, FixtureDefinition, FixtureMode, PhysicalRange } from "../types";

interface OflCapability {
  dmxRange?: [number, number];
  type: string;
  comment?: string;
  color?: string;
  effectName?: string;
  shutterEffect?: string;
  slotNumber?: number;
  wheel?: string;
  angleStart?: string | number;
  angleEnd?: string | number;
  speedStart?: string | number;
  speedEnd?: string | number;
  speed?: string | number;
  distanceStart?: string | number;
  distanceEnd?: string | number;
  openPercentStart?: string | number;
  openPercentEnd?: string | number;
  brightnessStart?: string | number;
  brightnessEnd?: string | number;
}

interface OflChannel {
  defaultValue?: number | string;
  fineChannelAliases?: string[];
  capability?: OflCapability;
  capabilities?: OflCapability[];
}

interface OflWheelSlot {
  type: string;
  name?: string;
  colors?: string[];
  resource?: string;
}

interface OflMode {
  name: string;
  shortName?: string;
  channels: (string | null | object)[];
}

interface OflFixture {
  name: string;
  shortName?: string;
  categories?: string[];
  physical?: {
    dimensions?: [number, number, number];
    weight?: number;
    power?: number;
    lens?: { degreesMinMax?: [number, number] };
  };
  wheels?: Record<string, { slots: OflWheelSlot[] }>;
  availableChannels?: Record<string, OflChannel>;
  templateChannels?: Record<string, unknown>;
  matrix?: unknown;
  modes: OflMode[];
}

const COLOR_NAME_TO_TYPE: Record<string, ChannelType> = {
  Red: "red",
  Green: "green",
  Blue: "blue",
  White: "white",
  Amber: "amber",
  UV: "uv",
  Cyan: "cyan",
  Magenta: "magenta",
  Yellow: "yellow",
};

// Best-effort hex for OFL color names we don't have a dedicated ChannelType
// for; the original name is kept in the capability's label either way.
const COLOR_NAME_TO_HEX: Record<string, string> = {
  Lime: "#66ff00",
  Indigo: "#4b0082",
  WarmWhite: "#ffe4b0",
  ColdWhite: "#eaf6ff",
};

function parsePhysical(value: string | number | undefined): number | null {
  if (value === undefined) return null;
  if (typeof value === "number") return value;
  const match = /^-?\d+(\.\d+)?/.exec(value.trim());
  return match ? Number(match[0]) : null;
}

function physicalRangeOf(
  start: string | number | undefined,
  end: string | number | undefined,
  unit: PhysicalRange["unit"],
): PhysicalRange | undefined {
  const min = parsePhysical(start);
  const max = parsePhysical(end);
  return min !== null && max !== null ? { min, max, unit } : undefined;
}

function wheelSlot(fixture: OflFixture, wheelName: string | undefined, slotNumber: number | undefined) {
  if (!wheelName || slotNumber === undefined) return undefined;
  const wheel = fixture.wheels?.[wheelName];
  // OFL allows half-integer slot numbers for "between two slots"; round down
  // to the nearest real slot for our discrete-slot model.
  return wheel?.slots[Math.floor(slotNumber) - 1];
}

function convertCapability(fixture: OflFixture, cap: OflCapability): ChannelCapability {
  const dmxRange: [number, number] = cap.dmxRange ?? [0, 255];
  let type: ChannelType = "generic";
  let label = cap.comment ?? cap.effectName ?? cap.type;
  let color: string | undefined;
  let image: string | undefined;
  let physicalRange: PhysicalRange | undefined;

  switch (cap.type) {
    case "Intensity":
      type = "dimmer";
      break;
    case "ColorIntensity": {
      const name = cap.color ?? "";
      type = COLOR_NAME_TO_TYPE[name] ?? "generic";
      color = COLOR_NAME_TO_HEX[name];
      label = name || label;
      physicalRange = physicalRangeOf(cap.brightnessStart, cap.brightnessEnd, "%");
      break;
    }
    case "Pan":
      type = "pan";
      physicalRange = physicalRangeOf(cap.angleStart, cap.angleEnd, "deg");
      break;
    case "Tilt":
      type = "tilt";
      physicalRange = physicalRangeOf(cap.angleStart, cap.angleEnd, "deg");
      break;
    case "ShutterStrobe":
      if (cap.shutterEffect === "Closed") {
        type = "nofunction";
        label = "Closed (blackout)";
      } else if (cap.shutterEffect === "Open") {
        type = "strobe";
        label = "Open (no strobe)";
      } else {
        type = "strobe";
        label = cap.shutterEffect ?? label;
        physicalRange = physicalRangeOf(cap.speedStart, cap.speedEnd, "hz");
      }
      break;
    case "WheelSlot": {
      const slot = wheelSlot(fixture, cap.wheel, cap.slotNumber);
      const isColor = slot?.type === "Color" || (fixture.wheels?.[cap.wheel ?? ""]?.slots ?? []).some((s) => s.type === "Color");
      type = isColor ? "colorwheel" : "gobo";
      label = slot?.name ?? label;
      color = slot?.colors?.[0];
      image = slot?.resource;
      break;
    }
    case "WheelRotation":
    case "WheelSlotRotation":
    case "WheelShake":
      type = cap.type === "WheelShake" ? "gobo" : "gobo";
      label = cap.type;
      break;
    case "Prism":
      type = "prism";
      physicalRange = physicalRangeOf(cap.angleStart, cap.angleEnd, "deg");
      break;
    case "Focus":
      type = "focus";
      physicalRange = physicalRangeOf(cap.distanceStart, cap.distanceEnd, "%");
      break;
    case "Zoom":
      type = "zoom";
      break;
    case "Iris":
      type = "iris";
      physicalRange = physicalRangeOf(cap.openPercentStart, cap.openPercentEnd, "%");
      break;
    case "Frost":
      type = "frost";
      break;
    case "Speed":
    case "EffectSpeed":
    case "PanTiltSpeed":
      type = "speed";
      break;
    case "Effect":
      type = "macro";
      break;
    case "Maintenance":
      type = "control";
      break;
    case "NoFunction":
      type = "nofunction";
      label = "No function";
      break;
    default:
      type = "generic";
      label = cap.type;
  }

  const capability: ChannelCapability = { dmxRange, type, label };
  if (color) capability.color = color;
  if (image) capability.image = image;
  if (physicalRange) capability.physicalRange = physicalRange;
  return capability;
}

function primaryTypeOf(capabilities: ChannelCapability[]): ChannelType {
  // Prefer the widest non-NoFunction capability as the channel's "headline"
  // type, for code that only looks at `FixtureChannel.type`.
  const real = capabilities.filter((c) => c.type !== "nofunction");
  const widest = (real.length > 0 ? real : capabilities).reduce((a, b) =>
    b.dmxRange[1] - b.dmxRange[0] > a.dmxRange[1] - a.dmxRange[0] ? b : a,
  );
  return widest.type;
}

export function importOfl(json: unknown): FixtureDefinition {
  const fixture = json as OflFixture;
  if (!fixture || typeof fixture !== "object" || !Array.isArray(fixture.modes)) {
    throw new Error("Not a valid Open Fixture Library JSON file");
  }
  if (fixture.matrix || fixture.templateChannels) {
    throw new Error(
      "This is a matrix/pixel fixture (multiple individually-addressed pixel groups), which isn't supported yet — that needs Phase 7's Pixel Map feature.",
    );
  }

  const available = fixture.availableChannels ?? {};

  // Map each fine-channel alias back to the coarse channel key that declares
  // it, so mode channel lists (which list the fine alias as its own entry)
  // don't get a duplicate FixtureChannel for it.
  const fineAliasToCoarse = new Map<string, string>();
  for (const [key, ch] of Object.entries(available)) {
    for (const alias of ch.fineChannelAliases ?? []) {
      fineAliasToCoarse.set(alias, key);
    }
  }

  const modes: FixtureMode[] = fixture.modes.map((mode) => {
    const keys = mode.channels.filter((c): c is string => typeof c === "string");
    const channels: FixtureChannel[] = [];

    keys.forEach((key, index) => {
      if (fineAliasToCoarse.has(key)) return; // represented via the coarse channel's fineOffset
      const channel = available[key];
      if (!channel) return; // unknown/null channel slot

      const capsRaw = channel.capabilities ?? (channel.capability ? [channel.capability] : []);
      const capabilities = capsRaw.map((c) => convertCapability(fixture, c));
      const fineAlias = channel.fineChannelAliases?.[0];
      const fineOffset = fineAlias ? keys.indexOf(fineAlias) + 1 : undefined;

      const fixtureChannel: FixtureChannel = {
        offset: index + 1,
        type: capabilities.length > 0 ? primaryTypeOf(capabilities) : "generic",
        label: key,
      };
      if (fineOffset && fineOffset > 0) fixtureChannel.fineOffset = fineOffset;
      if (capabilities.length > 1) fixtureChannel.capabilities = capabilities;
      else if (capabilities.length === 1 && capabilities[0].physicalRange) {
        fixtureChannel.physicalRange = capabilities[0].physicalRange;
      }
      if (typeof channel.defaultValue === "number") fixtureChannel.defaultValue = channel.defaultValue;

      channels.push(fixtureChannel);
    });

    return { name: mode.name, channelCount: keys.length, channels };
  });

  const def: FixtureDefinition = {
    id: (fixture.shortName ?? fixture.name).toLowerCase().replace(/[^a-z0-9]+/g, "-"),
    manufacturer: "Imported",
    model: fixture.name,
    type: fixture.categories?.[0] ?? "Other",
    modes,
  };

  if (fixture.physical) {
    def.physical = {
      weightKg: fixture.physical.weight,
      dimensionsMm: fixture.physical.dimensions,
      powerW: fixture.physical.power,
      beamAngleDeg: fixture.physical.lens?.degreesMinMax,
    };
  }

  return def;
}
