import { Popover, Pressable } from "@heroui/react";
import { useId, useState } from "react";
import type { Lang } from "../api/types";
import { useLang, useT } from "../i18n/useT";
import { useTheme } from "../lib/useTheme";
import { enterExpert } from "../state/settings";
import { useApp } from "../state/store";

interface SegmentedRowProps<V extends string> {
  label: string;
  value: V;
  options: readonly { value: V; label: string }[];
  disabled?: boolean;
  onChange(value: V): void;
}

/** A small grey label over a switch of two equally wide buttons (the design's `prefRows`). */
function SegmentedRow<V extends string>({ label, value, options, disabled, onChange }: SegmentedRowProps<V>) {
  const labelId = useId();
  return (
    <div className="flex flex-col gap-1.5">
      <span id={labelId} className="text-xs text-[var(--muted)]">
        {label}
      </span>
      <div role="group" aria-labelledby={labelId} className="grid grid-cols-2 gap-0.5 rounded-xl bg-[var(--d100)] p-1">
        {options.map((o) => {
          const on = o.value === value;
          return (
            <button
              key={o.value}
              type="button"
              aria-pressed={on}
              disabled={disabled}
              onClick={() => onChange(o.value)}
              className={`cursor-pointer rounded-lg px-2.5 py-[7px] text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)] disabled:cursor-default disabled:opacity-50 ${
                on ? "bg-[var(--seg-bg)] text-[var(--fg)] shadow-[var(--card-shadow)]" : "bg-transparent text-[var(--muted)]"
              }`}
            >
              {o.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}

/**
 * The title bar's "App settings" button and its popover: appearance, mode and language.
 * Also keeps the `dark` class on `<html>` in step with the theme (via `useTheme`), which must happen
 * while the popover is closed too; this component is always mounted, only the popover content is not.
 */
export function AppSettingsPopover() {
  const t = useT();
  const lang = useLang();
  const { dark, setDark } = useTheme();
  const expert = useApp((s) => s.settings.expert);
  // The mode stays fixed while a run is active (also while it is paused).
  const busy = useApp((s) => s.run === "running" || s.run === "paused");
  const updateSettings = useApp((s) => s.updateSettings);
  const [open, setOpen] = useState(false);

  const setMode = (mode: "normal" | "expert") => {
    if (mode === "normal") {
      updateSettings({ expert: false });
      return;
    }
    // Switching to expert starts from the values of the chosen level; already in expert mode: leave them.
    const settings = useApp.getState().settings;
    if (!settings.expert) updateSettings(enterExpert(settings));
  };

  const background = open ? "var(--d200)" : "var(--d100)";

  return (
    <Popover isOpen={open} onOpenChange={setOpen}>
      <Pressable>
        <button
          type="button"
          aria-label={t("prefs.title")}
          title={t("prefs.title")}
          className={`flex size-[38px] flex-none cursor-pointer items-center justify-center rounded-[10px] text-[var(--fg)] outline-none transition-colors focus-visible:ring-2 focus-visible:ring-[var(--accent)] ${
            open ? "bg-[var(--d200)]" : "bg-[var(--d100)]"
          }`}
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
            <line x1="4" y1="6" x2="20" y2="6" />
            <line x1="4" y1="12" x2="20" y2="12" />
            <line x1="4" y1="18" x2="20" y2="18" />
            <circle cx="9" cy="6" r="2.4" style={{ fill: background }} />
            <circle cx="15" cy="12" r="2.4" style={{ fill: background }} />
            <circle cx="8" cy="18" r="2.4" style={{ fill: background }} />
          </svg>
        </button>
      </Pressable>
      <Popover.Content
        placement="bottom end"
        offset={7}
        className="w-[300px] rounded-[14px] border border-[var(--d200)] bg-[var(--card)] text-[var(--fg)] shadow-[0_12px_40px_rgba(0,0,0,.18)]"
      >
        <Popover.Dialog className="flex flex-col gap-3.5">
          <Popover.Heading className="text-[15px] font-semibold">{t("prefs.title")}</Popover.Heading>
          <SegmentedRow
            label={t("prefs.appearance")}
            value={dark ? "dark" : "light"}
            options={[
              { value: "light", label: t("prefs.light") },
              { value: "dark", label: t("prefs.dark") },
            ]}
            onChange={(v) => setDark(v === "dark")}
          />
          <SegmentedRow
            label={t("prefs.mode")}
            value={expert ? "expert" : "normal"}
            options={[
              { value: "normal", label: t("prefs.normal") },
              { value: "expert", label: t("prefs.expert") },
            ]}
            disabled={busy}
            onChange={setMode}
          />
          <SegmentedRow<Lang>
            label={t("prefs.language")}
            value={lang}
            options={[
              { value: "de", label: t("prefs.langDe") },
              { value: "en", label: t("prefs.langEn") },
            ]}
            onChange={(v) => updateSettings({ lang: v })}
          />
        </Popover.Dialog>
      </Popover.Content>
    </Popover>
  );
}
