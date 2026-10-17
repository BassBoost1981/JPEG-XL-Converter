import { useEffect, useMemo, useRef, useState } from "react";
import { backend } from "../api";
import type { InputKind, Mode, OutputFormat, PreviewDto } from "../api/types";
import { useLang, useT } from "../i18n/useT";
import { formatDistance, formatSize } from "../lib/format";
import { toJobSettings } from "../state/settings";
import { useApp } from "../state/store";

const DEBOUNCE_MS = 300;
const FALLBACK_SIZE = { w: 800, h: 240 };

const KIND_LABEL: Record<InputKind, string> = {
  jpeg: "JPG",
  png: "PNG",
  gif: "GIF",
  tiff: "TIFF",
  webp: "WEBP",
  bmp: "BMP",
  jxl: "JXL",
};

// "auto" shows the catalog's "Original".
const FORMAT_LABEL: Record<Exclude<OutputFormat, "auto">, string> = {
  png: "PNG",
  jpg: "JPG",
  webp: "WEBP",
  tiff: "TIFF",
};

const baseName = (path: string) => path.split(/[\\/]/).pop() ?? path;

const TAG = "pointer-events-none absolute top-2.5 rounded-full px-2.5 py-1 text-xs";

export function CompareSlider() {
  const mode: Mode = useApp((s) => s.settings.mode);
  const t = useT();
  const lang = useLang();
  const settings = useApp((s) => s.settings);
  const row = useApp((s) => {
    const id = s.selected[s.settings.mode];
    return id === null ? undefined : s.queues[s.settings.mode].find((r) => r.id === id);
  });
  const itemId = row?.id ?? null;

  // Only what changes the preview may trigger a reload (theme, window state etc. must not).
  // The recycle-bin switch never touches the pixels; metadata can (EXIF orientation).
  const job = useMemo(() => toJobSettings(settings), [settings]);
  const jobKey = JSON.stringify([mode, { ...job, trashOriginals: undefined }]);

  const [split, setSplit] = useState(50);
  const [preview, setPreview] = useState<PreviewDto | null>(null);
  const [failed, setFailed] = useState(false);
  const field = useRef<HTMLDivElement>(null);
  const lastId = useRef<number | null>(null);
  const jobRef = useRef(job);
  jobRef.current = job;

  useEffect(() => {
    if (itemId === null) {
      lastId.current = null;
      setPreview(null);
      setFailed(false);
      return;
    }
    const previousId = lastId.current;
    const selectionChanged = previousId !== itemId;
    lastId.current = itemId;
    if (selectionChanged) {
      setPreview(null);
      setFailed(false);
    }

    let stale = false;
    let started = false;
    const load = () => {
      started = true;
      const el = field.current;
      const dpr = window.devicePixelRatio || 1;
      const viewW = Math.round((el?.clientWidth || FALLBACK_SIZE.w) * dpr);
      const viewH = Math.round((el?.clientHeight || FALLBACK_SIZE.h) * dpr);
      backend
        .preview(itemId, mode, jobRef.current, viewW, viewH)
        .then((p) => {
          if (stale) return;
          setPreview(p);
          setFailed(false);
        })
        .catch(() => {
          if (stale) return;
          setPreview(null);
          setFailed(true);
        });
    };

    let timer: ReturnType<typeof setTimeout> | undefined;
    if (selectionChanged) load();
    else timer = setTimeout(load, DEBOUNCE_MS);

    return () => {
      stale = true;
      clearTimeout(timer);
      // A request that never started must not make the next run look like a repeat.
      if (!started) lastId.current = previousId;
    };
  }, [itemId, jobKey, mode]);

  const finishedSize =
    row && (row.status === "done" || row.status === "doneWithNote") && row.outSize !== undefined
      ? ` · ${formatSize(row.outSize, lang)}`
      : "";
  const leftTag = row
    ? `${mode === "enc" ? KIND_LABEL[row.entry.kind] : "JXL"} · ${formatSize(row.entry.size, lang)}`
    : "";
  const rightTag = row
    ? mode === "enc"
      ? `${job.encode.distance === 0 ? t("compare.jxlLossless") : `JXL · d ${formatDistance(job.encode.distance)}`}${finishedSize}`
      : `${job.decode.format === "auto" ? t("col.original") : FORMAT_LABEL[job.decode.format]}${finishedSize}`
    : "";

  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex items-center gap-2 px-1 py-0.5">
        <span className="font-semibold">{t("compare.title")}</span>
        {row && (
          <span className="text-xs text-[var(--muted)]">
            {baseName(row.entry.path)} · {row.entry.width} × {row.entry.height}
          </span>
        )}
      </div>
      <div ref={field} className="relative h-[240px] overflow-hidden rounded-[10px] bg-[repeating-linear-gradient(135deg,var(--stripe-a)_0_10px,var(--stripe-b)_10px_20px)]">
        {!row ? (
          <div className="absolute inset-0 flex items-center justify-center text-[var(--muted)]">
            {t("compare.none")}
          </div>
        ) : (
          <>
            {preview && (
              <>
                <img
                  src={preview.before}
                  alt={t("compare.before")}
                  draggable={false}
                  className="absolute inset-0 h-full w-full object-cover"
                />
                <img
                  data-testid="compare-after"
                  src={preview.after}
                  alt={t("compare.after")}
                  draggable={false}
                  className="absolute inset-0 h-full w-full object-cover"
                  style={{ clipPath: `inset(0 0 0 ${split}%)` }}
                />
              </>
            )}
            {failed && (
              <div className="absolute inset-0 flex items-center justify-center text-[var(--muted)]">
                {t("preview.unavailable")}
              </div>
            )}
            <span className={`${TAG} left-2.5 bg-[var(--overlay)]`}>{leftTag}</span>
            <span className={`${TAG} right-2.5 bg-[var(--accent)] text-white`}>{rightTag}</span>
            <div
              data-testid="compare-divider"
              className="pointer-events-none absolute inset-y-0 -ml-px w-0.5 bg-white"
              style={{ left: `${split}%` }}
            >
              <div className="absolute left-1/2 top-1/2 -ml-3.5 -mt-3.5 flex h-7 w-7 items-center justify-center rounded-full bg-white text-xs text-[#11181C] shadow-[0_2px_8px_rgba(0,0,0,.25)]">
                ⇔
              </div>
            </div>
            <input
              type="range"
              min={0}
              max={100}
              value={split}
              aria-label={t("compare.divider")}
              onChange={(e) => setSplit(Number(e.target.value))}
              className="absolute inset-0 m-0 h-full w-full cursor-ew-resize opacity-0"
            />
          </>
        )}
      </div>
    </div>
  );
}
