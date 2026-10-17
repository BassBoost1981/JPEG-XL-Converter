import { Tooltip } from "@heroui/react";
import { memo, useEffect, useState, type CSSProperties } from "react";
import { backend } from "../api";
import type { ItemStatus } from "../api/types";
import { msgText } from "../i18n";
import { useLang } from "../i18n/useT";
import { formatDelta, formatSize } from "../lib/format";
import type { Row } from "../state/store";

// 54 px row + 2 px gap, as in the design.
export const ROW_HEIGHT = 56;
export const GRID_COLUMNS = "40px minmax(0,1fr) 80px 80px 72px 110px";

// One lookup per item id, failures included: scrolling back never asks the backend again.
// At most MAX_THUMBS requests run at once; a request still waiting for its turn is dropped
// when its row unmounts (fast scrolling), a started one finishes and fills the cache.
const MAX_THUMBS = 3;

interface ThumbJob {
  id: number;
  promise: Promise<string | null>;
  start(): void;
  refs: number;
  started: boolean;
}

const thumbJobs = new Map<number, ThumbJob>();
const thumbQueue: ThumbJob[] = [];
let thumbsInFlight = 0;

function pumpThumbs(): void {
  while (thumbsInFlight < MAX_THUMBS && thumbQueue.length > 0) thumbQueue.shift()!.start();
}

function requestThumb(id: number): ThumbJob {
  let job = thumbJobs.get(id);
  if (!job) {
    let resolve!: (u: string | null) => void;
    const promise = new Promise<string | null>((r) => (resolve = r));
    const created: ThumbJob = {
      id,
      promise,
      refs: 0,
      started: false,
      start() {
        created.started = true;
        thumbsInFlight++;
        backend
          .thumbnail(id)
          .catch(() => null)
          .then((u) => {
            thumbsInFlight--;
            resolve(u);
            pumpThumbs();
          });
      },
    };
    job = created;
    thumbJobs.set(id, job);
    thumbQueue.push(job);
    pumpThumbs();
  }
  job.refs++;
  return job;
}

function releaseThumb(job: ThumbJob): void {
  if (--job.refs > 0 || job.started) return;
  const i = thumbQueue.indexOf(job);
  if (i >= 0) thumbQueue.splice(i, 1);
  thumbJobs.delete(job.id);
}

function useThumb(id: number): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    const job = requestThumb(id);
    void job.promise.then((u) => {
      if (live) setUrl(u);
    });
    return () => {
      live = false;
      releaseThumb(job);
    };
  }, [id]);
  return url;
}

interface Look {
  label: string;
  color: string;
  bg: string;
}

const GREY = { color: "var(--muted)", bg: "var(--d100)" };
const BLUE = { color: "var(--run)", bg: "var(--run-bg)" };
const GREEN = { color: "var(--good)", bg: "var(--good-bg)" };
const RED = { color: "#f31260", bg: "color-mix(in srgb, #f31260 15%, transparent)" };

function look(status: ItemStatus, progress: number): Look {
  switch (status) {
    case "waiting":
      return { label: "Wartet", ...GREY };
    case "running":
      return { label: `${progress} %`, ...BLUE };
    case "done":
      return { label: "Fertig", ...GREEN };
    case "doneWithNote":
      return { label: "Fertig mit Hinweis", ...GREEN };
    case "skipped":
      return { label: "Übersprungen", ...GREY };
    case "error":
      return { label: "Fehler", ...RED };
    case "verifyFailed":
      return { label: "Prüfung fehlgeschlagen", ...RED };
  }
}

function InfoIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true">
      <circle cx="8" cy="8" r="6.5" />
      <path d="M8 7.2V11.2M8 4.8V5" strokeLinecap="round" />
    </svg>
  );
}

function StatusChip({ row }: { row: Row }) {
  const lang = useLang();
  const l = look(row.status, row.progress);
  const chip = (
    <span
      className="inline-flex items-center gap-1 self-start rounded-full px-2 py-px text-xs"
      style={{ background: l.bg, color: l.color }}
    >
      {l.label}
      {row.status === "doneWithNote" && <InfoIcon />}
    </span>
  );
  if (row.notes.length === 0) return chip;
  return (
    <Tooltip>
      <Tooltip.Trigger>{chip}</Tooltip.Trigger>
      <Tooltip.Content>
        {row.notes.map((note, i) => (
          <div key={i}>{msgText(lang, note)}</div>
        ))}
      </Tooltip.Content>
    </Tooltip>
  );
}

function baseName(path: string): string {
  return path.slice(Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/")) + 1);
}

function extension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot < 0 ? "" : name.slice(dot + 1).toLowerCase();
}

interface Props {
  row: Row;
  selected: boolean;
  style: CSSProperties;
  onSelect(id: number): void;
}

export const QueueRow = memo(function QueueRow({ row, selected, style, onSelect }: Props) {
  const thumb = useThumb(row.id);
  const lang = useLang();
  const { entry } = row;
  const name = baseName(entry.path);
  const isDone = row.status === "done" || row.status === "doneWithNote";
  const out = isDone && row.outSize !== undefined ? row.outSize : null;
  const l = look(row.status, row.progress);
  const barWidth = row.status === "waiting" ? 0 : row.status === "running" ? row.progress : 100;
  const smaller = out !== null && out < entry.size;
  const deltaLook = smaller ? { color: "var(--good-fg)", bg: "var(--good-bg)" } : GREY;

  return (
    <div
      data-testid="queue-row"
      onClick={() => onSelect(row.id)}
      className="absolute left-0 top-0 w-full cursor-pointer px-3"
      style={style}
    >
      <div
        className="grid items-center gap-3 rounded-[10px] px-2.5"
        style={{
          gridTemplateColumns: GRID_COLUMNS,
          height: ROW_HEIGHT - 2,
          marginTop: 2,
          background: selected ? "var(--sel-bg)" : undefined,
        }}
      >
        <div
          className="flex h-10 w-10 items-end justify-center overflow-hidden rounded-[10px] pb-[3px]"
          style={{
            background: thumb
              ? `center / cover no-repeat url("${thumb}")`
              : "repeating-linear-gradient(135deg, var(--stripe-a) 0 4px, var(--stripe-b) 4px 8px)",
          }}
        >
          {!thumb && <span className="text-[8px] font-semibold text-[var(--muted)]">{extension(name)}</span>}
        </div>
        <div className="min-w-0">
          <div className="truncate text-sm">{name}</div>
          <div className="text-xs text-[var(--muted)]">
            {entry.width} × {entry.height}
          </div>
        </div>
        <span className="text-[13px]">{formatSize(entry.size, lang)}</span>
        <span className="text-[13px]">{out === null ? "—" : formatSize(out, lang)}</span>
        {out === null ? (
          <span className="justify-self-start rounded-full px-2 py-0.5 text-xs font-medium text-[var(--muted)]">—</span>
        ) : (
          <span
            className="justify-self-start rounded-full px-2 py-0.5 text-xs font-medium"
            style={{ background: deltaLook.bg, color: deltaLook.color }}
          >
            {formatDelta(entry.size, out)}
          </span>
        )}
        <div className="flex flex-col gap-[5px]">
          <StatusChip row={row} />
          <div className="h-1 overflow-hidden rounded-full bg-[var(--d200)]">
            <div className="h-full rounded-full" style={{ width: `${barWidth}%`, background: l.color }} />
          </div>
        </div>
      </div>
    </div>
  );
});
