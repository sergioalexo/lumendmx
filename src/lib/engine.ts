import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CommandResult, EngineFrame, Group, PatchIndexEntry, Preset, PresetFamily } from "./showfile/generated";
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

// --- Groups (Phase 5) ---

export function recordGroup(name: string, fixtureNumbers: number[]): Promise<number> {
  return invoke("engine_record_group", { name, fixtureNumbers });
}

export function listGroups(): Promise<Group[]> {
  return invoke("engine_list_groups");
}

export function renameGroup(id: number, name: string): Promise<void> {
  return invoke("engine_rename_group", { id, name });
}

export function deleteGroup(id: number): Promise<void> {
  return invoke("engine_delete_group", { id });
}

export function applyGroupMaster(id: number, percent: number): Promise<string> {
  return invoke("engine_apply_group_master", { id, percent });
}

// --- Presets (Phase 5) ---

export function recordPreset(
  family: PresetFamily,
  name: string,
  fixtureNumbers: number[],
  color?: string,
): Promise<number> {
  return invoke("engine_record_preset", { family, name, fixtureNumbers, color: color ?? null });
}

export function updatePreset(id: number, fixtureNumbers: number[], color?: string): Promise<void> {
  return invoke("engine_update_preset", { id, fixtureNumbers, color: color ?? null });
}

export function renamePreset(id: number, name: string): Promise<void> {
  return invoke("engine_rename_preset", { id, name });
}

export function deletePreset(id: number): Promise<void> {
  return invoke("engine_delete_preset", { id });
}

export function listPresets(family?: PresetFamily): Promise<Preset[]> {
  return invoke("engine_list_presets", { family: family ?? null });
}

export function applyPreset(id: number, fixtureNumbers: number[]): Promise<number> {
  return invoke("engine_apply_preset", { id, fixtureNumbers });
}

export type { CommandResult, EngineFrame, Group, PatchIndexEntry, Preset, PresetFamily } from "./showfile/generated";
