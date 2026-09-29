import { describe, expect, it } from "vitest";
import { hexToRgb, hsvToRgb, rgbToHex, rgbToHsv } from "./color";

describe("rgbToHsv / hsvToRgb", () => {
  const cases: [string, { r: number; g: number; b: number }, { h: number; s: number; v: number }][] = [
    ["black", { r: 0, g: 0, b: 0 }, { h: 0, s: 0, v: 0 }],
    ["white", { r: 255, g: 255, b: 255 }, { h: 0, s: 0, v: 100 }],
    ["pure red", { r: 255, g: 0, b: 0 }, { h: 0, s: 100, v: 100 }],
    ["pure green", { r: 0, g: 255, b: 0 }, { h: 120, s: 100, v: 100 }],
    ["pure blue", { r: 0, g: 0, b: 255 }, { h: 240, s: 100, v: 100 }],
  ];

  for (const [label, rgb, hsv] of cases) {
    it(`converts ${label} rgb -> hsv`, () => {
      const result = rgbToHsv(rgb);
      expect(Math.round(result.h)).toBe(hsv.h);
      expect(Math.round(result.s)).toBe(hsv.s);
      expect(Math.round(result.v)).toBe(hsv.v);
    });

    it(`converts ${label} hsv -> rgb`, () => {
      const result = hsvToRgb(hsv);
      expect(result).toEqual(rgb);
    });
  }

  it("round-trips an arbitrary color through hsv and back", () => {
    const original = { r: 12, g: 200, b: 90 };
    const roundTripped = hsvToRgb(rgbToHsv(original));
    // Rounding through hue/sat/val can be off by a shade; allow +-1 per channel.
    expect(Math.abs(roundTripped.r - original.r)).toBeLessThanOrEqual(1);
    expect(Math.abs(roundTripped.g - original.g)).toBeLessThanOrEqual(1);
    expect(Math.abs(roundTripped.b - original.b)).toBeLessThanOrEqual(1);
  });
});

describe("hex conversions", () => {
  it("converts rgb to hex", () => {
    expect(rgbToHex({ r: 255, g: 0, b: 0 })).toBe("#ff0000");
    expect(rgbToHex({ r: 0, g: 0, b: 0 })).toBe("#000000");
    expect(rgbToHex({ r: 18, g: 52, b: 86 })).toBe("#123456");
  });

  it("parses hex to rgb, with or without a leading #", () => {
    expect(hexToRgb("#ff0000")).toEqual({ r: 255, g: 0, b: 0 });
    expect(hexToRgb("00ff00")).toEqual({ r: 0, g: 255, b: 0 });
  });

  it("rejects malformed hex", () => {
    expect(hexToRgb("not-a-color")).toBeNull();
    expect(hexToRgb("#fff")).toBeNull();
    expect(hexToRgb("#gggggg")).toBeNull();
  });
});
