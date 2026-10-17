import type { Lang, Mode, RunSummary } from "../api/types";
import { formatNumber, translate } from "../i18n";

export type RunState = "idle" | "running" | "paused" | "finished";

const KB = 1024;
const MB = KB * 1024;
const GB = MB * 1024;

/** `.one` for exactly 1, otherwise `.other` (German and English share this rule). */
const plural = (n: number): "one" | "other" => (n === 1 ? "one" : "other");

function decimal(n: number, digits: number, lang: Lang): string {
  const text = n.toFixed(digits);
  return lang === "de" ? text.replace(".", ",") : text;
}

/** Binary units (1 MB = 1 048 576 B); decimal comma in German, point in English. */
export function formatSize(bytes: number, lang: Lang): string {
  if (bytes < MB) return `${Math.round(bytes / KB)} KB`;
  if (bytes < 10 * MB) return `${decimal(bytes / MB, 2, lang)} MB`;
  if (bytes < GB) return `${decimal(bytes / MB, 1, lang)} MB`;
  return `${decimal(bytes / GB, 1, lang)} GB`;
}

/** Language-neutral: real minus sign (U+2212), plus sign. */
export function formatDelta(inB: number, outB: number): string {
  if (inB <= 0) return "0 %";
  const pct = Math.round(((outB - inB) / inB) * 100);
  if (pct < 0) return `−${-pct} %`;
  if (pct > 0) return `+${pct} %`;
  return "0 %";
}

/** Thousands separator: dot in German, comma in English. */
export function formatCount(n: number, lang: Lang): string {
  return formatNumber(lang, n);
}

/** Decimal point in both languages (as in the design): 0.5, 1.2, 1.18. */
export function formatDistance(d: number): string {
  return String(Number(d.toFixed(2)));
}

export function summaryLine(
  s: Pick<RunSummary, "done" | "skipped" | "errors" | "bytesIn" | "bytesOut">,
  lang: Lang,
): string {
  return [
    translate(lang, "footer.summaryDone", { n: formatCount(s.done, lang) }),
    translate(lang, "footer.summarySkipped", { n: formatCount(s.skipped, lang) }),
    translate(lang, `footer.summaryErrors.${plural(s.errors)}`, { n: formatCount(s.errors, lang) }),
    `${formatSize(s.bytesIn, lang)} → ${formatSize(s.bytesOut, lang)}`,
  ].join(" · ");
}

export function footerHint(
  p: { done: number; total: number; bytesIn?: number; bytesOut?: number },
  lang: Lang,
): string {
  if (p.done === 0) return translate(lang, "footer.hintEmpty");
  const head = translate(lang, "footer.doneOf", {
    done: formatCount(p.done, lang),
    total: formatCount(p.total, lang),
  });
  if (p.bytesIn === undefined || p.bytesOut === undefined) return head;
  return `${head} · ${formatSize(p.bytesIn, lang)} → ${formatSize(p.bytesOut, lang)}`;
}

/** What the main button does; `empty` is "nothing waiting yet" (button disabled). */
export type CtaState = "start" | "pause" | "resume" | "retry" | "allDone" | "empty";

export interface CtaInput {
  mode: Mode;
  run: RunState;
  waiting: number;
  failed: number;
}

export function ctaState(p: CtaInput): CtaState {
  if (p.run === "running") return "pause";
  if (p.run === "paused") return "resume";
  if (p.waiting > 0) return "start";
  if (p.run === "finished") return p.failed > 0 ? "retry" : "allDone";
  return "empty";
}

export function ctaLabel(p: CtaInput, lang: Lang): string {
  switch (ctaState(p)) {
    case "pause":
      return translate(lang, "cta.stop");
    case "resume":
      return translate(lang, "cta.resume");
    case "retry":
      return translate(lang, "cta.retry");
    case "allDone":
      return translate(lang, "cta.allDone");
    case "start":
    case "empty":
      return translate(lang, `cta.${p.mode}.${plural(p.waiting)}`, { n: formatCount(p.waiting, lang) });
  }
}
