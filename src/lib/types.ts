/** A single step in a Scene/Chase/FX: a sparse channel patch plus its timing. */
export interface DmxStep {
  /** channel (1-512) -> value (0-255). Unlisted channels are left untouched. */
  channels: Record<number, number>;
  fade_time_ms: number;
  hold_time_ms: number;
}

export type AssetKind = "scene" | "chase" | "fx";

export interface LightAsset {
  id: string;
  name: string;
  kind: AssetKind;
  steps: DmxStep[];
  /** Chases/FX loop their steps; Scenes always apply steps[0] once. */
  loop: boolean;
  source: "ai" | "manual";
  createdAt: number;
}
