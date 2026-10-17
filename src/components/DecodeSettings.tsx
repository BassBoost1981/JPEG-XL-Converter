import type { OutputFormat } from "../api/types";
import { useT } from "../i18n/useT";
import { useApp } from "../state/store";
import { Field, NumberSlider, OptionCard } from "./SettingsParts";

// The hint of each card is `format.<id>.sub` in the catalog.
const GRID: { id: Exclude<OutputFormat, "auto">; label: string }[] = [
  { id: "png", label: "PNG" },
  { id: "jpg", label: "JPEG" },
  { id: "webp", label: "WebP" },
  { id: "tiff", label: "TIFF" },
];

export function DecodeSettings({ locked }: { locked: boolean }) {
  const s = useApp((st) => st.settings);
  const update = useApp((st) => st.updateSettings);
  const t = useT();
  const showInfo = s.decodeFormat === "auto" || s.decodeFormat === "jpg";
  const showBitDepth = s.expert && (s.decodeFormat === "png" || s.decodeFormat === "tiff");

  return (
    <div className="contents">
      <div role="radiogroup" aria-label={t("settings.outputFormat")} className="grid grid-cols-2 gap-2">
        <span className="col-span-2 text-[13px] text-[var(--muted)]">{t("settings.targetFormat")}</span>
        <OptionCard
          className="col-span-2"
          label={t("settings.auto")}
          hint={t("settings.autoHint")}
          checked={s.decodeFormat === "auto"}
          disabled={locked}
          onSelect={() => update({ decodeFormat: "auto" })}
        />
        {GRID.map((f) => (
          <OptionCard
            key={f.id}
            label={f.label}
            hint={t(`format.${f.id}.sub`)}
            checked={s.decodeFormat === f.id}
            disabled={locked}
            onSelect={() => update({ decodeFormat: f.id })}
          />
        ))}
      </div>

      {showInfo && (
        <p className="text-pretty rounded-xl bg-[var(--info-bg)] px-3 py-2.5 text-xs text-[var(--info-fg)]">
          {t("settings.reconstructInfo")}
        </p>
      )}

      {s.expert && s.decodeFormat === "jpg" && (
        <Field label={t("settings.jpegQuality")} value={String(s.jpegQuality)}>
          <NumberSlider
            label={t("settings.jpegQuality")}
            min={50}
            max={100}
            value={s.jpegQuality}
            disabled={locked}
            onChange={(jpegQuality) => update({ jpegQuality })}
          />
        </Field>
      )}

      {showBitDepth && (
        <div className="flex items-center gap-3">
          <span className="flex-1">{t("settings.bitDepth")}</span>
          <div role="radiogroup" aria-label={t("settings.bitDepth")} className="flex gap-0.5 rounded-[10px] bg-[var(--d100)] p-[3px]">
            {([8, 16] as const).map((d) => (
              <OptionCard
                key={d}
                layout="segment"
                label={t("settings.bits", { d })}
                hint=""
                checked={s.bitDepth === d}
                disabled={locked}
                onSelect={() => update({ bitDepth: d })}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
