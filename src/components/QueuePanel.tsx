import { Button } from "@heroui/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef } from "react";
import { backend } from "../api";
import { formatCount } from "../lib/format";
import { useApp } from "../state/store";
import { GRID_COLUMNS, QueueRow, ROW_HEIGHT } from "./QueueRow";

const ENC_HINT = "JPEG, PNG, GIF, TIFF, WebP, BMP";
const DEC_HINT = ".jxl-Dateien";

export function QueuePanel() {
  const mode = useApp((s) => s.settings.mode);
  const rows = useApp((s) => s.queues[s.settings.mode]);
  const selectedId = useApp((s) => s.selected[s.settings.mode]);
  const running = useApp((s) => s.run === "running" || s.run === "paused");
  const notice = useApp((s) => s.notice);
  const select = useApp((s) => s.select);
  const scrollRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 6,
    getItemKey: (i) => rows[i].id,
  });

  const add = async (paths: string[]) => {
    if (paths.length > 0) await useApp.getState().addPaths(paths);
  };
  const pickFiles = async () => add(await backend.pickFiles(mode));
  const pickFolder = async () => {
    const folder = await backend.pickFolder();
    if (folder) await add([folder]);
  };

  return (
    <>
      <div className="flex flex-none items-center gap-2 px-4 pb-2.5 pt-3.5">
        <span className="text-[15px] font-semibold">Warteschlange</span>
        <span className="rounded-full bg-[var(--line)] px-2 py-0.5 text-xs text-[var(--muted)]">
          {rows.length === 1 ? "1 Datei" : `${formatCount(rows.length)} Dateien`}
        </span>
        <div className="flex-1" />
        <Button size="sm" variant="outline" onPress={() => void pickFiles()}>
          Dateien
        </Button>
        <Button size="sm" variant="secondary" onPress={() => void pickFolder()}>
          Ordner hinzufügen
        </Button>
        <Button size="sm" variant="danger-soft" isDisabled={running} onPress={() => void useApp.getState().clear()}>
          Leeren
        </Button>
      </div>

      {notice && (
        <div role="status" className="mx-3 mb-2 rounded-lg bg-[var(--line)] px-3 py-2 text-xs">
          {notice}
        </div>
      )}

      <div
        className="mx-3 grid flex-none gap-3 rounded-lg bg-[var(--line)] px-2.5 py-2 text-[11px] font-semibold uppercase tracking-[.03em] text-[var(--muted)]"
        style={{ gridTemplateColumns: GRID_COLUMNS }}
      >
        <span />
        <span>Datei</span>
        <span>Original</span>
        <span>Ergebnis</span>
        <span>{mode === "enc" ? "Ersparnis" : "Änderung"}</span>
        <span>Status</span>
      </div>

      {rows.length === 0 ? (
        <div className="mx-3 mb-3 mt-2.5 flex h-[140px] flex-none flex-col items-center justify-center gap-1 rounded-[14px] border-2 border-dashed border-[var(--line)] text-[var(--muted)]">
          <span className="font-semibold text-[var(--fg)]">Bilder oder Ordner hierher ziehen</span>
          <span className="text-xs">{mode === "enc" ? ENC_HINT : DEC_HINT}</span>
        </div>
      ) : (
        <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto pb-3 pt-1">
          <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
            {virtualizer.getVirtualItems().map((v) => {
              const row = rows[v.index];
              return (
                <QueueRow
                  key={row.id}
                  row={row}
                  selected={row.id === selectedId}
                  style={{ height: v.size, transform: `translateY(${v.start}px)` }}
                  onSelect={select}
                />
              );
            })}
          </div>
        </div>
      )}
    </>
  );
}
