import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { backend } from "../api";
import type { PreviewDto } from "../api/types";
import { DEFAULT_SETTINGS } from "../state/settings";
import { useApp, type Row } from "../state/store";
import { CompareSlider } from "./CompareSlider";

function row(id: number, extra: Partial<Row> = {}): Row {
  return {
    id,
    entry: { path: `C:\\Bilder\\Foto_${id}.jpg`, root: null, kind: "jpeg", size: 7_130_000, width: 4000, height: 3000 },
    status: "waiting",
    progress: 0,
    notes: [],
    ...extra,
  };
}

const dto: PreviewDto = { before: "data:image/png;base64,AAAA", after: "data:image/png;base64,BBBB", lossless: true };

function setup(rows: Row[] = [row(1)], settings = DEFAULT_SETTINGS) {
  useApp.setState({
    settings,
    queues: { enc: rows, dec: [] },
    selected: { enc: rows[0]?.id ?? null, dec: null },
  });
}

beforeEach(() => {
  useApp.setState({ systemLang: "de" });
  vi.useFakeTimers();
  vi.spyOn(backend, "preview").mockResolvedValue(dto);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const advance = (ms: number) => act(async () => void (await vi.advanceTimersByTimeAsync(ms)));

describe("CompareSlider", () => {
  it("zeigt ohne Auswahl „Keine Datei ausgewählt“ und lädt nichts", async () => {
    setup([]);
    render(<CompareSlider />);
    await advance(0);
    expect(screen.getByText("Keine Datei ausgewählt")).toBeInTheDocument();
    expect(backend.preview).not.toHaveBeenCalled();
  });

  it("zeigt Kopf, Etiketten und lädt die Vorschau sofort", async () => {
    setup();
    render(<CompareSlider />);
    await advance(0);
    expect(screen.getByText("Vorher / Nachher")).toBeInTheDocument();
    expect(screen.getByText("Foto_1.jpg · 4000 × 3000")).toBeInTheDocument();
    expect(screen.getByText("JPG · 6,80 MB")).toBeInTheDocument();
    expect(backend.preview).toHaveBeenCalledTimes(1);
    expect(screen.getByAltText("Vorher")).toHaveAttribute("src", dto.before);
    expect(screen.getByAltText("Nachher")).toHaveAttribute("src", dto.after);
  });

  it("verschiebt die Trennlinie beim Input auf 30 nach left: 30%", async () => {
    setup();
    render(<CompareSlider />);
    await advance(0);
    expect(screen.getByTestId("compare-divider")).toHaveStyle({ left: "50%" });
    fireEvent.change(screen.getByRole("slider", { name: "Trennlinie" }), { target: { value: "30" } });
    expect(screen.getByTestId("compare-divider")).toHaveStyle({ left: "30%" });
    expect(screen.getByTestId("compare-after")).toHaveStyle({ clipPath: "inset(0 0 0 30%)" });
  });

  it("nennt rechts „JXL · verlustfrei“ bei ll und „JXL · d 2.6“ bei sm", async () => {
    setup();
    render(<CompareSlider />);
    await advance(0);
    expect(screen.getByText("JXL · verlustfrei")).toBeInTheDocument();
    act(() => useApp.getState().updateSettings({ preset: "sm" }));
    expect(screen.getByText("JXL · d 2.6")).toBeInTheDocument();
    await advance(600);
  });

  it("hängt nach der Umwandlung die Zielgröße an", async () => {
    setup([row(1, { status: "done", progress: 100, outSize: 5_700_000 })]);
    render(<CompareSlider />);
    await advance(0);
    expect(screen.getByText("JXL · verlustfrei · 5,44 MB")).toBeInTheDocument();
  });

  it("zeigt im Dec-Modus JXL links und das Zielformat rechts", async () => {
    const jxl: Row = {
      ...row(1),
      entry: { path: "C:\\Bilder\\a.jxl", root: null, kind: "jxl", size: 1024 * 1024, width: 10, height: 20 },
    };
    useApp.setState({
      settings: { ...DEFAULT_SETTINGS, mode: "dec", decodeFormat: "png" },
      queues: { enc: [], dec: [{ ...jxl, status: "done", progress: 100, outSize: 2 * 1024 * 1024 }] },
      selected: { enc: null, dec: 1 },
    });
    render(<CompareSlider />);
    await advance(0);
    expect(screen.getByText("JXL · 1,00 MB")).toBeInTheDocument();
    expect(screen.getByText("PNG · 2,00 MB")).toBeInTheDocument();
  });

  it("löst nach einer Einstellungsänderung nach 300 ms genau einen preview-Aufruf aus", async () => {
    setup();
    render(<CompareSlider />);
    await advance(0);
    expect(backend.preview).toHaveBeenCalledTimes(1);
    act(() => useApp.getState().updateSettings({ preset: "bal" }));
    await advance(299);
    expect(backend.preview).toHaveBeenCalledTimes(1);
    await advance(1);
    expect(backend.preview).toHaveBeenCalledTimes(2);
    await advance(1000);
    expect(backend.preview).toHaveBeenCalledTimes(2);
  });

  it("der Papierkorb-Schalter lädt die Vorschau nicht neu", async () => {
    setup();
    render(<CompareSlider />);
    await advance(0);
    expect(backend.preview).toHaveBeenCalledTimes(1);
    act(() => useApp.getState().updateSettings({ trashOriginals: true }));
    await advance(1000);
    expect(backend.preview).toHaveBeenCalledTimes(1);
  });

  it("lädt beim Wechsel der Auswahl sofort", async () => {
    setup([row(1), row(2)]);
    render(<CompareSlider />);
    await advance(0);
    act(() => useApp.getState().select(2));
    await advance(0);
    expect(backend.preview).toHaveBeenCalledTimes(2);
    expect(vi.mocked(backend.preview).mock.calls[1][0]).toBe(2);
  });

  it("ignoriert eine veraltete, langsamere Antwort", async () => {
    let resolveOld!: (d: PreviewDto) => void;
    const newer: PreviewDto = { before: "data:new-b", after: "data:new-a", lossless: false };
    vi.mocked(backend.preview)
      .mockImplementationOnce(() => new Promise((r) => (resolveOld = r)))
      .mockResolvedValueOnce(newer);
    setup([row(1), row(2)]);
    render(<CompareSlider />);
    await advance(0);
    act(() => useApp.getState().select(2));
    await advance(0);
    expect(screen.getByAltText("Nachher")).toHaveAttribute("src", "data:new-a");
    await act(async () => resolveOld(dto));
    expect(screen.getByAltText("Nachher")).toHaveAttribute("src", "data:new-a");
  });

  it("zeigt bei einem Vorschaufehler einen kurzen Text", async () => {
    vi.mocked(backend.preview).mockRejectedValue(new Error("boom"));
    setup();
    render(<CompareSlider />);
    await advance(0);
    expect(screen.getByText("Vorschau nicht möglich")).toBeInTheDocument();
  });
});
