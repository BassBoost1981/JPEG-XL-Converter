import { Tabs } from "@heroui/react";
import logo from "../assets/jxl-logo-v2.svg";
import { backend } from "../api";
import type { Mode } from "../api/types";
import { useT } from "../i18n/useT";
import { useApp } from "../state/store";
import { AppSettingsPopover } from "./AppSettingsPopover";

const WINDOW_BUTTON =
  "flex h-full w-[46px] items-center justify-center text-xs text-[var(--muted)] hover:bg-black/5 dark:hover:bg-white/10";

export function TitleBar() {
  const mode = useApp((s) => s.settings.mode);
  // The mode stays fixed while a run is active (also while it is paused).
  const busy = useApp((s) => s.run === "running" || s.run === "paused");
  const updateSettings = useApp((s) => s.updateSettings);
  const t = useT();

  return (
    <header
      data-tauri-drag-region
      className="grid h-[65px] flex-none grid-cols-[1fr_auto_1fr] items-center border-b border-[var(--line)] bg-[var(--card)] pl-5"
    >
      <div data-tauri-drag-region className="flex items-center gap-2.5 self-stretch">
        <img src={logo} alt={t("app.title")} className="pointer-events-none size-8" />
        <span data-tauri-drag-region className="text-base font-semibold">
          JPEG XL
        </span>
      </div>

      <Tabs isDisabled={busy} selectedKey={mode} onSelectionChange={(k) => updateSettings({ mode: k as Mode })}>
        <Tabs.ListContainer>
          <Tabs.List aria-label={t("tab.direction")}>
            <Tabs.Tab id="enc" className="whitespace-nowrap">
              {t("tab.enc")}
              <Tabs.Indicator />
            </Tabs.Tab>
            <Tabs.Tab id="dec" className="whitespace-nowrap">
              {t("tab.dec")}
              <Tabs.Indicator />
            </Tabs.Tab>
          </Tabs.List>
        </Tabs.ListContainer>
      </Tabs>

      <div data-tauri-drag-region className="flex h-full items-center justify-end gap-[18px]">
        <AppSettingsPopover />
        <div className="flex h-full">
          <button type="button" aria-label={t("app.minimize")} className={WINDOW_BUTTON} onClick={() => backend.minimize()}>
            —
          </button>
          <button type="button" aria-label={t("app.maximize")} className={WINDOW_BUTTON} onClick={() => backend.toggleMaximize()}>
            ▢
          </button>
          <button type="button" aria-label={t("app.close")} className={`${WINDOW_BUTTON} hover:!bg-red-600 hover:text-white`} onClick={() => backend.close()}>
            ✕
          </button>
        </div>
      </div>
    </header>
  );
}
