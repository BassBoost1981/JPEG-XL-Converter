import { create } from "zustand";
import { backend } from "../api";
import type { ItemEvent, ItemStatus, Mode, PauseReason, QueueItem, RunSummary, ToolStatus } from "../api/types";
import type { RunState } from "../lib/format";
import { DEFAULT_SETTINGS, restoreSettings, toJobSettings, type UiSettings } from "./settings";

export type Row = QueueItem & {
  status: ItemStatus;
  progress: number;
  outSize?: number;
  note?: string;
  thumb?: string;
};

export interface AppState {
  settings: UiSettings;
  queues: Record<Mode, Row[]>;
  selected: Record<Mode, number | null>;
  run: RunState;
  pauseReason: PauseReason | null;
  summary: RunSummary | null;
  tools: ToolStatus | null;
  notice: string | null;

  addPaths(paths: string[]): Promise<void>;
  clear(): Promise<void>;
  start(): Promise<void>;
  pause(): Promise<void>;
  resume(): Promise<void>;
  retryFailed(): Promise<void>;
  select(id: number | null): void;
  updateSettings(patch: Partial<UiSettings>): void;
  applyEvent(e: ItemEvent): void;
  /** `run-paused` event from the backend. */
  paused(reason: PauseReason): void;
  /** `run-finished` event from the backend. */
  finished(summary: RunSummary): void;
  /** Loads tools and settings and subscribes to backend events. Returns the unsubscribe function. */
  init(): Promise<() => void>;
}

const errorMessage = (e: unknown): string => (e instanceof Error ? e.message : String(e));

const SAVE_DELAY_MS = 500;
let saveTimer: ReturnType<typeof setTimeout> | undefined;

const isFailed = (s: ItemStatus) => s === "error" || s === "verifyFailed";

export const useApp = create<AppState>()((set, get) => ({
  settings: DEFAULT_SETTINGS,
  queues: { enc: [], dec: [] },
  selected: { enc: null, dec: null },
  run: "idle",
  pauseReason: null,
  summary: null,
  tools: null,
  notice: null,

  async addPaths(paths) {
    const mode = get().settings.mode;
    try {
      const { items, rejected } = await backend.addPaths(paths, mode);
      set((st) => {
        const known = new Set(st.queues[mode].map((r) => r.id));
        const fresh: Row[] = items
          .filter((i) => !known.has(i.id))
          .map((i) => ({ ...i, status: "waiting", progress: 0 }));
        const queue = [...st.queues[mode], ...fresh];
        return {
          queues: { ...st.queues, [mode]: queue },
          selected: { ...st.selected, [mode]: st.selected[mode] ?? queue[0]?.id ?? null },
          // Rows added after a finished run are waiting again.
          run: st.run === "finished" && fresh.length > 0 ? "idle" : st.run,
          notice:
            rejected > 0
              ? rejected === 1
                ? "1 Datei passt nicht zu diesem Modus und wurde nicht aufgenommen."
                : `${rejected} Dateien passen nicht zu diesem Modus und wurden nicht aufgenommen.`
              : st.notice,
        };
      });
    } catch (e) {
      set({ notice: errorMessage(e) });
    }
  },

  async clear() {
    const { run, settings } = get();
    if (run === "running" || run === "paused") return;
    const mode = settings.mode;
    try {
      await backend.clearQueue(mode);
    } catch (e) {
      set({ notice: errorMessage(e) });
      return;
    }
    set((st) => ({
      queues: { ...st.queues, [mode]: [] },
      selected: { ...st.selected, [mode]: null },
      run: "idle",
      summary: null,
      pauseReason: null,
    }));
  },

  async start() {
    const { settings } = get();
    set({ run: "running", summary: null, pauseReason: null, notice: null });
    try {
      await backend.startRun(settings.mode, toJobSettings(settings));
    } catch (e) {
      set({ run: "idle", notice: errorMessage(e) });
    }
  },

  async pause() {
    try {
      await backend.pauseRun();
    } catch (e) {
      set({ notice: errorMessage(e) });
    }
  },

  async resume() {
    set({ run: "running", pauseReason: null });
    try {
      await backend.resumeRun();
    } catch (e) {
      set({ run: "paused", notice: errorMessage(e) });
    }
  },

  async retryFailed() {
    const { settings } = get();
    const mode = settings.mode;
    set((st) => ({
      run: "running",
      summary: null,
      pauseReason: null,
      queues: {
        ...st.queues,
        [mode]: st.queues[mode].map((r) =>
          isFailed(r.status) ? { ...r, status: "waiting", progress: 0, outSize: undefined, note: undefined } : r,
        ),
      },
    }));
    try {
      await backend.retryFailed(mode, toJobSettings(settings));
    } catch (e) {
      set({ run: "finished", notice: errorMessage(e) });
    }
  },

  select(id) {
    const mode = get().settings.mode;
    set((st) => ({ selected: { ...st.selected, [mode]: id } }));
  },

  updateSettings(patch) {
    const settings = { ...get().settings, ...patch };
    set({ settings });
    // Debounced (trailing): sliders fire many updates, only the latest state is persisted.
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      saveTimer = undefined;
      backend.saveSettings(get().settings).catch((e) => set({ notice: errorMessage(e) }));
    }, SAVE_DELAY_MS);
  },

  applyEvent(e) {
    set((st) => {
      for (const mode of ["enc", "dec"] as const) {
        const i = st.queues[mode].findIndex((r) => r.id === e.id);
        if (i < 0) continue;
        const queue = st.queues[mode].slice();
        queue[i] = {
          ...queue[i],
          status: e.status,
          progress: e.progress,
          outSize: e.outSize ?? undefined,
          note: e.note ?? undefined,
        };
        return { queues: { ...st.queues, [mode]: queue } };
      }
      return st;
    });
  },

  paused(reason) {
    set({ run: "paused", pauseReason: reason });
  },

  finished(summary) {
    set({ run: "finished", summary, pauseReason: null });
  },

  async init() {
    const unsubs = [
      backend.onItemProgress((e) => get().applyEvent(e)),
      backend.onRunPaused((r) => get().paused(r)),
      backend.onRunFinished((s) => get().finished(s)),
      backend.onFilesDropped((paths) => void get().addPaths(paths)),
    ];
    try {
      set({ tools: await backend.checkTools() });
    } catch (e) {
      set({ notice: errorMessage(e) });
    }
    try {
      const saved = await backend.loadSettings();
      // The recycle-bin switch always starts off (Global Constraints).
      if (saved) set({ settings: { ...restoreSettings(saved), trashOriginals: false } });
    } catch {
      // Unreadable settings: keep defaults.
    }
    // A pending save is written when the page goes away; losing it would only lose the last change.
    const flush = () => {
      if (saveTimer === undefined) return;
      clearTimeout(saveTimer);
      saveTimer = undefined;
      backend.saveSettings(get().settings).catch(() => {});
    };
    window.addEventListener("pagehide", flush);
    return () => {
      window.removeEventListener("pagehide", flush);
      unsubs.forEach((u) => u());
    };
  },
}));
