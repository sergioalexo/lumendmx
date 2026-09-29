import { useRef, useState } from "react";
import { Trash2 } from "lucide-react";
import { useDmxStore } from "../store/useDmxStore";
import { usePatchStore, type PatchedFixture } from "../store/usePatchStore";
import { useAllFixtures, useFixtureLibraryStore } from "../store/useFixtureLibraryStore";
import type { ChannelType, FixtureDefinition } from "../lib/fixtures/types";
import { validateFixtureDefinition } from "../lib/fixtures/validate";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Select } from "./ui/input";

const COLOR_PRESETS: Record<string, ChannelType[]> = {
  White: ["dimmer", "white"],
  Red: ["dimmer", "red"],
  Green: ["dimmer", "green"],
  Blue: ["dimmer", "blue"],
  UV: ["dimmer", "uv"],
  Off: [],
};

function findFixture(all: FixtureDefinition[], id: string) {
  return all.find((f) => f.id === id);
}

export function FixturePatchPanel() {
  const applyPatch = useDmxStore((s) => s.applyPatch);
  const connected = useDmxStore((s) => s.connection.connected);

  const { fixtures, addFixture, removeFixture } = usePatchStore();
  const addCustomFixture = useFixtureLibraryStore((s) => s.addCustomFixture);
  const allFixtures = useAllFixtures();

  const fileInputRef = useRef<HTMLInputElement>(null);
  const [importError, setImportError] = useState<string | null>(null);

  const [newFixtureId, setNewFixtureId] = useState(allFixtures[0]?.id ?? "");
  const [newModeIndex, setNewModeIndex] = useState(0);
  const [newAddress, setNewAddress] = useState(1);
  const [newName, setNewName] = useState("");

  const selectedDef = findFixture(allFixtures, newFixtureId);

  const handleImport = async (file: File) => {
    setImportError(null);
    try {
      const json = JSON.parse(await file.text());
      const errors = validateFixtureDefinition(json);
      if (errors.length > 0) {
        setImportError(errors.join("; "));
        return;
      }
      addCustomFixture(json as FixtureDefinition);
    } catch (err) {
      setImportError(err instanceof Error ? err.message : String(err));
    }
  };

  const applyChannelValues = (fixture: PatchedFixture, values: Record<number, number>) => {
    const patch: Record<number, number> = {};
    for (const [offset, value] of Object.entries(values)) {
      patch[fixture.address + Number(offset) - 1] = value;
    }
    void applyPatch(patch);
  };

  const handleAdd = () => {
    const def = findFixture(allFixtures, newFixtureId);
    const mode = def?.modes[newModeIndex];
    if (!def || !mode) return;

    const fixture = addFixture({
      fixtureId: def.id,
      modeIndex: newModeIndex,
      address: newAddress,
      name: newName.trim() || `${def.model} @${newAddress}`,
    });

    // Pin any function/mode-select channel to its safe "obey DMX" value right
    // away, so the fixture doesn't start out running a built-in auto-program.
    const defaults: Record<number, number> = {};
    for (const ch of mode.channels) {
      if (ch.defaultValue !== undefined) defaults[ch.offset] = ch.defaultValue;
    }
    if (Object.keys(defaults).length > 0) {
      applyChannelValues(fixture, defaults);
    }

    setNewName("");
  };

  return (
    <div className="flex flex-col gap-3 border-t border-border p-4">
      <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        Fixture Patch
      </h2>
      {!connected && (
        <p className="text-xs text-destructive">Connect a DMX interface above first.</p>
      )}

      <div className="flex flex-col gap-2 rounded-md border border-border p-2">
        <div className="grid grid-cols-2 gap-2 text-xs">
          <label className="flex flex-col gap-1">
            Fixture
            <Select
              value={newFixtureId}
              onChange={(e) => {
                setNewFixtureId(e.target.value);
                setNewModeIndex(0);
              }}
            >
              {allFixtures.map((f) => (
                <option key={f.id} value={f.id}>
                  {f.manufacturer} {f.model}
                </option>
              ))}
            </Select>
          </label>
          <label className="flex flex-col gap-1">
            Mode
            <Select value={newModeIndex} onChange={(e) => setNewModeIndex(Number(e.target.value))}>
              {selectedDef?.modes.map((m, i) => (
                <option key={m.name} value={i}>
                  {m.name} ({m.channelCount}ch)
                </option>
              ))}
            </Select>
          </label>
          <label className="flex flex-col gap-1">
            Address
            <Input
              type="number"
              min={1}
              max={512}
              value={newAddress}
              onChange={(e) => setNewAddress(Number(e.target.value) || 1)}
            />
          </label>
          <label className="flex flex-col gap-1">
            Name
            <Input
              placeholder="optional"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
            />
          </label>
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" onClick={handleAdd} disabled={!selectedDef}>
            Add to patch
          </Button>
          <Button size="sm" variant="secondary" onClick={() => fileInputRef.current?.click()}>
            Import fixture JSON
          </Button>
          <input
            ref={fileInputRef}
            type="file"
            accept="application/json"
            className="hidden"
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) void handleImport(file);
              e.target.value = "";
            }}
          />
        </div>
        {importError && <p className="text-xs text-destructive">{importError}</p>}
      </div>

      <div className="flex flex-col gap-3">
        {fixtures.map((instance) => (
          <PatchedFixtureCard
            key={instance.id}
            instance={instance}
            def={findFixture(allFixtures, instance.fixtureId)}
            onRemove={() => removeFixture(instance.id)}
            onApply={(values) => applyChannelValues(instance, values)}
          />
        ))}
      </div>
    </div>
  );
}

function PatchedFixtureCard({
  instance,
  def,
  onRemove,
  onApply,
}: {
  instance: PatchedFixture;
  def: FixtureDefinition | undefined;
  onRemove: () => void;
  onApply: (values: Record<number, number>) => void;
}) {
  const mode = def?.modes[instance.modeIndex];
  const [values, setValues] = useState<Record<number, number>>({});

  if (!def || !mode) {
    return (
      <div className="flex items-center justify-between rounded-md border border-destructive/50 p-2 text-xs text-destructive">
        Unknown fixture "{instance.fixtureId}" (address {instance.address})
        <Button size="iconSm" variant="ghost" onClick={onRemove} title="Remove">
          <Trash2 />
        </Button>
      </div>
    );
  }

  const setValue = (offset: number, value: number) => {
    setValues((v) => ({ ...v, [offset]: value }));
    onApply({ [offset]: value });
  };

  const applyPreset = (types: ChannelType[]) => {
    const next: Record<number, number> = {};
    for (const ch of mode.channels) {
      next[ch.offset] =
        ch.type === "function" ? (ch.defaultValue ?? 0) : types.includes(ch.type) ? 255 : 0;
    }
    setValues((v) => ({ ...v, ...next }));
    onApply(next);
  };

  return (
    <div className="flex flex-col gap-2 rounded-md border border-border p-2">
      <div className="flex items-center justify-between">
        <div>
          <div className="text-sm font-medium">{instance.name}</div>
          <div className="text-xs text-muted-foreground">
            {def.manufacturer} {def.model} · {mode.name} · addr {instance.address}
          </div>
        </div>
        <Button size="iconSm" variant="ghost" onClick={onRemove} title="Remove">
          <Trash2 />
        </Button>
      </div>

      <div className="flex flex-col gap-1.5">
        {mode.channels.map((ch) => (
          <div key={ch.offset} className="flex items-center gap-2">
            <span className="w-16 truncate text-xs text-muted-foreground" title={ch.notes}>
              {ch.label ?? ch.type}
            </span>
            <input
              type="range"
              min={0}
              max={255}
              value={values[ch.offset] ?? ch.defaultValue ?? 0}
              onChange={(e) => setValue(ch.offset, Number(e.target.value))}
              className="h-1.5 flex-1 accent-primary"
            />
            <span className="w-8 text-right text-xs tabular-nums">
              {values[ch.offset] ?? ch.defaultValue ?? 0}
            </span>
          </div>
        ))}
      </div>

      <div className="flex flex-wrap gap-1">
        {Object.entries(COLOR_PRESETS).map(([name, types]) => (
          <Button key={name} size="sm" variant="secondary" onClick={() => applyPreset(types)}>
            {name}
          </Button>
        ))}
      </div>
    </div>
  );
}
