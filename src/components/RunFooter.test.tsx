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
    ...extra,
  };
}

const actions = { start: vi.fn(), pause: vi.fn(), resume: vi.fn(), retryFailed: vi.fn() };

function setup(rows: Row[], extra: Partial<ReturnType<typeof useApp.getState>> = {}) {
  useApp.setState({
    settings: DEFAULT_SETTINGS,
    queues: { enc: rows, dec: [] },
    run: "idle",
    pauseReason: null,
    summary: null,
    tools: { ok: true, cjxlVersion: "0.12", djxlVersion: "0.12", message: null },
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
    setup([row(1)], { tools: { ok: false, cjxlVersion: null, djxlVersion: null, message: "fehlt" } });
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
