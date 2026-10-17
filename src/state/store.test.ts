import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS } from "./settings";

let dispose: () => void;
let useApp: typeof import("./store").useApp;
let backend: typeof import("../api").backend;

// Fresh module graph per test: every test gets its own MockBackend and store.
beforeEach(async () => {
  vi.useFakeTimers();
  vi.resetModules();
  ({ useApp } = await import("./store"));
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

  it("ein Speicherfehler setzt nur den Hinweis", async () => {
    vi.spyOn(backend, "saveSettings").mockRejectedValue(new Error("kaputt"));
    useApp.getState().updateSettings({ effort: 3 });
    await vi.advanceTimersByTimeAsync(500);
    expect(useApp.getState().notice).toBe("kaputt");
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
