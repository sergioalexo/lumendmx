export type ChannelType =
  | "dimmer"
  | "red"
  | "green"
  | "blue"
  | "white"
  | "amber"
  | "uv"
  | "cyan"
  | "magenta"
  | "yellow"
  | "strobe"
  | "function"
  | "speed"
  | "pan"
  | "tilt"
  | "colorwheel"
  | "gobo"
  | "macro"
  | "generic";

export interface FixtureChannel {
  /** 1-based offset within the mode's channel block. */
  offset: number;
  type: ChannelType;
  /** Display label; defaults to `type` if omitted. */
  label?: string;
  /**
   * For channels whose value switches the fixture's overall behavior -- most
   * commonly a "function/mode" channel where some ranges select built-in
   * auto-programs instead of responding to the fixture's other DMX channels --
   * the inclusive [min, max] that keeps it in plain per-channel DMX control.
   */
  manualRange?: [number, number];
  /** Value written into this channel when the fixture is added to the patch, so
   * it starts in manual mode by default. Should fall inside `manualRange`. */
  defaultValue?: number;
  notes?: string;
}

export interface FixtureMode {
  name: string;
  channelCount: number;
  channels: FixtureChannel[];
}

export interface FixtureDefinition {
  /** Unique slug. Convention: filename (minus .json) matches this id. */
  id: string;
  manufacturer: string;
  model: string;
  /** Freeform category, e.g. "LED Par", "Moving Head", "Strobe". */
  type: string;
  modes: FixtureMode[];
  notes?: string;
}
