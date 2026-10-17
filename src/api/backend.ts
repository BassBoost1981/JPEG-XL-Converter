import type {
  AddResult,
  ItemEvent,
  JobSettings,
  Lang,
  Mode,
  PauseReason,
  PreviewDto,
  RunSummary,
  StorageInfo,
  ToolStatus,
} from "./types";
import type { UiSettings } from "../state/settings";

/**
 * An error a method throws is a `Msg` (key plus parameters), not a string:
 * the UI shows it in the current language.
 */
export interface Backend {
  checkTools(): Promise<ToolStatus>;
  addPaths(paths: string[], mode: Mode): Promise<AddResult>;
  clearQueue(mode: Mode): Promise<void>;
  /** `lang` is the language of the run log. */
  startRun(mode: Mode, s: JobSettings, lang: Lang): Promise<void>;
  retryFailed(mode: Mode, s: JobSettings, lang: Lang): Promise<void>;
  pauseRun(): Promise<void>;
  resumeRun(): Promise<void>;
  cancelRun(): Promise<void>;
  preview(itemId: number, mode: Mode, s: JobSettings, viewW: number, viewH: number): Promise<PreviewDto>;
  thumbnail(itemId: number): Promise<string>;
  loadSettings(): Promise<UiSettings | null>;
  saveSettings(s: UiSettings): Promise<void>;
  openLog(path: string): Promise<void>;
  pickFiles(mode: Mode, labels: { title: string; filterImages: string }): Promise<string[]>;
  pickFolder(title: string): Promise<string | null>;
  /** Windows display language as a BCP 47 tag (e.g. `de-DE`), `null` when unknown. */
  systemLocale(): Promise<string | null>;
  storageInfo(): Promise<StorageInfo>;
  onItemProgress(cb: (e: ItemEvent) => void): () => void;
  onRunPaused(cb: (r: PauseReason) => void): () => void;
  onRunFinished(cb: (s: RunSummary) => void): () => void;
  onFilesDropped(cb: (paths: string[]) => void): () => void;
  minimize(): void;
  toggleMaximize(): void;
  close(): void;
  setTitle(title: string): void;
  /** `cb` returns `true` when the window may close, `false` to keep it open. */
  onCloseRequested(cb: () => boolean): () => void;
}
