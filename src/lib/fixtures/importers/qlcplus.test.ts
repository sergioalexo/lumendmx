// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { importQlcPlus } from "./qlcplus";

/** A representative QLC+ .qxf excerpt, built from QLC+'s documented
 * FixtureDefinition XML structure (Channel/Group/Capability, Mode/Physical)
 * and the same real fixture (Cameo Auro Spot 300) used for the OFL importer's
 * tests, so the two importers can be compared on equivalent input. */
const QXF = `<?xml version="1.0" encoding="UTF-8"?>
<FixtureDefinition xmlns="http://www.qlcplus.org/FixtureDefinition">
  <Manufacturer>Cameo</Manufacturer>
  <Model>Auro Spot 300</Model>
  <Type>Moving Head</Type>
  <Channel Name="Dimmer">
    <Group Byte="0">Intensity</Group>
  </Channel>
  <Channel Name="Pan">
    <Group Byte="0">Pan</Group>
  </Channel>
  <Channel Name="Pan fine">
    <Group Byte="1">Pan</Group>
  </Channel>
  <Channel Name="Tilt">
    <Group Byte="0">Tilt</Group>
  </Channel>
  <Channel Name="Strobe">
    <Group Byte="0">Shutter</Group>
    <Capability Min="0" Max="10" Preset="ShutterOpen">Strobe open</Capability>
    <Capability Min="11" Max="255">Strobe, slow to fast</Capability>
  </Channel>
  <Channel Name="Red">
    <Group Byte="0">Colour</Group>
  </Channel>
  <Channel Name="Color Wheel">
    <Group Byte="0">Colour</Group>
    <Capability Min="0" Max="15">Open</Capability>
    <Capability Min="16" Max="31">Red</Capability>
  </Channel>
  <Mode Name="15 Channel">
    <Physical>
      <Bulb Type="LED" Lumens="4371" ColourTemperature="9300"/>
      <Dimensions Weight="8.75" Width="285" Height="485" Depth="180"/>
      <Lens Name="Other" DegreesMin="12" DegreesMax="12"/>
      <Focus Type="Head" PanMax="630" TiltMax="235"/>
      <Technical PowerConsumption="320" DmxConnector="3-pin and 5-pin"/>
    </Physical>
    <Channel Number="0">Pan</Channel>
    <Channel Number="1">Pan fine</Channel>
    <Channel Number="2">Tilt</Channel>
    <Channel Number="3">Dimmer</Channel>
    <Channel Number="4">Strobe</Channel>
    <Channel Number="5">Color Wheel</Channel>
  </Mode>
</FixtureDefinition>`;

describe("importQlcPlus", () => {
  it("reads manufacturer/model/type", () => {
    const def = importQlcPlus(QXF);
    expect(def.manufacturer).toBe("Cameo");
    expect(def.model).toBe("Auro Spot 300");
    expect(def.type).toBe("Moving Head");
  });

  it("builds the mode's channel list in Number order", () => {
    const def = importQlcPlus(QXF);
    const mode = def.modes[0];
    expect(mode.name).toBe("15 Channel");
    expect(mode.channels.map((c) => c.label)).toEqual(["Pan", "Tilt", "Dimmer", "Strobe", "Color Wheel"]);
  });

  it("resolves a Byte 0/1 pair as a 16-bit fine channel", () => {
    const def = importQlcPlus(QXF);
    const pan = def.modes[0].channels.find((c) => c.label === "Pan");
    expect(pan?.offset).toBe(1);
    expect(pan?.fineOffset).toBe(2);
    expect(pan?.type).toBe("pan");
    // "Pan fine" must not become its own channel.
    expect(def.modes[0].channels.some((c) => c.label === "Pan fine")).toBe(false);
  });

  it("converts a Shutter group with a Preset-tagged capability", () => {
    const def = importQlcPlus(QXF);
    const strobe = def.modes[0].channels.find((c) => c.label === "Strobe");
    expect(strobe?.capabilities).toHaveLength(2);
    expect(strobe?.capabilities?.[0]).toMatchObject({ dmxRange: [0, 10], type: "strobe", label: "Strobe open" });
    expect(strobe?.capabilities?.[1]).toMatchObject({ dmxRange: [11, 255], type: "strobe" });
  });

  it("infers a specific color from the channel name when the group is generic Colour", () => {
    const def = importQlcPlus(QXF);
    // "Red" isn't referenced by the mode above, so import it directly via a
    // minimal single-mode document to check name inference in isolation.
    const singleChannelDoc = QXF.replace(
      '<Channel Number="5">Color Wheel</Channel>',
      '<Channel Number="5">Color Wheel</Channel><Channel Number="6">Red</Channel>',
    );
    const def2 = importQlcPlus(singleChannelDoc);
    const red = def2.modes[0].channels.find((c) => c.label === "Red");
    expect(red?.type).toBe("red");
    // A Colour-group channel with no color-name match (Color Wheel here)
    // falls back to colorwheel.
    const colorWheel = def.modes[0].channels.find((c) => c.label === "Color Wheel");
    expect(colorWheel?.type).toBe("colorwheel");
  });

  it("carries over Physical dimensions/power/beam angle", () => {
    const def = importQlcPlus(QXF);
    expect(def.physical).toEqual({
      weightKg: 8.75,
      dimensionsMm: [285, 485, 180],
      powerW: 320,
      beamAngleDeg: [12, 12],
    });
  });

  it("rejects non-QLC+ XML", () => {
    expect(() => importQlcPlus("<NotAFixture/>")).toThrow(/valid QLC\+/);
  });

  it("rejects malformed XML", () => {
    expect(() => importQlcPlus("<Fixture")).toThrow();
  });
});
