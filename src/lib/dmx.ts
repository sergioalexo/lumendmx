import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ArtNetNode, DriverConfig, DriverDefaults, UniverseStatus } from "./showfile/generated";

export const UNIVERSE_SIZE = 512;

export interface SerialPortDescriptor {
  port_name: string;
  /** Human-readable label (USB product/manufacturer when available). */
  label: string;
}

export function listSerialPorts(): Promise<SerialPortDescriptor[]> {
  return invoke("list_serial_ports");
}

/** Creates or replaces `universeId`'s driver; stops its previous driver first. */
export function configureUniverse(universeId: number, driver: DriverConfig): Promise<void> {
  return invoke("output_configure_universe", { universeId, driver });
}

export function removeUniverse(universeId: number): Promise<void> {
  return invoke("output_remove_universe", { universeId });
}

/** Overwrites all 512 channels of `universeId`. The universe must already have
 * a driver configured (see `configureUniverse`). */
export function updateUniverseData(universeId: number, channels: Uint8Array): Promise<void> {
  if (channels.length !== UNIVERSE_SIZE) {
    throw new Error(`Expected ${UNIVERSE_SIZE} channels, got ${channels.length}`);
  }
  return invoke("output_update_universe_data", { universeId, channels: Array.from(channels) });
}

export function getUniverseData(universeId: number): Promise<number[]> {
  return invoke("output_get_universe_data", { universeId });
}

export function getUniverseStatus(universeId: number): Promise<UniverseStatus | null> {
  return invoke("output_universe_status", { universeId });
}

export function getAllStatuses(): Promise<UniverseStatus[]> {
  return invoke("output_all_statuses");
}

export function getUniverseConfig(universeId: number): Promise<DriverConfig | null> {
  return invoke("output_get_universe_config", { universeId });
}

export function getDriverDefaults(): Promise<DriverDefaults> {
  return invoke("output_driver_defaults");
}

export function getDriverLabel(driver: DriverConfig): Promise<string> {
  return invoke("output_driver_label", { driver });
}

/** Broadcasts an ArtPoll and collects replies for a few seconds. */
export function discoverArtNetNodes(): Promise<ArtNetNode[]> {
  return invoke("output_discover_artnet_nodes");
}

/** Global safety override across every active universe. */
export function setBlackout(active: boolean): Promise<void> {
  return invoke("output_set_blackout", { active });
}

export function getBlackout(): Promise<boolean> {
  return invoke("output_get_blackout");
}

export function onUniverseStatusChanged(cb: (status: UniverseStatus) => void) {
  return listen<UniverseStatus>("dmx://universe-status-changed", (event) => cb(event.payload));
}
