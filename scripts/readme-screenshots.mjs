// Creates the README screenshots in docs/screenshots/ from the UI running in a browser
// (demo mode: mock backend with sample data). Starts nothing itself.
//
// Needs (Node 24: global fetch and WebSocket, no packages):
//   1. the Vite dev server:   npm run dev                       (http://localhost:1420)
//   2. Edge or Chrome with a debugging port and a throwaway profile, for example
//        msedge --headless=new --remote-debugging-port=9222 --user-data-dir=%TEMP%\jxl-shots
// Then:
//   node scripts/readme-screenshots.mjs [debugPort=9222] [appUrl=http://localhost:1420]
//
// Per language and theme: viewport 1200 x 760 at device scale 2, choose the language in the app
// settings popover, add the sample folder, select DSC_4022.jpg, start the run, wait about 2.6 s, save a PNG.
// The "settings" shots keep the popover open (light theme only).

import { mkdir, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const port = Number(process.argv[2] ?? 9222);
const appUrl = process.argv[3] ?? "http://localhost:1420";
const outDir = join(dirname(fileURLToPath(import.meta.url)), "..", "docs", "screenshots");

// The button label is still in the browser language before a language is chosen, so accept both.
const SETTINGS = /^(App settings|Programm-Einstellungen)$/;

const TEXT = {
  en: { language: "English", addFolder: "Add folder", start: /^Convert \d+ files? to JPEG XL$/ },
  de: { language: "Deutsch", addFolder: "Ordner hinzufügen", start: /^\d+ Dateien? in JPEG XL konvertieren$/ },
};

// [language, theme, file name, popover open]
const SHOTS = [
  ["en", "light", "app-en-light.png", false],
  ["en", "dark", "app-en-dark.png", false],
  ["de", "light", "app-de-hell.png", false],
  ["de", "dark", "app-de-dunkel.png", false],
  ["en", "light", "app-en-settings.png", true],
  ["de", "light", "app-de-einstellungen.png", true],
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function openPage() {
  // PUT is required by current Chromium; a new empty tab keeps each shot independent.
  const res = await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: "PUT" });
  if (!res.ok) throw new Error(`Cannot open a tab on port ${port}: HTTP ${res.status}`);
  const target = await res.json();
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = () => reject(new Error("DevTools WebSocket failed"));
  });
  let nextId = 1;
  const pending = new Map();
  ws.onmessage = (ev) => {
    const msg = JSON.parse(ev.data);
    const p = pending.get(msg.id);
    if (!p) return;
    pending.delete(msg.id);
    if (msg.error) p.reject(new Error(`${msg.error.message}`));
    else p.resolve(msg.result);
  };
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject });
      ws.send(JSON.stringify({ id, method, params }));
    });
  const close = async () => {
    ws.close();
    await fetch(`http://127.0.0.1:${port}/json/close/${target.id}`).catch(() => {});
  };
  return { send, close };
}

async function evaluate(page, expression) {
  const r = await page.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
  return r.result.value;
}

// Clicks the first button whose accessible name (aria-label or text) matches; waits for it to appear.
async function clickButton(page, match, what) {
  const source = match instanceof RegExp ? match.source : `^${match.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`;
  const expression = `(async () => {
    const re = new RegExp(${JSON.stringify(source)});
    for (let i = 0; i < 50; i++) {
      const b = [...document.querySelectorAll("button")].find(
        (el) => !el.disabled && re.test((el.getAttribute("aria-label") ?? el.textContent ?? "").trim()));
      if (b) { b.click(); return true; }
      await new Promise((r) => setTimeout(r, 100));
    }
    return false;
  })()`;
  if (!(await evaluate(page, expression))) throw new Error(`Button not found: ${what}`);
}

async function shoot([lang, theme, file, popover]) {
  const t = TEXT[lang];
  const page = await openPage();
  try {
    await page.send("Emulation.setDeviceMetricsOverride", { width: 1200, height: 760, deviceScaleFactor: 2, mobile: false });
    await page.send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: theme }] });
    await page.send("Page.enable");
    await page.send("Page.navigate", { url: appUrl });
    await sleep(1500);

    // The language default follows the browser until a choice is made, so pick it explicitly.
    // The popover has a short fade animation: wait ~400 ms after every open/close.
    await clickButton(page, SETTINGS, "settings button");
    await sleep(400);
    await clickButton(page, t.language, `language ${t.language}`);
    await sleep(400);
    await page.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
    await page.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Escape", code: "Escape", windowsVirtualKeyCode: 27 });
    await sleep(400);

    // The mock backend adds its sample files; select DSC_4022.jpg and start the simulated run.
    await clickButton(page, t.addFolder, "add folder");
    await sleep(600);
    const picked = await evaluate(
      page,
      `(() => { const r = [...document.querySelectorAll('[data-testid="queue-row"]')].find((el) => el.textContent.includes("DSC_4022")); if (!r) return false; r.click(); return true; })()`,
    );
    if (!picked) throw new Error("Row DSC_4022.jpg not found");
    await clickButton(page, t.start, "start button");
    await sleep(2600);

    if (popover) {
      await clickButton(page, SETTINGS, "settings button");
      await sleep(400);
    }

    const shot = await page.send("Page.captureScreenshot", { format: "png" });
    await writeFile(join(outDir, file), Buffer.from(shot.data, "base64"));
    console.log(`${file} (${lang}, ${theme}${popover ? ", popover open" : ""})`);
  } finally {
    await page.close();
  }
}

await mkdir(outDir, { recursive: true });
for (const s of SHOTS) await shoot(s);
