import { fetch } from "@tauri-apps/plugin-http";
import type { AiAdapter, AiConfig, AiDmxSequence, AiGenerateParams } from "../types";
import { DMX_SEQUENCE_SCHEMA } from "../schema";

export class OpenAiAdapter implements AiAdapter {
  readonly backend = "openai";

  constructor(private config: AiConfig) {}

  async generate({ systemPrompt, userPrompt }: AiGenerateParams): Promise<AiDmxSequence> {
    if (!this.config.apiKey) throw new Error("OpenAI adapter requires an API key");

    const res = await fetch("https://api.openai.com/v1/chat/completions", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${this.config.apiKey}`,
      },
      body: JSON.stringify({
        model: this.config.model,
        messages: [
          { role: "system", content: systemPrompt },
          { role: "user", content: userPrompt },
        ],
        response_format: {
          type: "json_schema",
          json_schema: {
            name: "dmx_sequence",
            schema: DMX_SEQUENCE_SCHEMA,
            strict: true,
          },
        },
      }),
    });

    if (!res.ok) {
      throw new Error(`OpenAI request failed (${res.status}): ${await res.text()}`);
    }

    const data = (await res.json()) as {
      choices?: { message?: { content?: string } }[];
    };
    const content = data.choices?.[0]?.message?.content;
    if (!content) throw new Error("OpenAI returned no message content");
    return JSON.parse(content) as AiDmxSequence;
  }
}
