import { Tooltip } from "@heroui/react";
import { memo, useEffect, useState, type CSSProperties } from "react";
import { backend } from "../api";
import type { ItemStatus, Mode } from "../api/types";
import { formatDelta, formatSize } from "../lib/format";
import type { Row } from "../state/store";

export const ROW_HEIGHT = 54;
export const GRID_COLUMNS = "40px minmax(0,1fr) 80px 80px 72px 110px";

// One lookup per item id, failures included: scrolling back never asks the backend again.
const thumbCache = new Map<number, Promise<string | null>>();

function loadThumb(id: number): Promise<string | null> {
  let p = thumbCache.get(id);
  if (!p) {
    p = backend.thumbnail(id).catch(() => null);
    thumbCache.set(id, p);
  }
  return p;
}

function useThumb(id: number): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    void loadThumb(id).then((u) => {
      if (live) setUrl(u);
    });
    return () => {
      live = false;
    };
  }, [id]);
  return url;
}

interface Look {
  label: string;
  color: string;
  bg: string;
}

const GREY = { color: "var(--muted)", bg: "color-mix(in srgb, var(--muted) 15%, transparent)" };
const BLUE = { color: "var(--accent)", bg: "color-mix(in srgb, var(--accent) 15%, transparent)" };
const GREEN = { color: "#17c964", bg: "color-mix(in srgb, #17c964 15%, transparent)" };
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
  const l = look(row.status, row.progress);
  const chip = (
    <span
      className="inline-flex items-center gap-1 self-start rounded-full px-2 text-xs"
      style={{ background: l.bg, color: l.color }}
    >
      {l.label}
      {row.status === "doneWithNote" && <InfoIcon />}
    </span>
  );
  if (!row.note) return chip;
  return (
    <Tooltip>
      <Tooltip.Trigger>{chip}</Tooltip.Trigger>
      <Tooltip.Content>{row.note}</Tooltip.Content>
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
  mode: Mode;
  selected: boolean;
  style: CSSProperties;
  onSelect(id: number): void;
}

export const QueueRow = memo(function QueueRow({ row, selected, style, onSelect }: Props) {
  const thumb = useThumb(row.id);
  const { entry } = row;
  const name = baseName(entry.path);
  const isDone = row.status === "done" || row.status === "doneWithNote";
  const out = isDone && row.outSize !== undefined ? row.outSize : null;
  const l = look(row.status, row.progress);
  const barWidth = row.status === "waiting" ? 0 : row.status === "running" ? row.progress : 100;
  const smaller = out !== null && out < entry.size;
  const deltaLook = smaller ? GREEN : GREY;

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
          background: selected ? "color-mix(in srgb, var(--accent) 12%, transparent)" : undefined,
        }}
      >
        <div
          className="flex h-10 w-10 items-end justify-center overflow-hidden rounded-[10px] pb-[3px]"
          style={{
            background: thumb
              ? `center / cover no-repeat url("${thumb}")`
              : "repeating-linear-gradient(135deg, var(--line) 0 4px, transparent 4px 8px)",
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
        <span className="text-[13px]">{formatSize(entry.size)}</span>
        <span className="text-[13px]">{out === null ? "—" : formatSize(out)}</span>
        {out === null ? (
          <span className="text-[13px]">—</span>
        ) : (
          <span
            className="justify-self-start rounded-full px-2 text-xs font-medium"
            style={{ background: deltaLook.bg, color: deltaLook.color }}
          >
            {formatDelta(entry.size, out)}
          </span>
        )}
        <div className="flex flex-col gap-[5px]">
          <StatusChip row={row} />
          <div className="h-1 overflow-hidden rounded-full bg-[var(--line)]">
            <div className="h-full rounded-full" style={{ width: `${barWidth}%`, background: l.color }} />
          </div>
        </div>
      </div>
    </div>
  );
});
