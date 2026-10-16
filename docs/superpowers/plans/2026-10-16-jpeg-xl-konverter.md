# JPEG XL Konverter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eine Windows-App (Tauri 2), die Bilder per Drag & Drop nach JPEG XL umwandelt und zurück: geprüft, ohne Datenverlust, mit der Oberfläche aus dem HeroUI-Entwurf.

**Architecture:** Rust-Hintergrund (`src-tauri/src/engine/`) mit kleinen, einzeln getesteten Bausteinen, der die offiziellen `cjxl.exe`/`djxl.exe` (libjxl v0.12.0) als Kindprozesse aufruft. Die React-Oberfläche (`src/`) spricht nur über die Schnittstelle `Backend` (`src/api/backend.ts`) mit dem Hintergrund. Es gibt eine Tauri-Umsetzung und eine simulierte Umsetzung für Browser und Tests.

**Tech Stack:** Tauri 2, Rust (stable, MSVC), React 19, TypeScript, Vite, Tailwind CSS v4, HeroUI v3 (`@heroui/react` ≥ 3.2.6, `@heroui/styles`), `@fontsource-variable/lexend`, zustand, `@tanstack/react-virtual`, Vitest + Testing Library. Rust-Crates: `image`, `png`, `miniz_oxide`, `webp`, `imagesize`, `jxl-oxide`, `walkdir`, `windows`, `chrono`, `serde`, `serde_json`, `base64`; Testhilfen: `tempfile`, `jpeg-encoder`.

**Spec:** `docs/superpowers/specs/2026-10-16-jpeg-xl-konverter-design.md`. Die Oberflächenvorlage ist `JPEG XL Konverter UI.zip` → `JPEG XL Konverter.dc.html`, Variante „2a HeroUI · Hell / Dunkel“.

## Global Constraints

- Nur Windows 11 x64. Shell-Befehle in diesem Plan sind PowerShell, ausgeführt im Projektordner `E:\Vibe-Coding\JPEG XL`.
- Produktname „JPEG XL Konverter“, Identifier `de.bassboost1981.jpegxlkonverter`, Oberflächensprache Deutsch.
- `cjxl.exe`/`djxl.exe` stammen aus libjxl **v0.12.0**, Datei `jxl-x64-windows-static.zip` der GitHub-Release, und liegen als `src-tauri/binaries/cjxl-x86_64-pc-windows-msvc.exe` bzw. `djxl-…` vor. Sie werden **nie eingecheckt**.
- Lexend ist Standardschrift der ganzen App und lokal eingebunden. Zur Laufzeit gibt es **keine Netzwerkzugriffe**.
- Akzentfarbe `#006FEE`. Fenster 1200 × 760, Mindestgröße 1000 × 640, ohne Windows-Rahmen (eigene Titelleiste).
- Das Programm löscht **niemals** Nutzerdateien endgültig. Gelöscht werden nur eigene Zwischendateien (`*.tmp` und Dateien im Arbeitsordner).
- Bestehende Zieldateien werden **nie überschrieben** → Status „Übersprungen“.
- Der Papierkorb-Schalter startet bei **jedem** Programmstart aus.
- Qualitätsstufen: Verlustfrei d 0 / e 7 · Hohe Qualität d 0.5 / e 7 · Ausgewogen d 1.2 / e 7 · Klein d 2.6 / e 8. Experte: `d = 0,1 + (100 − q) · 0,09`, q = 100 → d 0.
- Zurückwandeln: JPEG-Qualität Standard 92, WebP-Ausgabe immer Qualität 90.
- Protokolle: `%LOCALAPPDATA%\JPEG XL Konverter\Protokolle\JJJJ-MM-TT_HH-MM-SS.txt`.
- Alle Texte der Oberfläche folgen der Spezifikation (Abschnitte 3.3, 3.4, 6). Wo die Spezifikation schweigt, gilt der Entwurf.

## Review Focus

1. **Gleicher Name, andere Endung im selben Ordner** (`foto.jpg` + `foto.png` → beide wollen `foto.jxl`), gleichzeitig von zwei Workern verarbeitet: Erwartet wird, dass genau eine Datei umgewandelt wird, die andere ist „Übersprungen“, und es gibt keine kaputte `.tmp`. Test in Task 10.
2. **Datei verschwindet oder ist gesperrt** zwischen Hinzufügen und Verarbeiten (gelöscht, von anderem Programm geöffnet): Erwartet wird „Fehler“ nur für diese Datei. Test in Task 8.
3. **Großgeschriebene Endungen** (`.JPG`, `.JPEG`, `.Tif`, `.JXL`): Sie werden wie kleingeschriebene erkannt. Test in Task 4.
4. **Schreibgeschütztes Original bei eingeschaltetem Papierkorb**, oder der Papierkorb verweigert das Verschieben: Erwartet wird, dass das Original bleibt, der Status „Fertig mit Hinweis“ ist und es keinen Absturz gibt. Test in Task 7.
5. **Derselbe Ordner zweimal hinzugefügt**, oder ein Ordner plus eine Datei daraus: In der Warteschlange gibt es keine doppelten Einträge. Test in Task 10.

---

## Dateistruktur

```
.gitignore
scripts/fetch-libjxl.ps1          lädt libjxl v0.12.0, legt Sidecars + Lizenz ab
scripts/package-portable.ps1      baut den portablen Ordner
docs/design/jpeg-xl-konverter.dc.html, support.js   Entwurf (entpackt, Referenz)
src-tauri/
  tauri.conf.json, capabilities/default.json, Cargo.toml
  src/main.rs                     ruft lib::run()
  src/lib.rs                      Tauri-Aufbau, Befehle registrieren
  src/commands.rs                 dünne Tauri-Befehle → engine
  src/settings_store.rs           Einstellungen als JSON laden/speichern
  src/engine/mod.rs               pub mod …
  src/engine/types.rs             gemeinsame Typen (serde camelCase)
  src/engine/formats.rs           Endungen ↔ InputKind
  src/engine/scan.rs              Pfade einsammeln
  src/engine/targets.rs           Zielpfade, .tmp-Pfade
  src/engine/tools.rs             cjxl/djxl finden, prüfen, aufrufen (CancelToken)
  src/engine/args.rs              Einstellungen → cjxl/djxl-Argumente
  src/engine/jxl_container.rs     Box-Struktur lesen (jbrd)
  src/engine/verify.rs            Byte- und Pixelvergleich
  src/engine/convert_io.rs        TIFF/WebP/BMP ↔ PNG mit Metadaten
  src/engine/originals.rs         Papierkorb-Regeln, RecycleBin, Datumsübernahme
  src/engine/disk.rs              freier Speicher
  src/engine/encode.rs            eine Datei → JXL
  src/engine/decode.rs            eine JXL → Zielformat
  src/engine/queue.rs             Warteschlange, Worker, Ereignisse
  src/engine/report.rs            Zusammenfassung, Protokolldatei
  src/engine/preview.rs           Vorher/Nachher-Ausschnitt, Vorschaubilder
  tests/common/mod.rs             Testbilder erzeugen, Tools finden
  tests/encode_roundtrip.rs, tests/decode_roundtrip.rs
src/
  main.tsx, App.tsx, styles.css
  api/types.ts, api/backend.ts, api/tauriBackend.ts, api/mockBackend.ts, api/index.ts
  state/settings.ts               UiSettings, Voreinstellungen, toJobSettings
  state/store.ts                  zustand-Store
  lib/format.ts                   Größen, Prozente, Summen, Knopftexte
  components/TitleBar.tsx, ToolsBanner.tsx, CompareSlider.tsx,
  components/QueuePanel.tsx, QueueRow.tsx, SettingsPanel.tsx,
  components/EncodeSettings.tsx, DecodeSettings.tsx, CommonSettings.tsx, RunFooter.tsx
  **/*.test.ts(x)                 Vitest
```

## Gemeinsame Typen (Rust `engine/types.rs` ↔ TS `api/types.ts`)

Serde mit `#[serde(rename_all = "camelCase")]`, Enums als kleingeschriebene Strings. Diese Namen gelten in **allen** Tasks.

```rust
pub enum Mode { Enc, Dec }                                   // "enc" | "dec"
pub enum InputKind { Jpeg, Png, Gif, Exr, Tiff, Webp, Bmp, Jxl }
pub enum OutputFormat { Auto, Png, Jpg, Webp, Tiff }         // "auto" | "png" | "jpg" | "webp" | "tiff"
pub enum OutputLocation { Next, Folder { path: PathBuf } }   // {"kind":"next"} | {"kind":"folder","path":…}
pub struct EncodeSettings { pub distance: f32, pub effort: u8, pub transcode_jpeg: bool }
pub struct DecodeSettings { pub format: OutputFormat, pub jpeg_quality: u8, pub reduce_to_8bit: bool }
pub struct JobSettings { pub encode: EncodeSettings, pub decode: DecodeSettings,
                         pub keep_metadata: bool, pub trash_originals: bool, pub output: OutputLocation }
pub struct ScanEntry { pub path: PathBuf, pub root: Option<PathBuf>, pub kind: InputKind,
                       pub size: u64, pub width: u32, pub height: u32 }
pub struct QueueItem { pub id: u64, pub entry: ScanEntry }
pub enum ItemStatus { Waiting, Running, Done, DoneWithNote, Skipped, Error, VerifyFailed }
pub struct Outcome { pub status: ItemStatus, pub out_path: Option<PathBuf>, pub out_size: Option<u64>, pub note: Option<String> }
pub struct ItemEvent { pub id: u64, pub status: ItemStatus, pub progress: u8, pub out_size: Option<u64>, pub note: Option<String> }
pub struct RunSummary { pub done: u32, pub skipped: u32, pub errors: u32, pub bytes_in: u64, pub bytes_out: u64, pub log_path: Option<PathBuf> }
pub enum PauseReason { User, DiskFull { drive: String } }
pub struct ToolStatus { pub ok: bool, pub cjxl_version: Option<String>, pub djxl_version: Option<String>, pub message: Option<String> }
#[derive(Clone)] pub struct CancelToken(Arc<AtomicBool>)     // cancel(), is_cancelled()
```

`OutputLocation` mit `#[serde(tag = "kind", rename_all = "camelCase")]`, `PauseReason` mit `#[serde(tag = "type", rename_all = "camelCase")]` → `{"type":"user"}` bzw. `{"type":"diskFull","drive":"D:"}`. `ItemStatus` als `"waiting" | "running" | "done" | "doneWithNote" | "skipped" | "error" | "verifyFailed"`.

`ScanEntry.root` ist der hinzugefügte Ordner (bei Einzeldateien `None`). „Fertig“ für Summen = `Done` oder `DoneWithNote`. Fehlerzähler = `Error` + `VerifyFailed`.

---

### Task 1: Projektgerüst (Tauri + React + Tailwind + HeroUI + Lexend + Vitest)

**Files:**
- Create: Gerüst von `create-tauri-app` (React + TypeScript) im Projektordner, `.gitignore`, `src/styles.css`, `docs/design/*`
- Modify: `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, `vite.config.ts`, `package.json`
- Test: `src/App.test.tsx`

**Interfaces:**
- Produces: Lib-Crate `jpeg_xl_konverter_lib` mit `pub mod engine;` (leer), npm-Skripte `dev`, `build`, `test` (`vitest run`), `tauri`.

- [ ] **Step 1: Gerüst erzeugen.** `npm create tauri-app@latest -- --help` lesen, dann mit Template `react-ts`, Paketmanager `npm`, App-Name `jpeg-xl-konverter`, Identifier `de.bassboost1981.jpegxlkonverter` in den Unterordner `scaffold` erzeugen, den Inhalt in den Projektordner verschieben und `scaffold` löschen. `npm install`.
- [ ] **Step 2: `.gitignore`** mit `node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/binaries/*.exe`, `src-tauri/binaries/LICENSE*`, `dist-portable/`, `*.tmp`.
- [ ] **Step 3: Entwurf als Referenz ablegen.** `JPEG XL Konverter.dc.html` und `support.js` aus der ZIP nach `docs/design/` entpacken (HTML umbenannt in `jpeg-xl-konverter.dc.html`, den Verweis auf `support.js` beibehalten).
- [ ] **Step 4: Abhängigkeiten.** `npm install @heroui/react @heroui/styles @fontsource-variable/lexend zustand @tanstack/react-virtual` und `npm install -D tailwindcss @tailwindcss/vite vitest jsdom @testing-library/react @testing-library/jest-dom @testing-library/user-event`. `@tailwindcss/vite` in `vite.config.ts` eintragen, Vitest mit `environment: "jsdom"` und Setup-Datei für jest-dom.
- [ ] **Step 5: `src/styles.css`.**

```css
@import "tailwindcss";
@import "@heroui/styles";
@theme { --font-sans: "Lexend Variable", ui-sans-serif, system-ui, sans-serif; }
:root, .light { --accent: #006FEE; --accent-foreground: #ffffff; }
.dark { --accent: #338ef7; --accent-foreground: #ffffff; }
body { font-variant-numeric: tabular-nums; }
```
  In `main.tsx`: `import "@fontsource-variable/lexend";` vor `import "./styles.css";`.
- [ ] **Step 6: Fehlschlagenden Test schreiben** `src/App.test.tsx`: Rendert `<App />`, erwartet den Text `JPEG XL`. Ausführen mit `npm test`. Erwartet: FAIL (das Template zeigt etwas anderes).
- [ ] **Step 7: `App.tsx`** auf einen Platzhalter mit HeroUI-`Button` „JPEG XL“ reduzieren. `npm test` → PASS.
- [ ] **Step 8: Fenster konfigurieren** in `tauri.conf.json`: `productName` „JPEG XL Konverter“, Fenster `title` „JPEG XL Konverter“, `width` 1200, `height` 760, `minWidth` 1000, `minHeight` 640, `decorations` false, `dragDropEnabled` true. In `Cargo.toml` den Lib-Namen `jpeg_xl_konverter_lib` sicherstellen. `src-tauri/src/engine/mod.rs` leer anlegen und `pub mod engine;` in `lib.rs`. Programmsymbol erzeugen: `npm run tauri icon assets/jxl-logo-v2.png`.
- [ ] **Step 9: Prüfen.** `cargo test --manifest-path src-tauri/Cargo.toml` → 0 Tests, kein Fehler. `npm run tauri dev` → rahmenloses Fenster mit dem Knopf in Lexend (Screenshot prüfen). Danach beenden.
- [ ] **Step 10: Commit** `feat: scaffold Tauri + React + HeroUI app`.

### Task 2: libjxl laden und `tools`/`args` (inkl. Klärung der Optionen)

**Files:**
- Create: `scripts/fetch-libjxl.ps1`, `src-tauri/src/engine/{tools,args,types}.rs`, `docs/libjxl-notes.md`
- Modify: `src-tauri/src/engine/mod.rs`, `src-tauri/tauri.conf.json` (`bundle.externalBin: ["binaries/cjxl", "binaries/djxl"]`)
- Test: Unit-Tests in `args.rs`, `tools.rs`

**Interfaces:**
- Consumes: Typen aus „Gemeinsame Typen“ (in diesem Task vollständig in `types.rs` anlegen).
- Produces:
  - `Tools { cjxl: PathBuf, djxl: PathBuf }`
  - `Tools::locate(exe_dir: &Path) -> Option<Tools>`: sucht `cjxl.exe`, dann `cjxl-x86_64-pc-windows-msvc.exe` in `exe_dir`, dann in `env!("CARGO_MANIFEST_DIR")/binaries`
  - `Tools::check(&self) -> ToolStatus`
  - `Tools::run_cjxl(&self, input: &Path, output: &Path, args: &[String], cancel: &CancelToken) -> Result<(), ToolError>` und `run_djxl(…)` mit gleicher Signatur
  - `ToolError { exit_code: Option<i32>, stderr: String, cancelled: bool }`
  - `args::cjxl_args(kind: InputKind, enc: &EncodeSettings, keep_metadata: bool) -> Vec<String>`
  - `args::djxl_args(target: DjxlTarget) -> Vec<String>`, `enum DjxlTarget { ReconstructJpeg, Png { reduce_to_8bit: bool }, PixelsToJpeg { quality: u8 } }`
  - `const TOOL_THREADS: u32 = 2`

- [ ] **Step 1: `scripts/fetch-libjxl.ps1`** lädt `https://github.com/libjxl/libjxl/releases/download/v0.12.0/jxl-x64-windows-static.zip` in einen neuen leeren Ordner unter `$env:TEMP`, prüft SHA-256 gegen die Konstante `$ExpectedSha256`, entpackt, kopiert `cjxl.exe`/`djxl.exe` als `src-tauri/binaries/cjxl-x86_64-pc-windows-msvc.exe` bzw. `djxl-…` und die Lizenzdatei als `src-tauri/binaries/LICENSE-libjxl`. Beim ersten Lauf ist `$ExpectedSha256` leer. Das Skript gibt dann den Hash aus, du trägst ihn ein und führst es erneut aus. Erwartet: beide Dateien vorhanden, `& src-tauri/binaries/cjxl-x86_64-pc-windows-msvc.exe --version` zeigt `0.12.0`.
- [ ] **Step 2: Optionen klären.** `cjxl --help -v -v -v -v` und `djxl --help -v -v -v -v` ausführen und in `docs/libjxl-notes.md` die tatsächlichen Schreibweisen festhalten für: JPEG-Umpacken an/aus, Distanz, Effort, Threads, EXIF/XMP entfernen, JPEG wiederherstellen, Pixel als JPEG + Qualität, Bittiefe der Ausgabe. Außerdem ausprobieren und notieren: (a) Übernimmt `cjxl` bei JPEG-Eingabe mit gesetztem `-d` trotzdem das Umpacken? (b) Funktionieren Ein- und Ausgabepfade mit `Ü`, `📷` und mit über 260 Zeichen? (c) Unterstützen `cjxl`/`djxl` `-` für stdin/stdout? Die Erwartungswerte in Step 3 gelten vorbehaltlich dieser Notizen. Weicht die Hilfe ab, gilt die Hilfe, und Test und Notiz werden angepasst.
- [ ] **Step 3: Fehlschlagende Tests in `args.rs`:**

```rust
#[test] fn jpeg_transcode_has_no_distance() {
    let a = cjxl_args(InputKind::Jpeg, &enc(1.2, 7, true), true);
    assert!(a.contains(&"--lossless_jpeg=1".into()));
    assert!(!a.contains(&"-d".into()));
    assert!(a.windows(2).any(|w| w == ["-e", "7"]));
    assert!(a.contains(&"--num_threads=2".into()));
}
#[test] fn jpeg_without_transcode_uses_distance() { /* Jpeg, enc(2.6,8,false) → "--lossless_jpeg=0", ["-d","2.6"], ["-e","8"] */ }
#[test] fn png_lossless_is_distance_zero() { /* Png, enc(0.0,7,true) → ["-d","0"] */ }
#[test] fn strip_only_when_not_transcoding() {
    // keep_metadata=false: Png → enthält die EXIF- und XMP-Strip-Optionen; Jpeg+transcode → enthält sie nicht
}
#[test] fn djxl_targets() {
    assert_eq!(djxl_args(DjxlTarget::ReconstructJpeg), vec!["--reconstruct_jpeg", "--num_threads=2"]);
    assert_eq!(djxl_args(DjxlTarget::PixelsToJpeg{quality:92}), vec!["--pixels_to_jpeg", "--jpeg_quality=92", "--num_threads=2"]);
    assert_eq!(djxl_args(DjxlTarget::Png{reduce_to_8bit:true}), vec!["--bits_per_sample=8", "--num_threads=2"]);
    assert_eq!(djxl_args(DjxlTarget::Png{reduce_to_8bit:false}), vec!["--num_threads=2"]);
}
```
  Die Distanz wird mit `format!("{}", d)` ausgegeben (`0`, `0.5`, `1.2`, `2.6`).
- [ ] **Step 4:** `cargo test --manifest-path src-tauri/Cargo.toml args` → FAIL. **Step 5:** `args.rs` umsetzen → PASS.
- [ ] **Step 6: Fehlschlagende Tests in `tools.rs`:** `locate_finds_dev_binaries` (`Tools::locate` mit einem leeren Temp-Ordner findet die Dateien unter `CARGO_MANIFEST_DIR/binaries`). `check_reports_versions` (`ok == true`, beide Versionen enthalten `0.12`). `cancel_kills_running_tool` (ein großes, erzeugtes PNG mit `-e 9`; `cancel()` nach 100 ms → `Err(ToolError{cancelled:true,..})` innerhalb von 2 s, keine Ausgabedatei). `unicode_and_long_paths_work` (Eingabe und Ausgabe in `…/Ünïcödé 📷/` und in einem Pfad mit mehr als 260 Zeichen → `Ok`, Ausgabe existiert).
- [ ] **Step 7: `tools.rs` umsetzen.** Kindprozess mit `CREATE_NO_WINDOW`, stderr sammeln, alle 50 ms `try_wait()` und bei `cancel` `kill()`. Pfadstrategie nach den Notizen aus Step 2, erste funktionierende Variante: (1) Pfade direkt übergeben, (2) stdin/stdout mit Datenübergabe durch Rust, (3) Arbeitsordner `std::env::temp_dir()/jpeg-xl-konverter/io` mit kurzen ASCII-Namen (Eingabe per Hardlink, sonst Kopie; Ausgabe danach an den Zielpfad verschieben). Die gewählte Variante kommt in `docs/libjxl-notes.md`. Tests → PASS.
- [ ] **Step 8: Commit** `feat: libjxl sidecars, tool runner and argument mapping`.

### Task 3: Testhilfen für Bilder

**Files:**
- Create: `src-tauri/tests/common/mod.rs`
- Modify: `src-tauri/Cargo.toml` (dev-dependencies `tempfile`, `jpeg-encoder`, `image`)
- Test: `src-tauri/tests/fixtures_smoke.rs`

**Interfaces:**
- Produces (für alle Integrationstests): `tools() -> Tools`; `gradient_rgba8(w, h) -> image::RgbaImage`; `write_jpeg(path, w, h, JpegOpts { progressive: bool, cmyk: bool, exif: Option<Vec<u8>> })`; `write_png(path, PngKind::{Rgb8, Rgba8, Rgb16, Gray8})`; `write_tiff16(path)`; `write_bmp(path)`; `write_gif(path)`; `write_webp_with_exif(path, exif: &[u8])`; `minimal_exif(marker: &str) -> Vec<u8>`; `read_png_exif(path) -> Option<Vec<u8>>`.

- [ ] **Step 1: Fehlschlagender Test** `fixtures_smoke.rs`: Jede Hilfsfunktion schreibt in ein `tempfile::tempdir()`. `image::open` liest alle Dateien außer der CMYK-JPEG mit den erwarteten Maßen 64 × 48. Die JPEG mit EXIF enthält `minimal_exif("TESTMARKER")` im APP1-Segment. Die WebP enthält den Marker in einem `EXIF`-Chunk.
- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml --test fixtures_smoke` → FAIL. **Step 3:** umsetzen. `jpeg-encoder` für baseline/progressiv/CMYK und APP1 (`b"Exif\0\0"` + Daten). `minimal_exif`: Big-Endian-TIFF-Kopf `MM\0*`, ein IFD-Eintrag `0x010E` ImageDescription (ASCII) mit dem Marker. `write_webp_with_exif`: verlustfreie WebP über `image` erzeugen und in einen RIFF-Container mit `VP8X` (EXIF-Flag gesetzt), dem `VP8L`-Chunk und einem `EXIF`-Chunk umbauen. **Step 4:** → PASS. **Step 5: Commit** `test: image fixture helpers`.

### Task 4: Formate, Einsammeln, Zielpfade

**Files:**
- Create: `src-tauri/src/engine/{formats,scan,targets}.rs`
- Test: Unit-Tests in den Dateien

**Interfaces:**
- Produces:
  - `formats::kind_from_path(p: &Path) -> Option<InputKind>` (Endung ohne Groß/Klein: jpg, jpeg, png, gif, exr, tif, tiff, webp, bmp, jxl)
  - `scan::scan_paths(paths: &[PathBuf], mode: Mode) -> ScanResult { entries: Vec<ScanEntry>, rejected: usize }`. Ordner werden rekursiv mit `walkdir` durchsucht. Bei `Enc` ist alles außer `Jxl` erlaubt, bei `Dec` nur `Jxl`. Maße per `imagesize`, bei JXL per `jxl-oxide`-Header. Unlesbare Maße → 0 × 0, die Datei bleibt aber drin.
  - `targets::target_path(entry: &ScanEntry, ext: &str, output: &OutputLocation) -> PathBuf`
  - `targets::tmp_path(target: &Path) -> PathBuf` (hängt `.tmp` an: `foto.jxl.tmp`)

- [ ] **Step 1: Fehlschlagende Tests:**
  - `uppercase_extensions_are_recognized`: `A.JPG`, `b.Jpeg`, `c.Tif`, `d.JXL` → Jpeg, Jpeg, Tiff, Jxl. `e.txt` → None.
  - `scan_recurses_and_rejects`: Ordner mit `a.jpg`, `sub/b.png`, `sub/c.jxl`, `notes.txt`. Bei `Enc` → 2 Einträge, `rejected == 2`, `root` = der Ordner. Bei `Dec` → 1 Eintrag, `rejected == 3`.
  - `single_file_has_no_root`.
  - `target_next_to_original`: `D:\F\a.jpg` + `Next` → `D:\F\a.jxl`.
  - `target_mirrors_from_added_folder`: Eintrag `D:\Fotos\2024\Urlaub\a.jpg` mit `root = D:\Fotos\2024`, `Folder(E:\Out)` → `E:\Out\2024\Urlaub\a.jxl`. Einzeldatei → `E:\Out\a.jxl`.
  - `tmp_path_appends_tmp`.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen. **Step 4:** PASS. **Step 5: Commit** `feat: format detection, scanning and target paths`.

### Task 5: JXL-Container und Prüfungen

**Files:**
- Create: `src-tauri/src/engine/{jxl_container,verify}.rs`
- Test: Unit-Tests in den Dateien und `src-tauri/tests/container.rs`

**Interfaces:**
- Produces:
  - `jxl_container::has_jpeg_reconstruction(path: &Path) -> io::Result<bool>`: liest die ISO-BMFF-Boxen (Größe 4 Byte BE, Typ 4 Byte; Größe 1 → 64-bit-Größe folgt; Größe 0 → bis Dateiende). `true`, wenn eine Box `jbrd` existiert. Eine reine Codestream-Datei (`FF 0A`) → `false`.
  - `verify::files_identical(a: &Path, b: &Path) -> io::Result<bool>` (erst Größe, dann blockweise)
  - `verify::same_pixels(a: &DynamicImage, b: &DynamicImage) -> bool` (Maße gleich und `to_rgba16()`-Puffer gleich)
  - `verify::image_dims(path: &Path) -> Result<(u32, u32), String>`

- [ ] **Step 1: Fehlschlagende Tests:** Unit-Tests mit handgebauten Byte-Folgen (Container mit `jbrd` → true, ohne → false, Codestream → false, Box mit Größe 1 und 64-bit-Länge wird übersprungen). `files_identical` mit gleicher, anders langer und gleich langer, aber abweichender Datei. `same_pixels`: RGB8 gegen RGBA8 mit Alpha 255 und gleichen Farben → true, ein abweichendes Pixel → false. Integrationstest `container.rs`: JPEG mit `cjxl` umgepackt → `has_jpeg_reconstruction == true`; PNG umgewandelt → false.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen. **Step 4:** PASS. **Step 5: Commit** `feat: jxl box reader and verification helpers`.

### Task 6: TIFF/WebP/BMP ↔ PNG mit Metadaten

**Files:**
- Create: `src-tauri/src/engine/convert_io.rs`
- Test: `src-tauri/tests/convert_io.rs`

**Interfaces:**
- Produces:
  - `enum MetaState { NotPresent, Transferred, Lost }`, `struct MetaTransfer { exif: MetaState, icc: MetaState }`, `MetaTransfer::complete(&self) -> bool` (kein Feld `Lost`)
  - `to_png(src: &Path, dst_png: &Path, keep_metadata: bool) -> Result<MetaTransfer, String>`
  - `from_png(src_png: &Path, dst: &Path, format: OutputFormat, keep_metadata: bool) -> Result<MetaTransfer, String>` für `Tiff` (verlustfrei) und `Webp` (verlustbehaftet, Qualität 90)
  - `strip_png_metadata(path: &Path) -> Result<(), String>`: schreibt das PNG ohne `eXIf`, ohne `iTXt`/`zTXt`/`tEXt` mit EXIF- oder XMP-Inhalt (Schlüsselwörter `XML:com.adobe.xmp`, `Raw profile type exif`, `Raw profile type xmp`) neu. `iCCP` bleibt erhalten.

- [ ] **Step 1: Fehlschlagende Tests:**
  - `tiff16_to_png_keeps_16bit`: Das PNG ist 16 Bit, die Pixel sind gleich dem TIFF.
  - `webp_exif_goes_into_png`: Bei `keep_metadata = true` enthält das PNG einen `eXIf`-Chunk mit `TESTMARKER`, und es gilt `exif == Transferred`.
  - `strip_when_metadata_off`: Bei `keep_metadata = false` gibt es kein `eXIf`, und es gilt `exif == NotPresent`.
  - `bmp_to_png_without_metadata`: `exif == NotPresent`, `icc == NotPresent`.
  - `png_to_webp_marks_exif_lost`: Ein PNG mit `eXIf` wird mit `keep_metadata = true` zu WebP. Die Datei ist lesbar, und es gilt `exif == Lost`.
  - `png_to_tiff_is_pixel_identical`.
  - `strip_png_metadata_keeps_icc`: Ein PNG mit `eXIf`, XMP-`iTXt` und `iCCP` wird bereinigt. Danach gibt es kein `eXIf` und kein XMP mehr, `iCCP` ist vorhanden, die Pixel sind gleich.
- [ ] **Step 2:** FAIL. **Step 3: Umsetzen.** Lesen über `image::ImageReader::open(src)?.with_guessed_format()?.into_decoder()`, dann `exif_metadata()`/`icc_profile()`, danach `DynamicImage::from_decoder`. Float-Bilder werden zu 16 Bit. Schreiben mit dem `png`-Crate: Kopf, dann `iCCP` (Name `ICC`, 0, 0, zlib per `miniz_oxide`), dann `eXIf`, dann Bilddaten. WebP über das `webp`-Crate mit Qualität 90. TIFF über `image`. Metadaten-Ausgabe für TIFF/WebP: vorhandene EXIF/ICC → `Lost`. **Step 4:** PASS. **Step 5: Commit** `feat: TIFF/WebP/BMP conversion with metadata hand-over`.

### Task 7: Originale — Papierkorb-Regeln, Papierkorb, Datum

**Files:**
- Create: `src-tauri/src/engine/{originals,disk}.rs`
- Test: Unit-Tests in `originals.rs`

**Interfaces:**
- Produces:
  - `struct TrashContext { enabled: bool, verified: bool, lossless: bool, metadata_complete: bool }`
  - `fn trash_wanted(ctx: &TrashContext) -> bool` (alle vier Bedingungen)
  - `enum Availability { Available, NoRecycleBin, TooLarge }`
  - `trait RecycleBin: Send + Sync { fn availability(&self, path: &Path, size: u64) -> Availability; fn recycle(&self, path: &Path) -> Result<(), String>; }`
  - `struct WindowsRecycleBin;` und `pub struct FakeRecycleBin { pub availability: Availability, pub fail: bool, pub recycled: Mutex<Vec<PathBuf>> }`
  - `fn handle_original(ctx: &TrashContext, original: &Path, bin: &dyn RecycleBin) -> Option<String>`. Rückgabe ist der Hinweistext oder `None`. Texte: „Original behalten: kein Papierkorb auf diesem Laufwerk“, „Original behalten: zu groß für den Papierkorb“, „Original behalten: Papierkorb nicht möglich (<Grund>)“.
  - `fn copy_file_times(src: &Path, dst: &Path) -> io::Result<()>` (Erstellungs- und Änderungsdatum über `std::fs::FileTimes` + `std::os::windows::fs::FileTimesExt::set_created`)
  - `disk::free_bytes(path: &Path) -> io::Result<u64>` (`GetDiskFreeSpaceExW`)

- [ ] **Step 1: Fehlschlagende Tests:**
  - `trash_needs_all_conditions`: Tabelle über alle 16 Kombinationen. Nur bei allen vier `true` ist das Ergebnis `true`.
  - `no_recycle_bin_keeps_original`: Fake mit `NoRecycleBin` → Hinweis „kein Papierkorb…“, `recycled` bleibt leer, die Datei existiert noch.
  - `too_large_keeps_original`.
  - `recycle_failure_keeps_original` (Fake mit `fail: true`, gilt auch für das schreibgeschützte Original aus Review Focus 4): Hinweis beginnt mit „Original behalten: Papierkorb nicht möglich“.
  - `recycles_when_allowed`.
  - `copy_file_times_copies_created_and_modified`: Quelle auf 2001-02-03 setzen, kopieren, Ziel vergleichen (sekundengenau).
  - `#[ignore] windows_recycle_bin_manual`: legt eine kleine Datei in `%TEMP%` an und verschiebt sie mit `WindowsRecycleBin` in den Papierkorb. Wird nur von Hand ausgeführt.
- [ ] **Step 2:** FAIL. **Step 3: Umsetzen.** `WindowsRecycleBin::availability`:
  1. `GetDriveTypeW` muss `DRIVE_FIXED` liefern, sonst `NoRecycleBin`.
  2. Die Volume-GUID per `GetVolumeNameForVolumeMountPointW` ermitteln.
  3. Unter `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\BitBucket\Volume\{GUID}` gilt: `NukeOnDelete = 1` → `NoRecycleBin`; `MaxCapacity` (MB) kleiner als die Dateigröße → `TooLarge`. Fehlt der Schlüssel, gilt `Available`.

  `recycle`: `IFileOperation` in einem eigenen STA-Thread mit den Flags `FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI | FOF_WANTNUKEWARNING | FOFX_EARLYFAILURE`. Danach `GetAnyOperationsAborted()` prüfen und kontrollieren, dass die Datei weg ist, sonst `Err`. `FOF_WANTNUKEWARNING` ist die letzte Sicherung: Würde Windows doch endgültig löschen, fragt es zuerst. **Step 4:** PASS. **Step 5:** `cargo test … -- --ignored windows_recycle_bin_manual` einmal ausführen und die Datei im Papierkorb sehen. **Step 6: Commit** `feat: recycle-bin rules, Windows recycle bin and file times`.

### Task 8: Eine Datei nach JXL (`encode`)

**Files:**
- Create: `src-tauri/src/engine/encode.rs`
- Test: `src-tauri/tests/encode_roundtrip.rs`

**Interfaces:**
- Consumes: Tasks 2, 4–7.
- Produces:
  - `struct Ctx<'a> { tools: &'a Tools, bin: &'a dyn RecycleBin, settings: &'a JobSettings, cancel: &'a CancelToken, work_dir: &'a Path, progress: &'a dyn Fn(u8) }`
  - `fn encode_one(entry: &ScanEntry, target: &Path, ctx: &Ctx) -> Outcome`
  - Fortschritt: 5 beim Start, 60 nach dem Umwandeln, 90 nach der Prüfung, 100 fertig.

Ablauf (Spezifikation 5.1):
1. Gibt es das Ziel schon → `Skipped` mit „Zieldatei existiert bereits“.
2. Eine alte `tmp_path(target)` löschen, Zielordner anlegen.
3. Umwandeln:
   - JPEG mit Umpacken: `cjxl` → tmp. Ist danach `has_jpeg_reconstruction(tmp) == false` oder schlägt `cjxl` fehl → tmp löschen, `Skipped` „JPEG lässt sich nicht verlustfrei umpacken“.
   - TIFF/WebP/BMP: erst `to_png` in `work_dir`.
4. Prüfen:
   - JPEG umgepackt: `djxl --reconstruct_jpeg` in `work_dir`, dann `files_identical`.
   - Distanz 0: `djxl` → PNG, dann `same_pixels` mit dem Ausgangsbild. EXR: nur Maße prüfen.
   - Sonst: `djxl` → PNG, dann Maße prüfen.
   - Bei Fehlschlag: tmp löschen, `VerifyFailed`.
5. tmp → Ziel umbenennen, `copy_file_times(original, target)`.
6. `handle_original` mit `lossless` = umgepackt oder Distanz 0 und `metadata_complete` = `keep_metadata` und `MetaTransfer::complete()` (bei JPEG umgepackt immer `true`).
7. Ergebnis `Done`, oder `DoneWithNote`, wenn ein Hinweis vorliegt (Original behalten oder Metadaten verloren: „Metadaten nicht vollständig übernommen“).

Fehler beim Lesen oder Umwandeln → `Error` mit einem kurzen deutschen Grund. Zwischendateien werden immer weggeräumt.

- [ ] **Step 1: Fehlschlagende Tests** (jeweils `tempdir`, `FakeRecycleBin`):
  - `jpeg_baseline_roundtrip_is_byte_identical` (gleiche Prüfung für progressive JPEG und JPEG mit EXIF): `Done`, `.jxl` existiert, eigene Rekonstruktion mit `djxl` ist byte-gleich.
  - `png_variants_lossless_pixel_identical` (Rgb8, Rgba8, Rgb16, Gray8): `Done`.
  - `tiff_webp_bmp_gif_encode`: alle `Done`. Bei WebP liefert eine Rückwandlung per `djxl` → PNG über `read_png_exif` den `TESTMARKER`.
  - `cmyk_jpeg_skipped`: `Skipped`, kein `.jxl`, keine `.tmp`.
  - `corrupt_file_errors`: Zufallsbytes als `kaputt.jpg` → `Error`, Original unverändert, keine `.tmp`.
  - `png_named_jpg_errors_or_converts_without_loss`: Ein PNG namens `x.jpg` ergibt `Error` oder `Skipped`, aber nie ein `.jxl`, das kein umgepacktes JPEG ist.
  - `existing_target_skipped`: Inhalt der vorhandenen `.jxl` bleibt unverändert.
  - `missing_or_locked_source_errors` (Review Focus 2): Die Datei wird nach dem Einsammeln gelöscht → `Error`. Die Datei ist mit `share_mode(0)` geöffnet → `Error`.
  - `unicode_and_long_paths`: `Done`.
  - `dates_copied`.
  - `trash_only_when_lossless`: Mit `trash_originals = true` und Distanz 1.2 → Original bleibt, `recycled` ist leer. Mit Distanz 0 → `recycled` enthält das Original.
  - `metadata_off_never_trashes`: Mit `keep_metadata = false` und Papierkorb an bleibt das Original.
- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml --test encode_roundtrip` → FAIL. **Step 3:** umsetzen. **Step 4:** PASS. **Step 5: Commit** `feat: single-file encode with verification`.

### Task 9: Eine JXL zurück (`decode`)

**Files:**
- Create: `src-tauri/src/engine/decode.rs`
- Test: `src-tauri/tests/decode_roundtrip.rs`

**Interfaces:**
- Consumes: `Ctx` aus Task 8.
- Produces:
  - `fn decode_ext(entry: &ScanEntry, format: OutputFormat) -> io::Result<&'static str>`: `Auto` → `jpg`, wenn `has_jpeg_reconstruction`, sonst `png`. `Jpg` → `jpg`, `Png` → `png`, `Webp` → `webp`, `Tiff` → `tif`.
  - `fn decode_one(entry: &ScanEntry, target: &Path, ctx: &Ctx) -> Outcome`

Ablauf (Spezifikation 5.2):
- Ziel prüfen, tmp anlegen, umwandeln:
  - `Auto`/`Jpg` mit jbrd: `ReconstructJpeg`.
  - `Jpg` ohne jbrd: `PixelsToJpeg { quality: jpeg_quality }`.
  - `Png`/`Auto` ohne jbrd: `Png { reduce_to_8bit }`.
  - `Tiff`/`Webp`: erst `djxl` → PNG in `work_dir`, dann `from_png`.
- Bei `keep_metadata = false` und PNG-Ausgabe: `strip_png_metadata` auf die Zwischendatei anwenden. Bei rekonstruiertem JPEG bleiben die Metadaten immer erhalten.
- Prüfen: Ergebnis mit `image_dims` öffnen, die Maße müssen der Eintrag sein.
- Fertigstellen und Datum übernehmen wie in Task 8. `lossless` = rekonstruiert oder (PNG/TIFF und nicht `reduce_to_8bit`).

- [ ] **Step 1: Fehlschlagende Tests:**
  - `auto_restores_original_jpeg`: JPEG → `encode_one` → `.jxl`, Original danach umbenennen, `decode_one` mit `Auto` → `.jpg` ist byte-gleich zum Original.
  - `auto_non_jpeg_gives_png`: PNG → JXL (Distanz 0) → `Auto` → `.png` ist pixelgleich.
  - `jpg_from_pixels_uses_quality`: Distanz-1.2-JXL → `Jpg` mit Qualität 92 → lesbar, Maße stimmen.
  - `tiff_and_webp_outputs`: beide lesbar, Maße stimmen.
  - `reduce_to_8bit_never_trashes`: 16-Bit-PNG → JXL → `Png` mit `reduce_to_8bit = true`, Papierkorb an → `.jxl` bleibt, Ergebnis-PNG ist 8 Bit.
  - `existing_target_skipped`.
  - `metadata_off_strips_png_exif`: Ein PNG mit EXIF wird zu JXL (Distanz 0) und mit `keep_metadata = false` zurück zu PNG. Danach ist kein `eXIf` mehr vorhanden.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen. **Step 4:** PASS. **Step 5: Commit** `feat: single-file decode with jpeg reconstruction`.

### Task 10: Warteschlange und Bericht

**Files:**
- Create: `src-tauri/src/engine/{queue,report}.rs`
- Test: Unit-Tests in beiden Dateien

**Interfaces:**
- Produces:
  - `trait Processor: Send + Sync { fn target_for(&self, mode: Mode, e: &ScanEntry, s: &JobSettings) -> io::Result<PathBuf>; fn has_space(&self, e: &ScanEntry, target: &Path) -> bool; fn process(&self, mode: Mode, e: &ScanEntry, target: &Path, s: &JobSettings, cancel: &CancelToken, progress: &dyn Fn(u8)) -> Outcome; }`
  - `struct RealProcessor { tools: Tools, bin: Arc<dyn RecycleBin>, work_dir: PathBuf }` → ruft `encode_one`/`decode_one`. `has_space`: `free_bytes(target) >= 2 * size`.
  - `trait ProgressSink: Send + Sync { fn item(&self, ev: ItemEvent); fn paused(&self, r: PauseReason); fn finished(&self, s: RunSummary); }`
  - `struct Queue`: `new(processor: Arc<dyn Processor>, sink: Arc<dyn ProgressSink>, workers: usize, log_dir: PathBuf)`, `add(&self, mode: Mode, entries: Vec<ScanEntry>) -> Vec<QueueItem>` (entfernt Pfade, die schon in der Warteschlange dieses Modus sind), `entry(&self, id: u64) -> Option<ScanEntry>`, `clear(&self, mode: Mode) -> Result<(), String>` (Fehler, solange ein Durchlauf läuft), `start(&self, mode: Mode, s: JobSettings)`, `retry_failed(&self, mode: Mode, s: JobSettings)`, `pause(&self)`, `resume(&self)`, `cancel(&self)`, `is_running(&self) -> bool`
  - `fn default_workers() -> usize` = `max(1, available_parallelism / 2)`
  - `report::RunReport::new(mode, &JobSettings)`, `add(&mut self, path: &Path, size_in: u64, o: &Outcome)`, `summary(&self) -> RunSummary`, `write_log(&self, dir: &Path) -> io::Result<PathBuf>`

Verhalten:
- Worker ziehen Einträge mit Status `Waiting`.
- Vor jeder Datei reserviert der Worker den Zielpfad in einem gemeinsamen `HashSet`. Ist er schon reserviert oder vorhanden → `Skipped` „Zieldatei existiert bereits“ (Review Focus 1).
- Kein Platz → das Element bleibt `Waiting`, `paused(DiskFull{drive})`, und die Warteschlange hält an.
- Hinzufügen während eines Laufs → die neuen Einträge laufen im selben Durchlauf mit.
- `pause` lässt laufende Dateien fertig werden. `cancel` setzt den `CancelToken`, abgebrochene Einträge gehen zurück auf `Waiting`.
- Am Ende: Protokoll schreiben (nur wenn es Problemfälle gibt) und `finished`.
- Protokollname `chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").txt`. Inhalt: Modus, Einstellungen, Summenzeile, je Problemfall eine Zeile `Status<TAB>Pfad<TAB>Grund`.

- [ ] **Step 1: Fehlschlagende Tests** mit `FakeProcessor` (konfigurierbares Ergebnis je Dateiname, künstliche Dauer) und `RecordingSink`:
  - `processes_all_and_reports_summary`: 5 Dateien (3 `Done`, 1 `Skipped`, 1 `Error`) → `finished` mit done 3, skipped 1, errors 1, Bytes nur über die Fertigen.
  - `same_target_reserved_once` (Review Focus 1): `foto.jpg` + `foto.png` mit gleichem Ziel und 2 Workern → genau ein `Done`, ein `Skipped`.
  - `duplicates_not_added` (Review Focus 5): Derselbe Pfad zweimal und ein Ordner plus eine Datei daraus → keine doppelten IDs oder Pfade.
  - `pause_finishes_running_then_stops`, `resume_continues`.
  - `cancel_returns_running_to_waiting`.
  - `disk_full_pauses_with_reason`.
  - `add_during_run_is_processed`.
  - `retry_failed_only_reruns_errors`.
  - `clear_rejected_while_running`.
  - `log_written_only_with_problems`: Inhalt enthält die Summenzeile und die Zeile für die Fehlerdatei. Dateiname passt zu `^\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}\.txt$`.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen (Worker-Threads, `Mutex` + `Condvar`). **Step 4:** PASS. **Step 5: Commit** `feat: work queue with pause, cancel, reservation and run log`.

### Task 11: Vorschau

**Files:**
- Create: `src-tauri/src/engine/preview.rs`
- Test: Unit-Tests + `src-tauri/tests/preview.rs`

**Interfaces:**
- Produces:
  - `fn crop_rect(img_w: u32, img_h: u32, view_w: u32, view_h: u32) -> (u32, u32, u32, u32)`: zentriert, auf das Bild begrenzt
  - `struct Preview { before_png: Vec<u8>, after_png: Vec<u8>, lossless: bool }`
  - `fn make_preview(entry: &ScanEntry, mode: Mode, s: &JobSettings, view_w: u32, view_h: u32, tools: &Tools, work_dir: &Path) -> Result<Preview, String>`
  - `fn thumbnail(entry: &ScanEntry, size: u32, tools: &Tools, work_dir: &Path) -> Result<Vec<u8>, String>` (PNG, längste Seite = `size`)

Ablauf `make_preview`:
- Ausgangsbild laden (JXL über `djxl` → PNG) und ausschneiden.
- `Enc`: Bei JPEG mit Umpacken oder Distanz 0 → `after = before`, `lossless = true`. Sonst den Ausschnitt mit `cjxl` (Einstellungen) umwandeln und mit `djxl` → PNG.
- `Dec`: Ziel `Jpg` → den Ausschnitt mit `PixelsToJpeg`-Qualität kodieren und wieder lesen. `Webp` → `from_png` und wieder lesen. Sonst `after = before`.

- [ ] **Step 1: Fehlschlagende Tests:**
  - `crop_rect_centers_and_clamps`: (6000, 4000, 1000, 240) → (2500, 1880, 1000, 240). (500, 300, 1000, 240) → (0, 30, 500, 240).
  - `jpeg_transcode_preview_is_lossless`.
  - `lossy_preview_differs`: Distanz 2.6 → `lossless == false`, die Pixel sind ungleich.
  - `thumbnail_size`: längste Seite 80.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen. **Step 4:** PASS. **Step 5: Commit** `feat: before/after preview and thumbnails`.

### Task 12: Tauri-Anbindung

**Files:**
- Create: `src-tauri/src/{commands,settings_store}.rs`, `src-tauri/capabilities/default.json` (anpassen)
- Modify: `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml` (`tauri-plugin-dialog`, `tauri-plugin-window-state`, `tauri-plugin-opener`), `package.json` (`@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-opener`)
- Test: Unit-Tests in `settings_store.rs`

**Interfaces:**
- Produces Tauri-Befehle (Namen exakt, Parameter camelCase auf JS-Seite):
  - `check_tools() -> ToolStatus`
  - `add_paths(paths: Vec<String>, mode: Mode) -> AddResult { items: Vec<QueueItem>, rejected: usize }`
  - `clear_queue(mode: Mode)`
  - `start_run(mode: Mode, settings: JobSettings)`, `retry_failed(mode, settings)`, `pause_run()`, `resume_run()`, `cancel_run()`, `is_running() -> bool`
  - `preview(item_id: u64, mode: Mode, settings: JobSettings, view_w: u32, view_h: u32) -> PreviewDto { before: String, after: String, lossless: bool }` (Data-URLs `data:image/png;base64,…`)
  - `thumbnail(item_id: u64) -> String` (Data-URL, 80 px)
  - `load_settings() -> Option<serde_json::Value>` (setzt `trashOriginals` immer auf `false`), `save_settings(value: serde_json::Value)` (Datei `settings.json` im App-Konfigurationsordner)
  - `open_log(path: String)` (öffnet die Datei mit dem Standardprogramm)
- Ereignisse: `item-progress` (`ItemEvent`), `run-paused` (`PauseReason`), `run-finished` (`RunSummary`)
- Langsame Befehle (`add_paths`, `preview`, `thumbnail`) laufen als `async` mit `tauri::async_runtime::spawn_blocking`.
- `Tools` wird beim Start einmal per `Tools::locate(exe_dir)` gesucht. Fehlen die Tools → `check_tools` meldet `ok: false` mit „cjxl.exe oder djxl.exe fehlt neben dem Programm.“
- Arbeitsordner: `temp_dir()/jpeg-xl-konverter/<pid>`, wird beim Start angelegt und beim Beenden gelöscht.
- Protokollordner: `%LOCALAPPDATA%\JPEG XL Konverter\Protokolle`.
- Capabilities: `core:default`, `core:window:allow-minimize`, `core:window:allow-toggle-maximize`, `core:window:allow-close`, `core:window:allow-destroy`, `core:window:allow-start-dragging`, `dialog:default`, `opener:default`, `window-state:default`.

- [ ] **Step 1: Fehlschlagende Tests** `settings_store.rs`: `load_forces_trash_off` (gespeichert mit `trashOriginals: true` → geladen `false`), `missing_file_gives_none`, `roundtrip_keeps_other_fields`.
- [ ] **Step 2:** FAIL. **Step 3:** `settings_store.rs`, `commands.rs`, `lib.rs` umsetzen. Der `ProgressSink` der App sendet mit `AppHandle::emit`. **Step 4:** Tests PASS, `cargo build --manifest-path src-tauri/Cargo.toml` ohne Warnungen.
- [ ] **Step 5: Handprüfung.** `npm run tauri dev`, in den DevTools `await window.__TAURI_INTERNALS__.invoke("check_tools")` → `ok: true`. Beenden.
- [ ] **Step 6: Commit** `feat: tauri commands, events and settings store`.

### Task 13: Oberflächen-Grundlage (Typen, Backend, Store, Formatierung)

**Files:**
- Create: `src/api/{types,backend,tauriBackend,mockBackend,index}.ts`, `src/state/{settings,store}.ts`, `src/lib/format.ts`
- Test: `src/state/settings.test.ts`, `src/lib/format.test.ts`, `src/api/mockBackend.test.ts`

**Interfaces:**
- Produces:
  - `api/types.ts`: TS-Gegenstücke aller Typen aus „Gemeinsame Typen“ sowie `AddResult` und `PreviewDto`.
  - `api/backend.ts`:

```ts
export interface Backend {
  checkTools(): Promise<ToolStatus>;
  addPaths(paths: string[], mode: Mode): Promise<AddResult>;
  clearQueue(mode: Mode): Promise<void>;
  startRun(mode: Mode, s: JobSettings): Promise<void>;
  retryFailed(mode: Mode, s: JobSettings): Promise<void>;
  pauseRun(): Promise<void>; resumeRun(): Promise<void>; cancelRun(): Promise<void>;
  preview(itemId: number, mode: Mode, s: JobSettings, viewW: number, viewH: number): Promise<PreviewDto>;
  thumbnail(itemId: number): Promise<string>;
  loadSettings(): Promise<UiSettings | null>; saveSettings(s: UiSettings): Promise<void>;
  openLog(path: string): Promise<void>;
  pickFiles(mode: Mode): Promise<string[]>; pickFolder(): Promise<string | null>;
  onItemProgress(cb: (e: ItemEvent) => void): () => void;
  onRunPaused(cb: (r: PauseReason) => void): () => void;
  onRunFinished(cb: (s: RunSummary) => void): () => void;
  onFilesDropped(cb: (paths: string[]) => void): () => void;
  minimize(): void; toggleMaximize(): void; close(): void; onCloseRequested(cb: () => boolean): () => void;
}
```
  - `api/index.ts`: `export const backend: Backend`. Ist `"__TAURI_INTERNALS__" in window`, gilt `tauriBackend`, sonst `mockBackend`.
  - `mockBackend` simuliert mit den Beispieldateien aus dem Entwurf (`ENC`/`DEC`-Listen in `docs/design/jpeg-xl-konverter.dc.html`) Fortschritt per Timer, erzeugt Ergebnisgrößen und liefert Vorschau-Data-URLs als einfarbige Canvas-Flächen.
  - `state/settings.ts`: `type Preset = "ll" | "hi" | "bal" | "sm"`, `PRESETS` (Distanz und Effort siehe Global Constraints; Qualitätswert für den Expertenmodus: ll 100, hi 95, bal 88, sm 72), `EFFORT_NAMES = ["", "Lightning", "Thunder", "Falcon", "Cheetah", "Hare", "Wombat", "Squirrel", "Kitten", "Tortoise"]`, `interface UiSettings { mode; expert; preset; quality; effort; lossless; transcodeJpeg; keepMetadata; trashOriginals; decodeFormat; jpegQuality; bitDepth: 8 | 16; output: OutputLocation; theme: "light" | "dark" | null }`, `DEFAULT_SETTINGS` (enc, nicht Experte, `ll`, 100, 7, true, true, true, false, `auto`, 92, 16, next, null), `distanceFromQuality(q: number): number`, `enterExpert(s: UiSettings): UiSettings`, `toJobSettings(s: UiSettings): JobSettings`.
  - `state/store.ts`: zustand-Store `useApp` mit `settings`, `queues: Record<Mode, Row[]>` (`Row = QueueItem & { status; progress; outSize?; note?; thumb? }`), `selected: Record<Mode, number | null>`, `run: "idle" | "running" | "paused" | "finished"`, `pauseReason`, `summary`, `tools`, `notice` und Aktionen `addPaths`, `clear`, `start`, `pause`, `resume`, `retryFailed`, `select`, `updateSettings`, `applyEvent`.
  - `lib/format.ts`:
    - `formatSize(bytes: number): string`, binär gerechnet: unter 1 MB → ganze KB; unter 10 MB → 2 Nachkommastellen; unter 1024 MB → 1 Nachkommastelle MB; sonst 1 Nachkommastelle GB; Dezimalkomma.
    - `formatDelta(inB: number, outB: number): string`, gerundete Prozent mit `−`/`+`.
    - `formatCount(n: number): string` (de-DE).
    - `summaryLine(s: RunSummary): string`.
    - `footerHint(p: { done: number; total: number; bytesIn?: number; bytesOut?: number }): string`.
    - `ctaLabel(p: { mode: Mode; run: RunState; waiting: number; failed: number }): string`.
    - `formatDistance(d: number): string` mit Punkt als Dezimaltrenner wie im Entwurf: `0.5`, `1.2`, `1.18`.

- [ ] **Step 1: Fehlschlagende Tests:**

```ts
// format.test.ts  (1 MB = 1 048 576 Bytes)
expect(formatSize(389_120)).toBe("380 KB");
expect(formatSize(7_130_317)).toBe("6,80 MB");
expect(formatSize(149_212_364)).toBe("142,3 MB");
expect(formatSize(44_238_131_200)).toBe("41,2 GB");
expect(formatDelta(100, 80)).toBe("−20 %");          // U+2212
expect(formatDelta(100, 105)).toBe("+5 %");
expect(formatCount(9812)).toBe("9.812");
expect(summaryLine({ done: 9812, skipped: 14, errors: 2, bytesIn: 44_238_131_200, bytesOut: 35_433_480_192 }))
  .toBe("9.812 fertig · 14 übersprungen · 2 Fehler · 41,2 GB → 33,0 GB");
expect(footerHint({ done: 0, total: 6 })).toBe("Ersparnis erscheint nach der ersten fertigen Datei");
expect(ctaLabel({ mode: "enc", run: "idle", waiting: 6, failed: 0 })).toBe("6 Dateien in JPEG XL konvertieren");
expect(ctaLabel({ mode: "enc", run: "idle", waiting: 1, failed: 0 })).toBe("1 Datei in JPEG XL konvertieren");
expect(ctaLabel({ mode: "dec", run: "idle", waiting: 3, failed: 0 })).toBe("3 Dateien zurückkonvertieren");
expect(ctaLabel({ mode: "enc", run: "running", waiting: 2, failed: 0 })).toBe("Anhalten");
expect(ctaLabel({ mode: "enc", run: "paused", waiting: 2, failed: 0 })).toBe("Fortsetzen");
expect(ctaLabel({ mode: "enc", run: "finished", waiting: 0, failed: 2 })).toBe("Fehlgeschlagene erneut versuchen");
expect(ctaLabel({ mode: "enc", run: "finished", waiting: 0, failed: 0 })).toBe("Alles erledigt");
// settings.test.ts
expect(distanceFromQuality(100)).toBe(0);
expect(distanceFromQuality(88)).toBeCloseTo(1.18);
expect(toJobSettings({ ...DEFAULT_SETTINGS }).encode).toEqual({ distance: 0, effort: 7, transcodeJpeg: true });
expect(toJobSettings({ ...DEFAULT_SETTINGS, preset: "sm" }).encode).toEqual({ distance: 2.6, effort: 8, transcodeJpeg: true });
expect(enterExpert({ ...DEFAULT_SETTINGS, preset: "bal" })).toMatchObject({ expert: true, quality: 88, effort: 7, lossless: false });
expect(toJobSettings({ ...DEFAULT_SETTINGS, expert: true, quality: 72, effort: 8, lossless: false }).encode.distance).toBeCloseTo(2.62);
expect(toJobSettings({ ...DEFAULT_SETTINGS, expert: true, lossless: true, quality: 50 }).encode.distance).toBe(0);
expect(toJobSettings({ ...DEFAULT_SETTINGS, expert: false, bitDepth: 8 }).decode.reduceTo8bit).toBe(false); // nur Experte reduziert
// mockBackend.test.ts: addPaths → Einträge; startRun → onItemProgress bis alle "done", danach genau ein onRunFinished.
```
- [ ] **Step 2:** `npm test` → FAIL. **Step 3:** umsetzen. In `tauriBackend` gilt: `invoke` mit den Befehlsnamen aus Task 12, `listen` für die Ereignisse, `getCurrentWebview().onDragDropEvent` (nur `type === "drop"` → `paths`), Dialoge über `@tauri-apps/plugin-dialog`, Fenster über `getCurrentWindow()`. **Step 4:** PASS. **Step 5: Commit** `feat: frontend api layer, settings model and store`.

### Task 14: Fensterrahmen — Titelleiste, Thema, Layout, Werkzeug-Hinweis, Beenden-Rückfrage

**Files:**
- Create: `src/components/{TitleBar,ToolsBanner}.tsx`
- Modify: `src/App.tsx`
- Test: `src/components/TitleBar.test.tsx`, `src/App.test.tsx`

**Interfaces:**
- Consumes: `useApp`, `backend`.
- Produces: `App` mit dem Layout aus dem Entwurf (Titelleiste 64 px, darunter ein Raster `minmax(0,1fr) 360px` mit `gap` 16 px und `padding` 16 px). Links liegen der Vorher/Nachher-Bereich und die Warteschlange, rechts die Einstellungen. Für Task 15–17 gibt es Platzhalter `<CompareSlider/>`, `<QueuePanel/>`, `<SettingsPanel/>` und `<RunFooter/>`.

- [ ] **Step 1: Fehlschlagende Tests:**
  - `TitleBar`: zeigt das Logo `assets/jxl-logo-v2.svg` (als `img` mit `alt="JPEG XL Konverter"`), den Text „JPEG XL“, die Tabs „In JPEG XL“/„Aus JPEG XL“ und die Schalter „Hell“/„Dunkel“ und „Experte“. Ein Klick auf „Aus JPEG XL“ setzt `settings.mode = "dec"`. Der Hell/Dunkel-Schalter setzt die Klasse `dark` auf `document.documentElement`.
  - `App`: Bei `tools.ok === false` steht der Text „cjxl.exe oder djxl.exe fehlt neben dem Programm.“ da, und der Start-Knopf ist deaktiviert. Ist das Theme `null`, folgt es `matchMedia("(prefers-color-scheme: dark)")`.
- [ ] **Step 2:** FAIL. **Step 3: Umsetzen.**
  - Die Titelleiste hat `data-tauri-drag-region`.
  - Die Fensterknöpfe (—, ▢, ✕) rufen `backend.minimize/toggleMaximize/close` auf.
  - HeroUI `Tabs` für den Modus, HeroUI `Switch` für Thema und Experte.
  - Einstellungen werden beim Start mit `loadSettings` geladen und bei Änderungen mit 500 ms Verzögerung gespeichert.
  - `onCloseRequested`: Läuft ein Durchlauf, öffnet sich ein HeroUI-`AlertDialog` „Wirklich beenden?“ mit dem Text „Laufende Umwandlungen werden abgebrochen.“ und den Knöpfen „Beenden“ und „Weiterarbeiten“. „Beenden“ → `cancelRun`, dann Fenster schließen.
  - Das Logo wird nach `src/assets/` kopiert.
  - **Step 4:** PASS. **Step 5: Commit** `feat: title bar, theme, layout shell and close confirmation`.

### Task 15: Warteschlange

**Files:**
- Create: `src/components/{QueuePanel,QueueRow}.tsx`
- Test: `src/components/QueuePanel.test.tsx`

**Interfaces:**
- Consumes: `useApp`, `backend.pickFiles/pickFolder/thumbnail`, `onFilesDropped`, `formatSize`, `formatDelta`.

Darstellung:
- Kopf „Warteschlange“ + Chip mit Anzahl („1 Datei“ / „N Dateien“), Knöpfe „Dateien“, „Ordner hinzufügen“, „Leeren“ (rot, während eines Laufs gesperrt).
- Spaltenkopf „Datei / Original / Ergebnis / Ersparnis (Enc) bzw. Änderung (Dec) / Status“, Spaltenraster `40px minmax(0,1fr) 80px 80px 72px 110px`.
- Liste mit `@tanstack/react-virtual` (Zeilenhöhe 54 px).
- Leere Liste → gestrichelte Ablagefläche „Bilder oder Ordner hierher ziehen“ mit Untertitel „JPEG, PNG, GIF, TIFF, WebP, BMP, EXR“ (Enc) bzw. „.jxl-Dateien“ (Dec).
- Statuschip: Wartet (grau), „N %“ (blau), Fertig (grün), Fertig mit Hinweis (grün + Info-Symbol), Übersprungen (grau), Fehler (rot), Prüfung fehlgeschlagen (rot). Der Hinweis- bzw. Fehlertext steht im HeroUI-`Tooltip`.
- Ergebnis/Ersparnis zeigen „—“, bis die Zeile fertig ist.
- Vorschaubilder werden nur für sichtbare Zeilen über `backend.thumbnail` geladen.
- Abgelehnte Dateien → Hinweis „N Dateien passen nicht zu diesem Modus und wurden nicht aufgenommen.“
- Drag & Drop gilt für das ganze Fenster.

- [ ] **Step 1: Fehlschlagende Tests** (mit `mockBackend`):
  - Leere Liste zeigt die Ablagefläche mit dem richtigen Untertitel je Modus.
  - Nach `addPaths` stehen 6 Zeilen da, und der Chip zeigt „6 Dateien“.
  - Eine wartende Zeile zeigt „—“ in Ergebnis und Ersparnis.
  - Eine fertige Zeile zeigt Größe und „−20 %“.
  - „Leeren“ ist während `run === "running"` deaktiviert.
  - Ein Klick auf eine Zeile setzt `selected`.
  - Bei 2 000 Einträgen stehen weniger als 60 Zeilen-Elemente im DOM.
  - Ein abgelehnter Drop zeigt den Hinweis mit der Anzahl.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen. **Step 4:** PASS. **Step 5: Commit** `feat: virtualized queue with drop zone and status chips`.

### Task 16: Einstellungen

**Files:**
- Create: `src/components/{SettingsPanel,EncodeSettings,DecodeSettings,CommonSettings}.tsx`
- Test: `src/components/SettingsPanel.test.tsx`

**Interfaces:**
- Consumes: `useApp`, `PRESETS`, `EFFORT_NAMES`, `enterExpert`, `distanceFromQuality`, `backend.pickFolder`.

Inhalt nach Spezifikation 3.3 und 3.4:
- Titel „Einstellungen“, Untertitel „Kodierung · nach JPEG XL“ bzw. „Dekodierung · aus JPEG XL“.
- **Enc einfach:** vier Stufen-Karten mit Untertiteln („empfohlen · pixelgenau“, „d 0.5“, „d 1.2“, „d 2.6“).
- **Enc Experte:** Slider „Qualität“ 30–100 mit Anzeige „q · Distanz x,xx“, Slider „Effort“ 1–9 mit Anzeige „7 · Squirrel“, Switch „Verlustfrei“ („Pixelgenau, ignoriert Qualität“).
- **Beide Enc-Ansichten:** Switch „JPEGs verlustfrei transkodieren“ („~20 % kleiner, bit-genau zurückwandelbar“).
- **Dec:** breite Karte „Automatisch“ („Original-JPEG, sonst PNG“) über einem 2×2-Raster PNG („verlustfrei“), JPEG („kompatibel“), WebP („Web“), TIFF („Druck“). Der Info-Kasten „Aus JPEG transkodierte Dateien werden bit-genau als Original-JPEG wiederhergestellt.“ erscheint bei Automatisch und JPEG. Experte: Slider „JPEG-Qualität“ 50–100 nur bei JPEG, Umschalter „Bittiefe“ 8 Bit / 16 Bit bei PNG und TIFF.
- **Gemeinsam:** Switch „Metadaten behalten“ („EXIF, XMP, Farbprofil“), Switch „Originale nach Prüfung in den Papierkorb“ („Nur bei verlustfreiem Ergebnis“). Ausgabeordner-Feld „Ausgabeordner · klicken zum Ändern“ öffnet ein HeroUI-`Dropdown` mit „Neben der Originaldatei“ und „Ordner wählen …“.
- Während eines Laufs sind alle Einstellungen gesperrt.

- [ ] **Step 1: Fehlschlagende Tests:**
  - Standard: „Verlustfrei“ ausgewählt, der Papierkorb-Schalter ist aus, „Automatisch“ ist im Dec-Modus ausgewählt.
  - Experte einschalten nach Wahl von „Ausgewogen“ → Qualität zeigt „88 · Distanz 1.18“ und Effort „7 · Squirrel“.
  - Im Dec-Modus mit JPEG und Experte erscheint der Slider „JPEG-Qualität“. Bei Automatisch erscheint er nicht, der Info-Kasten aber schon.
  - „Ordner wählen …“ ruft `pickFolder` auf und zeigt den Pfad.
  - Während `run === "running"` sind die Schalter deaktiviert.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen. **Step 4:** PASS. **Step 5: Commit** `feat: settings panel for both modes`.

### Task 17: Vorher/Nachher und Fußbereich

**Files:**
- Create: `src/components/{CompareSlider,RunFooter}.tsx`
- Test: `src/components/CompareSlider.test.tsx`, `src/components/RunFooter.test.tsx`

**Interfaces:**
- Consumes: `backend.preview`, `toJobSettings`, `ctaLabel`, `summaryLine`, `footerHint`, `backend.openLog`.

**CompareSlider:**
- Höhe 240 px, Kopf „Vorher / Nachher“ + „Name · Breite × Höhe“.
- Zwei Bilder übereinander, das rechte per `clip-path: inset(0 0 0 X%)` beschnitten.
- Ein Bereichs-Input über die ganze Fläche verschiebt die Trennlinie (Start 50 %), ein runder Griff „⇔“.
- Etiketten links „JPG · 6,80 MB“ und rechts „JXL · verlustfrei“ bzw. „JXL · d 1.2“, nach der Umwandlung „… · 5,44 MB“.
- Die Vorschau lädt beim Wechsel der Auswahl sofort und bei geänderten Einstellungen 300 ms verzögert (`viewW`/`viewH` = Größe des Feldes × `devicePixelRatio`).
- Ohne Auswahl ein leeres Feld mit „Keine Datei ausgewählt“.

**RunFooter:**
- Linke Zeile mit `footerHint` bzw. „N von M fertig · X → Y“.
- Rechts die Gesamt-Ersparnis (grün, wenn negativ) oder „–“.
- Gesamtfortschritt als HeroUI-`ProgressBar` (nach Bytes gewichtet).
- Start-Knopf mit `ctaLabel`, Höhe 48 px. Klick-Logik: idle → `start`, running → `pause`, paused → `resume`, finished mit Fehlern → `retryFailed`.
- Bei `pauseReason` vom Typ `diskFull` zusätzlich „Kein Platz mehr auf Laufwerk X:“.
- Nach dem Lauf erscheint die `summaryLine` mit dem Knopf „Protokoll öffnen“ (nur wenn `logPath` gesetzt ist).

- [ ] **Step 1: Fehlschlagende Tests:**
  - Der Slider verschiebt beim Input auf 30 die Trennlinie auf `left: 30%`.
  - Das rechte Etikett zeigt „JXL · verlustfrei“ bei Preset `ll` und „JXL · d 2.6“ bei `sm`.
  - Eine Einstellungsänderung löst nach 300 ms genau einen `preview`-Aufruf aus (Fake-Timer).
  - Der Footer zeigt vor der ersten fertigen Datei den Hinweistext.
  - Der Start-Knopf ruft je nach Zustand die richtige Aktion auf.
  - Nach `onRunFinished` mit `logPath` ruft „Protokoll öffnen“ `openLog(logPath)` auf.
  - Bei `diskFull` steht der Laufwerkshinweis da.
- [ ] **Step 2:** FAIL. **Step 3:** umsetzen. **Step 4:** PASS. **Step 5: Commit** `feat: compare slider and run footer`.

### Task 18: Abgleich mit dem Entwurf

**Files:**
- Modify: Komponenten aus Task 14–17 (nur Darstellung)
- Create: `docs/design/screens/` (Screenshots App vs. Entwurf)

- [ ] **Step 1:** `npm run dev` (Vite, simuliertes Backend) im Browser-Fenster mit 1200 × 760 öffnen. Den Entwurf `docs/design/jpeg-xl-konverter.dc.html` in einem zweiten Tab öffnen.
- [ ] **Step 2:** Je Zustand Screenshots von App und Entwurf nach `docs/design/screens/` speichern: hell/einfach/Enc mit 6 Dateien, dunkel, Experte an, Dec mit Automatisch, leere Liste, laufender Durchlauf.
- [ ] **Step 3:** Abweichungen bei Abständen, Rundungen, Schriftgrößen, Farben und Schatten beheben, bis die Paare nebeneinander gleich wirken. Ausgenommen sind die gewollten Änderungen aus Spezifikation 3.4 und das Logo.
- [ ] **Step 4:** `npm test` → alle Tests PASS.
- [ ] **Step 5: Commit** `style: match HeroUI design`.

### Task 19: Belastungstest, portabler Build, Abnahme-Anleitung

**Files:**
- Create: `scripts/package-portable.ps1`, `scripts/make-stress-set.ps1`, `src-tauri/examples/make_stress_set.rs` (bindet die Testhilfen per `#[path = "../tests/common/mod.rs"] mod common;` ein), `docs/ABNAHME.md`
- Modify: ggf. `default_workers()` in `queue.rs`

- [ ] **Step 1: `scripts/make-stress-set.ps1 -Count 1000 -Out <Ordner>`** erzeugt einen Ordner mit 1.000 Bildern: JPEGs, PNGs und einige TIFFs, je 2–12 MP, in Unterordnern. Das Skript ruft dafür das Testhilfe-Binary `cargo run --manifest-path src-tauri/Cargo.toml --example make_stress_set` auf, das die Funktionen aus Task 3 nutzt.
- [ ] **Step 2:** `npm run tauri dev`, den Ordner hineinziehen und umwandeln. Erwartet:
  - Die Liste scrollt flüssig.
  - Anhalten/Fortsetzen funktioniert.
  - Die Summen stimmen mit `(Get-ChildItem -Recurse *.jxl | Measure-Object Length -Sum)` überein.
  - Der Prozessor ist ausgelastet, die Oberfläche reagiert. Bei Bedarf `default_workers()` anpassen und den Wert in `docs/libjxl-notes.md` begründen.
- [ ] **Step 3: `scripts/package-portable.ps1`** führt `npm run tauri build -- --no-bundle` aus und legt `dist-portable/JPEG XL Konverter/` an mit `JPEG XL Konverter.exe`, `cjxl.exe`, `djxl.exe` und `LICENSES/` (`libjxl.txt` aus `src-tauri/binaries/LICENSE-libjxl`, `Lexend-OFL.txt` aus `node_modules/@fontsource-variable/lexend/LICENSE`). Erwartet: Die `.exe` startet aus dem Ordner, `check_tools` meldet ok, und es gibt keinen Netzwerkzugriff (Prüfung im Ressourcenmonitor).
- [ ] **Step 4: `docs/ABNAHME.md`:** Schritt-für-Schritt-Abnahme nach Spezifikation 7.4 mit Haken-Liste:
  1. Kopie eines Foto-Ordners anlegen.
  2. Durchlauf ohne Papierkorb.
  3. Durchlauf mit Papierkorb.
  4. Rückwandlung „Automatisch“ und Byte-Vergleich per `Get-FileHash`.
  5. USB-Stick-Test (Original bleibt, Hinweis erscheint).
  6. Erst danach das echte Archiv.
- [ ] **Step 5: Commit** `chore: stress test, portable build and acceptance checklist`.
