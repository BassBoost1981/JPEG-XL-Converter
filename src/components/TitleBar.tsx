import { Switch, Tabs } from "@heroui/react";
import logo from "../assets/jxl-logo-v2.svg";
import { backend } from "../api";
import type { Mode } from "../api/types";
import { useTheme } from "../lib/useTheme";
import { useApp } from "../state/store";

const WINDOW_BUTTON =
  "flex h-full w-[46px] items-center justify-center text-xs text-[var(--muted)] hover:bg-black/5 dark:hover:bg-white/10";

function LabeledSwitch({ label, selected, onChange }: { label: string; selected: boolean; onChange(v: boolean): void }) {
  return (
    <Switch isSelected={selected} onChange={onChange} className="flex-row-reverse items-center gap-2">
      <Switch.Control>
        <Switch.Thumb />
      </Switch.Control>
      <Switch.Content>
        <span className="text-[13px] font-normal text-[var(--muted)]">{label}</span>
      </Switch.Content>
    </Switch>
  );
}

export function TitleBar() {
  const mode = useApp((s) => s.settings.mode);
  const expert = useApp((s) => s.settings.expert);
  // The mode stays fixed while a run is active (also while it is paused).
  const busy = useApp((s) => s.run === "running" || s.run === "paused");
  const updateSettings = useApp((s) => s.updateSettings);
  const { dark, setDark } = useTheme();

  return (
    <header
      data-tauri-drag-region
      className="grid h-[65px] flex-none grid-cols-[1fr_auto_1fr] items-center border-b border-[var(--line)] bg-[var(--card)] pl-5"
    >
      <div data-tauri-drag-region className="flex items-center gap-2.5 self-stretch">
        <img src={logo} alt="JPEG XL Konverter" className="pointer-events-none size-8" />
        <span data-tauri-drag-region className="text-base font-semibold">
          JPEG XL
        </span>
      </div>

      <Tabs isDisabled={busy} selectedKey={mode} onSelectionChange={(k) => updateSettings({ mode: k as Mode })}>
        <Tabs.ListContainer>
          <Tabs.List aria-label="Richtung">
            <Tabs.Tab id="enc" className="whitespace-nowrap">
              In JPEG XL
              <Tabs.Indicator />
            </Tabs.Tab>
            <Tabs.Tab id="dec" className="whitespace-nowrap">
              Aus JPEG XL
              <Tabs.Indicator />
            </Tabs.Tab>
          </Tabs.List>
        </Tabs.ListContainer>
      </Tabs>

      <div data-tauri-drag-region className="flex h-full items-center justify-end gap-[18px]">
        <LabeledSwitch label={dark ? "Dunkel" : "Hell"} selected={dark} onChange={setDark} />
        <LabeledSwitch label="Experte" selected={expert} onChange={(v) => updateSettings({ expert: v })} />
        <div className="flex h-full">
          <button type="button" aria-label="Minimieren" className={WINDOW_BUTTON} onClick={() => backend.minimize()}>
            —
          </button>
          <button type="button" aria-label="Maximieren" className={WINDOW_BUTTON} onClick={() => backend.toggleMaximize()}>
            ▢
          </button>
          <button type="button" aria-label="Schließen" className={`${WINDOW_BUTTON} hover:!bg-red-600 hover:text-white`} onClick={() => backend.close()}>
            ✕
          </button>
        </div>
      </div>
    </header>
  );
}
