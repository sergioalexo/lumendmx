/** A reversible edit: `redo` applies it, `undo` reverses it. */
export interface UndoableCommand {
  label: string;
  redo: () => void;
  undo: () => void;
}

/**
 * Generic command-pattern undo/redo stack (Phase 1 foundations). Any store
 * that performs a "programming edit" pushes a command here instead of
 * mutating state directly; the real programmer built in Phase 3 reuses this
 * same stack for selection/attribute/cue edits.
 */
export class UndoStack {
  private undone: UndoableCommand[] = [];
  private done: UndoableCommand[] = [];

  constructor(private readonly capacity = 100) {}

  /** Runs `command.redo()` and pushes it onto the stack. Clears any redo history. */
  push(command: UndoableCommand): void {
    command.redo();
    this.done.push(command);
    if (this.done.length > this.capacity) {
      this.done.shift();
    }
    this.undone = [];
  }

  undo(): boolean {
    const command = this.done.pop();
    if (!command) return false;
    command.undo();
    this.undone.push(command);
    return true;
  }

  redo(): boolean {
    const command = this.undone.pop();
    if (!command) return false;
    command.redo();
    this.done.push(command);
    return true;
  }

  get canUndo(): boolean {
    return this.done.length > 0;
  }

  get canRedo(): boolean {
    return this.undone.length > 0;
  }

  get undoLabel(): string | undefined {
    return this.done.at(-1)?.label;
  }

  get redoLabel(): string | undefined {
    return this.undone.at(-1)?.label;
  }
}
