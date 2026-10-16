// TypeScript counterparts of src-tauri/src/engine/types.rs and the DTOs of commands.rs.
// Serde shapes: camelCase fields, lower-case string enums.

export type Mode = "enc" | "dec";
export type InputKind = "jpeg" | "png" | "gif" | "tiff" | "webp" | "bmp" | "jxl";
export type OutputFormat = "auto" | "png" | "jpg" | "webp" | "tiff";
export type OutputLocation = { kind: "next" } | { kind: "folder"; path: string };

export interface EncodeSettings {
  distance: number;
  effort: number;
  transcodeJpeg: boolean;
}

export interface DecodeSettings {
  format: OutputFormat;
  jpegQuality: number;
  reduceTo8bit: boolean;
}

export interface JobSettings {
  encode: EncodeSettings;
  decode: DecodeSettings;
  keepMetadata: boolean;
  trashOriginals: boolean;
  output: OutputLocation;
}

export interface ScanEntry {
  path: string;
  root: string | null;
  kind: InputKind;
  size: number;
  width: number;
  height: number;
}

export interface QueueItem {
  id: number;
  entry: ScanEntry;
}

export type ItemStatus =
  | "waiting"
  | "running"
  | "done"
  | "doneWithNote"
  | "skipped"
  | "error"
  | "verifyFailed";

export interface ItemEvent {
  id: number;
  status: ItemStatus;
  progress: number;
  outSize: number | null;
  note: string | null;
}

export interface RunSummary {
  done: number;
  skipped: number;
  errors: number;
  bytesIn: number;
  bytesOut: number;
  logPath?: string | null;
}

export type PauseReason = { type: "user" } | { type: "diskFull"; drive: string };

export interface ToolStatus {
  ok: boolean;
  cjxlVersion: string | null;
  djxlVersion: string | null;
  message: string | null;
}

export interface AddResult {
  items: QueueItem[];
  rejected: number;
}

export interface PreviewDto {
  before: string;
  after: string;
  lossless: boolean;
}
