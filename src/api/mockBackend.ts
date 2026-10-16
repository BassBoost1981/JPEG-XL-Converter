// Simulated backend for browser development and tests. Uses the sample files of the
// design (docs/design/jpeg-xl-konverter.dc.html) and timers instead of real conversion.
import type { Backend } from "./backend";
import type {
  AddResult,
  InputKind,
  ItemEvent,
  JobSettings,
  Mode,
  PauseReason,
  PreviewDto,
  QueueItem,
  RunSummary,
  ToolStatus,
} from "./types";
import type { UiSettings } from "../state/settings";

const MB = 1024 * 1024;

// [name, extension, size in MB, "width × height"]
type Sample = readonly [string, string, number, string];

const ENC: Sample[] = [
  ["DSC_4021", "jpg", 6.8, "6000 × 4000"],
  ["DSC_4022", "jpg", 7.1, "6000 × 4000"],
  ["Panorama_Alpen", "tif", 142.3, "14200 × 3800"],
  ["Portrait_Lena", "jpg", 5.3, "4000 × 6000"],
  ["Screenshot_2026-10-12", "png", 2.4, "2560 × 1440"],
  ["Logo_Export", "png", 0.38, "1200 × 1200"],
];
const ENC_X: Sample[] = [
  ["Urlaub_0", "jpg", 4.9, "5472 × 3648"],
  ["Urlaub_0", "jpg", 5.6, "5472 × 3648"],
  ["Scan_Seite_", "png", 11.2, "4960 × 7016"],
];
const DEC: Sample[] = [
  ["DSC_4021", "jxl", 5.4, "6000 × 4000"],
  ["DSC_4022", "jxl", 5.7, "6000 × 4000"],
  ["Panorama_Alpen", "jxl", 31.6, "14200 × 3800"],
  ["Portrait_Lena", "jxl", 4.2, "4000 × 6000"],
  ["Screenshot_2026-10-12", "jxl", 0.9, "2560 × 1440"],
  ["Logo_Export", "jxl", 0.12, "1200 × 1200"],
];
const DEC_X: Sample[] = [
  ["Archiv_0", "jxl", 3.1, "4032 × 3024"],
  ["Archiv_0", "jxl", 1.8, "3000 × 2000"],
  ["Archiv_0", "jxl", 2.7, "4032 × 3024"],
];

const KINDS: Record<string, InputKind> = {
  jpg: "jpeg",
  png: "png",
  tif: "tiff",
  jxl: "jxl",
};

export interface MockOptions {
  /** Delay between two progress steps. */
  tickMs?: number;
  /** Progress steps per file. */
  steps?: number;
}

interface MockItem {
  item: QueueItem;
  mode: Mode;
  status: ItemEvent["status"];
}

function solidDataUrl(color: string, w: number, h: number): string {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}"><rect width="100%" height="100%" fill="${color}"/></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

export class MockBackend implements Backend {
  private readonly tickMs: number;
  private readonly steps: number;
  private items: MockItem[] = [];
  private nextId = 1;
  private batch: Record<Mode, number> = { enc: 0, dec: 0 };
  private settings: UiSettings | null = null;

  private progressCbs = new Set<(e: ItemEvent) => void>();
  private pausedCbs = new Set<(r: PauseReason) => void>();
  private finishedCbs = new Set<(s: RunSummary) => void>();

  private timer: ReturnType<typeof setTimeout> | null = null;
  private runMode: Mode | null = null;
  private runJob: JobSettings | null = null;
  private pending: MockItem[] = [];
  private current: MockItem | null = null;
  private step = 0;
  private paused = false;
  private claimed = new Set<string>();
  private summary = { done: 0, skipped: 0, errors: 0, bytesIn: 0, bytesOut: 0 };

  constructor(opts: MockOptions = {}) {
    this.tickMs = opts.tickMs ?? 250;
    this.steps = opts.steps ?? 4;
  }

  async checkTools(): Promise<ToolStatus> {
    return { ok: true, cjxlVersion: "0.12.0 (Simulation)", djxlVersion: "0.12.0 (Simulation)", message: null };
  }

  async addPaths(_paths: string[], mode: Mode): Promise<AddResult> {
    const batches = mode === "enc" ? [ENC, ENC_X] : [DEC, DEC_X];
    const samples = batches[this.batch[mode]++] ?? [];
    const items = samples.map(([name, ext, mb, dim]): QueueItem => {
      const [width, height] = dim.split(" × ").map(Number);
      const item: QueueItem = {
        id: this.nextId++,
        entry: {
          path: `C:\\Users\\Lena\\Bilder\\${name}.${ext}`,
          root: null,
          kind: KINDS[ext],
          size: Math.round(mb * MB),
          width,
          height,
        },
      };
      this.items.push({ item, mode, status: "waiting" });
      return item;
    });
    return { items, rejected: 0 };
  }

  async clearQueue(mode: Mode): Promise<void> {
    this.items = this.items.filter((i) => i.mode !== mode);
  }

  async startRun(mode: Mode, s: JobSettings): Promise<void> {
    this.begin(mode, s, (i) => i.status === "waiting");
  }

  async retryFailed(mode: Mode, s: JobSettings): Promise<void> {
    this.begin(mode, s, (i) => i.status === "error" || i.status === "verifyFailed");
  }

  async pauseRun(): Promise<void> {
    if (this.runMode === null || this.paused) return;
    this.paused = true;
    this.clearTimer();
    this.pausedCbs.forEach((cb) => cb({ type: "user" }));
  }

  async resumeRun(): Promise<void> {
    if (this.runMode === null || !this.paused) return;
    this.paused = false;
    this.schedule();
  }

  async cancelRun(): Promise<void> {
    if (this.runMode === null) return;
    this.clearTimer();
    if (this.current) {
      this.current.status = "waiting";
      this.emit({ id: this.current.item.id, status: "waiting", progress: 0, outSize: null, note: null });
    }
    this.finish();
  }

  async preview(itemId: number, _mode: Mode, s: JobSettings, _viewW: number, _viewH: number): Promise<PreviewDto> {
    const lossless = s.encode.distance === 0;
    const hue = (itemId * 47) % 360;
    return {
      before: solidDataUrl(`hsl(${hue} 35% 62%)`, 400, 240),
      after: solidDataUrl(`hsl(${hue} 35% ${lossless ? 62 : 58}%)`, 400, 240),
      lossless,
    };
  }

  async thumbnail(itemId: number): Promise<string> {
    return solidDataUrl(`hsl(${(itemId * 47) % 360} 35% 62%)`, 80, 80);
  }

  async loadSettings(): Promise<UiSettings | null> {
    return this.settings;
  }

  async saveSettings(s: UiSettings): Promise<void> {
    this.settings = s;
  }

  async openLog(_path: string): Promise<void> {}

  async pickFiles(mode: Mode): Promise<string[]> {
    return [mode === "enc" ? "C:\\Users\\Lena\\Bilder\\DSC_4021.jpg" : "C:\\Users\\Lena\\Bilder\\DSC_4021.jxl"];
  }

  async pickFolder(): Promise<string | null> {
    return "C:\\Users\\Lena\\Bilder";
  }

  onItemProgress(cb: (e: ItemEvent) => void): () => void {
    this.progressCbs.add(cb);
    return () => this.progressCbs.delete(cb);
  }

  onRunPaused(cb: (r: PauseReason) => void): () => void {
    this.pausedCbs.add(cb);
    return () => this.pausedCbs.delete(cb);
  }

  onRunFinished(cb: (s: RunSummary) => void): () => void {
    this.finishedCbs.add(cb);
    return () => this.finishedCbs.delete(cb);
  }

  onFilesDropped(_cb: (paths: string[]) => void): () => void {
    return () => {};
  }

  minimize(): void {}
  toggleMaximize(): void {}
  close(): void {}
  onCloseRequested(_cb: () => boolean): () => void {
    return () => {};
  }

  // ---- simulated run ----

  private begin(mode: Mode, job: JobSettings, pick: (i: MockItem) => boolean): void {
    if (this.runMode !== null) return;
    this.runMode = mode;
    this.runJob = job;
    this.paused = false;
    this.claimed = new Set();
    this.summary = { done: 0, skipped: 0, errors: 0, bytesIn: 0, bytesOut: 0 };
    this.pending = this.items.filter((i) => i.mode === mode && pick(i));
    this.current = null;
    this.schedule();
  }

  private schedule(): void {
    this.clearTimer();
    this.timer = setTimeout(() => this.tick(), this.tickMs);
  }

  private clearTimer(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }

  private tick(): void {
    this.timer = null;
    if (this.current === null) {
      this.current = this.pending.shift() ?? null;
      if (this.current === null) return this.finish();
      this.step = 0;
      this.current.status = "running";
      this.emit({ id: this.current.item.id, status: "running", progress: 0, outSize: null, note: null });
      return this.schedule();
    }
    this.step++;
    if (this.step < this.steps) {
      const progress = Math.round((this.step / this.steps) * 100);
      this.emit({ id: this.current.item.id, status: "running", progress, outSize: null, note: null });
      return this.schedule();
    }
    this.complete(this.current);
    this.current = null;
    this.schedule();
  }

  private complete(m: MockItem): void {
    const { entry } = m.item;
    const base = entry.path.replace(/\.[^.\\]+$/, "");
    // Dec: the sample's original extension is not modelled, so the target key is name + output format.
    const key = this.runMode === "enc" ? `${base}.jxl` : `${base}.${this.runJob?.decode.format ?? "auto"}`;
    this.summary.bytesIn += entry.size;
    if (this.claimed.has(key)) {
      m.status = "skipped";
      this.summary.skipped++;
      this.emit({ id: m.item.id, status: "skipped", progress: 100, outSize: null, note: "Zieldatei existiert bereits" });
      return;
    }
    this.claimed.add(key);
    const outSize = this.resultSize(entry.kind, entry.size);
    m.status = "done";
    this.summary.done++;
    this.summary.bytesOut += outSize;
    this.emit({ id: m.item.id, status: "done", progress: 100, outSize, note: null });
  }

  private resultSize(kind: InputKind, size: number): number {
    if (this.runMode === "dec") return Math.round(size * (kind === "jxl" ? 1.3 : 1));
    const d = this.runJob?.encode.distance ?? 0;
    if (d === 0) return Math.round(size * (kind === "jpeg" ? 0.8 : 0.55));
    return Math.max(1, Math.round(size * Math.max(0.04, 0.4 / (1 + d))));
  }

  private finish(): void {
    const s: RunSummary = { ...this.summary, logPath: null };
    this.runMode = null;
    this.runJob = null;
    this.pending = [];
    this.current = null;
    this.clearTimer();
    this.finishedCbs.forEach((cb) => cb(s));
  }

  private emit(e: ItemEvent): void {
    this.progressCbs.forEach((cb) => cb(e));
  }
}
