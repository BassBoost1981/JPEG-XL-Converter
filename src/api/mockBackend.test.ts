import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MockBackend } from "./mockBackend";
import type { ItemEvent, RunSummary } from "./types";
import { DEFAULT_SETTINGS, toJobSettings } from "../state/settings";

const job = toJobSettings(DEFAULT_SETTINGS);

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("MockBackend", () => {
  it("addPaths liefert Einträge mit eindeutigen IDs", async () => {
    const b = new MockBackend();
    const r = await b.addPaths(["C:\\Bilder"], "enc");
    expect(r.items).toHaveLength(6);
    expect(r.rejected).toBe(0);
    expect(r.items[0].entry).toMatchObject({ kind: "jpeg", width: 6000, height: 4000 });
    const r2 = await b.addPaths(["C:\\Mehr"], "enc");
    const ids = [...r.items, ...r2.items].map((i) => i.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("startRun meldet Fortschritt bis alle fertig sind, danach genau ein onRunFinished", async () => {
    const b = new MockBackend();
    const { items } = await b.addPaths(["x"], "enc");
    const events: ItemEvent[] = [];
    const finished: RunSummary[] = [];
    b.onItemProgress((e) => events.push(e));
    b.onRunFinished((s) => finished.push(s));

    await b.startRun("enc", job, "de");
    await vi.advanceTimersByTimeAsync(60_000);

    for (const it of items) {
      const mine = events.filter((e) => e.id === it.id);
      expect(mine[0].status).toBe("running");
      const last = mine[mine.length - 1];
      expect(last.status).toBe("done");
      expect(last.progress).toBe(100);
      expect(last.outSize).toBeGreaterThan(0);
    }
    expect(finished).toHaveLength(1);
    expect(finished[0]).toMatchObject({ done: 6, skipped: 0, errors: 0 });
    expect(finished[0].bytesIn).toBe(items.reduce((a, i) => a + i.entry.size, 0));

    await vi.advanceTimersByTimeAsync(60_000);
    expect(finished).toHaveLength(1);
  });

  it("pausieren hält den Lauf an und meldet den Grund, Fortsetzen beendet ihn", async () => {
    const b = new MockBackend();
    await b.addPaths(["x"], "dec");
    const paused: unknown[] = [];
    const finished: RunSummary[] = [];
    b.onRunPaused((r) => paused.push(r));
    b.onRunFinished((s) => finished.push(s));
    await b.startRun("dec", job, "de");
    await vi.advanceTimersByTimeAsync(300);
    await b.pauseRun();
    expect(paused).toEqual([{ type: "user" }]);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(finished).toHaveLength(0);
    await b.resumeRun();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(finished).toHaveLength(1);
  });

  it("gleicher Zielname wird als übersprungen gemeldet", async () => {
    const b = new MockBackend();
    await b.addPaths(["x"], "enc");
    const { items } = await b.addPaths(["y"], "enc");
    const finished: RunSummary[] = [];
    b.onRunFinished((s) => finished.push(s));
    await b.startRun("enc", job, "de");
    await vi.advanceTimersByTimeAsync(120_000);
    expect(items).toHaveLength(3);
    expect(finished[0].skipped).toBe(1);
    expect(finished[0].done).toBe(8);
  });

  it("Hinweise sind Msg-Listen: leer bei Erfolg, Schlüssel bei Übersprungenem", async () => {
    const b = new MockBackend();
    await b.addPaths(["x"], "enc");
    await b.addPaths(["y"], "enc");
    const events: ItemEvent[] = [];
    b.onItemProgress((e) => events.push(e));
    await b.startRun("enc", job, "en");
    await vi.advanceTimersByTimeAsync(120_000);
    expect(events.every((e) => Array.isArray(e.notes))).toBe(true);
    expect(events.filter((e) => e.status === "done").every((e) => e.notes.length === 0)).toBe(true);
    const skipped = events.filter((e) => e.status === "skipped");
    expect(skipped).toHaveLength(1);
    expect(skipped[0].notes).toEqual([{ key: "error.targetExists" }]);
  });

  it("retryFailed nimmt die Sprache an und startet ohne Fehler", async () => {
    const b = new MockBackend();
    await b.addPaths(["x"], "enc");
    const finished: RunSummary[] = [];
    b.onRunFinished((s) => finished.push(s));
    await b.retryFailed("enc", job, "en");
    await vi.advanceTimersByTimeAsync(1000);
    expect(finished).toHaveLength(1);
    expect(finished[0]).toMatchObject({ done: 0 });
  });

  it("Werkzeugstatus hat eine leere messages-Liste", async () => {
    expect(await new MockBackend().checkTools()).toMatchObject({ ok: true, messages: [] });
  });

  it("systemLocale liefert die Browsersprache, storageInfo ist nie schreibgeschützt", async () => {
    const b = new MockBackend();
    expect(await b.systemLocale()).toBe(navigator.language);
    expect(await b.storageInfo()).toEqual({ readOnly: false });
  });

  it("setTitle setzt den Dokumenttitel", () => {
    const b = new MockBackend();
    b.setTitle("JPEG XL Converter");
    expect(document.title).toBe("JPEG XL Converter");
    b.setTitle("JPEG XL Konverter");
    expect(document.title).toBe("JPEG XL Konverter");
  });

  it("Dialoge liefern Beispielwerte und nehmen die Beschriftungen an", async () => {
    const b = new MockBackend();
    expect(await b.pickFiles("enc", { title: "Select files", filterImages: "Images" })).toHaveLength(1);
    expect(await b.pickFolder("Select folder")).toBe("C:\\Users\\Lena\\Bilder");
  });

  it("Abbrechen beendet den Lauf mit genau einem onRunFinished", async () => {
    const b = new MockBackend();
    await b.addPaths(["x"], "enc");
    const finished: RunSummary[] = [];
    b.onRunFinished((s) => finished.push(s));
    await b.startRun("enc", job, "de");
    await vi.advanceTimersByTimeAsync(300);
    await b.cancelRun();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(finished).toHaveLength(1);
    expect(finished[0].done).toBeLessThan(6);
  });

  it("Vorschau und Vorschaubild sind Data-URLs", async () => {
    const b = new MockBackend();
    const { items } = await b.addPaths(["x"], "enc");
    const p = await b.preview(items[0].id, "enc", job, 400, 240);
    expect(p.before).toMatch(/^data:image\//);
    expect(p.after).toMatch(/^data:image\//);
    expect(p.lossless).toBe(true);
    const lossy = await b.preview(items[0].id, "enc", { ...job, encode: { ...job.encode, distance: 1.2 } }, 1, 1);
    expect(lossy.lossless).toBe(false);
    expect(await b.thumbnail(items[0].id)).toMatch(/^data:image\//);
  });

  it("clearQueue leert nur den Modus", async () => {
    const b = new MockBackend();
    await b.addPaths(["x"], "enc");
    await b.clearQueue("enc");
    const finished: RunSummary[] = [];
    b.onRunFinished((s) => finished.push(s));
    await b.startRun("enc", job, "de");
    await vi.advanceTimersByTimeAsync(1000);
    expect(finished[0]).toMatchObject({ done: 0 });
  });
});
