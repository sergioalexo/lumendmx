import { useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { useAppUpdateStore } from "../store/useAppUpdateStore";
import { AppUpdateCard } from "./AppUpdateCard";
import { OllamaCard } from "./OllamaCard";

/** Collapsible "Components" section (mirrors MediaFetch's Binaries page pattern,
 * condensed into the sidebar): app self-update + local Ollama model management. */
export function SystemPanel() {
  const [open, setOpen] = useState(false);
  const updateAvailable = Boolean(useAppUpdateStore((s) => s.update));

  return (
    <div className="border-b border-border text-xs">
      <button
        className="flex w-full items-center justify-between px-4 py-2 text-muted-foreground hover:text-foreground"
        onClick={() => setOpen((v) => !v)}
      >
        <span className="flex items-center gap-2">
          System
          {updateAvailable && <span className="h-2 w-2 rounded-full bg-primary" />}
        </span>
        {open ? <ChevronUp className="h-3.5 w-3.5" /> : <ChevronDown className="h-3.5 w-3.5" />}
      </button>

      {open && (
        <div className="flex flex-col gap-2 px-4 pb-3">
          <AppUpdateCard />
          <OllamaCard />
        </div>
      )}
    </div>
  );
}
