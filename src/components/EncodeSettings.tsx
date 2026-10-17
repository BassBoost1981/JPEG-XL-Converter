import { useT } from "../i18n/useT";
import { formatDistance } from "../lib/format";
import { distanceFromQuality, EFFORT_NAMES, PRESETS, type Preset } from "../state/settings";
import { useApp } from "../state/store";
import { Field, NumberSlider, OptionCard, SwitchRow } from "./SettingsParts";

const HINTS: Record<Preset, string> = {
  ll: "empfohlen · pixelgenau",
  hi: "d 0.5",
  bal: "d 1.2",
  sm: "d 2.6",
};

export function EncodeSettings({ locked }: { locked: boolean }) {
  const s = useApp((st) => st.settings);
  const update = useApp((st) => st.updateSettings);
  const t = useT();

  return (
    <div className="contents">
      {s.expert ? (
        <>
          <Field label="Qualität" value={`${s.quality} · Distanz ${formatDistance(distanceFromQuality(s.quality))}`}>
            <NumberSlider
              label="Qualität"
              min={30}
              max={100}
              value={s.quality}
              disabled={locked || s.lossless}
              onChange={(quality) => update({ quality })}
            />
          </Field>
          <Field label="Effort" value={`${s.effort} · ${EFFORT_NAMES[s.effort]}`}>
            <NumberSlider
              label="Effort"
              min={1}
              max={9}
              value={s.effort}
              disabled={locked}
              onChange={(effort) => update({ effort })}
            />
          </Field>
          <SwitchRow
            label="Verlustfrei"
            hint="Pixelgenau, ignoriert Qualität"
            selected={s.lossless}
            disabled={locked}
            onChange={(lossless) => update({ lossless })}
          />
        </>
      ) : (
        <div role="radiogroup" aria-label="Qualität" className="flex flex-col gap-2">
          <span className="text-[13px] text-[var(--muted)]">Qualität</span>
          {(Object.keys(PRESETS) as Preset[]).map((p) => (
            <OptionCard
              key={p}
              layout="row"
              label={t(`preset.${p}.label`)}
              hint={HINTS[p]}
              checked={s.preset === p}
              disabled={locked}
              onSelect={() => update({ preset: p })}
            />
          ))}
        </div>
      )}
      <SwitchRow
        label="JPEGs verlustfrei transkodieren"
        hint="~20 % kleiner, bit-genau zurückwandelbar"
        selected={s.transcodeJpeg}
        disabled={locked}
        onChange={(transcodeJpeg) => update({ transcodeJpeg })}
      />
    </div>
  );
}
