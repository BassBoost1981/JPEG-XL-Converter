import type {
  AddResult,
  ItemEvent,
  JobSettings,
  Mode,
  PauseReason,
  PreviewDto,
  RunSummary,
  ToolStatus,
} from "./types";
import type { UiSettings } from "../state/settings";

export interface Backend {
  checkTools(): Promise<ToolStatus>;
  addPaths(paths: string[], mode: Mode): Promise<AddResult>;
  clearQueue(mode: Mode): Promise<void>;
  startRun(mode: Mode, s: JobSettings): Promise<void>;
  retryFailed(mode: Mode, s: JobSettings): Promise<void>;
  pauseRun(): Promise<void>;
  resumeRun(): Promise<void>;
  cancelRun(): Promise<void>;
  preview(itemId: number, mode: Mode, s: JobSettings, viewW: number, viewH: number): Promise<PreviewDto>;
  thumbnail(itemId: number): Promise<string>;
  loadSettings(): Promise<UiSettings | null>;
  saveSettings(s: UiSettings): Promise<void>;
  openLog(path: string): Promise<void>;
  pickFiles(mode: Mode): Promise<string[]>;
  pickFolder(): Promise<string | null>;
  onItemProgress(cb: (e: ItemEvent) => void): () => void;
  onRunPaused(cb: (r: PauseReason) => void): () => void;
  onRunFinished(cb: (s: RunSummary) => void): () => void;
  onFilesDropped(cb: (paths: string[]) => void): () => void;
  minimize(): void;
  toggleMaximize(): void;
  close(): void;
  /** `cb` returns `true` when the window may close, `false` to keep it open. */
  onCloseRequested(cb: () => boolean): () => void;
}
