/** Imports a QLC+ fixture definition (`.qxf`, an XML format) into our
 * `FixtureDefinition` schema. Built against QLC+'s documented
 * `FixtureDefinition` XML structure and a real fixture (Cameo Auro Spot 300)
 * from the QLC+ fixtures repository.
 *
 * QLC+ carries less structured detail than OFL: channel "Group" is a broad
 * category (Colour/Beam/Gobo/...) with no explicit color name or wheel-slot
 * colors, so color/gobo specifics are inferred from the channel's own name
 * where possible and otherwise left generic — an honest limitation of the
 * source format, not a shortcut in this importer.
 */
import type { ChannelCapability, ChannelType, FixtureChannel, FixtureDefinition, FixtureMode, FixturePhysical } from "../types";

interface QlcChannelDef {
  group: string;
  byte: number;
  capabilities: ChannelCapability[];
}

function textOf(parent: Element, tag: string): string | undefined {
  return parent.getElementsByTagName(tag)[0]?.textContent?.trim() || undefined;
}

function colorFromName(name: string): ChannelType | undefined {
  const n = name.toLowerCase();
  const table: [string, ChannelType][] = [
    ["red", "red"],
    ["green", "green"],
    ["blue", "blue"],
    ["white", "white"],
    ["amber", "amber"],
    ["uv", "uv"],
    ["cyan", "cyan"],
    ["magenta", "magenta"],
    ["yellow", "yellow"],
  ];
  return table.find(([needle]) => n.includes(needle))?.[1];
}

function typeFromGroupAndName(group: string, name: string): ChannelType {
  switch (group) {
    case "Intensity":
      return "dimmer";
    case "Colour":
      return colorFromName(name) ?? "colorwheel";
    case "Gobo":
      return "gobo";
    case "Prism":
      return "prism";
    case "Shutter":
      return "strobe";
    case "Pan":
      return "pan";
    case "Tilt":
      return "tilt";
    case "Speed":
      return "speed";
    case "Effect":
      return "macro";
    case "Maintenance":
      return "control";
    case "Nothing":
      return "nofunction";
    case "Beam": {
      const n = name.toLowerCase();
      if (n.includes("focus")) return "focus";
      if (n.includes("zoom")) return "zoom";
      if (n.includes("iris")) return "iris";
      if (n.includes("frost")) return "frost";
      return "generic";
    }
    default:
      return "generic";
  }
}

function typeFromPreset(preset: string, fallback: ChannelType): ChannelType {
  if (preset.startsWith("Shutter")) return "strobe";
  if (preset.startsWith("Pan")) return "pan";
  if (preset.startsWith("Tilt")) return "tilt";
  if (preset.startsWith("ColorWheel") || preset.startsWith("ColorMacro")) return "colorwheel";
  if (preset.startsWith("Gobo")) return "gobo";
  if (preset.startsWith("Prism")) return "prism";
  if (preset.startsWith("Speed")) return "speed";
  if (preset === "NoFunction") return "nofunction";
  return fallback;
}

function parsePhysical(physicalEl: Element | undefined): FixturePhysical | undefined {
  if (!physicalEl) return undefined;
  const dims = physicalEl.getElementsByTagName("Dimensions")[0];
  const lens = physicalEl.getElementsByTagName("Lens")[0];
  const technical = physicalEl.getElementsByTagName("Technical")[0];

  const weight = dims?.getAttribute("Weight");
  const width = dims?.getAttribute("Width");
  const height = dims?.getAttribute("Height");
  const depth = dims?.getAttribute("Depth");
  const degMin = lens?.getAttribute("DegreesMin");
  const degMax = lens?.getAttribute("DegreesMax");
  const power = technical?.getAttribute("PowerConsumption");

  const physical: FixturePhysical = {};
  if (weight) physical.weightKg = Number(weight);
  if (width && height && depth) physical.dimensionsMm = [Number(width), Number(height), Number(depth)];
  if (power) physical.powerW = Number(power);
  if (degMin && degMax) physical.beamAngleDeg = [Number(degMin), Number(degMax)];
  return Object.keys(physical).length > 0 ? physical : undefined;
}

export function importQlcPlus(xml: string): FixtureDefinition {
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  if (doc.getElementsByTagName("parsererror").length > 0) {
    throw new Error("Not a valid QLC+ fixture file (XML parse error)");
  }
  const root = doc.getElementsByTagName("FixtureDefinition")[0];
  if (!root) {
    throw new Error("Not a valid QLC+ fixture file (missing <FixtureDefinition>)");
  }

  const manufacturer = textOf(root, "Manufacturer") ?? "Imported";
  const model = textOf(root, "Model") ?? "Unknown";
  const type = textOf(root, "Type") ?? "Other";

  const channelDefs = new Map<string, QlcChannelDef>();
  for (const channelEl of Array.from(root.children).filter((el) => el.tagName === "Channel")) {
    const name = channelEl.getAttribute("Name");
    if (!name) continue;
    const groupEl = channelEl.getElementsByTagName("Group")[0];
    const group = groupEl?.textContent?.trim() ?? "Nothing";
    const byte = Number(groupEl?.getAttribute("Byte") ?? "0");
    const fallbackType = typeFromGroupAndName(group, name);

    const capEls = Array.from(channelEl.getElementsByTagName("Capability"));
    const capabilities: ChannelCapability[] = capEls.map((capEl) => {
      const min = Number(capEl.getAttribute("Min") ?? "0");
      const max = Number(capEl.getAttribute("Max") ?? "255");
      const preset = capEl.getAttribute("Preset");
      const label = capEl.textContent?.trim() || undefined;
      return {
        dmxRange: [min, max],
        type: preset ? typeFromPreset(preset, fallbackType) : fallbackType,
        label,
      };
    });

    channelDefs.set(name, { group, byte, capabilities });
  }

  let physical: FixturePhysical | undefined;
  const modes: FixtureMode[] = Array.from(root.getElementsByTagName("Mode")).map((modeEl) => {
    if (!physical) physical = parsePhysical(modeEl.getElementsByTagName("Physical")[0]);

    const entries = Array.from(modeEl.getElementsByTagName("Channel"))
      .map((el) => ({ number: Number(el.getAttribute("Number") ?? "0"), name: el.textContent?.trim() ?? "" }))
      .sort((a, b) => a.number - b.number);

    const channels: FixtureChannel[] = [];
    const skip = new Set<number>();

    entries.forEach((entry, index) => {
      if (skip.has(index)) return;
      const def = channelDefs.get(entry.name);
      if (!def) return;

      const fixtureChannel: FixtureChannel = {
        offset: index + 1,
        type: def.capabilities.length > 0 ? def.capabilities[0].type : typeFromGroupAndName(def.group, entry.name),
        label: entry.name,
      };
      if (def.capabilities.length > 1) fixtureChannel.capabilities = def.capabilities;

      // Two consecutive channels sharing a Group with Byte 0 then 1 are a
      // 16-bit coarse/fine pair, by QLC+ convention.
      const next = entries[index + 1];
      const nextDef = next ? channelDefs.get(next.name) : undefined;
      if (def.byte === 0 && nextDef && nextDef.byte === 1 && nextDef.group === def.group) {
        fixtureChannel.fineOffset = index + 2;
        skip.add(index + 1);
      }

      channels.push(fixtureChannel);
    });

    return {
      name: modeEl.getAttribute("Name") ?? "Mode",
      channelCount: channels.length,
      channels,
    };
  });

  const def: FixtureDefinition = {
    id: `${manufacturer}-${model}`.toLowerCase().replace(/[^a-z0-9]+/g, "-"),
    manufacturer,
    model,
    type,
    modes,
  };
  if (physical) def.physical = physical;
  return def;
}
