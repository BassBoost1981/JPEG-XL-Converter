import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import type { Backend } from "./backend";
import type { UiSettings } from "../state/settings";
import type { AddResult, ItemEvent, JobSettings, Mode, PauseReason, PreviewDto, RunSummary, ToolStatus } from "./types";

const ENC_EXTENSIONS = ["jpg", "jpeg", "png", "gif", "tif", "tiff", "webp", "bmp"];

/** Tauri registers listeners asynchronously; the returned disposer works at any time. */
function disposer(pending: Promise<() => void>): () => void {
  return () => {
    void pending.then((unlisten) => unlisten());
  };
}

function listenTo<T>(event: string, cb: (payload: T) => void): () => void {
  return disposer(listen<T>(event, (e) => cb(e.payload)));
}

export const tauriBackend: Backend = {
  checkTools: () => invoke<ToolStatus>("check_tools"),
  addPaths: (paths, mode) => invoke<AddResult>("add_paths", { paths, mode }),
  clearQueue: (mode) => invoke<void>("clear_queue", { mode }),
  startRun: (mode: Mode, settings: JobSettings) => invoke<void>("start_run", { mode, settings }),
  retryFailed: (mode: Mode, settings: JobSettings) => invoke<void>("retry_failed", { mode, settings }),
  pauseRun: () => invoke<void>("pause_run"),
  resumeRun: () => invoke<void>("resume_run"),
  cancelRun: () => invoke<void>("cancel_run"),
  preview: (itemId, mode, settings, viewW, viewH) =>
    invoke<PreviewDto>("preview", { itemId, mode, settings, viewW, viewH }),
  thumbnail: (itemId) => invoke<string>("thumbnail", { itemId }),
  loadSettings: () => invoke<UiSettings | null>("load_settings"),
  saveSettings: (value: UiSettings) => invoke<void>("save_settings", { value }),
  openLog: (path) => invoke<void>("open_log", { path }),

  async pickFiles(mode) {
    const filters =
      mode === "enc"
        ? [{ name: "Bilder", extensions: ENC_EXTENSIONS }]
        : [{ name: "JPEG XL", extensions: ["jxl"] }];
    const picked = await open({ multiple: true, directory: false, filters });
    return picked ?? [];
  },
  async pickFolder() {
    const picked = await open({ multiple: false, directory: true });
    return picked ?? null;
  },

  onItemProgress: (cb: (e: ItemEvent) => void) => listenTo<ItemEvent>("item-progress", cb),
  onRunPaused: (cb: (r: PauseReason) => void) => listenTo<PauseReason>("run-paused", cb),
  onRunFinished: (cb: (s: RunSummary) => void) => listenTo<RunSummary>("run-finished", cb),
  onFilesDropped: (cb) =>
    disposer(
      getCurrentWebview().onDragDropEvent((e) => {
        if (e.payload.type === "drop") cb(e.payload.paths);
      }),
    ),

  minimize: () => void getCurrentWindow().minimize(),
  toggleMaximize: () => void getCurrentWindow().toggleMaximize(),
  close: () => void getCurrentWindow().close(),
  onCloseRequested: (cb) =>
    disposer(
      getCurrentWindow().onCloseRequested((e) => {
        if (!cb()) e.preventDefault();
      }),
    ),
};
