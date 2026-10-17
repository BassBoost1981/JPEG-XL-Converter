import { Button, ProgressBar } from "@heroui/react";
import { backend } from "../api";
import { ctaLabel, footerHint, formatDelta, summaryLine } from "../lib/format";
import { useApp } from "../state/store";

const DONE = new Set(["done", "doneWithNote"]);
const FAILED = new Set(["error", "verifyFailed"]);

/** `D` or `D:` → `D:`; a network share (`\\server\share`) stays as it is. */
const driveLabel = (drive: string) => (/^[A-Za-z]:?$/.test(drive) ? `${drive[0]}:` : drive);

export function RunFooter() {
  const mode = useApp((s) => s.settings.mode);
  const rows = useApp((s) => s.queues[s.settings.mode]);
  const run = useApp((s) => s.run);
  const pauseReason = useApp((s) => s.pauseReason);
  const summary = useApp((s) => s.summary);
  const toolsMissing = useApp((s) => s.tools?.ok === false);
  const { start, pause, resume, retryFailed } = useApp.getState();

  let done = 0;
  let waiting = 0;
  let failed = 0;
  let bytesIn = 0;
  let bytesOut = 0;
  let totalBytes = 0;
  let doneBytes = 0;
  for (const r of rows) {
    totalBytes += r.entry.size;
    if (r.status === "waiting") waiting++;
    if (FAILED.has(r.status)) failed++;
    if (DONE.has(r.status)) {
      done++;
      bytesIn += r.entry.size;
      bytesOut += r.outSize ?? 0;
    }
    // Skipped and failed files are finished business, running ones count partially.
    doneBytes += r.entry.size * (r.status === "running" ? r.progress / 100 : r.status === "waiting" ? 0 : 1);
  }
  const percent = totalBytes > 0 ? Math.round((doneBytes / totalBytes) * 100) : 0;

  const shownIn = summary ? summary.bytesIn : bytesIn;
  const shownOut = summary ? summary.bytesOut : bytesOut;
  const hasResult = summary ? summary.done > 0 : done > 0;
  const saved = hasResult && shownIn > 0 ? formatDelta(shownIn, shownOut) : "–";
  const line = summary
    ? summaryLine(summary)
    : footerHint({ done, total: rows.length, bytesIn, bytesOut });

  const label = ctaLabel({ mode, run, waiting, failed });
  const canStart = run === "idle" || (run === "finished" && waiting > 0);
  const disabled = toolsMissing || (run === "idle" && waiting === 0) || label === "Alles erledigt";

  const onPress = () => {
    if (run === "running") void pause();
    else if (run === "paused") void resume();
    else if (canStart) void start();
    else if (failed > 0) void retryFailed();
  };

  return (
    <div className="flex flex-none flex-col gap-3 border-t border-[var(--line)] px-[18px] py-4">
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-pretty text-xs text-[var(--muted)]">{line}</span>
        <span
          className="text-[22px] font-semibold"
          style={{ color: saved.startsWith("−") ? "var(--good)" : "var(--muted)" }}
        >
          {saved}
        </span>
      </div>
      <ProgressBar aria-label="Gesamtfortschritt" value={percent} size="sm" className="h-2 gap-0">
        <ProgressBar.Track className="h-2 rounded-full bg-[var(--d200)]">
          <ProgressBar.Fill />
        </ProgressBar.Track>
      </ProgressBar>
      {pauseReason?.type === "diskFull" && (
        <p className="m-0 text-xs text-[var(--danger,#f31260)]">
          Kein Platz mehr auf Laufwerk {driveLabel(pauseReason.drive)}
        </p>
      )}
      {summary?.logPath && (
        <Button size="sm" variant="ghost" onPress={() => void backend.openLog(summary.logPath!)}>
          Protokoll öffnen
        </Button>
      )}
      <Button
        isDisabled={disabled}
        onPress={onPress}
        className="h-12 w-full rounded-[14px] bg-[var(--accent)] text-[15px] font-medium text-white shadow-[0_6px_16px_rgba(0,111,238,.35)] hover:opacity-90 data-[disabled]:opacity-50 data-[disabled]:shadow-none"
      >
        {label}
      </Button>
    </div>
  );
}
