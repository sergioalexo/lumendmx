import { describe, expect, it } from "vitest";
import { importOfl } from "./ofl";

/** A trimmed but structurally faithful excerpt of a real OFL fixture (Cameo
 * Auro Spot 300, fetched from the Open Fixture Library repository) — enough
 * to exercise fine channels, multi-capability channels, and wheel slots. */
const CAMEO_AURO_SPOT_300 = {
  name: "Auro Spot 300",
  shortName: "CLAS300",
  categories: ["Moving Head", "Color Changer"],
  physical: {
    dimensions: [285, 485, 180],
    weight: 8.75,
    power: 320,
    lens: { degreesMinMax: [12, 12] },
  },
  wheels: {
    "Color Wheel": {
      slots: [
        { type: "Open" },
        { type: "Color", name: "Red", colors: ["#ff0000"] },
        { type: "Color", name: "Blue", colors: ["#0000ff"] },
      ],
    },
    "Gobo Wheel 1": {
      slots: [{ type: "Open" }, { type: "Gobo" }, { type: "Gobo" }],
    },
  },
  availableChannels: {
    Pan: {
      fineChannelAliases: ["Pan fine"],
      defaultValue: "50%",
      capability: { type: "Pan", angleStart: "0deg", angleEnd: "630deg" },
    },
    "Pan fine": {},
    Tilt: {
      fineChannelAliases: ["Tilt fine"],
      capability: { type: "Tilt", angleStart: "0deg", angleEnd: "235deg" },
    },
    "Tilt fine": {},
    Dimmer: {
      defaultValue: 0,
      capability: { type: "Intensity" },
    },
    Strobe: {
      defaultValue: 0,
      capabilities: [
        { dmxRange: [0, 10] as [number, number], type: "ShutterStrobe", shutterEffect: "Open" },
        {
          dmxRange: [11, 255] as [number, number],
          type: "ShutterStrobe",
          shutterEffect: "Strobe",
          speedStart: "1Hz",
          speedEnd: "20Hz",
        },
      ],
    },
    "Color Wheel": {
      defaultValue: 0,
      capabilities: [
        { dmxRange: [0, 5] as [number, number], type: "WheelSlot", slotNumber: 1 },
        { dmxRange: [6, 127] as [number, number], type: "WheelSlot", slotNumber: 2 },
        { dmxRange: [128, 255] as [number, number], type: "WheelSlot", slotNumber: 3 },
      ],
    },
    "Gobo Wheel 1": {
      defaultValue: 0,
      capabilities: [
        { dmxRange: [0, 5] as [number, number], type: "WheelSlot", slotNumber: 1, wheel: "Gobo Wheel 1" },
        { dmxRange: [6, 127] as [number, number], type: "WheelSlot", slotNumber: 2, wheel: "Gobo Wheel 1" },
      ],
    },
    Prism: {
      capabilities: [
        { dmxRange: [0, 10] as [number, number], type: "NoFunction" },
        { dmxRange: [11, 255] as [number, number], type: "Prism", angleStart: "0deg", angleEnd: "360deg" },
      ],
    },
  },
  modes: [
    { name: "5-channel", channels: ["Dimmer", "Strobe", null] },
    {
      name: "15-channel",
      channels: ["Pan", "Pan fine", "Tilt", "Tilt fine", "Dimmer", "Strobe", "Color Wheel", "Gobo Wheel 1", "Prism"],
    },
  ],
};

// The "Color Wheel" capability's `wheel` isn't set in the fixture above
// (OFL infers it from the channel name when omitted); the importer looks it
// up by channel key in that case.
function withWheelBackref() {
  const fixture = structuredClone(CAMEO_AURO_SPOT_300);
  for (const cap of fixture.availableChannels["Color Wheel"].capabilities) {
    (cap as { wheel?: string }).wheel = "Color Wheel";
  }
  return fixture;
}

describe("importOfl", () => {
  it("converts modes and channel offsets", () => {
    const def = importOfl(withWheelBackref());
    expect(def.modes.map((m) => m.name)).toEqual(["5-channel", "15-channel"]);
    expect(def.modes[0].channelCount).toBe(2); // the trailing null slot isn't a real channel
  });

  it("resolves a fine channel's offset within its mode", () => {
    const def = importOfl(withWheelBackref());
    const mode = def.modes[1];
    const pan = mode.channels.find((c) => c.label === "Pan");
    expect(pan?.offset).toBe(1);
    expect(pan?.fineOffset).toBe(2); // "Pan fine" is the 2nd channel in this mode
    expect(pan?.type).toBe("pan");
    expect(pan?.physicalRange).toEqual({ min: 0, max: 630, unit: "deg" });

    // "Pan fine"/"Tilt fine" must not appear as their own channels.
    expect(mode.channels.some((c) => c.label === "Pan fine")).toBe(false);
  });

  it("converts a multi-capability strobe channel", () => {
    const def = importOfl(withWheelBackref());
    const strobe = def.modes[1].channels.find((c) => c.label === "Strobe");
    expect(strobe?.capabilities).toHaveLength(2);
    expect(strobe?.capabilities?.[0]).toMatchObject({ dmxRange: [0, 10], type: "strobe", label: "Open (no strobe)" });
    expect(strobe?.capabilities?.[1]).toMatchObject({ dmxRange: [11, 255], type: "strobe" });
    expect(strobe?.capabilities?.[1].physicalRange).toEqual({ min: 1, max: 20, unit: "hz" });
  });

  it("resolves color wheel slots to named colors", () => {
    const def = importOfl(withWheelBackref());
    const colorWheel = def.modes[1].channels.find((c) => c.label === "Color Wheel");
    expect(colorWheel?.type).toBe("colorwheel");
    expect(colorWheel?.capabilities?.[1]).toMatchObject({ type: "colorwheel", label: "Red", color: "#ff0000" });
    expect(colorWheel?.capabilities?.[2]).toMatchObject({ type: "colorwheel", label: "Blue", color: "#0000ff" });
  });

  it("resolves gobo wheel slots without a color as gobo, not colorwheel", () => {
    const def = importOfl(withWheelBackref());
    const gobo = def.modes[1].channels.find((c) => c.label === "Gobo Wheel 1");
    expect(gobo?.type).toBe("gobo");
  });

  it("maps NoFunction and Prism capabilities", () => {
    const def = importOfl(withWheelBackref());
    const prism = def.modes[1].channels.find((c) => c.label === "Prism");
    expect(prism?.capabilities?.[0].type).toBe("nofunction");
    expect(prism?.capabilities?.[1]).toMatchObject({ type: "prism", physicalRange: { min: 0, max: 360, unit: "deg" } });
  });

  it("carries over physical properties", () => {
    const def = importOfl(withWheelBackref());
    expect(def.physical).toEqual({
      weightKg: 8.75,
      dimensionsMm: [285, 485, 180],
      powerW: 320,
      beamAngleDeg: [12, 12],
    });
    expect(def.type).toBe("Moving Head");
  });

  it("rejects matrix fixtures with a clear error", () => {
    const matrixFixture = { ...CAMEO_AURO_SPOT_300, matrix: { pixelCount: [4, 1, 1] } };
    expect(() => importOfl(matrixFixture)).toThrow(/Pixel Map/);
  });

  it("rejects non-fixture JSON", () => {
    expect(() => importOfl({ foo: "bar" })).toThrow(/valid Open Fixture Library/);
  });
});
