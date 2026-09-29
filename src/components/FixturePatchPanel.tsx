import { useRef, useState } from "react";
import { Trash2, X } from "lucide-react";
import { useDmxStore } from "../store/useDmxStore";
import { usePatchStore, type PatchedFixture } from "../store/usePatchStore";
import { useAllFixtures, useFixtureLibraryStore } from "../store/useFixtureLibraryStore";
import type { ChannelType, FixtureDefinition, FixtureMode } from "../lib/fixtures/types";
import { validateFixtureDefinition } from "../lib/fixtures/validate";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Select } from "./ui/input";
import { cn } from "../lib/utils";

const COLOR_PRESETS: Record<string, ChannelType[]> = {
  White: ["dimmer", "white"],
  Red: ["dimmer", "red"],
  Green: ["dimmer", "green"],
  Blue: ["dimmer", "blue"],
  UV: ["dimmer", "uv"],
  Off: [],
};

interface ResolvedFixture {
  instance: PatchedFixture;
  def: FixtureDefinition;
  mode: FixtureMode;
}

function findFixture(all: FixtureDefinition[], id: string) {
  return all.find((f) => f.id === id);
}

function resolve(instance: PatchedFixture, all: FixtureDefinition[]): ResolvedFixture | null {
  const def = findFixture(all, instance.fixtureId);
  const mode = def?.modes[instance.modeIndex];
  return def && mode ? { instance, def, mode } : null;
}

/** Reads a patched fixture's live channel values straight from the shared DMX
 * universe -- the single source of truth -- instead of a separate copy of state,
 * so the grid tile's color and the detail sliders can never drift from what's
 * actually being sent to the fixture. */
function useFixtureChannelValues(instance: PatchedFixture, mode: FixtureMode | undefined) {
  const channels = useDmxStore((s) => s.channels);
  const values: Record<number, number> = {};
  if (!mode) return values;
  for (const ch of mode.channels) {
    const idx = instance.address + ch.offset - 1;
    values[ch.offset] = idx >= 0 && idx < channels.length ? channels[idx] : 0;
  }
  return values;
}

/** Approximates what the fixture is actually outputting right now, like a lighting
 * console's patch/fixture grid does, so the grid reads as a live rig overview. */
function swatchColor(mode: FixtureMode, values: Record<number, number>): string {
  const offsetFor = (t: ChannelType) => mode.channels.find((c) => c.type === t)?.offset;
  const valueOf = (t: ChannelType) => {
    const offset = offsetFor(t);
    return offset === undefined ? 0 : (values[offset] ?? 0);
  };

  const hasDimmer = offsetFor("dimmer") !== undefined;
  const dimmer = hasDimmer ? valueOf("dimmer") / 255 : 1;

  let r = valueOf("red");
  let g = valueOf("green");
  let b = valueOf("blue");
  const white = valueOf("white");
  // Fold white into RGB so a fixture with only a White channel still shows as
  // white on the tile instead of black.
  r = Math.max(r, white);
  g = Math.max(g, white);
  b = Math.max(b, white);

  if (r === 0 && g === 0 && b === 0 && hasDimmer) {
    // No color channels at all (a dimmer-only fixture) -- show a plain warm glow
    // scaled by the dimmer instead of always rendering black.
    r = g = b = 255;
  }

  r = Math.round(r * dimmer);
  g = Math.round(g * dimmer);
  b = Math.round(b * dimmer);
  return `rgb(${r}, ${g}, ${b})`;
}

export function FixturePatchPanel() {
  const connected = useDmxStore((s) => s.status.connected);
  const applyPatch = useDmxStore((s) => s.applyPatch);
  const liveChannels = useDmxStore((s) => s.channels);

  const { fixtures, addFixture, removeFixture } = usePatchStore();
  const addCustomFixture = useFixtureLibraryStore((s) => s.addCustomFixture);
  const allFixtures = useAllFixtures();

  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const selected = fixtures.filter((f) => selectedIds.includes(f.id));
  const resolvedSelected = selected
    .map((instance) => resolve(instance, allFixtures))
    .filter((r): r is ResolvedFixture => r !== null);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const [importError, setImportError] = useState<string | null>(null);

  const [newFixtureId, setNewFixtureId] = useState(allFixtures[0]?.id ?? "");
  const [newModeIndex, setNewModeIndex] = useState(0);
  const [newAddress, setNewAddress] = useState(1);
  const [newName, setNewName] = useState("");

  const newFixtureDef = findFixture(allFixtures, newFixtureId);

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
      if (ch.defaultValue !== undefined) defaults[newAddress + ch.offset - 1] = ch.defaultValue;
    }
    if (Object.keys(defaults).length > 0) {
      void applyPatch(defaults);
    }

    setNewName("");
    setSelectedIds([fixture.id]);
  };

  const handleTileClick = (id: string, e: React.MouseEvent) => {
    setSelectedIds((prev) => {
      if (e.shiftKey || e.ctrlKey || e.metaKey) {
        return prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id];
      }
      return [id];
    });
  };

  return (
    <div className="flex h-full flex-col gap-4 overflow-hidden p-4">
      {!connected && (
        <p className="shrink-0 text-xs text-destructive">
          Connect a DMX interface in the top bar first.
        </p>
      )}

      <div className="flex shrink-0 flex-col gap-3 rounded-md border border-border p-3">
        <div className="grid grid-cols-2 gap-3 text-xs sm:grid-cols-4">
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
              {newFixtureDef?.modes.map((m, i) => (
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
          <Button size="sm" onClick={handleAdd} disabled={!newFixtureDef}>
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

      {fixtures.length > 0 && (
        <div className="flex shrink-0 items-center gap-2 text-xs text-muted-foreground">
          <span>
            {selectedIds.length > 0 ? `${selectedIds.length} selected` : `${fixtures.length} patched`}
          </span>
          <span className="text-muted-foreground/50">·</span>
          <button
            className="underline-offset-2 hover:text-foreground hover:underline"
            onClick={() => setSelectedIds(fixtures.map((f) => f.id))}
          >
            Select all
          </button>
          {selectedIds.length > 0 && (
            <button
              className="underline-offset-2 hover:text-foreground hover:underline"
              onClick={() => setSelectedIds([])}
            >
              Clear
            </button>
          )}
          <span className="text-muted-foreground/50">(shift/ctrl-click to multi-select)</span>
        </div>
      )}

      <div className="flex flex-1 gap-4 overflow-hidden">
        {fixtures.length === 0 ? (
          <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
            No fixtures patched yet -- add one above.
          </div>
        ) : (
          <div className="grid flex-1 content-start gap-3 overflow-y-auto grid-cols-[repeat(auto-fill,minmax(110px,1fr))]">
            {fixtures.map((instance) => (
              <FixtureTile
                key={instance.id}
                instance={instance}
                def={findFixture(allFixtures, instance.fixtureId)}
                selected={selectedIds.includes(instance.id)}
                onClick={(e) => handleTileClick(instance.id, e)}
              />
            ))}
          </div>
        )}

        {resolvedSelected.length > 0 && (
          <FixtureDetailPanel
            resolved={resolvedSelected}
            liveChannels={liveChannels}
            onClose={() => setSelectedIds([])}
            onRemove={
              resolvedSelected.length === 1
                ? () => {
                    removeFixture(resolvedSelected[0].instance.id);
                    setSelectedIds([]);
                  }
                : undefined
            }
          />
        )}
      </div>
    </div>
  );
}

function FixtureTile({
  instance,
  def,
  selected,
  onClick,
}: {
  instance: PatchedFixture;
  def: FixtureDefinition | undefined;
  selected: boolean;
  onClick: (e: React.MouseEvent) => void;
}) {
  const mode = def?.modes[instance.modeIndex];
  const values = useFixtureChannelValues(instance, mode);

  if (!def || !mode) {
    return (
      <button
        onClick={onClick}
        className="flex aspect-square flex-col items-center justify-center rounded-md border border-destructive/50 p-2 text-center text-[10px] text-destructive"
      >
        unknown
        <br />
        addr {instance.address}
      </button>
    );
  }

  return (
    <button
      onClick={onClick}
      style={{ backgroundColor: swatchColor(mode, values) }}
      className={cn(
        "flex aspect-square flex-col justify-between rounded-md border p-2 text-left transition-all",
        selected ? "border-primary ring-2 ring-primary" : "border-border hover:border-primary/50",
      )}
    >
      <span className="w-fit rounded bg-black/50 px-1 text-[10px] font-semibold text-white">
        {instance.address}
      </span>
      <span className="truncate rounded bg-black/50 px-1 text-[10px] text-white">
        {instance.name}
      </span>
    </button>
  );
}

/** Union of channel info across every selected fixture, keyed by channel type and
 * built in first-seen order -- lets one slider drive "Red" (say) on every selected
 * fixture at once even if they're different models with different offsets. */
function unionChannels(resolved: ResolvedFixture[]) {
  const byType = new Map<ChannelType, { label?: string; notes?: string }>();
  for (const { mode } of resolved) {
    for (const ch of mode.channels) {
      if (!byType.has(ch.type)) byType.set(ch.type, { label: ch.label, notes: ch.notes });
    }
  }
  return [...byType.entries()].map(([type, info]) => ({ type, ...info }));
}

function FixtureDetailPanel({
  resolved,
  liveChannels,
  onClose,
  onRemove,
}: {
  resolved: ResolvedFixture[];
  liveChannels: Uint8Array;
  onClose: () => void;
  onRemove?: () => void;
}) {
  const applyPatch = useDmxStore((s) => s.applyPatch);
  const isGroup = resolved.length > 1;
  const channels = unionChannels(resolved);

  const valueOf = (type: ChannelType): number => {
    for (const { instance, mode } of resolved) {
      const ch = mode.channels.find((c) => c.type === type);
      if (ch) {
        const idx = instance.address + ch.offset - 1;
        if (idx >= 0 && idx < liveChannels.length) return liveChannels[idx];
      }
    }
    return 0;
  };

  const setValue = (type: ChannelType, value: number) => {
    const patch: Record<number, number> = {};
    for (const { instance, mode } of resolved) {
      const ch = mode.channels.find((c) => c.type === type);
      if (ch) patch[instance.address + ch.offset - 1] = value;
    }
    void applyPatch(patch);
  };

  const applyPreset = (types: ChannelType[]) => {
    const patch: Record<number, number> = {};
    for (const { instance, mode } of resolved) {
      for (const ch of mode.channels) {
        patch[instance.address + ch.offset - 1] =
          ch.type === "function" ? (ch.defaultValue ?? 0) : types.includes(ch.type) ? 255 : 0;
      }
    }
    void applyPatch(patch);
  };

  return (
    <div className="flex w-72 shrink-0 flex-col gap-3 overflow-y-auto rounded-md border border-border bg-card p-3">
      <div className="flex items-start justify-between">
        <div>
          {isGroup ? (
            <div className="text-sm font-medium">{resolved.length} fixtures selected</div>
          ) : (
            <>
              <div className="text-sm font-medium">{resolved[0].instance.name}</div>
              <div className="text-xs text-muted-foreground">
                {resolved[0].def.manufacturer} {resolved[0].def.model} · {resolved[0].mode.name} ·
                addr {resolved[0].instance.address}
              </div>
            </>
          )}
        </div>
        <div className="flex shrink-0 gap-1">
          {onRemove && (
            <Button size="iconSm" variant="ghost" onClick={onRemove} title="Remove">
              <Trash2 />
            </Button>
          )}
          <Button size="iconSm" variant="ghost" onClick={onClose} title="Close">
            <X />
          </Button>
        </div>
      </div>

      <div className="flex flex-col gap-2">
        {channels.map((ch) => (
          <div key={ch.type} className="flex items-center gap-3">
            <span className="w-20 shrink-0 truncate text-xs text-muted-foreground" title={ch.notes}>
              {ch.label ?? ch.type}
            </span>
            <input
              type="range"
              min={0}
              max={255}
              value={valueOf(ch.type)}
              onChange={(e) => setValue(ch.type, Number(e.target.value))}
              className="h-1.5 flex-1 accent-primary"
            />
            <span className="w-8 shrink-0 text-right text-xs tabular-nums">{valueOf(ch.type)}</span>
          </div>
        ))}
      </div>

      <div className="flex flex-wrap gap-1.5">
        {Object.entries(COLOR_PRESETS).map(([name, types]) => (
          <Button key={name} size="sm" variant="secondary" onClick={() => applyPreset(types)}>
            {name}
          </Button>
        ))}
      </div>
    </div>
  );
}
