/**
 * Builds the system prompt handed to every adapter (SRS 2.2: "a standard System
 * Prompt containing the fixture map and current DMX state").
 *
 * NOTE: there is no fixture/personality library yet (flagged in the PRD review) --
 * channels are described to the model as raw 1-512 numbers, not "pan" or "gobo".
 * Once fixture profiles exist, inject them here instead of the generic channel list.
 */
export function buildSystemPrompt(universe: Uint8Array): string {
  const active: string[] = [];
  universe.forEach((value, idx) => {
    if (value > 0) active.push(`ch${idx + 1}=${value}`);
  });

  return [
    "You are the lighting-programmer AI inside LumenDMX, a DMX512 control app for live electronic music.",
    "There are 512 raw DMX channels (1-512), each 0-255. No fixture personalities are defined -- treat channel numbers literally as given in the current state or the user's instruction.",
    active.length > 0
      ? `Current non-zero channels: ${active.join(", ")}.`
      : "Current state: all channels are at 0 (blackout).",
    "Respond ONLY with JSON matching the provided schema. A Scene is a single step with loop=false. A Chase/FX loops multiple steps (loop=true). fade_time_ms and hold_time_ms are per-step timings in milliseconds.",
  ].join("\n");
}
