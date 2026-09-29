import { useState } from "react";
import { Plus, Trash2, X } from "lucide-react";
import type { ChannelType, FixtureChannel, FixtureDefinition, FixtureMode } from "../lib/fixtures/types";
import { validateFixtureDefinition } from "../lib/fixtures/validate";
import { useFixtureLibraryStore } from "../store/useFixtureLibraryStore";
import { Button } from "./ui/button";
import { Input, Select, Textarea } from "./ui/input";

const CHANNEL_TYPES: ChannelType[] = [
  "dimmer",
  "red",
  "green",
  "blue",
  "white",
  "amber",
  "uv",
  "cyan",
  "magenta",
  "yellow",
  "strobe",
  "function",
  "speed",
  "pan",
  "tilt",
  "colorwheel",
  "gobo",
  "macro",
  "generic",
  "prism",
  "focus",
  "zoom",
  "iris",
  "frost",
  "control",
  "reset",
  "lamp",
  "nofunction",
];

function emptyChannel(offset: number): FixtureChannel {
  return { offset, type: "generic", label: "" };
}

function emptyMode(): FixtureMode {
  return { name: "Mode 1", channelCount: 1, channels: [emptyChannel(1)] };
}

function emptyDefinition(): FixtureDefinition {
  return { id: "", manufacturer: "", model: "", type: "Other", modes: [emptyMode()] };
}

/** Create/edit a custom fixture definition in the UI, with validation
 * (BUILD_PLAN Phase 4). Editing only ever writes to the custom-fixture store
 * (`addCustomFixture` upserts by id) — bundled fixtures aren't mutated in
 * place; "editing" one here saves it as a custom override under the same id. */
export function FixtureEditor({ initial, onClose }: { initial?: FixtureDefinition; onClose: () => void }) {
  const addCustomFixture = useFixtureLibraryStore((s) => s.addCustomFixture);
  const [def, setDef] = useState<FixtureDefinition>(initial ?? emptyDefinition());
  const [modeIndex, setModeIndex] = useState(0);
  const [errors, setErrors] = useState<string[]>([]);

  const mode = def.modes[modeIndex] ?? def.modes[0];

  const updateMode = (patch: Partial<FixtureMode>) => {
    setDef((d) => ({ ...d, modes: d.modes.map((m, i) => (i === modeIndex ? { ...m, ...patch } : m)) }));
  };

  const updateChannel = (index: number, patch: Partial<FixtureChannel>) => {
    updateMode({ channels: mode.channels.map((c, i) => (i === index ? { ...c, ...patch } : c)) });
  };

  const addChannel = () => {
    const channels = [...mode.channels, emptyChannel(mode.channels.length + 1)];
    updateMode({ channels, channelCount: channels.length });
  };

  const removeChannel = (index: number) => {
    const channels = mode.channels.filter((_, i) => i !== index).map((c, i) => ({ ...c, offset: i + 1 }));
    updateMode({ channels, channelCount: channels.length });
  };

  const addMode = () => {
    setDef((d) => ({ ...d, modes: [...d.modes, emptyMode()] }));
    setModeIndex(def.modes.length);
  };

  const removeMode = () => {
    if (def.modes.length <= 1) return;
    setDef((d) => ({ ...d, modes: d.modes.filter((_, i) => i !== modeIndex) }));
    setModeIndex(0);
  };

  const handleSave = () => {
    const toSave: FixtureDefinition = {
      ...def,
      id: def.id.trim() || def.model.toLowerCase().replace(/[^a-z0-9]+/g, "-"),
    };
    const validationErrors = validateFixtureDefinition(toSave);
    if (validationErrors.length > 0) {
      setErrors(validationErrors);
      return;
    }
    addCustomFixture(toSave);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
      <div className="flex max-h-[90vh] w-full max-w-2xl flex-col gap-3 overflow-y-auto rounded-lg border border-border bg-card p-4">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold uppercase tracking-wide">{initial ? "Edit Fixture" : "New Fixture"}</h2>
          <Button size="iconSm" variant="ghost" onClick={onClose}>
            <X />
          </Button>
        </div>

        <div className="grid grid-cols-2 gap-2 text-xs sm:grid-cols-4">
          <label className="flex flex-col gap-1">
            Manufacturer
            <Input value={def.manufacturer} onChange={(e) => setDef((d) => ({ ...d, manufacturer: e.target.value }))} />
          </label>
          <label className="flex flex-col gap-1">
            Model
            <Input value={def.model} onChange={(e) => setDef((d) => ({ ...d, model: e.target.value }))} />
          </label>
          <label className="flex flex-col gap-1">
            Type
            <Input
              value={def.type}
              onChange={(e) => setDef((d) => ({ ...d, type: e.target.value }))}
              placeholder="LED Par, Moving Head…"
            />
          </label>
          <label className="flex flex-col gap-1">
            ID (slug)
            <Input value={def.id} onChange={(e) => setDef((d) => ({ ...d, id: e.target.value }))} placeholder="auto from model" />
          </label>
        </div>

        <Textarea
          className="h-16"
          placeholder="Notes (optional)"
          value={def.notes ?? ""}
          onChange={(e) => setDef((d) => ({ ...d, notes: e.target.value }))}
        />

        <div className="flex flex-wrap items-center gap-2 border-t border-border pt-2">
          <span className="text-xs font-medium text-muted-foreground">Modes</span>
          {def.modes.map((m, i) => (
            <Button key={i} size="sm" variant={i === modeIndex ? "default" : "secondary"} onClick={() => setModeIndex(i)}>
              {m.name || `Mode ${i + 1}`}
            </Button>
          ))}
          <Button size="iconSm" variant="ghost" onClick={addMode} title="Add mode">
            <Plus />
          </Button>
          {def.modes.length > 1 && (
            <Button size="iconSm" variant="ghost" onClick={removeMode} title="Remove this mode">
              <Trash2 />
            </Button>
          )}
        </div>

        <label className="flex flex-col gap-1 text-xs">
          Mode name
          <Input value={mode.name} onChange={(e) => updateMode({ name: e.target.value })} />
        </label>

        <div className="flex flex-col gap-1.5">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-muted-foreground">Channels ({mode.channels.length})</span>
            <Button size="iconSm" variant="ghost" onClick={addChannel} title="Add channel">
              <Plus />
            </Button>
          </div>
          {mode.channels.map((ch, i) => (
            <div key={i} className="flex items-center gap-1.5 text-xs">
              <span className="w-6 shrink-0 text-center text-muted-foreground">{ch.offset}</span>
              <Select className="w-32" value={ch.type} onChange={(e) => updateChannel(i, { type: e.target.value as ChannelType })}>
                {CHANNEL_TYPES.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </Select>
              <Input
                className="flex-1"
                placeholder="Label"
                value={ch.label ?? ""}
                onChange={(e) => updateChannel(i, { label: e.target.value })}
              />
              <Input
                type="number"
                className="w-20"
                placeholder="Default"
                value={ch.defaultValue ?? ""}
                onChange={(e) =>
                  updateChannel(i, { defaultValue: e.target.value === "" ? undefined : Number(e.target.value) })
                }
              />
              <Button size="iconSm" variant="ghost" onClick={() => removeChannel(i)} title="Remove channel">
                <Trash2 />
              </Button>
            </div>
          ))}
        </div>

        {errors.length > 0 && (
          <ul className="list-disc pl-4 text-xs text-destructive">
            {errors.map((e) => (
              <li key={e}>{e}</li>
            ))}
          </ul>
        )}

        <div className="flex justify-end gap-2 border-t border-border pt-2">
          <Button variant="secondary" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={handleSave}>Save fixture</Button>
        </div>
      </div>
    </div>
  );
}
