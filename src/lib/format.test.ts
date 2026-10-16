import { describe, expect, it } from "vitest";
import {
  ctaLabel,
  footerHint,
  formatCount,
  formatDelta,
  formatDistance,
  formatSize,
  summaryLine,
} from "./format";

describe("formatSize (1 MB = 1 048 576 Bytes)", () => {
  it("rechnet binär mit Dezimalkomma", () => {
    expect(formatSize(389_120)).toBe("380 KB");
    expect(formatSize(7_130_317)).toBe("6,80 MB");
    expect(formatSize(149_212_364)).toBe("142,3 MB");
    expect(formatSize(44_238_131_200)).toBe("41,2 GB");
  });
});

describe("formatDelta", () => {
  it("nutzt das echte Minuszeichen und ein Plus", () => {
    expect(formatDelta(100, 80)).toBe("−20 %");
    expect(formatDelta(100, 105)).toBe("+5 %");
    expect(formatDelta(100, 100)).toBe("0 %");
    expect(formatDelta(0, 10)).toBe("0 %");
  });
});

describe("formatCount / formatDistance", () => {
  it("formatiert Zahlen", () => {
    expect(formatCount(9812)).toBe("9.812");
    expect(formatDistance(0.5)).toBe("0.5");
    expect(formatDistance(1.2)).toBe("1.2");
    expect(formatDistance(1.18)).toBe("1.18");
    expect(formatDistance(0)).toBe("0");
  });
});

describe("Texte", () => {
  it("summaryLine", () => {
    expect(
      summaryLine({ done: 9812, skipped: 14, errors: 2, bytesIn: 44_238_131_200, bytesOut: 35_433_480_192 }),
    ).toBe("9.812 fertig · 14 übersprungen · 2 Fehler · 41,2 GB → 33,0 GB");
  });

  it("footerHint", () => {
    expect(footerHint({ done: 0, total: 6 })).toBe("Ersparnis erscheint nach der ersten fertigen Datei");
    expect(footerHint({ done: 2, total: 6, bytesIn: 7_130_317, bytesOut: 5_704_253 })).toBe(
      "2 von 6 fertig · 6,80 MB → 5,44 MB",
    );
    expect(footerHint({ done: 2, total: 6 })).toBe("2 von 6 fertig");
  });

  it("ctaLabel", () => {
    expect(ctaLabel({ mode: "enc", run: "idle", waiting: 6, failed: 0 })).toBe("6 Dateien in JPEG XL konvertieren");
    expect(ctaLabel({ mode: "enc", run: "idle", waiting: 1, failed: 0 })).toBe("1 Datei in JPEG XL konvertieren");
    expect(ctaLabel({ mode: "dec", run: "idle", waiting: 3, failed: 0 })).toBe("3 Dateien zurückkonvertieren");
    expect(ctaLabel({ mode: "dec", run: "idle", waiting: 1, failed: 0 })).toBe("1 Datei zurückkonvertieren");
    expect(ctaLabel({ mode: "enc", run: "running", waiting: 2, failed: 0 })).toBe("Anhalten");
    expect(ctaLabel({ mode: "enc", run: "paused", waiting: 2, failed: 0 })).toBe("Fortsetzen");
    expect(ctaLabel({ mode: "enc", run: "finished", waiting: 0, failed: 2 })).toBe("Fehlgeschlagene erneut versuchen");
    expect(ctaLabel({ mode: "enc", run: "finished", waiting: 0, failed: 0 })).toBe("Alles erledigt");
  });

  it("ctaLabel (R2): neue wartende Dateien nach einem Lauf starten, auch bei Fehlern", () => {
    expect(ctaLabel({ mode: "enc", run: "finished", waiting: 3, failed: 1 })).toBe("3 Dateien in JPEG XL konvertieren");
    expect(ctaLabel({ mode: "dec", run: "finished", waiting: 1, failed: 0 })).toBe("1 Datei zurückkonvertieren");
  });
});
