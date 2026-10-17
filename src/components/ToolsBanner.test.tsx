import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { ToolStatus } from "../api/types";
import { translate } from "../i18n";
import { DEFAULT_SETTINGS } from "../state/settings";
import { useApp } from "../state/store";
import { ToolsBanner } from "./ToolsBanner";

const missing = (messages: ToolStatus["messages"]): ToolStatus => ({
  ok: false,
  cjxlVersion: null,
  djxlVersion: null,
  messages,
});

beforeEach(() => {
  useApp.setState({ settings: DEFAULT_SETTINGS, systemLang: "de", tools: null, readOnly: false });
});

describe("ToolsBanner", () => {
  it("zeigt nichts, solange die Werkzeuge unbekannt oder in Ordnung sind", () => {
    const { container, rerender } = render(<ToolsBanner />);
    expect(container).toBeEmptyDOMElement();
    useApp.setState({ tools: { ok: true, cjxlVersion: "0.12", djxlVersion: "0.12", messages: [] } });
    rerender(<ToolsBanner />);
    expect(container).toBeEmptyDOMElement();
  });

  it("zeigt ohne Meldungen den Standardtext in der Sprache", () => {
    useApp.setState({ tools: missing([]) });
    const a = render(<ToolsBanner />);
    expect(screen.getByRole("alert")).toHaveTextContent(translate("de", "banner.toolsMissing"));
    a.unmount();

    useApp.setState({ settings: { ...DEFAULT_SETTINGS, lang: "en" } });
    render(<ToolsBanner />);
    expect(screen.getByRole("alert")).toHaveTextContent("cjxl.exe or djxl.exe is missing next to the app.");
  });

  it("zeigt jede Meldung übersetzt als eigene Zeile", () => {
    useApp.setState({
      settings: { ...DEFAULT_SETTINGS, lang: "en" },
      tools: missing([
        { key: "tool.noResponse", params: { name: "cjxl.exe" } },
        { key: "tool.noVersion", params: { name: "djxl.exe" } },
      ]),
    });
    render(<ToolsBanner />);
    // `getByText` only finds a line that is an element of its own.
    expect(screen.getByText("cjxl.exe does not respond.")).toBeInTheDocument();
    expect(screen.getByText("djxl.exe reports no version.")).toBeInTheDocument();
    expect(screen.queryByText(/is missing next to the app/)).not.toBeInTheDocument();
  });

  it("wechselt mit der Sprache", () => {
    useApp.setState({ tools: missing([{ key: "banner.toolsMissing" }]) });
    render(<ToolsBanner />);
    expect(screen.getByText("cjxl.exe oder djxl.exe fehlt neben dem Programm.")).toBeInTheDocument();
    act(() => useApp.setState({ settings: { ...DEFAULT_SETTINGS, lang: "en" } }));
    expect(screen.getByText("cjxl.exe or djxl.exe is missing next to the app.")).toBeInTheDocument();
  });

  it("zeigt im Nur-Lesen-Modus den Hinweis, auch wenn die Werkzeuge da sind", () => {
    useApp.setState({
      readOnly: true,
      tools: { ok: true, cjxlVersion: "0.12", djxlVersion: "0.12", messages: [] },
    });
    const a = render(<ToolsBanner />);
    expect(screen.getByText(translate("de", "banner.readOnly"))).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    a.unmount();

    useApp.setState({ settings: { ...DEFAULT_SETTINGS, lang: "en" } });
    render(<ToolsBanner />);
    expect(
      screen.getByText("App folder is read-only – settings are not saved, logs go to the temp folder."),
    ).toBeInTheDocument();
  });

  it("zeigt Werkzeugfehler und Nur-Lesen-Hinweis zusammen", () => {
    useApp.setState({ readOnly: true, tools: missing([]) });
    render(<ToolsBanner />);
    expect(screen.getByRole("alert")).toHaveTextContent(translate("de", "banner.toolsMissing"));
    expect(screen.getByText(translate("de", "banner.readOnly"))).toBeInTheDocument();
  });
});
