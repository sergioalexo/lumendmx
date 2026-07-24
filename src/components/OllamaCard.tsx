import { useEffect } from "react";
import { Download, RefreshCw } from "lucide-react";
import { useAiConfigStore } from "../store/useAiConfigStore";
import { useOllamaStore } from "../store/useOllamaStore";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Progress, formatBytes } from "./ui/progress";

/** Manages the local Ollama model used for offline/on-prem AI generation --
 * mirrors the "runtime dependency" cards in MediaFetch's Components page, scoped
 * to whichever model is currently configured for the Ollama backend. */
export function OllamaCard() {
  const { config } = useAiConfigStore();
  const { reachable, installedModels, pulling, progress, error, refresh, pull } =
    useOllamaStore();

  const baseUrl = config.baseUrl ?? "http://localhost:11434";

  useEffect(() => {
    if (config.backend === "ollama") void refresh(baseUrl);
  }, [config.backend, baseUrl, refresh]);

  if (config.backend !== "ollama") {
    return (
      <Card>
        <CardContent className="p-4 text-xs text-muted-foreground">
          Switch the AI backend to Ollama to manage local models here.
        </CardContent>
      </Card>
    );
  }

  const installed = installedModels.includes(config.model);
  const statusBadge =
    reachable === null ? (
      <Badge variant="secondary">Checking…</Badge>
    ) : !reachable ? (
      <Badge variant="destructive">Server unreachable</Badge>
    ) : installed ? (
      <Badge variant="success">Installed</Badge>
    ) : (
      <Badge variant="outline">Not installed</Badge>
    );

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0 p-4">
        <div>
          <CardTitle className="text-sm">Ollama · {config.model}</CardTitle>
          <p className="mt-0.5 text-xs text-muted-foreground">{baseUrl}</p>
        </div>
        <Button variant="ghost" size="iconSm" onClick={() => void refresh(baseUrl)}>
          <RefreshCw />
        </Button>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 p-4 pt-0">
        <div className="flex items-center justify-between">
          {statusBadge}
          <Button
            size="sm"
            disabled={pulling || reachable === false}
            onClick={() => void pull(baseUrl, config.model)}
          >
            <Download />
            {installed ? "Update" : "Pull"}
          </Button>
        </div>

        {pulling && progress && (
          <div className="flex flex-col gap-1">
            <Progress
              value={progress.total ? ((progress.completed ?? 0) / progress.total) * 100 : 0}
            />
            <p className="text-xs text-muted-foreground">
              {progress.status}
              {progress.total ? ` · ${formatBytes(progress.completed ?? 0)} / ${formatBytes(progress.total)}` : ""}
            </p>
          </div>
        )}

        {error && <p className="text-xs text-destructive">{error}</p>}
      </CardContent>
    </Card>
  );
}
