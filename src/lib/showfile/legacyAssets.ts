/** Pure mapping between the pre-showfile `LightAsset` (localStorage) shape and
 * the `.lumen` show file's `LegacyAsset` schema. Used when creating a show
 * from the old localStorage library, and when saving/loading a show file. */
import type { AssetKind, DmxStep, LightAsset } from "../types";
import type { LegacyAsset, LegacyStep } from "./generated";

function stepToLegacy(step: DmxStep): LegacyStep {
  return {
    channels: { ...step.channels } as unknown as Record<string, number>,
    fadeTimeMs: step.fade_time_ms,
    holdTimeMs: step.hold_time_ms,
  };
}

function legacyToStep(step: LegacyStep): DmxStep {
  return {
    channels: { ...step.channels } as unknown as Record<number, number>,
    fade_time_ms: step.fadeTimeMs,
    hold_time_ms: step.holdTimeMs,
  };
}

export function assetsToLegacy(assets: LightAsset[]): LegacyAsset[] {
  return assets.map((asset) => ({
    id: asset.id,
    name: asset.name,
    kind: asset.kind,
    steps: asset.steps.map(stepToLegacy),
    loop: asset.loop,
    source: asset.source,
    createdAt: asset.createdAt,
  }));
}

export function legacyToAssets(legacyAssets: LegacyAsset[]): LightAsset[] {
  return legacyAssets.map((legacy) => ({
    id: legacy.id,
    name: legacy.name,
    kind: legacy.kind as AssetKind,
    steps: legacy.steps.map(legacyToStep),
    loop: legacy.loop,
    source: legacy.source as LightAsset["source"],
    createdAt: legacy.createdAt,
  }));
}
