import { useApp } from "../state/store";
import { CommonSettings } from "./CommonSettings";
import { DecodeSettings } from "./DecodeSettings";
import { EncodeSettings } from "./EncodeSettings";

export function SettingsPanel() {
  const mode = useApp((s) => s.settings.mode);
  const locked = useApp((s) => s.run === "running" || s.run === "paused");

  return (
    <div data-testid="SettingsPanel" className="flex flex-col gap-[18px] p-[18px]">
      <div className="flex flex-col gap-0.5">
        <span className="text-[17px] font-semibold">Einstellungen</span>
        <span className="text-xs text-[var(--muted)]">
          {mode === "enc" ? "Kodierung · nach JPEG XL" : "Dekodierung · aus JPEG XL"}
        </span>
      </div>
      {mode === "enc" ? <EncodeSettings locked={locked} /> : <DecodeSettings locked={locked} />}
      <CommonSettings locked={locked} />
    </div>
  );
}
