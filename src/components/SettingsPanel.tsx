import { useT } from "../i18n/useT";
import { useApp } from "../state/store";
import { CommonSettings } from "./CommonSettings";
import { DecodeSettings } from "./DecodeSettings";
import { EncodeSettings } from "./EncodeSettings";

export function SettingsPanel() {
  const mode = useApp((s) => s.settings.mode);
  const locked = useApp((s) => s.run === "running" || s.run === "paused");
  const t = useT();

  return (
    <div data-testid="SettingsPanel" className="flex flex-col gap-[18px] p-[18px]">
      <div className="flex flex-col gap-0.5">
        <span className="text-[17px] font-semibold">{t("settings.title")}</span>
        <span className="text-xs text-[var(--muted)]">
          {mode === "enc" ? t("settings.encSubtitle") : t("settings.decSubtitle")}
        </span>
      </div>
      {mode === "enc" ? <EncodeSettings locked={locked} /> : <DecodeSettings locked={locked} />}
      <CommonSettings locked={locked} />
    </div>
  );
}
