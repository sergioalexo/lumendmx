import type { AiAdapter, AiConfig, AiDmxSequence } from "./types";
import { OllamaAdapter } from "./adapters/ollama";
import { OpenAiAdapter } from "./adapters/openai";
import { GeminiAdapter } from "./adapters/gemini";
import { AnthropicAdapter } from "./adapters/anthropic";
import type { AssetKind, DmxStep, LightAsset } from "../types";

export { buildSystemPrompt } from "./prompt";
export type { AiConfig, AiDmxSequence } from "./types";

/** Factory (SRS 2.2: pluggable adapter pattern so backends swap seamlessly). */
export function createAiAdapter(config: AiConfig): AiAdapter {
  switch (config.backend) {
    case "ollama":
      return new OllamaAdapter(config);
    case "openai":
      return new OpenAiAdapter(config);
    case "gemini":
      return new GeminiAdapter(config);
    case "anthropic":
      return new AnthropicAdapter(config);
  }
}

/** Converts the wire format (channel arrays) into the internal sparse-record format
 * the chase engine and library store use. */
export function sequenceToAsset(
  sequence: AiDmxSequence,
  kind: AssetKind,
  idGenerator: () => string = () => crypto.randomUUID(),
): LightAsset {
  const steps: DmxStep[] = sequence.steps.map((step) => ({
    fade_time_ms: step.fade_time_ms,
    hold_time_ms: step.hold_time_ms,
    channels: Object.fromEntries(step.channels.map((c) => [c.channel, c.value])),
  }));

  return {
    id: idGenerator(),
    name: sequence.name,
    kind,
    steps,
    loop: sequence.loop,
    source: "ai",
    createdAt: Date.now(),
  };
}
