import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(cleanup);

// jsdom has no ResizeObserver (HeroUI's ScrollShadow in Tabs needs it).
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
};

// jsdom has no Web Animations API (HeroUI's Tabs indicator calls getAnimations).
Element.prototype.getAnimations ??= () => [];
