import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { backend } from "../api";
import type { ItemStatus } from "../api/types";
import { DEFAULT_SETTINGS } from "../state/settings";
import { useApp, type Row } from "../state/store";
import { RunFooter } from "./RunFooter";

const MB = 1024 * 1024;

function row(id: number, status: ItemStatus = "waiting", extra: Partial<Row> = {}): Row {
  return {
    id,
    entry: { path: `C:\\Bilder\\Foto_${id}.jpg`, root: null, kind: "jpeg", size: 10 * MB, width: 4000, height: 3000 },
    status,
    progress: status === "waiting" ? 0 : 100,
    notes: [],
    ...extra,
  };
}

const actions = { start: vi.fn(), pause: vi.fn(), resume: vi.fn(), retryFailed: vi.fn() };

function setup(rows: Row[], extra: Partial<ReturnType<typeof useApp.getState>> = {}) {
  useApp.setState({
    settings: DEFAULT_SETTINGS,
    systemLang: "de",
    queues: { enc: rows, dec: [] },
    run: "idle",
    pauseReason: null,
    summary: null,
    tools: { ok: true, cjxlVersion: "0.12", djxlVersion: "0.12", messages: [] },
    ...actions,
    ...extra,
  });
}

beforeEach(() => {
  Object.values(actions).forEach((f) => f.mockReset().mockResolvedValue(undefined));
});
afterEach(() => vi.restoreAllMocks());

describe("RunFooter", () => {
  it("zeigt vor der ersten fertigen Datei den Hinweistext, „–“ und den Startknopf", () => {
    setup([row(1), row(2)]);
    render(<RunFooter />);
    expect(screen.getByText("Ersparnis erscheint nach der ersten fertigen Datei")).toBeInTheDocument();
    expect(screen.getByText("–")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "2 Dateien in JPEG XL konvertieren" })).toBeEnabled();
  });

  it("folgt der gewählten Sprache, sonst der Windows-Sprache", () => {
    setup([row(1), row(2)], { systemLang: "en" });
    const a = render(<RunFooter />);
    expect(screen.getByText("Savings appear after the first finished file")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Convert 2 files to JPEG XL" })).toBeEnabled();
    a.unmount();

    setup([row(1), row(2)], { systemLang: "en", settings: { ...DEFAULT_SETTINGS, lang: "de" } });
    render(<RunFooter />);
    expect(screen.getByText("Ersparnis erscheint nach der ersten fertigen Datei")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "2 Dateien in JPEG XL konvertieren" })).toBeEnabled();
  });

  describe("auf Englisch", () => {
    const en = { settings: { ...DEFAULT_SETTINGS, lang: "en" as const } };

    it("beschriftet Hinweis, Startknopf, Fortschritt, Laufwerk und Protokoll", async () => {
      const user = userEvent.setup();
      setup(Array.from({ length: 6 }, (_, i) => row(i + 1)), en);
      const a = render(<RunFooter />);
      expect(screen.getByText("Savings appear after the first finished file")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Convert 6 files to JPEG XL" })).toBeEnabled();
      expect(screen.getByRole("progressbar", { name: "Overall progress" })).toBeInTheDocument();
      a.unmount();

      setup([row(1, "running", { progress: 10 })], { ...en, run: "running" });
      const b = render(<RunFooter />);
      await user.click(screen.getByRole("button", { name: "Stop" }));
      expect(actions.pause).toHaveBeenCalledTimes(1);
      b.unmount();

      setup([row(1, "running", { progress: 10 })], { ...en, run: "paused", pauseReason: { type: "diskFull", drive: "D:" } });
      const c = render(<RunFooter />);
      expect(screen.getByText("No space left on drive D:")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Resume" })).toBeEnabled();
      c.unmount();

      setup([row(1, "done", { outSize: 5 * MB })], {
        ...en,
        run: "finished",
        summary: { done: 1, skipped: 0, errors: 0, bytesIn: 10 * MB, bytesOut: 5 * MB, logPath: "C:\\Logs\\a.txt" },
      });
      const openLog = vi.spyOn(backend, "openLog").mockResolvedValue(undefined);
      const d = render(<RunFooter />);
      expect(screen.getByText("1 done · 0 skipped · 0 errors · 10.0 MB → 5.00 MB")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "All done" })).toBeDisabled();
      await user.click(screen.getByRole("button", { name: "Open log" }));
      expect(openLog).toHaveBeenCalledWith("C:\\Logs\\a.txt");
      d.unmount();

      setup([row(1, "error")], { ...en, run: "finished" });
      render(<RunFooter />);
      await user.click(screen.getByRole("button", { name: "Retry failed" }));
      expect(actions.retryFailed).toHaveBeenCalledTimes(1);
    });

    it("sperrt den Knopf bei „All done“ ebenso wie bei „Alles erledigt“", () => {
      setup([row(1, "done", { outSize: MB })], { ...en, run: "finished" });
      const a = render(<RunFooter />);
      expect(screen.getByRole("button", { name: "All done" })).toBeDisabled();
      a.unmount();

      setup([row(1, "done", { outSize: MB })], { run: "finished", settings: { ...DEFAULT_SETTINGS, lang: "de" } });
      render(<RunFooter />);
      expect(screen.getByRole("button", { name: "Alles erledigt" })).toBeDisabled();
    });

    it("schreibt große Zahlen englisch und deutsch", () => {
      const many = Array.from({ length: 1200 }, (_, i) => row(i + 1, i === 0 ? "done" : "waiting", { outSize: MB }));
      setup(many, en);
      const a = render(<RunFooter />);
      expect(screen.getByRole("button", { name: "Convert 1,199 files to JPEG XL" })).toBeEnabled();
      a.unmount();

      setup(many, { settings: { ...DEFAULT_SETTINGS, lang: "de" } });
      render(<RunFooter />);
      expect(screen.getByRole("button", { name: "1.199 Dateien in JPEG XL konvertieren" })).toBeEnabled();
    });
  });

  it("färbt die Ersparnis nach dem Zahlenwert: nur kleiner als das Original ist grün", () => {
    setup([row(1, "done", { outSize: 12 * MB })]);
    const a = render(<RunFooter />);
    expect(screen.getByText("+20 %")).toHaveStyle({ color: "var(--muted)" });
    a.unmount();

    // Rounds to "0 %" but is still smaller.
    setup([row(1, "done", { outSize: 10 * MB - 1024 })]);
    render(<RunFooter />);
    expect(screen.getByText("0 %")).toHaveStyle({ color: "var(--good)" });
  });

  it("zeigt Fortschrittszeile, grüne Ersparnis und bytegewichteten Fortschritt", () => {
    setup([row(1, "done", { outSize: 5 * MB }), row(2), row(3, "running", { progress: 50 }), row(4)]);
    render(<RunFooter />);
    expect(screen.getByText("1 von 4 fertig · 10,0 MB → 5,00 MB")).toBeInTheDocument();
    expect(screen.getByText("−50 %")).toHaveStyle({ color: "var(--good)" });
    // (10 + 5) / 40 MB
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "38");
  });

  it("ruft je nach Zustand die richtige Aktion auf", async () => {
    const user = userEvent.setup();
    setup([row(1)]);
    const r1 = render(<RunFooter />);
    await user.click(screen.getByRole("button", { name: "1 Datei in JPEG XL konvertieren" }));
    expect(actions.start).toHaveBeenCalledTimes(1);
    r1.unmount();

    setup([row(1, "running", { progress: 10 })], { run: "running" });
    const r2 = render(<RunFooter />);
    await user.click(screen.getByRole("button", { name: "Anhalten" }));
    expect(actions.pause).toHaveBeenCalledTimes(1);
    r2.unmount();

    setup([row(1, "running", { progress: 10 })], { run: "paused", pauseReason: { type: "user" } });
    const r3 = render(<RunFooter />);
    await user.click(screen.getByRole("button", { name: "Fortsetzen" }));
    expect(actions.resume).toHaveBeenCalledTimes(1);
    r3.unmount();

    setup([row(1, "error")], { run: "finished" });
    render(<RunFooter />);
    await user.click(screen.getByRole("button", { name: "Fehlgeschlagene erneut versuchen" }));
    expect(actions.retryFailed).toHaveBeenCalledTimes(1);
  });

  it("startet nach einem Lauf mit wartenden Dateien erneut (R2)", async () => {
    const user = userEvent.setup();
    setup([row(1, "done", { outSize: MB }), row(2)], { run: "finished" });
    render(<RunFooter />);
    await user.click(screen.getByRole("button", { name: "1 Datei in JPEG XL konvertieren" }));
    expect(actions.start).toHaveBeenCalledTimes(1);
  });

  it("sperrt den Knopf ohne Werkzeuge (R1), ohne wartende Dateien und bei „Alles erledigt“", () => {
    setup([row(1)], { tools: { ok: false, cjxlVersion: null, djxlVersion: null, messages: [{ key: "tool.noResponse", params: { name: "cjxl.exe" } }] },
    });
    const a = render(<RunFooter />);
    expect(screen.getByRole("button", { name: "1 Datei in JPEG XL konvertieren" })).toBeDisabled();
    a.unmount();

    setup([]);
    const b = render(<RunFooter />);
    expect(screen.getByRole("button", { name: "0 Dateien in JPEG XL konvertieren" })).toBeDisabled();
    b.unmount();

    setup([row(1, "done", { outSize: MB })], { run: "finished" });
    render(<RunFooter />);
    expect(screen.getByRole("button", { name: "Alles erledigt" })).toBeDisabled();
  });

  it("nennt bei diskFull das Laufwerk", () => {
    setup([row(1, "running", { progress: 10 })], { run: "paused", pauseReason: { type: "diskFull", drive: "D:" } });
    render(<RunFooter />);
    expect(screen.getByText("Kein Platz mehr auf Laufwerk D:")).toBeInTheDocument();
  });

  it("zeigt bei diskFull eine Netzwerkfreigabe ohne angehängten Doppelpunkt", () => {
    const share = "\\\\server\\fotos";
    setup([row(1, "running", { progress: 10 })], { run: "paused", pauseReason: { type: "diskFull", drive: share } });
    render(<RunFooter />);
    expect(screen.getByText(`Kein Platz mehr auf Laufwerk ${share}`)).toBeInTheDocument();
  });

  it("zeigt nach dem Lauf die Zusammenfassung und öffnet das Protokoll", async () => {
    const user = userEvent.setup();
    const openLog = vi.spyOn(backend, "openLog").mockResolvedValue(undefined);
    setup([row(1, "done", { outSize: 5 * MB })], { run: "running" });
    render(<RunFooter />);
    expect(screen.queryByRole("button", { name: "Protokoll öffnen" })).not.toBeInTheDocument();
    act(() =>
      useApp.getState().finished({
        done: 1,
        skipped: 0,
        errors: 0,
        bytesIn: 10 * MB,
        bytesOut: 5 * MB,
        logPath: "C:\\Logs\\a.txt",
      }),
    );
    expect(screen.getByText("1 fertig · 0 übersprungen · 0 Fehler · 10,0 MB → 5,00 MB")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Protokoll öffnen" }));
    expect(openLog).toHaveBeenCalledWith("C:\\Logs\\a.txt");
  });

  it("blendet „Protokoll öffnen“ ohne logPath aus", () => {
    setup([row(1, "done", { outSize: MB })], {
      run: "finished",
      summary: { done: 1, skipped: 0, errors: 0, bytesIn: 10 * MB, bytesOut: MB, logPath: null },
    });
    render(<RunFooter />);
    expect(screen.queryByRole("button", { name: "Protokoll öffnen" })).not.toBeInTheDocument();
  });
});
