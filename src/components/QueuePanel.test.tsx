import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { backend } from "../api";
import { DEFAULT_SETTINGS } from "../state/settings";
import { useApp, type Row } from "../state/store";
import { QueuePanel } from "./QueuePanel";

const proto = HTMLElement.prototype as unknown as Record<string, unknown>;
const saved: Record<string, PropertyDescriptor | undefined> = {};

function row(id: number, extra: Partial<Row> = {}): Row {
  return {
    id,
    entry: { path: `C:\\Bilder\\Foto_${id}.jpg`, root: null, kind: "jpeg", size: 1024 * 1024, width: 4000, height: 3000 },
    status: "waiting",
    progress: 0,
    ...extra,
  };
}

beforeEach(() => {
  // jsdom lays nothing out: give elements a size so the virtualizer renders rows.
  for (const [prop, value] of [
    ["clientHeight", 400],
    ["clientWidth", 800],
  ] as const) {
    saved[prop] = Object.getOwnPropertyDescriptor(HTMLElement.prototype, prop);
    Object.defineProperty(HTMLElement.prototype, prop, { configurable: true, get: () => value });
  }
  useApp.setState({
    settings: DEFAULT_SETTINGS,
    queues: { enc: [], dec: [] },
    selected: { enc: null, dec: null },
    run: "idle",
    notice: null,
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  for (const prop of ["clientHeight", "clientWidth"]) {
    if (saved[prop]) Object.defineProperty(HTMLElement.prototype, prop, saved[prop]);
    else delete proto[prop];
  }
});

describe("QueuePanel", () => {
  it("zeigt die Ablagefläche mit Untertitel je Modus", () => {
    const { unmount } = render(<QueuePanel />);
    expect(screen.getByText("Bilder oder Ordner hierher ziehen")).toBeInTheDocument();
    expect(screen.getByText("JPEG, PNG, GIF, TIFF, WebP, BMP")).toBeInTheDocument();
    unmount();
    useApp.setState({ settings: { ...DEFAULT_SETTINGS, mode: "dec" } });
    render(<QueuePanel />);
    expect(screen.getByText(".jxl-Dateien")).toBeInTheDocument();
  });

  it("zeigt nach addPaths 6 Zeilen und den Chip „6 Dateien“", async () => {
    render(<QueuePanel />);
    await useApp.getState().addPaths(["C:\\Bilder"]);
    expect(await screen.findByText("6 Dateien")).toBeInTheDocument();
    expect(screen.getAllByTestId("queue-row")).toHaveLength(6);
    expect(screen.queryByText("Bilder oder Ordner hierher ziehen")).not.toBeInTheDocument();
  });

  it("nennt eine einzelne Datei im Singular", () => {
    useApp.setState({ queues: { enc: [row(1)], dec: [] } });
    render(<QueuePanel />);
    expect(screen.getByText("1 Datei")).toBeInTheDocument();
  });

  it("beschriftet die Ersparnis-Spalte je Modus", () => {
    const { unmount } = render(<QueuePanel />);
    expect(screen.getByText("Ersparnis")).toBeInTheDocument();
    unmount();
    useApp.setState({ settings: { ...DEFAULT_SETTINGS, mode: "dec" } });
    render(<QueuePanel />);
    expect(screen.getByText("Änderung")).toBeInTheDocument();
  });

  it("zeigt „—“ bei wartender Zeile, Größe und −20 % bei fertiger", () => {
    useApp.setState({
      queues: { enc: [row(1), row(2, { status: "done", progress: 100, outSize: Math.round(1024 * 1024 * 0.8) })], dec: [] },
    });
    render(<QueuePanel />);
    const [waiting, done] = screen.getAllByTestId("queue-row");
    expect(within(waiting).getAllByText("—")).toHaveLength(2);
    expect(within(done).getByText("819 KB")).toBeInTheDocument();
    expect(within(done).getByText("−20 %")).toBeInTheDocument();
  });

  it("zeigt Statuschips", () => {
    useApp.setState({
      queues: {
        enc: [
          row(1),
          row(2, { status: "running", progress: 42 }),
          row(3, { status: "done", progress: 100, outSize: 1 }),
          row(4, { status: "doneWithNote", progress: 100, outSize: 1, note: "Original blieb liegen" }),
          row(5, { status: "skipped", progress: 100, note: "Zieldatei existiert bereits" }),
          row(6, { status: "error", progress: 0, note: "Datei fehlt" }),
          row(7, { status: "verifyFailed", progress: 100, note: "Abweichung" }),
        ],
        dec: [],
      },
    });
    render(<QueuePanel />);
    for (const t of ["Wartet", "42 %", "Fertig", "Fertig mit Hinweis", "Übersprungen", "Fehler", "Prüfung fehlgeschlagen"]) {
      expect(screen.getByText(t)).toBeInTheDocument();
    }
  });

  it("zeigt den Fehlertext im Tooltip", async () => {
    useApp.setState({ queues: { enc: [row(6, { status: "error", note: "Datei fehlt" })], dec: [] } });
    render(<QueuePanel />);
    // Keyboard focus: Dateien, Ordner hinzufügen, Leeren, then the status chip.
    for (let i = 0; i < 4; i++) await userEvent.tab();
    expect(await screen.findByRole("tooltip", {}, { timeout: 3000 })).toHaveTextContent("Datei fehlt");
  });

  it("sperrt „Leeren“ während eines Laufs", () => {
    useApp.setState({ queues: { enc: [row(1)], dec: [] }, run: "running" });
    render(<QueuePanel />);
    expect(screen.getByRole("button", { name: "Leeren" })).toBeDisabled();
  });

  it("leert die Liste", async () => {
    useApp.setState({ queues: { enc: [row(1)], dec: [] } });
    render(<QueuePanel />);
    await userEvent.click(screen.getByRole("button", { name: "Leeren" }));
    expect(await screen.findByText("Bilder oder Ordner hierher ziehen")).toBeInTheDocument();
  });

  it("setzt selected bei Klick auf eine Zeile", async () => {
    useApp.setState({ queues: { enc: [row(1), row(2)], dec: [] } });
    render(<QueuePanel />);
    await userEvent.click(screen.getAllByTestId("queue-row")[1]);
    expect(useApp.getState().selected.enc).toBe(2);
  });

  it("rendert bei 2 000 Einträgen weniger als 60 Zeilen", () => {
    const many = Array.from({ length: 2000 }, (_, i) => row(100_000 + i));
    useApp.setState({ queues: { enc: many, dec: [] } });
    render(<QueuePanel />);
    expect(screen.getByText("2.000 Dateien")).toBeInTheDocument();
    const n = screen.getAllByTestId("queue-row").length;
    expect(n).toBeGreaterThan(0);
    expect(n).toBeLessThan(60);
  });

  it("lädt Vorschaubilder nur für sichtbare Zeilen, je Zeile einmal", async () => {
    const thumb = vi.spyOn(backend, "thumbnail").mockResolvedValue("data:image/png;base64,AA==");
    const many = Array.from({ length: 2000 }, (_, i) => row(200_000 + i));
    useApp.setState({ queues: { enc: many, dec: [] } });
    const { rerender } = render(<QueuePanel />);
    const rendered = screen.getAllByTestId("queue-row").length;
    await vi.waitFor(() => expect(thumb).toHaveBeenCalledTimes(rendered));
    rerender(<QueuePanel />);
    expect(thumb).toHaveBeenCalledTimes(rendered);
  });

  it("behält die Endungskachel, wenn das Vorschaubild fehlschlägt", async () => {
    const thumb = vi.spyOn(backend, "thumbnail").mockRejectedValue(new Error("kaputt"));
    useApp.setState({ queues: { enc: [row(300_001)], dec: [] } });
    render(<QueuePanel />);
    await vi.waitFor(() => expect(thumb).toHaveBeenCalled());
    expect(screen.getByText("jpg")).toBeInTheDocument();
  });

  it("öffnet die Dateiauswahl und den Ordnerdialog", async () => {
    const files = vi.spyOn(backend, "pickFiles").mockResolvedValue(["C:\\a.jpg"]);
    const folder = vi.spyOn(backend, "pickFolder").mockResolvedValue("C:\\Bilder");
    const add = vi.spyOn(backend, "addPaths").mockResolvedValue({ items: [], rejected: 0 });
    render(<QueuePanel />);
    await userEvent.click(screen.getByRole("button", { name: "Dateien" }));
    await vi.waitFor(() => expect(add).toHaveBeenCalledWith(["C:\\a.jpg"], "enc"));
    await userEvent.click(screen.getByRole("button", { name: "Ordner hinzufügen" }));
    await vi.waitFor(() => expect(add).toHaveBeenCalledWith(["C:\\Bilder"], "enc"));
    expect(files).toHaveBeenCalledWith("enc");
    expect(folder).toHaveBeenCalled();
  });

  it("zeigt bei abgelehnten Dateien den Hinweis mit Anzahl", async () => {
    vi.spyOn(backend, "addPaths").mockResolvedValue({ items: [], rejected: 3 });
    render(<QueuePanel />);
    await useApp.getState().addPaths(["C:\\x.txt"]);
    expect(await screen.findByText("3 Dateien passen nicht zu diesem Modus und wurden nicht aufgenommen.")).toBeInTheDocument();
  });

  it("nennt eine abgelehnte Datei im Singular", async () => {
    vi.spyOn(backend, "addPaths").mockResolvedValue({ items: [], rejected: 1 });
    render(<QueuePanel />);
    await useApp.getState().addPaths(["C:\\x.txt"]);
    expect(await screen.findByText("1 Datei passt nicht zu diesem Modus und wurde nicht aufgenommen.")).toBeInTheDocument();
  });
});
