import { describe, expect, it } from "vitest";
import type { LightAsset } from "../types";
import { assetsToLegacy, legacyToAssets } from "./legacyAssets";

describe("legacy asset <-> show file mapping", () => {
  const assets: LightAsset[] = [
    {
      id: "a1",
      name: "Warm Wash",
      kind: "scene",
      steps: [{ channels: { 1: 255, 2: 128 }, fade_time_ms: 500, hold_time_ms: 0 }],
      loop: false,
      source: "manual",
      createdAt: 1700000000000,
    },
    {
      id: "a2",
      name: "Chase",
      kind: "chase",
      steps: [
        { channels: { 1: 0 }, fade_time_ms: 0, hold_time_ms: 200 },
        { channels: { 1: 255 }, fade_time_ms: 0, hold_time_ms: 200 },
      ],
      loop: true,
      source: "ai",
      createdAt: 1700000001000,
    },
  ];

  it("converts to the legacyAssets showfile shape", () => {
    const legacy = assetsToLegacy(assets);
    expect(legacy).toHaveLength(2);
    expect(legacy[0]).toMatchObject({ id: "a1", loop: false, source: "manual" });
    expect(legacy[1].steps[1]).toEqual({ channels: { "1": 255 }, fadeTimeMs: 0, holdTimeMs: 200 });
  });

  it("round-trips assets -> legacy -> assets", () => {
    const roundTripped = legacyToAssets(assetsToLegacy(assets));
    expect(roundTripped).toEqual(assets);
  });

  it("round-trips an empty list", () => {
    expect(legacyToAssets(assetsToLegacy([]))).toEqual([]);
  });
});
