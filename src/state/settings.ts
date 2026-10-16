import type { JobSettings, Mode, OutputFormat, OutputLocation } from "../api/types";

export type Preset = "ll" | "hi" | "bal" | "sm";

export const PRESETS: Record<Preset, { label: string; distance: number; effort: number; quality: number }> = {
  ll: { label: "Verlustfrei", distance: 0, effort: 7, quality: 100 },
  hi: { label: "Hohe Qualität", distance: 0.5, effort: 7, quality: 95 },
  bal: { label: "Ausgewogen", distance: 1.2, effort: 7, quality: 88 },
  sm: { label: "Klein", distance: 2.6, effort: 8, quality: 72 },
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
