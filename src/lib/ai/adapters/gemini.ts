import { fetch } from "@tauri-apps/plugin-http";
import type { AiAdapter, AiConfig, AiDmxSequence, AiGenerateParams } from "../types";
import { DMX_SEQUENCE_SCHEMA } from "../schema";

export class GeminiAdapter implements AiAdapter {
  readonly backend = "gemini";

  constructor(private config: AiConfig) {}

  async generate({ systemPrompt, userPrompt }: AiGenerateParams): Promise<AiDmxSequence> {
    if (!this.config.apiKey) throw new Error("Gemini adapter requires an API key");

    const url = `https://generativelanguage.googleapis.com/v1beta/models/${this.config.model}:generateContent?key=${this.config.apiKey}`;
    const res = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        systemInstruction: { parts: [{ text: systemPrompt }] },
        contents: [{ role: "user", parts: [{ text: userPrompt }] }],
        generationConfig: {
          responseMimeType: "application/json",
          responseSchema: DMX_SEQUENCE_SCHEMA,
        },
      }),
    });

    if (!res.ok) {
      throw new Error(`Gemini request failed (${res.status}): ${await res.text()}`);
    }

    const data = (await res.json()) as {
      candidates?: { content?: { parts?: { text?: string }[] } }[];
    };
    const text = data.candidates?.[0]?.content?.parts?.[0]?.text;
    if (!text) throw new Error("Gemini returned no content");
    return JSON.parse(text) as AiDmxSequence;
  }
}
