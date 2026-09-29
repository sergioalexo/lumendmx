import { create } from "zustand";
import { UndoStack, type UndoableCommand } from "../lib/undo";

const stack = new UndoStack(100);

interface UndoState {
  canUndo: boolean;
  canRedo: boolean;
  undoLabel?: string;
  redoLabel?: string;
  push: (command: UndoableCommand) => void;
  undo: () => void;
  redo: () => void;
}

function snapshot() {
  return {
    canUndo: stack.canUndo,
    canRedo: stack.canRedo,
    undoLabel: stack.undoLabel,
    redoLabel: stack.redoLabel,
  };
}

export const useUndoStore = create<UndoState>((set) => ({
  ...snapshot(),
  push: (command) => {
    stack.push(command);
    set(snapshot());
  },
  undo: () => {
    stack.undo();
    set(snapshot());
  },
  redo: () => {
    stack.redo();
    set(snapshot());
  },
}));
