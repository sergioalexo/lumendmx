import { fetch } from "@tauri-apps/plugin-http";
import type { AiAdapter, AiConfig, AiDmxSequence, AiGenerateParams } from "../types";
import { DMX_SEQUENCE_SCHEMA } from "../schema";

const TOOL_NAME = "emit_dmx_sequence";

/** Anthropic has no bare structured-output mode, so we force a single tool call
 * whose input_schema is the DMX sequence schema -- the standard structured-JSON
 * pattern for Claude models. */
export class AnthropicAdapter implements AiAdapter {
  readonly backend = "anthropic";

  constructor(private config: AiConfig) {}

  async generate({ systemPrompt, userPrompt }: AiGenerateParams): Promise<AiDmxSequence> {
    if (!this.config.apiKey) throw new Error("Anthropic adapter requires an API key");

    const res = await fetch("https://api.anthropic.com/v1/messages", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "x-api-key": this.config.apiKey,
        "anthropic-version": "2023-06-01",
        "anthropic-dangerous-direct-browser-access": "true",
      },
      body: JSON.stringify({
        model: this.config.model,
        max_tokens: 2048,
        system: systemPrompt,
        messages: [{ role: "user", content: userPrompt }],
        tools: [
          {
            name: TOOL_NAME,
            description: "Emit the generated DMX scene/chase sequence.",
            input_schema: DMX_SEQUENCE_SCHEMA,
          },
        ],
        tool_choice: { type: "tool", name: TOOL_NAME },
      }),
    });

    if (!res.ok) {
      throw new Error(`Anthropic request failed (${res.status}): ${await res.text()}`);
    }

    const data = (await res.json()) as {
      content?: { type: string; input?: AiDmxSequence }[];
    };
    const toolUse = data.content?.find((block) => block.type === "tool_use");
    if (!toolUse?.input) throw new Error("Anthropic returned no tool_use block");
    return toolUse.input;
  }
}
