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
  | "generic"
  // Added in Phase 4 (fixture library extension) to cover OFL/QLC+ import fidelity.
  | "prism"
  | "focus"
  | "zoom"
  | "iris"
  | "frost"
  | "control"
  | "reset"
  | "lamp"
  | "nofunction";

/** A named physical quantity a capability sweeps across its DMX sub-range,
 * e.g. a pan channel's `{min: 0, max: 540, unit: "deg"}`. Purely descriptive
 * today (shown in the fixture editor / patch detail panel); nothing consumes
 * it for motion control yet. */
export interface PhysicalRange {
  min: number;
  max: number;
  unit: "deg" | "%" | "hz" | "s";
}

/**
 * One DMX sub-range's meaning within a channel — the "per-channel
 * capabilities (DMX range -> meaning)" BUILD_PLAN Phase 4 asks for, modeled
 * after Open Fixture Library's `capability` objects. Optional: a simple
 * channel that does one thing across its whole range (most of our own
 * hand-authored fixtures) just uses `FixtureChannel.type` and doesn't need
 * this — `capabilities` is for channels where the DMX range actually
 * subdivides into different behaviors (strobe speed vs. open, a color/gobo
 * wheel's discrete slots, a function-select channel's auto-programs).
 */
export interface ChannelCapability {
  /** Inclusive DMX range, 0-255. */
  dmxRange: [number, number];
  type: ChannelType;
  label?: string;
  /** A ColorWheel slot's actual color, as a hex string. */
  color?: string;
  /** A Gobo slot's image reference/filename, if known. */
  image?: string;
  physicalRange?: PhysicalRange;
}

export interface FixtureChannel {
  /** 1-based offset within the mode's channel block. */
  offset: number;
  /**
   * This channel's fine (LSB) counterpart's offset, for a 16-bit pair —
   * `offset` is the coarse/MSB channel. Absent for plain 8-bit channels.
   */
  fineOffset?: number;
  /**
   * Which repeated cell/pixel this channel belongs to, for fixtures with
   * multiple individually-controllable segments (e.g. an 8-cell RGB bar).
   * Omitted (or 0) for a whole-fixture channel. This is just enough to group
   * a multi-cell fixture's channels for patching/display — full pixel-map
   * addressing is Phase 7's Pixel Map feature, not this.
   */
  cell?: number;
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
  /** For a simple channel whose one meaning still has a physical range worth
   * showing (e.g. a plain pan channel with no sub-ranges). */
  physicalRange?: PhysicalRange;
  /** Present when this channel's DMX range actually subdivides into distinct
   * behaviors; see `ChannelCapability`. Authoritative over `type` for range-
   * specific meaning when present, e.g. an imported OFL/QLC+ wheel or strobe
   * channel. */
  capabilities?: ChannelCapability[];
}

export interface FixtureMode {
  name: string;
  channelCount: number;
  channels: FixtureChannel[];
}

export interface FixturePhysical {
  weightKg?: number;
  /** [width, height, depth] in mm. */
  dimensionsMm?: [number, number, number];
  powerW?: number;
  /** [min, max] beam angle in degrees. */
  beamAngleDeg?: [number, number];
}

export interface FixtureDefinition {
  /** Unique slug. Convention: filename (minus .json) matches this id. */
  id: string;
  manufacturer: string;
  model: string;
  /** Freeform category, e.g. "LED Par", "Moving Head", "Strobe". */
  type: string;
  physical?: FixturePhysical;
  modes: FixtureMode[];
  notes?: string;
}
