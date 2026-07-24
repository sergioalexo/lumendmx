/**
 * JSON Schema for AiDmxSequence, shared across every adapter (SRS 2.2 "The Schema").
 * Kept to a plain object/array/number/string vocabulary so it works unmodified as:
 *  - OpenAI `response_format.json_schema.schema`
 *  - Gemini `responseSchema`
 *  - Ollama `format` (passed straight through)
 *  - Anthropic forced-tool-use `input_schema`
 */
export const DMX_SEQUENCE_SCHEMA = {
  type: "object",
  properties: {
    name: { type: "string" },
    loop: { type: "boolean" },
    steps: {
      type: "array",
      items: {
        type: "object",
        properties: {
          fade_time_ms: { type: "integer", minimum: 0 },
          hold_time_ms: { type: "integer", minimum: 0 },
          channels: {
            type: "array",
            items: {
              type: "object",
              properties: {
                channel: { type: "integer", minimum: 1, maximum: 512 },
                value: { type: "integer", minimum: 0, maximum: 255 },
              },
              required: ["channel", "value"],
            },
          },
        },
        required: ["fade_time_ms", "hold_time_ms", "channels"],
      },
    },
  },
  required: ["name", "loop", "steps"],
} as const;
