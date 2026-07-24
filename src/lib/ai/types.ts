/** Wire format returned by every AI backend (SRS 2.2: strict JSON, channel arrays). */
export interface AiChannelValue {
  channel: number; // 1-512
  value: number; // 0-255
}

export interface AiStep {
  fade_time_ms: number;
  hold_time_ms: number;
  channels: AiChannelValue[];
}

export interface AiDmxSequence {
  name: string;
  loop: boolean;
  steps: AiStep[];
}

export interface AiGenerateParams {
  /** Fixture map + current DMX state, per SRS 2.2's adapter interface. */
  systemPrompt: string;
  userPrompt: string;
}

export interface AiAdapter {
  readonly backend: string;
  generate(params: AiGenerateParams): Promise<AiDmxSequence>;
}

export type AiBackendId = "ollama" | "openai" | "gemini" | "anthropic";

export interface AiConfig {
  backend: AiBackendId;
  model: string;
  apiKey?: string;
  /** Ollama only; defaults to http://localhost:11434 */
  baseUrl?: string;
}
