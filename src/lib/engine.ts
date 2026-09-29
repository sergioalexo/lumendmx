import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CommandResult, EngineFrame, PatchIndexEntry } from "./showfile/generated";
import type { LightAsset } from "./types";

export function setHtpChannels(universe: number, channels: number[]): Promise<void> {
  return invoke("engine_set_htp_channels", { universe, channels });
}

export function setPatchIndex(entries: PatchIndexEntry[]): Promise<void> {
  return invoke("engine_set_patch_index", { entries });
}

/** Immediate (unfaded) programmer set: sliders, color-preset buttons, the
 * Live AI console. */
export function setChannels(universe: number, values: Record<number, number>): Promise<void> {
  return invoke("engine_set_channels", { universe, values });
}

export function getUniverseData(universe: number): Promise<number[]> {
  return invoke("engine_get_universe_data", { universe });
}

export interface LegacyStepPayload {
  channels: Record<string, number>;
  fadeMs: number;
  holdMs: number;
}

/** Starts (or restarts) a Scene/Chase/FX asset as a frame-accurate engine
 * playback layer, replacing the old `chaseEngine.ts` setTimeout loop. */
export function triggerAsset(
  id: string,
  universe: number,
  steps: LegacyStepPayload[],
  looped: boolean,
): Promise<void> {
  return invoke("engine_trigger_asset", { id, universe, steps, looped });
}

export function stopAsset(id: string): Promise<void> {
  return invoke("engine_stop_asset", { id });
}

/** Converts a Scene/Chase/FX's steps into the shape `triggerAsset` expects. */
export function legacyAssetToSteps(asset: LightAsset): LegacyStepPayload[] {
  return asset.steps.map((step) => ({
    channels: { ...step.channels } as unknown as Record<string, number>,
    fadeMs: step.fade_time_ms,
    holdMs: step.hold_time_ms,
  }));
}

export function executeCommand(text: string): Promise<CommandResult> {
  return invoke("engine_execute_command", { text });
}

export function onUniverseFrame(cb: (frame: EngineFrame) => void) {
  return listen<EngineFrame>("engine://universe-frame", (event) => cb(event.payload));
}

export type { CommandResult, EngineFrame, PatchIndexEntry } from "./showfile/generated";
