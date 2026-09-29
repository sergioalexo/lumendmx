import { useState } from "react";
import * as engineApi from "../lib/engine";
import { cn } from "../lib/utils";
import { Input } from "./ui/input";

interface HistoryEntry {
  id: string;
  text: string;
  ok: boolean;
}

/** The Onyx-style command line (BUILD_PLAN Phase 3): `1 THRU 8 @ 50`, `GROUP
 * 2 @ 0`, etc. Selection/intensity commands work now; GROUP/cue commands
 * parse but report "not implemented yet" until Phases 5/6 exist. There's no
 * attribute encoder bar yet (progressive UI work, see DECISIONS.md), so this
 * is the only manual programmer surface for now. */
export function CommandLine() {
  const [input, setInput] = useState("");
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [selectionCount, setSelectionCount] = useState(0);

  const run = async () => {
    const text = input.trim();
    if (!text) return;
    setInput("");
    try {
      const result = await engineApi.executeCommand(text);
      setSelectionCount(result.selection.length);
      setHistory((prev) =>
        [...prev, { id: crypto.randomUUID(), text: `${text} → ${result.message}`, ok: result.ok }].slice(-20),
      );
    } catch (err) {
      // Expected outside the Tauri runtime (plain browser preview).
      console.warn("Engine backend unavailable:", err);
    }
  };

  return (
    <div className="flex flex-col gap-2 border-t border-border p-3 font-mono text-xs">
      <div className="flex items-center justify-between text-muted-foreground">
        <h2 className="font-sans text-xs font-semibold uppercase tracking-wide">Command Line</h2>
        <span>{selectionCount} selected</span>
      </div>
      {history.length > 0 && (
        <div className="max-h-24 space-y-0.5 overflow-y-auto">
          {history.map((entry) => (
            <div key={entry.id} className={cn(entry.ok ? "text-success" : "text-destructive")}>
              {entry.text}
            </div>
          ))}
        </div>
      )}
      <div className="flex items-center gap-2">
        <span className="text-primary">›</span>
        <Input
          className="h-7 flex-1 font-mono"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void run()}
          placeholder="1 THRU 8 @ 50"
        />
      </div>
    </div>
  );
}
