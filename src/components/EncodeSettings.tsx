import { useT } from "../i18n/useT";
import { formatDistance } from "../lib/format";
import { distanceFromQuality, EFFORT_NAMES, PRESETS, type Preset } from "../state/settings";
import { useApp } from "../state/store";
import { Field, NumberSlider, OptionCard, SwitchRow } from "./SettingsParts";

export function EncodeSettings({ locked }: { locked: boolean }) {
  const s = useApp((st) => st.settings);
  const update = useApp((st) => st.updateSettings);
  const t = useT();

  return (
    <div className="contents">
      {s.expert ? (
        <>
          <Field
            label={t("settings.quality")}
            value={t("settings.qualityValue", { q: s.quality, d: formatDistance(distanceFromQuality(s.quality)) })}
          >
            <NumberSlider
              label={t("settings.quality")}
              min={30}
              max={100}
              value={s.quality}
              disabled={locked || s.lossless}
              onChange={(quality) => update({ quality })}
            />
          </Field>
          <Field label={t("settings.effort")} value={`${s.effort} · ${EFFORT_NAMES[s.effort]}`}>
            <NumberSlider
              label={t("settings.effort")}
              min={1}
              max={9}
              value={s.effort}
              disabled={locked}
              onChange={(effort) => update({ effort })}
            />
          </Field>
          <SwitchRow
            label={t("settings.lossless")}
            hint={t("settings.losslessHint")}
            selected={s.lossless}
            disabled={locked}
            onChange={(lossless) => update({ lossless })}
          />
        </>
      ) : (
        <div role="radiogroup" aria-label={t("settings.quality")} className="flex flex-col gap-2">
          <span className="text-[13px] text-[var(--muted)]">{t("settings.quality")}</span>
          {(Object.keys(PRESETS) as Preset[]).map((p) => (
            <OptionCard
              key={p}
              layout="row"
              label={t(`preset.${p}.label`)}
              hint={t(`preset.${p}.sub`)}
              checked={s.preset === p}
              disabled={locked}
              onSelect={() => update({ preset: p })}
            />
          ))}
        </div>
      )}
      <SwitchRow
        label={t("settings.transcode")}
        hint={t("settings.transcodeHint")}
        selected={s.transcodeJpeg}
        disabled={locked}
        onChange={(transcodeJpeg) => update({ transcodeJpeg })}
      />
    </div>
  );
}
