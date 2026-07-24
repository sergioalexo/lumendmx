import { useState } from "react";
import { useDmxStore } from "../store/useDmxStore";
import { useAiConfigStore } from "../store/useAiConfigStore";
import { createAiAdapter, buildSystemPrompt, sequenceToAsset } from "../lib/ai/ai-service";
import { AssetRunner } from "../lib/chaseEngine";

interface LogEntry {
  id: string;
  text: string;
  kind: "input" | "ok" | "error";
}

/** Real-time tweaking (SRS 1.2 Live AI Mode): types a natural-language instruction,
 * the AI edits the running universe in place without touching the hardware output
 * loop's cadence. */
export function LiveAiConsole() {
  const { config } = useAiConfigStore();
  const [log, setLog] = useState<LogEntry[]>([
    { id: "boot", text: 'Live AI Mode ready. Try: "make it strobe faster and go deep purple"', kind: "ok" },
  ]);
  const [input, setInput] = useState("");
  const [busy, setBusy] = useState(false);

  const pushLog = (entry: Omit<LogEntry, "id">) =>
    setLog((prev) => [...prev, { ...entry, id: crypto.randomUUID() }].slice(-50));

  const handleSubmit = async () => {
    const command = input.trim();
    if (!command || busy) return;
    setInput("");
    pushLog({ text: `> ${command}`, kind: "input" });
    setBusy(true);

    try {
      const dmx = useDmxStore.getState();
      const adapter = createAiAdapter(config);
      const sequence = await adapter.generate({
        systemPrompt: buildSystemPrompt(dmx.channels),
        userPrompt: command,
      });
      const asset = sequenceToAsset(sequence, sequence.loop ? "chase" : "scene");

      const runner = new AssetRunner(
        asset,
        (channel) => useDmxStore.getState().channels[channel - 1] ?? 0,
        (patch) => void useDmxStore.getState().applyPatch(patch),
      );
      runner.start();

      pushLog({ text: `applied "${asset.name}"`, kind: "ok" });
    } catch (err) {
      pushLog({ text: err instanceof Error ? err.message : String(err), kind: "error" });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex h-56 flex-col border-t border-border bg-popover font-mono text-xs">
      <div className="flex-1 space-y-1 overflow-y-auto p-3">
        {log.map((entry) => (
          <div
            key={entry.id}
            className={
              entry.kind === "input"
                ? "text-foreground/70"
                : entry.kind === "error"
                  ? "text-destructive"
                  : "text-success"
            }
          >
            {entry.text}
          </div>
        ))}
        {busy && <div className="text-muted-foreground">thinking…</div>}
      </div>
      <div className="flex items-center gap-2 border-t border-border px-3 py-2">
        <span className="text-primary">›</span>
        <input
          className="flex-1 bg-transparent text-foreground outline-none placeholder:text-muted-foreground"
          value={input}
          disabled={busy}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void handleSubmit()}
          placeholder="Type a live lighting instruction…"
        />
      </div>
    </div>
  );
}
