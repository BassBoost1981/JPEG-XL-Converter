import { describe, expect, it } from "vitest";
import de from "./de.json";
import en from "./en.json";
import { langFromLocale, msgText, translate } from "./index";

const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe("catalogs", () => {
  it("catalogs have identical keys and no empty values", () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(de).sort());
    for (const v of [...Object.values(de), ...Object.values(en)]) expect(v.trim()).not.toBe("");
  });

  it("placeholders match between languages", () => {
    const deText = de as Record<string, string>;
    const enText = en as Record<string, string>;
    for (const key of Object.keys(deText)) {
      expect({ key, placeholders: placeholders(enText[key]) }).toEqual({
        key,
        placeholders: placeholders(deText[key]),
      });
    }
  });

  it("every .one key has an .other twin and the other way round", () => {
    const keys = new Set(Object.keys(de));
    for (const key of keys) {
      if (key.endsWith(".one")) expect(keys.has(key.replace(/\.one$/, ".other"))).toBe(true);
      if (key.endsWith(".other")) expect(keys.has(key.replace(/\.other$/, ".one"))).toBe(true);
    }
  });

  it("English texts contain no German umlauts", () => {
    for (const [key, value] of Object.entries(en)) expect({ key, umlaut: /[äöüÄÖÜß]/.test(value) }).toEqual({ key, umlaut: false });
  });

  it("has every key that later tasks use by name", () => {
    const named = [
      "app.title",
      "prefs.title", "prefs.appearance", "prefs.mode", "prefs.language",
      "prefs.light", "prefs.dark", "prefs.normal", "prefs.expert",
      "note.keptNoRecycleBin", "note.keptTooLarge", "note.keptRecycleFailed",
      "reason.readOnly", "reason.accessDenied", "reason.inUse", "reason.cancelled",
      "reason.notFound", "reason.internal", "reason.windowsError",
      "error.targetExists", "error.saveSettings", "error.internal",
      "tool.wontStart", "tool.noResponse", "tool.noVersion",
      "banner.toolsMissing", "banner.readOnly",
      "dialog.pickFiles", "dialog.pickFolder", "dialog.filterImages",
      "notice.rejected.one", "notice.rejected.other",
      "footer.noSpace",
      "log.title",
    ];
    for (const key of named) expect({ key, de: key in de, en: key in en }).toEqual({ key, de: true, en: true });
  });

  it("uses the agreed English status words and wording overrides", () => {
    const t = (key: string, params?: Record<string, string | number>) => translate("en", key, params);
    expect([
      t("status.waiting"), t("status.running", { n: 42 }), t("status.done"), t("status.doneWithNote"),
      t("status.skipped"), t("status.error"), t("status.verifyFailed"),
    ]).toEqual(["Queued", "42 %", "Done", "Done with note", "Skipped", "Error", "Check failed"]);
    expect(t("queue.dropHintEnc")).toBe("JPEG, PNG, GIF, TIFF, WebP, BMP");
    expect(t("settings.trashHint")).toBe("Only if the result is lossless");
    expect(t("cta.retry")).toBe("Retry failed");
    expect(t("cta.allDone")).toBe("All done");
    expect(t("cta.enc.one", { n: 1 })).toBe("Convert 1 file to JPEG XL");
    expect(t("cta.enc.other", { n: 6 })).toBe("Convert 6 files to JPEG XL");
    expect(t("cta.dec.one", { n: 1 })).toBe("Convert 1 file back");
    expect(t("cta.dec.other", { n: 3 })).toBe("Convert 3 files back");
    expect(translate("de", "cta.enc.one", { n: 1 })).toBe("1 Datei in JPEG XL konvertieren");
    expect(translate("de", "cta.dec.one", { n: 1 })).toBe("1 Datei zurückkonvertieren");
    expect(translate("de", "cta.retry")).toBe("Fehlgeschlagene erneut versuchen");
    expect(translate("de", "cta.allDone")).toBe("Alles erledigt");
  });

  it("window title and log header differ by language", () => {
    expect(translate("de", "app.title")).toBe("JPEG XL Konverter");
    expect(translate("en", "app.title")).toBe("JPEG XL Converter");
    expect(translate("de", "log.title")).toBe("JPEG XL Konverter – Protokoll");
    expect(translate("en", "log.title")).toBe("JPEG XL Converter – Log");
  });
});

describe("translate", () => {
  it("translate substitutes params once", () => {
    expect(translate("de", "note.keptNoRecycleBin")).toBe("Original behalten: kein Papierkorb auf diesem Laufwerk");
    expect(translate("en", "error.targetExists")).toBe("Target file already exists");
    expect(translate("en", "footer.noSpace", { drive: "D:" })).toBe("No space left on drive D:");
    expect(translate("de", "footer.noSpace", { drive: "D:" })).toBe("Kein Platz mehr auf Laufwerk D:");
  });

  it("does not substitute again inside an inserted value", () => {
    expect(translate("en", "footer.noSpace", { drive: "{drive}" })).toBe("No space left on drive {drive}");
    expect(translate("de", "footer.doneOf", { done: "{total}", total: 5 })).toBe("{total} von 5 fertig");
  });

  it("numbers are inserted as text", () => {
    expect(translate("en", "queue.count.other", { n: 12 })).toBe("12 files");
    expect(translate("de", "queue.count.one", { n: 1 })).toBe("1 Datei");
  });

  it("nested Msg params are translated", () => {
    const m = { key: "note.keptRecycleFailed", params: { reason: { key: "reason.readOnly" } } };
    expect(msgText("de", m)).toBe("Original behalten: Papierkorb nicht möglich (Datei ist schreibgeschützt)");
    expect(msgText("en", m)).toBe("Original kept: Recycle Bin not possible (File is read-only)");
  });

  it("nested Msg with its own params", () => {
    const m = { key: "note.keptRecycleFailed", params: { reason: { key: "reason.windowsError", params: { code: "0x80270058" } } } };
    expect(msgText("de", m)).toBe("Original behalten: Papierkorb nicht möglich (Windows-Fehler 0x80270058)");
  });

  it("unknown key returns the key, missing param keeps the placeholder", () => {
    expect(translate("en", "does.not.exist")).toBe("does.not.exist");
    expect(translate("en", "footer.noSpace")).toContain("{drive}");
  });

  it("inherited object properties are not keys", () => {
    expect(translate("en", "toString")).toBe("toString");
    expect(translate("de", "constructor")).toBe("constructor");
  });
});

describe("langFromLocale", () => {
  it.each([
    ["de-DE", "de"],
    ["de", "de"],
    ["DE", "de"],
    ["de_AT", "de"],
    ["de-CH", "de"],
    ["en-US", "en"],
    ["fr-FR", "en"],
    ["", "en"],
    [null, "en"],
    [undefined, "en"],
  ])("langFromLocale(%s) = %s", (tag, lang) => {
    expect(langFromLocale(tag as string | null | undefined)).toBe(lang);
  });
});
