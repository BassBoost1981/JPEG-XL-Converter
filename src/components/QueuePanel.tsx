import { Button } from "@heroui/react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef } from "react";
import { backend } from "../api";
import { msgText } from "../i18n";
import { useLang, useT } from "../i18n/useT";
import { useApp } from "../state/store";
import { GRID_COLUMNS, QueueRow, ROW_HEIGHT } from "./QueueRow";

const BTN = "h-[30px] min-h-0 rounded-[10px] text-[13px] font-normal";

export function QueuePanel() {
  const mode = useApp((s) => s.settings.mode);
  const rows = useApp((s) => s.queues[s.settings.mode]);
  const selectedId = useApp((s) => s.selected[s.settings.mode]);
  const running = useApp((s) => s.run === "running" || s.run === "paused");
  const notice = useApp((s) => s.notice);
  const select = useApp((s) => s.select);
  const t = useT();
  const lang = useLang();
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
  const pickFiles = async () =>
    add(
      await backend.pickFiles(mode, {
        title: t("dialog.pickFiles"),
        filterImages: t("dialog.filterImages"),
      }),
    );
  const pickFolder = async () => {
    const folder = await backend.pickFolder(t("dialog.pickFolder"));
    if (folder) await add([folder]);
  };

  return (
    <>
      <div className="flex flex-none items-center gap-2 px-4 pb-2.5 pt-3.5">
        <span className="text-[15px] font-semibold">{t("queue.title")}</span>
        <span className="rounded-full bg-[var(--d100)] px-2 py-0.5 text-xs text-[var(--muted)]">
          {t(rows.length === 1 ? "queue.count.one" : "queue.count.other", { n: rows.length })}
        </span>
        <div className="flex-1" />
        <Button
          variant="outline"
          className={`${BTN} border-2 border-[var(--d200)] bg-transparent px-3 text-[var(--fg)]`}
          onPress={() => void pickFiles()}
        >
          {t("queue.files")}
        </Button>
        <Button
          variant="secondary"
          className={`${BTN} bg-[var(--info-bg)] px-3.5 text-[var(--info-fg)]`}
          onPress={() => void pickFolder()}
        >
          {t("queue.addFolder")}
        </Button>
        <Button
          variant="ghost"
          isDisabled={running}
          className={`${BTN} bg-transparent px-2.5 text-[#f31260]`}
          onPress={() => void useApp.getState().clear()}
        >
          {t("queue.clear")}
        </Button>
      </div>

      {notice && (
        <div role="status" className="mx-3 mb-2 rounded-lg bg-[var(--line)] px-3 py-2 text-xs">
          {msgText(lang, notice)}
        </div>
      )}

      <div
        className="mx-3 grid flex-none gap-3 rounded-lg bg-[var(--d100)] px-2.5 py-2 text-[11px] font-semibold uppercase tracking-[.03em] text-[var(--muted)]"
        style={{ gridTemplateColumns: GRID_COLUMNS }}
      >
        <span />
        <span>{t("col.file")}</span>
        <span>{t("col.original")}</span>
        <span>{t("col.result")}</span>
        <span>{mode === "enc" ? t("col.savings") : t("col.change")}</span>
        <span>{t("col.status")}</span>
      </div>

      {rows.length === 0 ? (
        <div className="mx-3 mb-3 mt-2.5 flex h-[140px] flex-none flex-col items-center justify-center gap-1 rounded-[14px] border-2 border-dashed border-[var(--d200)] text-[var(--muted)]">
          <span className="font-semibold text-[var(--fg)]">{t("queue.dropTitle")}</span>
          <span className="text-xs">{mode === "enc" ? t("queue.dropHintEnc") : t("queue.dropHintDec")}</span>
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
