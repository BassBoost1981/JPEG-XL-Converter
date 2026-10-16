import type { Mode, RunSummary } from "../api/types";

export type RunState = "idle" | "running" | "paused" | "finished";

const KB = 1024;
const MB = KB * 1024;
const GB = MB * 1024;

const nf = new Intl.NumberFormat("de-DE");

function comma(n: number, digits: number): string {
  return n.toFixed(digits).replace(".", ",");
}

/** Binary units (1 MB = 1 048 576 B), decimal comma. */
export function formatSize(bytes: number): string {
  if (bytes < MB) return `${Math.round(bytes / KB)} KB`;
  if (bytes < 10 * MB) return `${comma(bytes / MB, 2)} MB`;
  if (bytes < GB) return `${comma(bytes / MB, 1)} MB`;
  return `${comma(bytes / GB, 1)} GB`;
}

export function formatDelta(inB: number, outB: number): string {
  if (inB <= 0) return "0 %";
  const pct = Math.round(((outB - inB) / inB) * 100);
  if (pct < 0) return `−${-pct} %`;
  if (pct > 0) return `+${pct} %`;
  return "0 %";
}

export function formatCount(n: number): string {
  return nf.format(n);
}

/** Decimal point on purpose (as in the design): 0.5, 1.2, 1.18. */
export function formatDistance(d: number): string {
  return String(Number(d.toFixed(2)));
}

export function summaryLine(s: Pick<RunSummary, "done" | "skipped" | "errors" | "bytesIn" | "bytesOut">): string {
  return [
    `${formatCount(s.done)} fertig`,
    `${formatCount(s.skipped)} übersprungen`,
    `${formatCount(s.errors)} Fehler`,
    `${formatSize(s.bytesIn)} → ${formatSize(s.bytesOut)}`,
  ].join(" · ");
}

export function footerHint(p: { done: number; total: number; bytesIn?: number; bytesOut?: number }): string {
  if (p.done === 0) return "Ersparnis erscheint nach der ersten fertigen Datei";
  const head = `${formatCount(p.done)} von ${formatCount(p.total)} fertig`;
  if (p.bytesIn === undefined || p.bytesOut === undefined) return head;
  return `${head} · ${formatSize(p.bytesIn)} → ${formatSize(p.bytesOut)}`;
}

export function ctaLabel(p: { mode: Mode; run: RunState; waiting: number; failed: number }): string {
  if (p.run === "running") return "Anhalten";
  if (p.run === "paused") return "Fortsetzen";
  if (p.waiting === 0 && p.run === "finished") {
    return p.failed > 0 ? "Fehlgeschlagene erneut versuchen" : "Alles erledigt";
  }
  const files = `${formatCount(p.waiting)} ${p.waiting === 1 ? "Datei" : "Dateien"}`;
  return p.mode === "enc" ? `${files} in JPEG XL konvertieren` : `${files} zurückkonvertieren`;
}
