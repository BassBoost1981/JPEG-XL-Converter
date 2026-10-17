import { AlertDialog, Button } from "@heroui/react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { backend } from "./api";
import { CompareSlider } from "./components/CompareSlider";
import { QueuePanel } from "./components/QueuePanel";
import { RunFooter } from "./components/RunFooter";
import { SettingsPanel } from "./components/SettingsPanel";
import { TitleBar } from "./components/TitleBar";
import { ToolsBanner } from "./components/ToolsBanner";
import { translate } from "./i18n";
import { useLang, useT } from "./i18n/useT";
import { useApp } from "./state/store";

const CARD = "rounded-[14px] bg-[var(--card)] shadow-[var(--card-shadow)]";

function Card({ className = "", children }: { className?: string; children: ReactNode }) {
  return <section className={`${CARD} ${className}`}>{children}</section>;
}

function App() {
  const lang = useLang();
  const t = useT();
  const [confirmClose, setConfirmClose] = useState(false);
  const closeAllowed = useRef(false);

  useEffect(() => {
    const ready = useApp.getState().init();
    return () => void ready.then((dispose) => dispose());
  }, []);

  // Window title and document language follow the language in use (also on the first render).
  useEffect(() => {
    backend.setTitle(translate(lang, "app.title"));
    document.documentElement.lang = lang;
  }, [lang]);

  useEffect(
    () =>
      backend.onCloseRequested(() => {
        if (closeAllowed.current) return true;
        const { run } = useApp.getState();
        if (run !== "running" && run !== "paused") return true;
        setConfirmClose(true);
        return false;
      }),
    [],
  );

  const quit = async () => {
    closeAllowed.current = true;
    setConfirmClose(false);
    try {
      await backend.cancelRun();
    } finally {
      backend.close();
    }
  };

  return (
    <div className="flex h-full flex-col">
      <TitleBar />
      <ToolsBanner />
      <main className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_360px] gap-4 p-4">
        <div className="flex min-h-0 flex-col gap-4">
          <Card className="flex-none p-3">
            <CompareSlider />
          </Card>
          <Card className="flex min-h-0 flex-1 flex-col">
            <QueuePanel />
          </Card>
        </div>
        <Card className="flex min-h-0 flex-col">
          <div className="min-h-0 flex-1 overflow-y-auto">
            <SettingsPanel />
          </div>
          <RunFooter />
        </Card>
      </main>

      <AlertDialog.Backdrop isOpen={confirmClose} onOpenChange={setConfirmClose}>
        <AlertDialog.Container>
          <AlertDialog.Dialog>
            <AlertDialog.Header>
              <AlertDialog.Heading>{t("dialog.quitTitle")}</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p>{t("dialog.quitBody")}</p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button variant="danger" onPress={() => void quit()}>
                {t("dialog.quit")}
              </Button>
              <Button variant="secondary" onPress={() => setConfirmClose(false)}>
                {t("dialog.keepWorking")}
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </div>
  );
}

export default App;
