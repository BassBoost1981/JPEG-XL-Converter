# Zweisprachigkeit, Programm-Einstellungen und USB-Portabilität – Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Die App wird komplett zweisprachig (Deutsch/Englisch, umschaltbar im neuen Klappfenster „Programm-Einstellungen“), heißt nach außen „JPEG XL Converter“, speichert alles neben der Exe (USB-portabel, nichts im Benutzerprofil), und GitHub präsentiert sich englisch zuerst mit deutscher Zweitfassung.

**Architecture:** Alle Texte stehen in zwei Katalogen `src/i18n/de.json` und `src/i18n/en.json` mit identischen, flachen Schlüsseln. Die Oberfläche übersetzt alles selbst. Der Rust-Hintergrund liefert statt deutscher Sätze `Msg { key, params }` und bindet dieselben Kataloge nur für die Protokolldatei ein. Ein neues Modul `storage.rs` legt alle Pfade fest (`data\` neben der Exe, sonst Nur-Lesen-Modus). Das Hauptfenster wird im Code gebaut, damit die WebView2-Daten in den Arbeitsordner im Temp gehen.

**Tech Stack:** wie bisher: Tauri 2, Rust, React 19, TypeScript, HeroUI v3, Tailwind v4, Vitest. Keine neue i18n-Bibliothek.

**Spec:** `docs/superpowers/specs/2026-10-17-zweisprachig-portabel-design.md` (baut auf `docs/superpowers/specs/2026-10-16-jpeg-xl-konverter-design.md` auf). **Anhang mit allen heutigen Textstellen:** `docs/superpowers/plans/2026-10-17-zweisprachig-textinventur.md`. **Oberflächenvorlage:** `docs/design/jpeg-xl-konverter.dc.html` (Klappfenster `prefsOpen`/`prefRows`, Textliste `I18N`).

## Global Constraints

- Alles aus der Spezifikation vom 2026-10-16 gilt weiter, soweit die neue Spezifikation nichts anderes sagt. **Die Sicherheitslogik (Prüfung, Papierkorb-Regeln, Status) ändert sich nicht, nur Texte.**
- Genau zwei Sprachen: `de`, `en`. Einzige Textquelle: `src/i18n/de.json` und `src/i18n/en.json`, flache Schlüssel mit Punkten (z. B. `"note.keptNoRecycleBin"`), Platzhalter `{name}`, identische Schlüsselmengen.
- Kein Bildschirm mischt Sprachen. Hinweise aus dem Hintergrund sind `Msg { key, params }` und werden in der aktuellen Sprache angezeigt.
- Programmname nach außen „JPEG XL Converter“; Fenstertitel DE „JPEG XL Konverter“, EN „JPEG XL Converter“. Tauri-`identifier` bleibt `de.bassboost1981.jpegxlkonverter`. Cargo-Paket- und Lib-Name bleiben.
- Zahlen: DE „6,80 MB“, „9.812“; EN „6.80 MB“, „9,812“; Distanzwerte in beiden Sprachen mit Punkt („d 1.2“); Minus U+2212.
- Statuswörter EN: Queued, Running (`N %`), Done, Done with note, Skipped, Error, Check failed.
- Abweichungen der ZIP gegenüber Beschlüssen (Spezifikation 6.2-Tabelle): Ablagehinweis ohne EXR, Papierkorb-Untertitel „Nur bei verlustfreiem Ergebnis“ / „Only if the result is lossless“, „Fehlgeschlagene erneut versuchen“ / „Retry failed“, „Alles erledigt“ / „All done“, Singularformen.
- Speicherorte: nur `data\` neben der Exe (`settings.json`, `window.json`, `logs\`) und `%TEMP%\jpeg-xl-konverter\` (Arbeitsordner `<pid>\` inkl. `webview\`; im Nur-Lesen-Modus `logs\`). **Nichts unter `%APPDATA%` oder `%LOCALAPPDATA%`.** Keine Netzwerkzugriffe.
- Berechtigungen: `core:window:allow-set-title` hinzufügen, `window-state:default` entfernen. **`core:window:allow-destroy` nie entfernen** (sonst lässt sich das Fenster nicht schließen).
- Arbeit auf Branch `feature/zweisprachig`; nie pushen ohne ausdrückliches OK des Nutzers.
- Commit-Nachrichten enden mit der Co-Authored-By-Zeile, die die eigene Umgebung vorgibt.

## Review Focus

1. **Windows-Sprachkennungen in allen Schreibweisen** (`de-CH`, `DE`, `de_AT`, `""`, `null`) → `de*` (ohne Groß/Klein, auch mit `_`) wird Deutsch, alles andere Englisch. Test in Task 1.
2. **Fremde oder alte `data\settings.json`** (ungültiger `lang`-Wert, fehlendes Feld, kaputtes JSON) → gültige Felder bleiben, `lang` wird `null` (Windows-Sprache), keine Fehlermeldung. Tests in Task 5 (Rust laden) und Task 7 (`restoreSettings`).
3. **USB-Stick während der Sitzung abgezogen** (Datenordner verschwindet) → Speichern der Einstellungen scheitert mit übersetzter Meldung im Hinweisbereich, die App läuft weiter. Tests in Task 5 (Speichern in nicht mehr vorhandenen Ordner ergibt `Err(Msg)`) und Task 7 (Store zeigt `notice`).
4. **„Protokoll öffnen“ im Nur-Lesen-Modus** öffnet die Datei aus `%TEMP%\jpeg-xl-konverter\logs\`; Pfade außerhalb bleiben gesperrt. Test in Task 5.
5. **Englische Texte passen ins Layout** bei 1200 × 760 (rechtes Feld 360 px, Titelleiste, Statusspalte 110 px, Klappfenster 300 px). Sichtprüfung beider Sprachen in Task 9, Screenshots in Task 10.

---

## Gemeinsame Schnittstellen

Diese Namen gelten in allen Tasks.

```rust
// src-tauri/src/engine/types.rs
#[serde(rename_all = "lowercase")] pub enum Lang { De, En }
// src-tauri/src/engine/msg.rs
pub struct Msg { pub key: String, pub params: BTreeMap<String, MsgParam> }   // serde: params weggelassen, wenn leer
#[serde(untagged)] pub enum MsgParam { Text(String), Msg(Box<Msg>) }        // Msg für verschachtelte Gründe
impl Msg { pub fn new(key: &str) -> Msg; pub fn with(self, name: &str, value: impl Into<String>) -> Msg; pub fn with_msg(self, name: &str, inner: Msg) -> Msg }
// Schlüssel als Konstanten, erzeugt mit einem Makro, das zugleich `pub const ALL: &[&str]` füllt:
pub mod keys { pub const TARGET_EXISTS: &str = "error.targetExists"; /* … */ }
```

```ts
// src/i18n/index.ts
export type Lang = "de" | "en";
export type Key = keyof typeof import("./de.json");
export interface Msg { key: string; params?: Record<string, string | Msg> }
export function translate(lang: Lang, key: Key | string, params?: Record<string, string | number | Msg>): string;
export function msgText(lang: Lang, msg: Msg): string;
export function langFromLocale(tag: string | null | undefined): Lang;
```

Geänderte Serde-Verträge (Rust ↔ `src/api/types.ts`): `Outcome.note` → `notes: Vec<Msg>`, `ItemEvent.note` → `notes: Vec<Msg>`, `ToolStatus.message` → `messages: Vec<Msg>`, Befehlsfehler `Err(String)` → `Err(Msg)`. Neu: `StorageInfo { readOnly: bool }`.

---

### Task 1: Kataloge und Übersetzungskern (TypeScript)

**Files:**
- Create: `src/i18n/de.json`, `src/i18n/en.json`, `src/i18n/index.ts`
- Test: `src/i18n/index.test.ts`
- Modify: `tsconfig.json` (nur falls `resolveJsonModule` fehlt)

**Interfaces:**
- Produces: die Kataloge mit **allen** Schlüsseln, die spätere Tasks brauchen (Oberfläche, Hinweise, Fehler, Statuswörter, Protokoll), sowie `Lang`, `Key`, `Msg`, `translate`, `msgText`, `langFromLocale` (siehe „Gemeinsame Schnittstellen“).

Schlüssel-Gliederung: `app.*` (Titel), `tab.*`, `prefs.*` (Klappfenster), `queue.*`, `col.*`, `status.*`, `settings.*`, `preset.<ll|hi|bal|sm>.label/.sub`, `format.<png|jpg|webp|tiff>.sub`, `compare.*`, `footer.*`, `cta.*` (mit `.one`/`.other`), `notice.*`, `dialog.*`, `banner.*`, `note.*`, `error.*`, `reason.*` (Papierkorb-Gründe), `tool.*`, `preview.*`, `log.*`. Deutsche Werte = heutige Texte laut Anhang A und B (wörtlich); englische Werte = `I18N.en` der ZIP, Abweichungen laut Global Constraints, Rest im Ton der ZIP. Für jede Konstante/jeden Text aus Anhang A (außer „nie sichtbar“) und B genau einen Schlüssel anlegen; doppelte Texte teilen sich einen Schlüssel. Dazu die neuen Texte aus der Spezifikation (Klappfenster, Nur-Lesen-Hinweis, Dialogtitel, Fenstertitel, englischer Protokollkopf).

**Diese Schlüssel benutzen spätere Tasks namentlich, sie müssen genau so heißen:** `app.title` (DE „JPEG XL Konverter“, EN „JPEG XL Converter“) · `prefs.title`, `prefs.appearance`, `prefs.mode`, `prefs.language`, `prefs.light`, `prefs.dark`, `prefs.normal`, `prefs.expert` · `note.keptNoRecycleBin`, `note.keptTooLarge`, `note.keptRecycleFailed` (Platzhalter `{reason}`) · `reason.readOnly`, `reason.accessDenied`, `reason.inUse`, `reason.cancelled`, `reason.notFound`, `reason.internal`, `reason.windowsError` (`{code}`) · `error.targetExists`, `error.saveSettings`, `error.internal` · `tool.wontStart`, `tool.noResponse`, `tool.noVersion` (je `{name}`) · `banner.toolsMissing`, `banner.readOnly` · `dialog.pickFiles`, `dialog.pickFolder`, `dialog.filterImages` · `notice.rejected.one`, `notice.rejected.other` (`{n}`) · `footer.noSpace` (`{drive}`) · `log.title` (DE „JPEG XL Konverter – Protokoll“, EN „JPEG XL Converter – Log“). Alle übrigen Namen wählt der Umsetzer nach der Gliederung oben.

- [ ] **Step 1: Fehlschlagende Tests** in `src/i18n/index.test.ts`:

```ts
it("catalogs have identical keys and no empty values", () => {
  expect(Object.keys(en).sort()).toEqual(Object.keys(de).sort());
  for (const v of [...Object.values(de), ...Object.values(en)]) expect(v.trim()).not.toBe("");
});
it("placeholders match between languages", () => { /* je Schlüssel gleiche Menge {…}-Namen in de und en */ });
it("translate substitutes params once", () => {
  expect(translate("de", "note.keptNoRecycleBin")).toBe("Original behalten: kein Papierkorb auf diesem Laufwerk");
  expect(translate("en", "error.targetExists")).toBe("Target file already exists");
  expect(translate("en", "footer.noSpace", { drive: "D:" })).toBe("No space left on drive D:");
});
it("nested Msg params are translated", () => {
  const m = { key: "note.keptRecycleFailed", params: { reason: { key: "reason.readOnly" } } };
  expect(msgText("de", m)).toBe("Original behalten: Papierkorb nicht möglich (Datei ist schreibgeschützt)");
});
it("unknown key returns the key, missing param keeps the placeholder", () => {
  expect(translate("en", "does.not.exist")).toBe("does.not.exist");
  expect(translate("en", "footer.noSpace")).toContain("{drive}");
});
it.each([["de-DE","de"],["de","de"],["DE","de"],["de_AT","de"],["de-CH","de"],["en-US","en"],["fr-FR","en"],["","en"],[null,"en"],[undefined,"en"]])(
  "langFromLocale(%s) = %s", (tag, lang) => expect(langFromLocale(tag as string | null | undefined)).toBe(lang));
```
  (`footer.noSpace` DE: „Kein Platz mehr auf Laufwerk {drive}“.)
- [ ] **Step 2:** `npx vitest run src/i18n` → FAIL.
- [ ] **Step 3: Umsetzen.** `translate`: einmalige Ersetzung `{name}` → Wert (Zahlen als `String(n)`, verschachtelte `Msg` über `msgText`), keine erneute Ersetzung im eingesetzten Text. `Key` wird aus `de.json` abgeleitet; `en.json` wird gegen `Record<Key, string>` typgeprüft.
- [ ] **Step 4:** PASS; `npm run build` ohne Fehler. **Step 5: Commit** `feat(i18n): de/en catalogs and translation core`.

### Task 2: Rust-Grundlage für Meldungen

**Files:**
- Create: `src-tauri/src/engine/msg.rs`, `src-tauri/src/engine/i18n.rs`
- Modify: `src-tauri/src/engine/types.rs` (`Lang`), `src-tauri/src/engine/mod.rs`
- Test: Unit-Tests in `msg.rs`, `i18n.rs`

**Interfaces:**
- Consumes: Kataloge aus Task 1.
- Produces: `Lang`, `Msg`, `MsgParam`, `keys::*` + `keys::ALL`, `i18n::text(lang: Lang, msg: &Msg) -> String`, `i18n::key(lang: Lang, key: &str) -> String` (fehlender Schlüssel → der Schlüssel selbst).

- [ ] **Step 1: Fehlschlagende Tests:**
  - `every_key_has_text_in_both_languages`: für jedes `k` in `keys::ALL`: `i18n::key(Lang::De, k) != k` und dasselbe für `En`.
  - `msg_serializes_compact`: `Msg::new("error.targetExists")` → `{"key":"error.targetExists"}`; mit `.with("drive","D:")` → `{"key":"…","params":{"drive":"D:"}}`; verschachtelt → `{"params":{"reason":{"key":"reason.readOnly"}}}`; Rundreise über `serde_json`.
  - `text_substitutes_and_nests` (gleiche Fälle wie Task 1, DE und EN).
  - `lang_serde`: `Lang::En` ↔ `"en"`.
- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml engine::msg engine::i18n` → FAIL. **Step 3:** umsetzen; Kataloge per `include_str!("../../../src/i18n/de.json")` (bzw. `en.json`), einmal geparst (`OnceLock`). **Step 4:** PASS. **Step 5: Commit** `feat(i18n): Msg type, key constants and Rust catalog lookup`.

### Task 3: Hintergrund-Meldungen als Kennungen

**Files:**
- Modify: `src-tauri/src/engine/{types,pipeline_util,encode,decode,convert_io,originals,preview,tools,queue,report}.rs`, `src-tauri/src/commands.rs`
- Test: alle bestehenden Rust-Tests; Anpassung der 51 Stellen aus Anhang A (außer `verify.rs`)

**Interfaces:**
- Consumes: Task 2.
- Produces: `Outcome.notes: Vec<Msg>`, `ItemEvent.notes: Vec<Msg>`, `ToolStatus.messages: Vec<Msg>`; `Stop { Skipped(Msg), Error(Msg), VerifyFailed(Msg), Cancelled }`; `finished(target, notes: Vec<Msg>)`; `convert_io`-Funktionen und `preview` `Result<_, Msg>`; `RecycleBin::recycle -> Result<(), Msg>`; `handle_original -> Option<Msg>` (Papierkorb-Fehler: `note.keptRecycleFailed` mit verschachteltem `reason`; Windows-Code als `reason.windowsError` mit `code` = `"0x80270058"`); `Queue::clear -> Result<(), Msg>`; alle Tauri-Befehle `Result<_, Msg>`; `RunReport.problems: Vec<(ItemStatus, PathBuf, Vec<Msg>)>`. `FakeRecycleBin` mit `fail: true` liefert `Msg::new("reason.internal")` (statt „Testfehler“).

Regeln:
- Jede Textstelle aus Anhang A wird zu genau dem Schlüssel aus Task 1; **die Bedingungen, wann ein Hinweis entsteht, bleiben unverändert**, ebenso die Reihenfolge der Hinweise.
- `io::Error`-Texte von Windows/Opener landen nicht mehr in Meldungen (Spezifikation 4: keine Sprachmischung); stattdessen der passende Schlüssel (z. B. `tool.wontStart` mit `name`).
- Die Protokolldatei bleibt in diesem Task **deutsch** (Task 4 macht sie sprachabhängig): `report.rs` setzt die Gründe vorläufig mit `i18n::text(Lang::De, …)` zusammen, verbunden mit `" · "`.

- [ ] **Step 1: Tests umstellen** (zuerst, sie schlagen dann fehl): Wo heute ein deutscher Text geprüft wird, wird der Schlüssel geprüft, z. B. `assert_eq!(keys_of(&o.notes), [keys::KEPT_NO_RECYCLE_BIN])` mit einer kleinen Testhilfe `keys_of(&[Msg]) -> Vec<&str>` in `tests/common/mod.rs`. `originals.rs`-Gründe: `reason.readOnly`, `reason.accessDenied`, `reason.inUse`, `reason.cancelled`, `reason.windowsError{code:"0x80270058"}`. Protokolltests in `report.rs`/`queue.rs` behalten ihre deutschen Erwartungen (Task 4 ergänzt Englisch). Neu: `types.rs`-JSON-Formtest prüft `"notes":[{"key":…}]` und `"messages"`.
- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml` → kompiliert nicht bzw. FAIL.
- [ ] **Step 3:** umsetzen. **Step 4:** alle Rust-Tests PASS, `cargo clippy --manifest-path src-tauri/Cargo.toml` ohne Warnungen, und `rg -n "[äöüÄÖÜß]" src-tauri/src` zeigt nur noch Kommentare, Arbeitsdateinamen, `verify.rs`/`jxl_container.rs`-Interna und Testdaten. **Step 5: Commit** `refactor(engine): user-visible messages as Msg keys`.

### Task 4: Protokolldatei in der Sprache des Durchlaufs

**Files:**
- Modify: `src-tauri/src/engine/{report,queue}.rs`, `src-tauri/src/commands.rs`
- Test: Unit-Tests in `report.rs`, `queue.rs`

**Interfaces:**
- Consumes: Task 2/3.
- Produces: `Queue::start(&self, mode: Mode, s: JobSettings, lang: Lang)`, `Queue::retry_failed(&self, mode: Mode, s: JobSettings, lang: Lang)`, `RunReport::new(mode, &JobSettings, lang)`; Tauri `start_run(mode, settings, lang)`, `retry_failed(mode, settings, lang)` (JS-Argument `lang: "de" | "en"`).

- [ ] **Step 1: Fehlschlagende Tests:**
  - `log_in_english`: derselbe Aufbau wie der bestehende deutsche Protokolltest, mit `Lang::En`. Erwartet: erste Zeile `JPEG XL Converter – Log`; die Statuswörter `Error`, `Skipped`, `Check failed`, `Done with note` am Zeilenanfang der Problemfälle; die Gründe sind `i18n::text(Lang::En, …)` der jeweiligen `Msg` (im Test über `i18n` berechnet, nicht als Literal); keine Zeile enthält Umlaute.
  - `log_in_german_unchanged`: die bisherigen deutschen Erwartungen bestehen weiter.
  - `retry_keeps_new_lang`: `retry_failed(.., Lang::En)` schreibt Englisch, auch wenn der erste Lauf Deutsch war.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen; alle festen Protokolltexte über `i18n::key(lang, "log.…")`; Gründe = übersetzte `Msg`s mit `" · "`. **Step 4:** PASS. **Step 5: Commit** `feat(log): write the run log in the run's language`.

### Task 5: Speicherorte (USB-portabel, Nur-Lesen-Modus) und Systemsprache

**Files:**
- Create: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/{lib,commands,settings_store}.rs`
- Test: Unit-Tests in `storage.rs`, `settings_store.rs`, `commands.rs` (Pfadprüfung als reine Funktion)

**Interfaces:**
- Produces:
  - `pub struct Storage { pub data_dir: PathBuf, pub read_only: bool, pub logs_dir: PathBuf, pub work_dir: PathBuf, pub webview_dir: PathBuf }`
  - `pub fn resolve(exe_dir: &Path, temp_dir: &Path, pid: u32, writable: &dyn Fn(&Path) -> bool) -> Storage`: `data_dir = exe_dir/data`; beschreibbar → `logs_dir = data_dir/logs`; sonst `read_only = true`, `logs_dir = temp_dir/jpeg-xl-konverter/logs`; immer `work_dir = temp_dir/jpeg-xl-konverter/<pid>`, `webview_dir = work_dir/webview`.
  - `pub fn probe_writable(dir: &Path) -> bool` (Ordner anlegen falls nötig, Testdatei schreiben und löschen).
  - `settings_store::load(dir)` unverändert; `settings_store::save(storage: &Storage, value) -> Result<(), Msg>`: im Nur-Lesen-Modus `Ok(())` ohne Schreiben; Fehler → `Msg::new("error.saveSettings")`.
  - `fn log_path_allowed(logs_dir: &Path, file: &Path) -> bool` (kanonisch verglichen) für `open_log`.
  - Tauri-Befehle `storage_info() -> StorageInfo { read_only: bool }` und `system_locale() -> Option<String>` (Windows-Anzeigesprache als BCP-47, z. B. `"de-DE"`; API-Wahl dem Umsetzer überlassen, z. B. `GetUserDefaultUILanguage` + `LCIDToLocaleName`).
  - `AppState` bekommt `storage: Storage` statt `config_dir`/`log_dir`.

- [ ] **Step 1: Fehlschlagende Tests:**
  - `resolve_portable` (writable → true): `data_dir == exe/data`, `logs_dir == exe/data/logs`, `!read_only`, `work_dir == temp/jpeg-xl-konverter/4711`, `webview_dir == work_dir/webview`.
  - `resolve_read_only` (writable → false): `read_only`, `logs_dir == temp/jpeg-xl-konverter/logs`, `data_dir` weiterhin `exe/data`.
  - `probe_writable_true_for_temp_dir`, `probe_writable_false_when_data_is_a_file` (eine Datei namens `data` blockiert das Anlegen).
  - `save_read_only_is_noop`: nichts wird geschrieben, `Ok(())`.
  - `save_into_vanished_dir_errors` (Review Focus 3): Datenordner liegt in einem gelöschten Temp-Ordner, dessen Elternteil entfernt wurde → `Err(Msg)` mit `key == "error.saveSettings"`.
  - `load_corrupt_or_foreign` (Review Focus 2): kaputtes JSON → `None`; `{"lang":"fr","preset":"bal"}` → wird geladen (Validierung macht die Oberfläche).
  - `log_path_allowed_*` (Review Focus 4): Datei im Protokollordner → true; `..\settings.json` → false; Datei im Temp-Protokollordner bei `read_only` → true.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen; `lib.rs` nutzt `storage::resolve(exe_dir, env::temp_dir(), process::id(), &probe_writable)`; die alte `log_dir()`-Funktion und `app_config_dir()` entfallen. **Step 4:** PASS. **Step 5: Commit** `feat(storage): portable data folder next to the exe and read-only mode`.

### Task 6: Fenster im Code, Fensterzustand, Programmname

**Files:**
- Create: `src-tauri/src/window_state.rs`
- Modify: `src-tauri/src/lib.rs`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/Cargo.toml` (Plugin entfernen), `package.json` (JS-Paket des Plugins entfernen, falls vorhanden), `index.html`, `scripts/package-portable.ps1`, `.gitignore` (nur falls `data/` im Repo-Wurzelordner beim Entwickeln entsteht)
- Test: Unit-Tests in `window_state.rs`

**Interfaces:**
- Consumes: `Storage` (Task 5).
- Produces:
  - `pub struct WindowState { pub x: i32, pub y: i32, pub width: u32, pub height: u32, pub maximized: bool }` (serde camelCase), `pub fn load(path: &Path) -> Option<WindowState>`, `pub fn save(path: &Path, s: &WindowState) -> io::Result<()>`, `pub fn visible_on(s: &WindowState, monitors: &[(i32, i32, u32, u32)]) -> bool` (mindestens 100 × 100 px der Titelleiste auf einem Monitor).
  - Hauptfenster `"main"` wird im `setup` mit `WebviewWindowBuilder` gebaut: Titel `"JPEG XL Converter"`, 1200 × 760, min 1000 × 640, `decorations(false)`, Drag & Drop an, `data_directory(storage.webview_dir)`; gespeicherter Zustand aus `data\window.json` wird angewendet, wenn `visible_on`; sonst zentriert. Gespeichert wird bei `WindowEvent::CloseRequested` (nicht im Nur-Lesen-Modus).
  - Beim Beenden: `cancel_and_wait(3 s)`, dann Arbeitsordner löschen mit bis zu 10 Versuchen im Abstand von 200 ms (WebView2 gibt Dateien verzögert frei).

Konfiguration:
- `tauri.conf.json`: `productName` „JPEG XL Converter“, `app.windows` leer (Fenster kommt aus dem Code), CSP unverändert.
- `capabilities/default.json`: `core:window:allow-set-title` dazu, `window-state:default` weg, alles andere unverändert (**`core:window:allow-destroy` bleibt**). `"windows": ["main"]` bleibt gültig.
- `index.html`: `<title>JPEG XL Converter</title>`, `lang="en"` (die Oberfläche setzt `lang` zur Laufzeit, Task 8).
- `scripts/package-portable.ps1`: Ziel `dist-portable\JPEG XL Converter\JPEG XL Converter.exe`, kein `data\`-Ordner im Paket.

- [ ] **Step 1: Fehlschlagende Tests:** `load_missing_gives_none`, `save_load_roundtrip`, `load_corrupt_gives_none`, `visible_on_primary` (true), `visible_on_detached_monitor` (Position nur auf einem nicht mehr vorhandenen zweiten Monitor → false).
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen. **Step 4:** Tests PASS; `cargo build` ohne Warnungen.
- [ ] **Step 5: Handprüfung** (Debug-Build `npx tauri build --debug --no-bundle`, Exe in einen Temp-Ordner kopieren samt `cjxl.exe`/`djxl.exe`): starten → `data\settings.json` erscheint erst nach einer Einstellungsänderung, `data\window.json` nach dem Schließen; Fenster per ✕ und per WM_CLOSE schließbar (Exit-Code 0); danach existiert `%TEMP%\jpeg-xl-konverter\<pid>` nicht mehr; unter `%APPDATA%` und `%LOCALAPPDATA%` ist kein neuer Ordner `de.bassboost1981.jpegxlkonverter` bzw. `JPEG XL Converter` entstanden (Zeitstempel vorher/nachher vergleichen). Ergebnis in den Bericht.
- [ ] **Step 6: Commit** `feat(app): build window in code, portable window state, rename to JPEG XL Converter`.

### Task 7: Oberflächen-Datenschicht

**Files:**
- Modify: `src/api/{types,backend,tauriBackend,mockBackend}.ts`, `src/state/{settings,store}.ts`, `src/lib/format.ts`
- Create: `src/i18n/useT.ts`
- Test: `src/state/settings.test.ts`, `src/state/store.test.ts`, `src/lib/format.test.ts`, `src/api/mockBackend.test.ts`

**Interfaces:**
- Consumes: Task 1; Rust-Verträge aus Task 3–5.
- Produces:
  - `types.ts`: `ItemEvent.notes: Msg[]`, `ToolStatus.messages: Msg[]`, `StorageInfo { readOnly: boolean }`, Re-Export `Lang`, `Msg`.
  - `Backend`: `startRun(mode, s, lang: Lang)`, `retryFailed(mode, s, lang: Lang)`, `pickFiles(mode, labels: { title: string; filterImages: string }): Promise<string[]>`, `pickFolder(title: string): Promise<string | null>`, `systemLocale(): Promise<string | null>`, `storageInfo(): Promise<StorageInfo>`, `setTitle(title: string): void`. Fehler, die das Backend wirft, sind `Msg`-Objekte.
  - `UiSettings.lang: Lang | null` (Standard `null`); `restoreSettings` validiert `lang` (`"de" | "en" | null`, sonst `null`).
  - `PRESETS` verliert `label` (Texte kommen aus `preset.*`).
  - Store: `systemLang: Lang`, `readOnly: boolean`, Selektor `selectLang(state): Lang` = `settings.lang ?? systemLang`, `Row.notes: Msg[]`, `notice: Msg | null` (abgelehnte Dateien: `notice.rejected.one`/`.other` mit `n`; unbekannte Fehler: `error.internal`); `init()` lädt zusätzlich `systemLocale()` → `langFromLocale` und `storageInfo()`; `start/retryFailed` übergeben `selectLang`.
  - `format.ts`: alle Funktionen bekommen `lang: Lang` als **letzten** Parameter: `formatSize(bytes, lang)`, `formatCount(n, lang)`, `formatDelta(inB, outB)` (sprachneutral), `formatDistance(d)` (Punkt), `summaryLine(s, lang)`, `footerHint(p, lang)`, `ctaLabel(p, lang)`. Neu `ctaState(p): "start" | "pause" | "resume" | "retry" | "allDone" | "empty"` (Logik ohne Text; ersetzt den Textvergleich in `RunFooter`).
  - `useT(): (key: Key, params?) => string` und `useLang(): Lang`.

- [ ] **Step 1: Fehlschlagende Tests:**

```ts
expect(formatSize(7_130_317, "en")).toBe("6.80 MB");
expect(formatSize(7_130_317, "de")).toBe("6,80 MB");
expect(formatCount(9812, "en")).toBe("9,812");
expect(summaryLine({ done: 9812, skipped: 14, errors: 2, bytesIn: 44_238_131_200, bytesOut: 35_433_480_192 }, "en"))
  .toBe("9,812 done · 14 skipped · 2 errors · 41.2 GB → 33.0 GB");
expect(ctaLabel({ mode: "enc", run: "idle", waiting: 1, failed: 0 }, "en")).toBe("Convert 1 file to JPEG XL");
expect(ctaLabel({ mode: "dec", run: "idle", waiting: 3, failed: 0 }, "en")).toBe("Convert 3 files back");
expect(ctaLabel({ mode: "enc", run: "finished", waiting: 0, failed: 2 }, "en")).toBe("Retry failed");
expect(ctaState({ mode: "enc", run: "finished", waiting: 0, failed: 0 })).toBe("allDone");
// alle bisherigen deutschen Erwartungen bleiben, mit "de" als letztem Argument
expect(restoreSettings({ lang: "fr" }).lang).toBeNull();            // Review Focus 2
expect(restoreSettings({ lang: "en", preset: "xx" })).toMatchObject({ lang: "en", preset: "ll" });
// store: systemLocale "de-AT" und settings.lang null → selectLang "de"; settings.lang "en" → "en"
// store: saveSettings wirft {key:"error.saveSettings"} → notice.key === "error.saveSettings", App läuft weiter (Review Focus 3)
// store: addPaths mit rejected 1 → notice {key:"notice.rejected.one", params:{n:"1"}}
// store: startRun wird mit lang "en" aufgerufen, wenn settings.lang "en"
```
- [ ] **Step 2:** `npm test` → FAIL. **Step 3:** umsetzen (Mock-Backend: `notes` als `Msg`, `systemLocale` liefert `navigator.language`, `storageInfo` `{ readOnly: false }`, `setTitle` setzt `document.title`). **Step 4:** PASS, `npm run build` sauber. **Step 5: Commit** `feat(ui): language-aware data layer, formats and backend contract`.

### Task 8: Titelleiste und Klappfenster „Programm-Einstellungen“

**Files:**
- Create: `src/components/AppSettingsPopover.tsx`
- Modify: `src/components/TitleBar.tsx`, `src/App.tsx`
- Test: `src/components/TitleBar.test.tsx`, `src/components/AppSettingsPopover.test.tsx`

**Interfaces:**
- Consumes: `useT`, `useLang`, Store (`settings.theme`, `settings.expert`, `settings.lang`, `run`, `updateSettings`, `enterExpert`), `backend.setTitle`.

Vorlage: ZIP-Block `prefsOpen`/`prefRows` (Maße und Farben siehe Spezifikation 5). Der Knopf mit Schieberegler-Symbol ersetzt die Schalter „Hell/Dunkel“ und „Experte“; `aria-label` und Tooltip `prefs.title`. Das Klappfenster ist ein HeroUI-Popover (oder ein gleichwertiges eigenes Element mit Fokusführung), rechtsbündig unter dem Knopf, ohne Abdunkeln; Zeilen: `prefs.appearance` (Hell/Dunkel), `prefs.mode` (Normal/Experte, bei `run` „running“/„paused“ gesperrt), `prefs.language` („Deutsch“/„English“ in beiden Sprachen gleich). `App` setzt bei Sprachwechsel `backend.setTitle(translate(lang, "app.title"))` und `document.documentElement.lang`.

- [ ] **Step 1: Fehlschlagende Tests:**
  - Titelleiste hat keinen Schalter „Hell“/„Experte“ mehr, aber einen Knopf mit Namen „Programm-Einstellungen“ (DE) bzw. „App settings“ (EN).
  - Klick öffnet das Klappfenster mit drei Zeilen; Klick außerhalb schließt; `Escape` schließt; erneuter Klick auf den Knopf schließt.
  - „English“ wählen → `settings.lang === "en"`, Überschrift wird „App settings“, `backend.setTitle` mit „JPEG XL Converter“, `document.documentElement.lang === "en"`.
  - „Dunkel“ → Klasse `dark` am Dokument; „Experte“ → `enterExpert` angewandt (Qualität der Stufe übernommen).
  - Bei `run === "running"` sind die Modus-Knöpfe deaktiviert, Sprache und Erscheinungsbild nicht.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen. **Step 4:** PASS. **Step 5: Commit** `feat(ui): app settings popover replaces theme and expert switches`.

### Task 9: Alle Komponenten übersetzen

**Files:**
- Modify: `src/App.tsx`, `src/components/{ToolsBanner,QueuePanel,QueueRow,SettingsPanel,EncodeSettings,DecodeSettings,CommonSettings,CompareSlider,RunFooter}.tsx`
- Test: die bestehenden Komponententests (Texte über `translate("de", …)` statt fester Zeichenketten, wo sinnvoll) plus englische Fälle

**Interfaces:**
- Consumes: Task 7 (`useT`, `useLang`, `format.ts` mit `lang`, `ctaState`, `Msg`).

Regeln:
- Jeder Text aus Anhang B kommt aus dem Katalog; keine deutschen Zeichenketten mehr in `src/components`, `src/App.tsx`, `src/state` (außer Tests).
- Hinweise/Fehler am Eintrag: `row.notes.map(m => msgText(lang, m)).join(" · ")`.
- `ToolsBanner`: zeigt `tools.messages` übersetzt, sonst `banner.toolsMissing`. Zusätzlich im Nur-Lesen-Modus `banner.readOnly` (einmal pro Start, Text aus Spezifikation 7.2).
- `RunFooter`: deaktiviert über `ctaState(...) === "allDone" | "empty"`, Farbe über den Zahlenwert der Ersparnis, nicht über Text.
- Datei-/Ordnerdialog mit Titeln `dialog.pickFiles`/`dialog.pickFolder` und Filtername `dialog.filterImages`.
- `notice` (Store) wird mit `msgText` angezeigt.

- [ ] **Step 1: Fehlschlagende Tests (englisch):**
  - Warteschlange: „Queue“, „Add folder“, „Clear“, Spalten „File/Original/Result/Savings/Status“, Ablagehinweis „Drop images or folders here“ + „JPEG, PNG, GIF, TIFF, WebP, BMP“.
  - Statuschips: „Queued“, „Done“, „Done with note“, „Skipped“, „Error“, „Check failed“.
  - Einstellungen: „Settings“, „Encoding · to JPEG XL“, Stufen „Lossless/High quality/Balanced/Small“, „Losslessly transcode JPEGs“, „Keep metadata“, „Move originals to Recycle Bin after check“ + „Only if the result is lossless“, „Output format“, „Automatic“ + „Original JPEG, otherwise PNG“.
  - Fußbereich: „Savings appear after the first finished file“, „Convert 6 files to JPEG XL“, „Stop“, „All done“, „No space left on drive D:“, „Open log“.
  - Vorher/Nachher: „Before / After“, „No file selected“, „JXL · lossless“.
  - Rückfrage beim Schließen auf Englisch.
  - **Sprachwechsel übersetzt angezeigte Hinweise:** Zeile mit `notes:[{key:"note.keptNoRecycleBin"}]` zeigt im Tooltip den deutschen Text, nach Umschalten auf Englisch den englischen.
  - Kein Text-Vergleich mehr: `RunFooter` deaktiviert bei „All done“ ebenso wie bei „Alles erledigt“.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen. **Step 4:** `npm test` PASS, `npm run build` sauber; `rg -n "[äöüÄÖÜß]" src --glob '!*.test.*' --glob '!src/i18n/*'` liefert nichts außer Kommentaren.
- [ ] **Step 5: Sichtprüfung (Review Focus 5):** `npm run dev` im Browser bei 1200 × 760, beide Sprachen, hell: Warteschlange mit den Demo-Dateien, Expertenmodus, Modus „Aus JPEG XL“, laufender Durchlauf, Klappfenster offen. Kein Text läuft über oder bricht hässlich um (rechtes Feld 360 px, Statusspalte 110 px, Klappfenster 300 px). Notfalls Texte kürzen (Katalog) statt Layout ändern. Ergebnis mit Screenshots in den Bericht.
- [ ] **Step 6: Commit** `feat(ui): translate all components`.

### Task 10: GitHub und Unterlagen

**Files:**
- Create: `scripts/readme-screenshots.mjs`, `README.de.md`, `docs/ACCEPTANCE.md`, `docs/screenshots/app-en-light.png`, `app-en-dark.png`, `app-de-hell.png`, `app-de-dunkel.png`, `app-en-settings.png`, `app-de-einstellungen.png`
- Modify: `README.md` (wird Englisch), `docs/ABNAHME.md`
- Delete: `docs/screenshots/app-hell.png`, `docs/screenshots/app-dunkel.png`

Vorgaben:
- `scripts/readme-screenshots.mjs`: startet nichts selbst; erwartet den Vite-Dev-Server auf `http://localhost:1420` und Edge/Chrome mit `--remote-debugging-port`. Ablauf je Sprache/Thema: Viewport 1200 × 760, `deviceScaleFactor` 2, `prefers-color-scheme` setzen, Seite laden, Sprache über das Klappfenster wählen, „Add folder“/„Ordner hinzufügen“ klicken, Zeile `DSC_4022.jpg` wählen, Start klicken, ca. 2,6 s warten, PNG speichern. Für `*-settings`/`*-einstellungen` das Klappfenster offen lassen. Aufruf und Voraussetzungen als Kommentar im Kopf.
- `README.md` (Englisch) und `README.de.md` (Deutsch) sind inhaltlich gleich: Sprachumschalter oben („**English** | [Deutsch](README.de.md)“ bzw. „[English](README.md) | **Deutsch**“), Screenshot per `<picture>` (hell/dunkel), Abschnitt „Settings/Programm-Einstellungen“ mit dem Klappfenster-Bild, Abschnitte aus der heutigen README, ergänzt um: Programmname „JPEG XL Converter“, **USB-portabel** (`data\` neben der Exe, nichts im Benutzerprofil, Nur-Lesen-Modus), Sprachwahl, Hinweis auf alte Ordner der Vorversion (`%APPDATA%\de.bassboost1981.jpegxlkonverter`, `%LOCALAPPDATA%\JPEG XL Konverter`, `%LOCALAPPDATA%\de.bassboost1981.jpegxlkonverter`), die man von Hand löschen kann, WebView2-Hinweis. Links: README.md → `docs/ACCEPTANCE.md`, README.de.md → `docs/ABNAHME.md`.
- `docs/ACCEPTANCE.md` = englische Fassung von `docs/ABNAHME.md`; beide mit neuem Exe-Namen, `data\`-Ordner und dem USB-Schritt aus Spezifikation 9.

- [ ] **Step 1:** Screenshots erzeugen (Dev-Server und Edge starten, Skript laufen lassen, beide Prozesse wieder beenden, Edge-Profil im Temp löschen). Jedes Bild ansehen: richtige Sprache, richtiges Thema, Klappfenster wo gewünscht.
- [ ] **Step 2:** READMEs, ACCEPTANCE, ABNAHME schreiben; alle relativen Links und Bildpfade prüfen (`rg -n "\]\(|src=\"|srcset=\"" README*.md docs/*.md` und jede Datei existiert).
- [ ] **Step 3: Commit** `docs: English-first README with German version, screenshots per language, acceptance in both languages`.

### Task 11: Gesamtprüfung portabel

**Files:**
- keine Codeänderung (nur bei Befund: Rückmeldung als Fehler, kein eigenmächtiges Umbauen)

- [ ] **Step 1:** `cargo test --manifest-path src-tauri/Cargo.toml`, `npm test`, `npm run build` → alles grün, Ausgabe sauber.
- [ ] **Step 2:** `powershell -ExecutionPolicy Bypass -File scripts/package-portable.ps1` → `dist-portable\JPEG XL Converter\` mit `JPEG XL Converter.exe`, `cjxl.exe`, `djxl.exe`, `LICENSES\`, **ohne** `data\`.
- [ ] **Step 3:** Den Ordner auf ein anderes Laufwerk bzw. einen anderen Ordner kopieren (z. B. `%TEMP%\usb-test\`), starten, per Skript/UI-Automation die Sprache auf Englisch stellen (oder `data\settings.json` vor dem Start mit `{"lang":"en"}` anlegen und prüfen, dass der Fenstertitel „JPEG XL Converter“ ist), schließen, erneut starten: Einstellung bleibt. Prüfen: `data\settings.json`, `data\window.json` existieren; `%TEMP%\jpeg-xl-konverter\<pid>` ist nach dem Schließen weg; unter `%APPDATA%`/`%LOCALAPPDATA%` kein neuer oder geänderter App-Ordner (Zeitstempel vorher/nachher).
- [ ] **Step 4:** Nur-Lesen-Modus: Kopie mit einer **Datei** namens `data` neben der Exe starten → Hinweisbalken erscheint, Fenster schließt sauber, im Benutzerprofil entsteht nichts.
- [ ] **Step 5:** Ergebnisse (Befehle und Ausgaben) in den Bericht; Testordner löschen.

## Abschluss (Controller, nach der Gesamtprüfung und mit OK des Nutzers)

1. Branch hochladen, Pull Request gegen `main`, zusammenführen (Merge-Commit), Branch auf GitHub und lokal löschen. Auf GitHub bleibt nur `main`.
2. Repo-Beschreibung setzen (über Git-Bash, PowerShell 5.1 zerlegt den Text): „Windows app to convert images to JPEG XL and back – JPEGs losslessly transcoded and verified byte for byte; originals only go to the Recycle Bin after a passed check. Portable, fully offline.“
3. Portablen Ordner neu bauen (`scripts/package-portable.ps1`).
