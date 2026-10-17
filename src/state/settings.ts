import type { JobSettings, Lang, Mode, OutputFormat, OutputLocation } from "../api/types";

export type Preset = "ll" | "hi" | "bal" | "sm";

/** The labels are in the catalog (`preset.<id>.label`). */
export const PRESETS: Record<Preset, { distance: number; effort: number; quality: number }> = {
  ll: { distance: 0, effort: 7, quality: 100 },
  hi: { distance: 0.5, effort: 7, quality: 95 },
  bal: { distance: 1.2, effort: 7, quality: 88 },
  sm: { distance: 2.6, effort: 8, quality: 72 },
};

export const EFFORT_NAMES = [
  "",
  "Lightning",
  "Thunder",
  "Falcon",
  "Cheetah",
  "Hare",
  "Wombat",
  "Squirrel",
  "Kitten",
  "Tortoise",
];

export interface UiSettings {
  mode: Mode;
  expert: boolean;
  preset: Preset;
  quality: number;
  effort: number;
  lossless: boolean;
  transcodeJpeg: boolean;
  keepMetadata: boolean;
  trashOriginals: boolean;
  decodeFormat: OutputFormat;
  jpegQuality: number;
  bitDepth: 8 | 16;
  output: OutputLocation;
  theme: "light" | "dark" | null;
  /** `null`: follow the Windows display language. */
  lang: Lang | null;
}

export const DEFAULT_SETTINGS: UiSettings = {
  mode: "enc",
  expert: false,
  preset: "ll",
  quality: 100,
  effort: 7,
  lossless: true,
  transcodeJpeg: true,
  keepMetadata: true,
  trashOriginals: false,
  decodeFormat: "auto",
  jpegQuality: 92,
  bitDepth: 16,
  output: { kind: "next" },
  theme: null,
  lang: null,
};

/** Expert slider: d = 0,1 + (100 − q) · 0,09; q = 100 is lossless (d = 0). */
export function distanceFromQuality(q: number): number {
  return q >= 100 ? 0 : 0.1 + (100 - q) * 0.09;
}

/** Switching to expert mode starts from the values of the chosen preset. */
export function enterExpert(s: UiSettings): UiSettings {
  const p = PRESETS[s.preset];
  return { ...s, expert: true, quality: p.quality, effort: p.effort, lossless: s.preset === "ll" };
}

export function toJobSettings(s: UiSettings): JobSettings {
  const p = PRESETS[s.preset];
  const distance = s.expert ? (s.lossless ? 0 : distanceFromQuality(s.quality)) : p.distance;
  const effort = s.expert ? s.effort : p.effort;
  return {
    encode: { distance, effort, transcodeJpeg: s.transcodeJpeg },
    decode: {
      format: s.decodeFormat,
      jpegQuality: s.jpegQuality,
      reduceTo8bit: s.expert && s.bitDepth === 8,
    },
    keepMetadata: s.keepMetadata,
    trashOriginals: s.trashOriginals,
    output: s.output,
  };
}

const pick = <T,>(value: unknown, allowed: readonly T[], fallback: T): T =>
  allowed.includes(value as T) ? (value as T) : fallback;

const intIn = (value: unknown, min: number, max: number, fallback: number): number =>
  typeof value === "number" && Number.isInteger(value) && value >= min && value <= max ? value : fallback;

const bool = (value: unknown, fallback: boolean): boolean => (typeof value === "boolean" ? value : fallback);

/** Saved settings from disk: every unknown or out-of-range field falls back to its default. */
export function restoreSettings(saved: unknown): UiSettings {
  const s = (typeof saved === "object" && saved !== null ? saved : {}) as Record<string, unknown>;
  const d = DEFAULT_SETTINGS;
  const out = s.output as { kind?: unknown; path?: unknown } | null | undefined;
  const output: OutputLocation =
    out?.kind === "folder" && typeof out.path === "string" && out.path !== ""
      ? { kind: "folder", path: out.path }
      : out?.kind === "next"
        ? { kind: "next" }
        : d.output;
  return {
    mode: pick<Mode>(s.mode, ["enc", "dec"], d.mode),
    expert: bool(s.expert, d.expert),
    preset: pick<Preset>(s.preset, ["ll", "hi", "bal", "sm"], d.preset),
    quality: intIn(s.quality, 30, 100, d.quality),
    effort: intIn(s.effort, 1, 9, d.effort),
    lossless: bool(s.lossless, d.lossless),
    transcodeJpeg: bool(s.transcodeJpeg, d.transcodeJpeg),
    keepMetadata: bool(s.keepMetadata, d.keepMetadata),
    trashOriginals: bool(s.trashOriginals, d.trashOriginals),
    decodeFormat: pick<OutputFormat>(s.decodeFormat, ["auto", "png", "jpg", "webp", "tiff"], d.decodeFormat),
    jpegQuality: intIn(s.jpegQuality, 50, 100, d.jpegQuality),
    bitDepth: pick<8 | 16>(s.bitDepth, [8, 16], d.bitDepth),
    output,
    theme: pick<UiSettings["theme"]>(s.theme, ["light", "dark", null], d.theme),
    lang: pick<UiSettings["lang"]>(s.lang, ["de", "en", null], d.lang),
  };
}
