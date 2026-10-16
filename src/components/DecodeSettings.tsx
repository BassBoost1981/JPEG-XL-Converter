import type { OutputFormat } from "../api/types";
import { useApp } from "../state/store";
import { Field, NumberSlider, OptionCard } from "./SettingsParts";

const GRID: { id: OutputFormat; label: string; hint: string }[] = [
  { id: "png", label: "PNG", hint: "verlustfrei" },
  { id: "jpg", label: "JPEG", hint: "kompatibel" },
  { id: "webp", label: "WebP", hint: "Web" },
  { id: "tiff", label: "TIFF", hint: "Druck" },
];

export function DecodeSettings({ locked }: { locked: boolean }) {
  const s = useApp((st) => st.settings);
  const update = useApp((st) => st.updateSettings);
  const showInfo = s.decodeFormat === "auto" || s.decodeFormat === "jpg";
  const showBitDepth = s.expert && (s.decodeFormat === "png" || s.decodeFormat === "tiff");

  return (
    <div className="contents">
      <div role="radiogroup" aria-label="Ausgabeformat" className="grid grid-cols-2 gap-2">
        <span className="col-span-2 text-[13px] text-[var(--muted)]">Zielformat</span>
        <OptionCard
          className="col-span-2"
          label="Automatisch"
          hint="Original-JPEG, sonst PNG"
          checked={s.decodeFormat === "auto"}
          disabled={locked}
          onSelect={() => update({ decodeFormat: "auto" })}
        />
        {GRID.map((f) => (
          <OptionCard
            key={f.id}
            label={f.label}
            hint={f.hint}
            checked={s.decodeFormat === f.id}
            disabled={locked}
            onSelect={() => update({ decodeFormat: f.id })}
          />
        ))}
      </div>

      {showInfo && (
        <p className="text-pretty rounded-xl bg-[var(--info-bg)] px-3 py-2.5 text-xs text-[var(--info-fg)]">
          Aus JPEG transkodierte Dateien werden bit-genau als Original-JPEG wiederhergestellt.
        </p>
      )}

      {s.expert && s.decodeFormat === "jpg" && (
        <Field label="JPEG-Qualität" value={String(s.jpegQuality)}>
          <NumberSlider
            label="JPEG-Qualität"
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
          <span className="flex-1">Bittiefe</span>
          <div role="radiogroup" aria-label="Bittiefe" className="flex gap-0.5 rounded-[10px] bg-[var(--d100)] p-[3px]">
            {([8, 16] as const).map((d) => (
              <OptionCard
                key={d}
                layout="segment"
                label={`${d} Bit`}
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
