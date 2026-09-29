import { FilePlus, FolderOpen, Redo2, Save, Undo2 } from "lucide-react";
import { useShowStore } from "../store/useShowStore";
import { useUndoStore } from "../store/useUndoStore";
import { Button } from "./ui/button";
import { Select } from "./ui/input";

/** File (New/Open/Save/Save As/Recent) and Undo/Redo controls for the top bar. */
export function ShowMenu() {
  const { meta, dirty, saving, recent, newShow, openShow, save, saveAs } = useShowStore();
  const { canUndo, canRedo, undoLabel, redoLabel, undo, redo } = useUndoStore();

  return (
    <div className="flex items-center gap-2">
      <Button variant="ghost" size="iconSm" title="New show" onClick={() => void newShow()}>
        <FilePlus />
      </Button>
      <Button variant="ghost" size="iconSm" title="Open show…" onClick={() => void openShow()}>
        <FolderOpen />
      </Button>
      <Button
        variant="ghost"
        size="iconSm"
        title="Save"
        disabled={saving}
        onClick={() => void save()}
      >
        <Save />
      </Button>
      {recent.length > 0 && (
        <Select
          className="w-40"
          title="Recent shows"
          value=""
          onChange={(e) => {
            if (e.target.value) void openShow(e.target.value);
          }}
        >
          <option value="" disabled>
            Recent…
          </option>
          {recent.map((entry) => (
            <option key={entry.path} value={entry.path}>
              {entry.name}
            </option>
          ))}
        </Select>
      )}

      <div className="mx-1 h-5 w-px bg-border" />

      <Button
        variant="ghost"
        size="iconSm"
        title={undoLabel ? `Undo ${undoLabel}` : "Undo"}
        disabled={!canUndo}
        onClick={() => undo()}
      >
        <Undo2 />
      </Button>
      <Button
        variant="ghost"
        size="iconSm"
        title={redoLabel ? `Redo ${redoLabel}` : "Redo"}
        disabled={!canRedo}
        onClick={() => redo()}
      >
        <Redo2 />
      </Button>

      <span className="text-sm text-muted-foreground">
        {meta?.name ?? "Untitled Show"}
        {dirty ? " •" : ""}
      </span>

      <Button variant="ghost" size="sm" onClick={() => void saveAs()}>
        Save As…
      </Button>
    </div>
  );
}
