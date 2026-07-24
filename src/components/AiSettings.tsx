import { useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { useAiConfigStore } from "../store/useAiConfigStore";
import type { AiBackendId } from "../lib/ai/types";
import { Select, Input } from "./ui/input";

const BACKENDS: { id: AiBackendId; label: string; defaultModel: string }[] = [
  { id: "ollama", label: "Ollama (local)", defaultModel: "llama3.1" },
  { id: "openai", label: "OpenAI", defaultModel: "gpt-4.1" },
  { id: "gemini", label: "Gemini", defaultModel: "gemini-2.0-flash" },
  { id: "anthropic", label: "Anthropic", defaultModel: "claude-sonnet-5" },
];

export function AiSettings() {
  const { config, setConfig } = useAiConfigStore();
  const [open, setOpen] = useState(false);

  return (
    <div className="border-b border-border text-xs">
      <button
        className="flex w-full items-center justify-between px-4 py-2 text-muted-foreground hover:text-foreground"
        onClick={() => setOpen((v) => !v)}
      >
        <span>
          AI backend: <span className="text-foreground">{config.backend}</span> / {config.model}
        </span>
        {open ? <ChevronUp className="h-3.5 w-3.5" /> : <ChevronDown className="h-3.5 w-3.5" />}
      </button>

      {open && (
        <div className="flex flex-col gap-2 px-4 pb-3">
          <Select
            value={config.backend}
            onChange={(e) => {
              const backend = e.target.value as AiBackendId;
              const preset = BACKENDS.find((b) => b.id === backend)!;
              setConfig({ ...config, backend, model: preset.defaultModel });
            }}
          >
            {BACKENDS.map((b) => (
              <option key={b.id} value={b.id}>
                {b.label}
              </option>
            ))}
          </Select>

          <Input
            placeholder="Model"
            value={config.model}
            onChange={(e) => setConfig({ ...config, model: e.target.value })}
          />

          {config.backend === "ollama" ? (
            <Input
              placeholder="Base URL"
              value={config.baseUrl ?? ""}
              onChange={(e) => setConfig({ ...config, baseUrl: e.target.value })}
            />
          ) : (
            <Input
              placeholder="API key"
              type="password"
              value={config.apiKey ?? ""}
              onChange={(e) => setConfig({ ...config, apiKey: e.target.value })}
            />
          )}
        </div>
      )}
    </div>
  );
}
