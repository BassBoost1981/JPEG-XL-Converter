import { Slider, Switch } from "@heroui/react";
import type { ReactNode } from "react";

/** Selectable card of a radio group (quality step, output format). */
export function OptionCard({
  label,
  hint,
  checked,
  disabled,
  onSelect,
  className = "",
}: {
  label: string;
  hint: string;
  checked: boolean;
  disabled: boolean;
  onSelect(): void;
  className?: string;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={checked}
      disabled={disabled}
      onClick={onSelect}
      className={`flex flex-col items-start gap-0.5 rounded-xl border-2 px-3 py-2.5 text-left text-[var(--fg)] transition-colors disabled:opacity-50 ${
        checked
          ? "border-[var(--accent)] bg-[color-mix(in_srgb,var(--accent)_10%,transparent)]"
          : "border-[var(--line)] hover:border-[var(--muted)]"
      } ${className}`}
    >
      <span className="text-[13px] font-semibold">{label}</span>
      <span className="text-xs text-[var(--muted)]">{hint}</span>
    </button>
  );
}

/** Switch with a label and a small grey subtitle. */
export function SwitchRow({
  label,
  hint,
  selected,
  disabled,
  onChange,
}: {
  label: string;
  hint: string;
  selected: boolean;
  disabled: boolean;
  onChange(v: boolean): void;
}) {
  return (
    <Switch
      isSelected={selected}
      isDisabled={disabled}
      onChange={onChange}
      className="flex w-full flex-row items-center justify-between gap-3"
    >
      <Switch.Content className="flex flex-1 flex-col items-start text-left">
        <span className="text-[13px] font-medium">{label}</span>
        <span className="text-xs text-[var(--muted)]">{hint}</span>
      </Switch.Content>
      <Switch.Control>
        <Switch.Thumb />
      </Switch.Control>
    </Switch>
  );
}

export function Field({ label, value, children }: { label: string; value: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-baseline justify-between">
        <span className="text-[13px] font-medium">{label}</span>
        <span className="text-xs text-[var(--muted)]">{value}</span>
      </div>
      {children}
    </div>
  );
}

export function NumberSlider({
  label,
  min,
  max,
  value,
  disabled,
  onChange,
}: {
  label: string;
  min: number;
  max: number;
  value: number;
  disabled: boolean;
  onChange(v: number): void;
}) {
  return (
    <Slider
      aria-label={label}
      minValue={min}
      maxValue={max}
      step={1}
      value={value}
      isDisabled={disabled}
      onChange={(v) => onChange(v as number)}
    >
      <Slider.Track>
        <Slider.Fill />
        <Slider.Thumb />
      </Slider.Track>
    </Slider>
  );
}
