import { useEffect } from "react";
import { TopBar } from "./components/TopBar";
import { TriggerGrid } from "./components/TriggerGrid";
import { AiSettings } from "./components/AiSettings";
import { SystemPanel } from "./components/SystemPanel";
import { FixturePatchPanel } from "./components/FixturePatchPanel";
import { ProgrammerPanel } from "./components/ProgrammerPanel";
import { LiveAiConsole } from "./components/LiveAiConsole";
import { useDmxStore } from "./store/useDmxStore";
import { useMidiStore } from "./store/useMidiStore";
import { useAppUpdateStore } from "./store/useAppUpdateStore";

function App() {
  const init = useDmxStore((s) => s.init);
  const initMidi = useMidiStore((s) => s.init);
  const checkForAppUpdate = useAppUpdateStore((s) => s.checkNow);

  useEffect(() => {
    void init();
    void initMidi();
    void checkForAppUpdate();
  }, [init, initMidi, checkForAppUpdate]);

  return (
    <div className="flex h-screen flex-col bg-background">
      <TopBar />
      <div className="flex flex-1 overflow-hidden">
        <main className="flex-1 overflow-hidden">
          <TriggerGrid />
        </main>
        <aside className="flex w-80 flex-col overflow-y-auto border-l border-border bg-card">
          <AiSettings />
          <SystemPanel />
          <FixturePatchPanel />
          <ProgrammerPanel />
          <div className="flex-1" />
          <LiveAiConsole />
        </aside>
      </div>
    </div>
  );
}

export default App;
