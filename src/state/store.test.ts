import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ToolStatus } from "../api/types";
import { DEFAULT_SETTINGS } from "./settings";

let dispose: () => void;
let useApp: typeof import("./store").useApp;
let selectLang: typeof import("./store").selectLang;
let backend: typeof import("../api").backend;

// Fresh module graph per test: every test gets its own MockBackend and store.
beforeEach(async () => {
  vi.useFakeTimers();
  vi.resetModules();
  ({ useApp, selectLang } = await import("./store"));
  ({ backend } = await import("../api"));
  dispose = await useApp.getState().init();
});

afterEach(() => {
  dispose();
  vi.useRealTimers();
});

describe("useApp", () => {
  it("fügt Einträge hinzu und wählt den ersten aus", async () => {
    await useApp.getState().addPaths(["x"]);
    const st = useApp.getState();
    expect(st.queues.enc).toHaveLength(6);
    expect(st.queues.enc[0]).toMatchObject({ status: "waiting", progress: 0 });
    expect(st.selected.enc).toBe(st.queues.enc[0].id);
    expect(st.queues.dec).toHaveLength(0);
  });

  it("ein Durchlauf übernimmt Ereignisse in die Zeilen und endet mit finished", async () => {
    await useApp.getState().addPaths(["x"]);
    await useApp.getState().start();
    expect(useApp.getState().run).toBe("running");
    await vi.advanceTimersByTimeAsync(60_000);
    const st = useApp.getState();
    expect(st.run).toBe("finished");
    expect(st.queues.enc.every((r) => r.status === "done" && r.progress === 100 && (r.outSize ?? 0) > 0)).toBe(true);
    expect(st.summary).toMatchObject({ done: 6, skipped: 0, errors: 0 });
  });

  it("Pausieren und Fortsetzen folgen den Backend-Ereignissen", async () => {
    await useApp.getState().addPaths(["x"]);
    await useApp.getState().start();
    await vi.advanceTimersByTimeAsync(300);
    await useApp.getState().pause();
    expect(useApp.getState()).toMatchObject({ run: "paused", pauseReason: { type: "user" } });
    await useApp.getState().resume();
    expect(useApp.getState().run).toBe("running");
    await vi.advanceTimersByTimeAsync(60_000);
    expect(useApp.getState().run).toBe("finished");
  });

  it("Leeren ist während des Laufs gesperrt, sonst leert es nur den aktuellen Modus", async () => {
    await useApp.getState().addPaths(["x"]);
    await useApp.getState().start();
    await useApp.getState().clear();
    expect(useApp.getState().queues.enc).toHaveLength(6);
    await vi.advanceTimersByTimeAsync(60_000);
    await useApp.getState().clear();
    expect(useApp.getState()).toMatchObject({ run: "idle", queues: { enc: [] }, summary: null });
  });

  it("updateSettings speichert über das Backend", async () => {
    useApp.getState().updateSettings({ preset: "sm" });
    expect(useApp.getState().settings.preset).toBe("sm");
    await vi.advanceTimersByTimeAsync(500);
    expect(await backend.loadSettings()).toMatchObject({ preset: "sm" });
  });

  it("entprellt das Speichern: viele Änderungen ergeben genau einen Speichervorgang mit dem letzten Wert", async () => {
    const save = vi.spyOn(backend, "saveSettings");
    for (let q = 50; q < 60; q++) {
      useApp.getState().updateSettings({ quality: q });
      await vi.advanceTimersByTimeAsync(50);
    }
    expect(useApp.getState().settings.quality).toBe(59);
    expect(save).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(500);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ quality: 59 }));
  });

  // Review Focus 3: the data folder vanishes during the session (USB stick pulled).
  it("ein Speicherfehler setzt nur den Hinweis (Msg) und die App läuft weiter", async () => {
    vi.spyOn(backend, "saveSettings").mockRejectedValue({ key: "error.saveSettings" });
    useApp.getState().updateSettings({ effort: 3 });
    await vi.advanceTimersByTimeAsync(500);
    expect(useApp.getState().notice).toEqual({ key: "error.saveSettings" });
    expect(useApp.getState().notice?.key).toBe("error.saveSettings");

    // Still usable: settings change, files are added and a run finishes.
    useApp.getState().updateSettings({ effort: 4 });
    expect(useApp.getState().settings.effort).toBe(4);
    await useApp.getState().addPaths(["x"]);
    await useApp.getState().start();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(useApp.getState().run).toBe("finished");
  });

  it("ein unbekannter Fehler wird zum Hinweis „error.internal“", async () => {
    vi.spyOn(backend, "saveSettings").mockRejectedValue(new Error("kaputt"));
    useApp.getState().updateSettings({ effort: 3 });
    await vi.advanceTimersByTimeAsync(500);
    expect(useApp.getState().notice).toEqual({ key: "error.internal" });

    vi.spyOn(backend, "clearQueue").mockRejectedValue("Zeichenkette");
    await useApp.getState().clear();
    expect(useApp.getState().notice).toEqual({ key: "error.internal" });

    vi.spyOn(backend, "addPaths").mockRejectedValue(null);
    await useApp.getState().addPaths(["x"]);
    expect(useApp.getState().notice).toEqual({ key: "error.internal" });
  });

  it("Fehler des Backends bleiben als Msg im Hinweis erhalten, auch beim Start und Fortsetzen", async () => {
    vi.spyOn(backend, "addPaths").mockRejectedValue({ key: "error.unknownItem" });
    await useApp.getState().addPaths(["x"]);
    expect(useApp.getState().notice).toEqual({ key: "error.unknownItem" });

    vi.spyOn(backend, "startRun").mockRejectedValue({ key: "error.workDirFailed", params: { reason: "x" } });
    await useApp.getState().start();
    expect(useApp.getState()).toMatchObject({
      run: "idle",
      notice: { key: "error.workDirFailed", params: { reason: "x" } },
    });

    vi.spyOn(backend, "resumeRun").mockRejectedValue({ key: "error.internal" });
    await useApp.getState().resume();
    expect(useApp.getState()).toMatchObject({ run: "paused", notice: { key: "error.internal" } });
  });

  it("abgelehnte Dateien: Hinweis mit Schlüssel und Anzahl, kein Hinweis bei 0", async () => {
    const add = vi.spyOn(backend, "addPaths");
    add.mockResolvedValue({ items: [], rejected: 1 });
    await useApp.getState().addPaths(["x.txt"]);
    expect(useApp.getState().notice).toEqual({ key: "notice.rejected.one", params: { n: 1 } });

    add.mockResolvedValue({ items: [], rejected: 3 });
    await useApp.getState().addPaths(["y.txt"]);
    expect(useApp.getState().notice).toEqual({ key: "notice.rejected.other", params: { n: 3 } });

    add.mockResolvedValue({ items: [], rejected: 0 });
    await useApp.getState().addPaths(["z.jpg"]);
    expect(useApp.getState().notice).toEqual({ key: "notice.rejected.other", params: { n: 3 } });
  });

  describe("Sprache", () => {
    const reinit = async () => {
      dispose();
      dispose = await useApp.getState().init();
    };

    it("ohne Wahl gilt die Windows-Sprache, mit Wahl die gewählte", async () => {
      vi.spyOn(backend, "systemLocale").mockResolvedValue("de-AT");
      await reinit();
      expect(useApp.getState()).toMatchObject({ systemLang: "de", settings: { lang: null } });
      expect(selectLang(useApp.getState())).toBe("de");

      useApp.getState().updateSettings({ lang: "en" });
      expect(selectLang(useApp.getState())).toBe("en");
      useApp.getState().updateSettings({ lang: null });
      expect(selectLang(useApp.getState())).toBe("de");
    });

    it("jede andere Windows-Sprache ergibt Englisch", async () => {
      vi.spyOn(backend, "systemLocale").mockResolvedValue("fr-FR");
      await reinit();
      expect(useApp.getState().systemLang).toBe("en");
      expect(selectLang(useApp.getState())).toBe("en");
    });

    it("ohne lesbare Windows-Sprache und ohne Speicherinfo: Englisch, nicht schreibgeschützt, kein Hinweis", async () => {
      useApp.setState({ systemLang: "de", readOnly: true });
      vi.spyOn(backend, "systemLocale").mockRejectedValue({ key: "error.internal" });
      vi.spyOn(backend, "storageInfo").mockRejectedValue({ key: "error.internal" });
      await reinit();
      expect(useApp.getState()).toMatchObject({ systemLang: "en", readOnly: false, notice: null });

      useApp.setState({ systemLang: "de" });
      vi.spyOn(backend, "systemLocale").mockResolvedValue(null);
      await reinit();
      expect(useApp.getState().systemLang).toBe("en");
    });

    it("Windows-Sprache, Speicherinfo und Einstellungen sind da, bevor die Werkzeugprüfung fertig ist", async () => {
      await backend.saveSettings({ ...DEFAULT_SETTINGS, preset: "hi", lang: "en" });
      vi.spyOn(backend, "systemLocale").mockResolvedValue("de-DE");
      vi.spyOn(backend, "storageInfo").mockResolvedValue({ readOnly: true });
      let finishTools!: (s: ToolStatus) => void;
      vi.spyOn(backend, "checkTools").mockReturnValue(new Promise<ToolStatus>((resolve) => (finishTools = resolve)));
      useApp.setState({ systemLang: "en", readOnly: false, settings: DEFAULT_SETTINGS, tools: null });
      dispose();
      const ready = useApp.getState().init();
      await vi.advanceTimersByTimeAsync(0);
      // The tool check is still running, yet everything that decides the language is known.
      expect(useApp.getState()).toMatchObject({
        tools: null,
        systemLang: "de",
        readOnly: true,
        settings: { preset: "hi", lang: "en" },
      });
      const tools: ToolStatus = { ok: true, cjxlVersion: "0.12", djxlVersion: "0.12", messages: [] };
      finishTools(tools);
      dispose = await ready;
      expect(useApp.getState().tools).toEqual(tools);
    });

    it("eine fehlgeschlagene Werkzeugprüfung verzögert die Sprache nicht und ändert das Fehlerverhalten nicht", async () => {
      vi.spyOn(backend, "systemLocale").mockResolvedValue("de-DE");
      vi.spyOn(backend, "checkTools").mockRejectedValue({ key: "error.internal" });
      useApp.setState({ systemLang: "en", notice: null });
      await reinit();
      expect(useApp.getState()).toMatchObject({ systemLang: "de", notice: { key: "error.internal" } });
    });

    it("übernimmt den Nur-Lesen-Zustand des Speichers", async () => {
      vi.spyOn(backend, "storageInfo").mockResolvedValue({ readOnly: true });
      await reinit();
      expect(useApp.getState().readOnly).toBe(true);
    });

    it("die gewählte Sprache wird mit den Einstellungen gespeichert und wieder geladen", async () => {
      useApp.getState().updateSettings({ lang: "en" });
      await vi.advanceTimersByTimeAsync(500);
      expect(await backend.loadSettings()).toMatchObject({ lang: "en" });
      useApp.setState({ settings: DEFAULT_SETTINGS });
      await reinit();
      expect(useApp.getState().settings.lang).toBe("en");
    });

    it("start und retryFailed übergeben die gewählte Sprache", async () => {
      const startRun = vi.spyOn(backend, "startRun");
      const retryFailed = vi.spyOn(backend, "retryFailed").mockResolvedValue();
      useApp.getState().updateSettings({ lang: "en" });
      await useApp.getState().addPaths(["x"]);
      await useApp.getState().start();
      expect(startRun).toHaveBeenLastCalledWith("enc", expect.objectContaining({ keepMetadata: true }), "en");
      await vi.advanceTimersByTimeAsync(60_000);

      useApp.setState({ queues: { enc: [{ ...useApp.getState().queues.enc[0], status: "error" }], dec: [] } });
      await useApp.getState().retryFailed();
      expect(retryFailed).toHaveBeenLastCalledWith("enc", expect.anything(), "en");
    });

    it("start übergibt ohne Wahl die Windows-Sprache", async () => {
      vi.spyOn(backend, "systemLocale").mockResolvedValue("de-CH");
      const startRun = vi.spyOn(backend, "startRun").mockResolvedValue();
      await reinit();
      await useApp.getState().start();
      expect(startRun).toHaveBeenLastCalledWith("enc", expect.anything(), "de");
    });
  });

  it("Ereignisse übernehmen die Hinweise als Msg-Liste; Wiederholen leert sie", async () => {
    await useApp.getState().addPaths(["x"]);
    await useApp.getState().addPaths(["y"]);
    await useApp.getState().start();
    await vi.advanceTimersByTimeAsync(120_000);
    const rows = useApp.getState().queues.enc;
    expect(rows.every((r) => Array.isArray(r.notes))).toBe(true);
    const skipped = rows.filter((r) => r.status === "skipped");
    expect(skipped).toHaveLength(1);
    expect(skipped[0].notes).toEqual([{ key: "error.targetExists" }]);
    expect(rows.filter((r) => r.status === "done").every((r) => r.notes.length === 0)).toBe(true);

    vi.spyOn(backend, "retryFailed").mockResolvedValue();
    const note = { key: "error.cannotRead" };
    useApp.setState({ queues: { enc: [{ ...rows[0], status: "error", notes: [note] }], dec: [] } });
    await useApp.getState().retryFailed();
    expect(useApp.getState().queues.enc[0]).toMatchObject({ status: "waiting", notes: [] });
  });

  it("Papierkorb-Schalter startet nach dem Laden immer aus", async () => {
    await backend.saveSettings({ ...DEFAULT_SETTINGS, trashOriginals: true, preset: "hi" });
    dispose();
    dispose = await useApp.getState().init();
    expect(useApp.getState().settings).toMatchObject({ preset: "hi", trashOriginals: false });
  });

  it("beschädigte gespeicherte Einstellungen fallen feldweise auf den Standard zurück", async () => {
    const corrupt = { ...DEFAULT_SETTINGS, preset: "ultra", mode: "x", effort: 99, output: { kind: "irgendwo" } };
    await backend.saveSettings(corrupt as unknown as typeof DEFAULT_SETTINGS);
    dispose();
    dispose = await useApp.getState().init();
    expect(useApp.getState().settings).toEqual(DEFAULT_SETTINGS);
    await useApp.getState().addPaths(["x"]);
    await useApp.getState().start();
    expect(useApp.getState().run).toBe("running");
  });
});
