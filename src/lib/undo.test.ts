import { describe, expect, it } from "vitest";
import { UndoStack } from "./undo";

describe("UndoStack", () => {
  it("applies redo immediately on push, and undo reverses it", () => {
    let value = 0;
    const stack = new UndoStack();

    stack.push({ label: "set to 5", redo: () => (value = 5), undo: () => (value = 0) });
    expect(value).toBe(5);
    expect(stack.canUndo).toBe(true);
    expect(stack.canRedo).toBe(false);

    stack.undo();
    expect(value).toBe(0);
    expect(stack.canUndo).toBe(false);
    expect(stack.canRedo).toBe(true);

    stack.redo();
    expect(value).toBe(5);
  });

  it("clears redo history on a new push", () => {
    let value = 0;
    const stack = new UndoStack();
    stack.push({ label: "a", redo: () => (value = 1), undo: () => (value = 0) });
    stack.undo();
    stack.push({ label: "b", redo: () => (value = 2), undo: () => (value = 0) });

    expect(stack.canRedo).toBe(false);
    expect(value).toBe(2);
  });

  it("undo/redo on an empty stack are no-ops", () => {
    const stack = new UndoStack();
    expect(stack.undo()).toBe(false);
    expect(stack.redo()).toBe(false);
  });

  it("caps history at the given capacity", () => {
    const applied: number[] = [];
    const stack = new UndoStack(3);
    for (let i = 1; i <= 5; i++) {
      stack.push({ label: `${i}`, redo: () => applied.push(i), undo: () => applied.pop() });
    }
    expect(stack.canUndo).toBe(true);
    let undone = 0;
    while (stack.undo()) undone++;
    expect(undone).toBe(3);
    expect(applied).toEqual([1, 2]);
  });
});
