import { Power, RefreshCw } from "lucide-react";
import { useDmxStore } from "../store/useDmxStore";
import { Button } from "./ui/button";
import { Select } from "./ui/input";
import { cn } from "../lib/utils";

export function TopBar() {
  const { connection, blackout, ports, connect, disconnect, toggleBlackout, refreshPorts } =
    useDmxStore();

  return (
    <header className="flex items-center gap-4 border-b border-border bg-card px-4 py-3">
      <h1 className="text-lg font-semibold tracking-tight">
        Lumen<span className="text-primary">DMX</span>
      </h1>

      <div className="flex flex-1 items-center gap-3">
        <span
          className={cn(
            "h-2.5 w-2.5 rounded-full",
            connection.connected ? "bg-success" : "bg-destructive",
          )}
          title={connection.error ?? undefined}
        />
        <span className="text-sm text-muted-foreground">
          {connection.connected
            ? `Connected: ${connection.port}`
            : connection.error
              ? `Error: ${connection.error}`
              : "Not connected"}
        </span>

        {!connection.connected && (
          <>
            <Select
              className="w-56"
              onFocus={() => void refreshPorts()}
              onChange={(e) => e.target.value && void connect(e.target.value)}
              value=""
            >
              <option value="" disabled>
                Select DMX interface…
              </option>
              {ports.map((port) => (
                <option key={port.port_name} value={port.port_name}>
                  {port.label}
                </option>
              ))}
            </Select>
            <Button variant="ghost" size="iconSm" onClick={() => void refreshPorts()} title="Refresh ports">
              <RefreshCw />
            </Button>
          </>
        )}
        {connection.connected && (
          <Button variant="outline" size="sm" onClick={() => void disconnect()}>
            Disconnect
          </Button>
        )}
      </div>

      <Button
        variant={blackout ? "destructive" : "outline"}
        onClick={() => void toggleBlackout()}
        className={cn(
          "font-bold uppercase tracking-wider",
          blackout ? "shadow-lg shadow-destructive/40" : "text-destructive border-destructive/60 hover:bg-destructive/10 hover:text-destructive",
        )}
      >
        <Power />
        Blackout
      </Button>
    </header>
  );
}
