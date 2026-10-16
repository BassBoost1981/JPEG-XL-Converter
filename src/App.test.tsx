import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "./App";

describe("App", () => {
  it("zeigt den Platzhalter mit dem Namen JPEG XL", () => {
    render(<App />);
    expect(screen.getByText("JPEG XL")).toBeInTheDocument();
  });
});
