import { fetch } from "@tauri-apps/plugin-http";
import type { AiAdapter, AiConfig, AiDmxSequence, AiGenerateParams } from "../types";
import { DMX_SEQUENCE_SCHEMA } from "../schema";

/** Local, offline-friendly backend (SRS: "Ollama for local offline gigs"). */
export class OllamaAdapter implements AiAdapter {
  readonly backend = "ollama";

  constructor(private config: AiConfig) {}

  async generate({ systemPrompt, userPrompt }: AiGenerateParams): Promise<AiDmxSequence> {
    const baseUrl = this.config.baseUrl ?? "http://localhost:11434";
    const res = await fetch(`${baseUrl}/api/chat`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        model: this.config.model,
        stream: false,
        messages: [
          { role: "system", content: systemPrompt },
          { role: "user", content: userPrompt },
        ],
        // Ollama accepts a JSON schema object directly via `format` (SRS 2.2).
        format: DMX_SEQUENCE_SCHEMA,
      }),
    });

    if (!res.ok) {
      throw new Error(`Ollama request failed (${res.status}): ${await res.text()}`);
    }

    const data = (await res.json()) as { message?: { content?: string } };
    if (!data.message?.content) {
      throw new Error("Ollama returned no message content");
    }
    return JSON.parse(data.message.content) as AiDmxSequence;
  }
}
