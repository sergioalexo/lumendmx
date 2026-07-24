import { Music2, Trash2 } from "lucide-react";
import { useLibraryStore } from "../store/useLibraryStore";
import { useMidiStore } from "../store/useMidiStore";
import { Card } from "./ui/card";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { cn } from "../lib/utils";
import type { AssetKind } from "../lib/types";

const KIND_BADGE: Record<AssetKind, "secondary" | "default" | "outline"> = {
  scene: "secondary",
  chase: "default",
  fx: "outline",
};

export function TriggerGrid() {
  const { assets, activeAssetIds, trigger, stop, removeAsset } = useLibraryStore();
  const { midiMap, learnTargetAssetId, startLearn, cancelLearn, unbindAsset, supported } =
    useMidiStore();

  const boundKeyFor = (assetId: string) =>
    Object.entries(midiMap).find(([, id]) => id === assetId)?.[0];

  if (assets.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        No Scenes/Chases yet — generate one from the Programmer panel →
      </div>
    );
  }

  return (
    <div className="grid grid-cols-3 gap-3 overflow-y-auto p-4 sm:grid-cols-4">
      {assets.map((asset) => {
        const active = activeAssetIds.has(asset.id);
        const boundKey = boundKeyFor(asset.id);
        const learning = learnTargetAssetId === asset.id;

        return (
          <Card
            key={asset.id}
            className={cn(
              "flex flex-col gap-2 p-3 transition-colors hover:border-primary/60",
              active && "border-primary shadow-lg shadow-primary/30",
            )}
          >
            <button
              className="flex flex-1 flex-col items-start gap-1 text-left"
              onClick={() => (active ? stop(asset.id) : trigger(asset.id))}
            >
              <Badge variant={KIND_BADGE[asset.kind]}>{asset.kind}</Badge>
              <span className="text-sm font-medium">{asset.name}</span>
            </button>

            <div className="flex items-center justify-between text-muted-foreground">
              <Button
                variant="ghost"
                size="iconSm"
                onClick={() => removeAsset(asset.id)}
                title="Delete"
              >
                <Trash2 />
              </Button>
              {supported && (
                <Button
                  variant="ghost"
                  size="sm"
                  className={cn("gap-1 text-xs", learning && "text-primary")}
                  onClick={() => (learning ? cancelLearn() : startLearn(asset.id))}
                >
                  <Music2 />
                  {learning ? "Listening…" : boundKey ? boundKey : "Learn"}
                </Button>
              )}
              {boundKey && !learning && (
                <button className="text-xs hover:text-foreground" onClick={() => unbindAsset(asset.id)}>
                  Unbind
                </button>
              )}
            </div>
          </Card>
        );
      })}
    </div>
  );
}
