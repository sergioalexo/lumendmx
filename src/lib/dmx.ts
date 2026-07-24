import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const UNIVERSE_SIZE = 512;

export interface ConnectionStatus {
  connected: boolean;
  port: string | null;
  error: string | null;
}

export function listSerialPorts(): Promise<string[]> {
  return invoke("list_serial_ports");
}

export function connectSerialPort(portName: string): Promise<void> {
  return invoke("connect_serial_port", { portName });
}

export function disconnectSerialPort(): Promise<void> {
  return invoke("disconnect_serial_port");
}

export function getConnectionStatus(): Promise<ConnectionStatus> {
  return invoke("connection_status");
}

/** Overwrites the entire 512-channel universe on the hardware output thread. */
export function updateUniverse(channels: Uint8Array): Promise<void> {
  if (channels.length !== UNIVERSE_SIZE) {
    throw new Error(`Expected ${UNIVERSE_SIZE} channels, got ${channels.length}`);
  }
  return invoke("update_universe", { channels: Array.from(channels) });
}

export function getUniverse(): Promise<number[]> {
  return invoke("get_universe");
}

export function setBlackout(active: boolean): Promise<void> {
  return invoke("set_blackout", { active });
}

export function getBlackout(): Promise<boolean> {
  return invoke("get_blackout");
}

export function onConnectionChanged(cb: (status: ConnectionStatus) => void) {
  return listen<ConnectionStatus>("dmx://connection-changed", (event) => cb(event.payload));
}
