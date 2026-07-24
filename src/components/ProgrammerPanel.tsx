import { useState } from "react";
import { useDmxStore } from "../store/useDmxStore";
import { useAiConfigStore } from "../store/useAiConfigStore";
import { useLibraryStore } from "../store/useLibraryStore";
import { createAiAdapter, buildSystemPrompt, sequenceToAsset } from "../lib/ai/ai-service";
import type { AssetKind } from "../lib/types";
import { Button } from "./ui/button";
import { Textarea } from "./ui/input";
import { cn } from "../lib/utils";

export function ProgrammerPanel() {
  const channels = useDmxStore((s) => s.channels);
  const { config } = useAiConfigStore();
  const addAsset = useLibraryStore((s) => s.addAsset);

  const [kind, setKind] = useState<AssetKind>("scene");
  const [prompt, setPrompt] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleGenerate = async () => {
    if (!prompt.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      const adapter = createAiAdapter(config);
      const sequence = await adapter.generate({
        systemPrompt: buildSystemPrompt(channels),
        userPrompt: prompt,
      });
      addAsset(sequenceToAsset(sequence, kind));
      setPrompt("");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col gap-3 border-t border-border p-4">
      <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        Programmer Mode
      </h2>

      <div className="flex gap-2">
        {(["scene", "chase", "fx"] as AssetKind[]).map((k) => (
          <Button
            key={k}
            size="sm"
            variant={kind === k ? "default" : "secondary"}
            className={cn("uppercase", kind !== k && "text-muted-foreground")}
            onClick={() => setKind(k)}
          >
            {k}
          </Button>
        ))}
      </div>

      <Textarea
        className="h-20 resize-none"
        placeholder="e.g. A slow purple-to-blue breathing wash across channels 1-8"
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
      />

      <Button onClick={() => void handleGenerate()} disabled={busy || !prompt.trim()}>
        {busy ? "Generating…" : `Generate ${kind}`}
      </Button>

      {error && <p className="text-xs text-destructive">{error}</p>}
    </div>
  );
}
