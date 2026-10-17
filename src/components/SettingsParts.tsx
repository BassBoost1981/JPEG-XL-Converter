import { Slider, Switch } from "@heroui/react";
import type { ReactNode } from "react";

/**
 * Selectable card of a radio group. "row": label left, hint right (quality steps);
 * "stack": label above hint (output formats); "segment": small button of a segmented control.
 */
export function OptionCard({
  label,
  hint,
  checked,
  disabled,
  onSelect,
  layout = "stack",
  className = "",
}: {
  label: string;
  hint: string;
  checked: boolean;
  disabled: boolean;
  onSelect(): void;
  layout?: "row" | "stack" | "segment";
  className?: string;
}) {
  const hintColor = checked ? "text-[var(--opt-sub-on)]" : "text-[var(--muted)]";
  if (layout === "segment") {
    return (
      <button
        type="button"
        role="radio"
        aria-checked={checked}
        disabled={disabled}
        onClick={onSelect}
        className={`rounded-[7px] px-3 py-1 text-xs text-[var(--fg)] disabled:opacity-50 ${
          checked ? "bg-[var(--sel-bg)]" : "bg-transparent"
        } ${className}`}
      >
        {label}
      </button>
    );
  }
  const row = layout === "row";
  return (
    <button
      type="button"
      role="radio"
      aria-checked={checked}
      disabled={disabled}
      onClick={onSelect}
      className={`flex rounded-xl border-2 text-left text-[var(--fg)] transition-colors disabled:opacity-50 ${
        row ? "items-center justify-between gap-2 px-3.5 py-2.5" : "flex-col items-start gap-0.5 px-3 py-2.5"
      } ${
        checked
          ? "border-[var(--accent)] bg-[var(--sel-bg)]"
          : "border-[var(--d200)] bg-[var(--card)] hover:border-[var(--muted)]"
      } ${className}`}
    >
      <span className={row ? "text-sm font-medium" : "text-sm font-semibold"}>{label}</span>
      <span className={`${row ? "text-xs" : "text-[11px]"} ${hintColor}`}>{hint}</span>
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
      <Switch.Content className="flex flex-1 flex-col items-start gap-0.5 text-left">
        <span className="text-sm font-normal text-[var(--fg)]">{label}</span>
        <span className="text-xs font-normal text-[var(--muted)]">{hint}</span>
      </Switch.Content>
      <Switch.Control>
        <Switch.Thumb />
      </Switch.Control>
    </Switch>
  );
}

export function Field({ label, value, children }: { label: string; value: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-baseline justify-between text-sm">
        <span>{label}</span>
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
