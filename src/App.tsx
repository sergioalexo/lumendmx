import { useEffect, useState } from "react";
import { TopBar } from "./components/TopBar";
import { TriggerGrid } from "./components/TriggerGrid";
import { AiSettings } from "./components/AiSettings";
import { SystemPanel } from "./components/SystemPanel";
import { FixturePatchPanel } from "./components/FixturePatchPanel";
import { ProgrammerPanel } from "./components/ProgrammerPanel";
import { LiveAiConsole } from "./components/LiveAiConsole";
import { SetupPanel } from "./components/SetupPanel";
import { Button } from "./components/ui/button";
import { cn } from "./lib/utils";
import { useDmxStore } from "./store/useDmxStore";
import { useMidiStore } from "./store/useMidiStore";
import { useAppUpdateStore } from "./store/useAppUpdateStore";
import { useShowStore } from "./store/useShowStore";
import { useUniverseStatusesStore } from "./store/useUniverseStatusesStore";

type MainView = "fixtures" | "cues" | "setup";

function App() {
  const init = useDmxStore((s) => s.init);
  const initMidi = useMidiStore((s) => s.init);
  const checkForAppUpdate = useAppUpdateStore((s) => s.checkNow);
  const initShow = useShowStore((s) => s.init);
  const initUniverseStatuses = useUniverseStatusesStore((s) => s.init);
  const [mainView, setMainView] = useState<MainView>("fixtures");

  useEffect(() => {
    void init();
    void initMidi();
    void checkForAppUpdate();
    void initShow();
    void initUniverseStatuses();
  }, [init, initMidi, checkForAppUpdate, initShow, initUniverseStatuses]);

  return (
    <div className="flex h-screen flex-col bg-background">
      <TopBar />
      <div className="flex flex-1 overflow-hidden">
        <main className="flex flex-1 flex-col overflow-hidden">
          <div className="flex gap-1 border-b border-border px-4 py-2">
            {(["fixtures", "cues", "setup"] as MainView[]).map((view) => (
              <Button
                key={view}
                size="sm"
                variant={mainView === view ? "default" : "ghost"}
                className={cn("uppercase", mainView !== view && "text-muted-foreground")}
                onClick={() => setMainView(view)}
              >
                {view === "fixtures" ? "Fixtures" : view === "cues" ? "Cues" : "Setup"}
              </Button>
            ))}
          </div>
          <div className="flex-1 overflow-hidden">
            {mainView === "fixtures" ? (
              <FixturePatchPanel />
            ) : mainView === "cues" ? (
              <TriggerGrid />
            ) : (
              <SetupPanel />
            )}
          </div>
        </main>
        <aside className="flex w-80 flex-col overflow-y-auto border-l border-border bg-card">
          <AiSettings />
          <SystemPanel />
          <ProgrammerPanel />
          <div className="flex-1" />
          <LiveAiConsole />
        </aside>
      </div>
    </div>
  );
}

export default App;
