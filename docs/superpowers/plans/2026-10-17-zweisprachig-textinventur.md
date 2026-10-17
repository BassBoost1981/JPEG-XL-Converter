# Anhang: Textinventur (Stand 2026-10-17, Branch `feature/zweisprachig`)

Gehört zu `2026-10-17-zweisprachig-portabel.md`. `R:` = `src-tauri/src/`. Zeilennummern zum Zeitpunkt der Inventur, können sich verschieben.

## A. Rust: sichtbare deutsche Texte

**Wie sie heute zur Oberfläche kommen:** `Outcome.note: Option<String>` → `ItemEvent.note` (Ereignis `item-progress`) → `store.applyEvent` → `Row.note` → Tooltip am Statuschip. Dieselbe Zeichenkette ist die „Grund“-Spalte im Protokoll. Mehrere Hinweise werden **nur** in `R:engine/pipeline_util.rs` `finished()` mit `" · "` verbunden (`NOTE_SEPARATOR`).

| Datei | Texte (Konstante bzw. Zeile) | Weg |
|---|---|---|
| `R:commands.rs` | `TOOLS_MISSING` „cjxl.exe oder djxl.exe fehlt neben dem Programm.“ · `UNKNOWN_ITEM` „Eintrag nicht gefunden“ · `OUTSIDE_LOG_DIR` „Nur Protokolldateien lassen sich öffnen“ · `INTERNAL` „Interner Fehler“ · „Einstellungen lassen sich nicht speichern: {e}“ · „Protokoll lässt sich nicht öffnen: {e}“ | Befehlsfehler `Err(String)`, `ToolStatus.message` |
| `R:engine/queue.rs` | `TARGET_UNKNOWN` „Zielpfad lässt sich nicht bestimmen“ · `CLEAR_WHILE_RUNNING` „Die Warteschlange lässt sich während eines Durchlaufs nicht leeren“ | Hinweis (Error) · Befehlsfehler |
| `R:engine/pipeline_util.rs` | `TARGET_EXISTS` „Zieldatei existiert bereits“ · `NOTE_METADATA` „Metadaten nicht vollständig übernommen“ · `NOTE_DATES` „Dateidatum nicht übernommen“ · „Datei nicht gefunden“ · „Datei wird von einem anderen Programm verwendet“ · „Datei lässt sich nicht öffnen“ · „Alte Zwischendatei lässt sich nicht löschen“ · „Zielordner lässt sich nicht anlegen“ · „Arbeitsordner lässt sich nicht anlegen“ · „Zieldatei lässt sich nicht anlegen“ | Hinweis |
| `R:engine/encode.rs` | `NOT_TRANSCODABLE` „JPEG lässt sich nicht verlustfrei umpacken“ · `NOTE_FIRST_PAGE` „Nur die erste Seite übernommen – Original behalten“ · `NOTE_ANIMATION` „Animation – Original behalten“ · `READ_ERROR` „Bild lässt sich nicht lesen“ · „Umwandlung nach JPEG XL fehlgeschlagen“ · „JPEG lässt sich nicht wiederherstellen“ · „Wiederhergestelltes JPEG weicht vom Original ab“ · „JPEG XL lässt sich nicht dekodieren“ · „Format wird hier nicht unterstützt“ · „Inhalt passt nicht zur Dateiendung“ · „Pixelvergleich nicht möglich“ · „Pixel weichen vom Original ab“ · „Abmessungen lassen sich nicht prüfen“ · „Abmessungen stimmen nicht“ · „Metadaten lassen sich nicht prüfen“ | Hinweis |
| `R:engine/decode.rs` | `NOTE_ANIMATION` „Animation – .jxl behalten“ · `NOTE_NOT_EXACT` „Gleitkomma- oder Mehrkanal-Bild – .jxl behalten“ · `DECODE_FAILED` „Umwandlung aus JPEG XL fehlgeschlagen“ · `READ_ERROR` „JPEG XL lässt sich nicht lesen“ · „Ergebnis lässt sich nicht lesen“ · „Abmessungen stimmen nicht“ · „Pixelvergleich nicht möglich“ · „Pixel weichen vom Original ab“ | Hinweis |
| `R:engine/convert_io.rs` | „Datei lässt sich nicht öffnen“ · „Bild lässt sich nicht lesen“ · „Datei lässt sich nicht schreiben“ · „Datei lässt sich nicht bearbeiten“ · „Bild ist zu groß zum Umwandeln“ · „Ausgabeformat wird nicht unterstützt“ · „Format wird hier nicht unterstützt“ · „CMYK-Bilder werden nicht unterstützt“ · „Gleitkomma-Bilder werden nicht unterstützt“ · „Bild ist für WebP zu groß (höchstens {WEBP_MAX_SIDE} Pixel pro Seite)“ · „WebP-Kodierung fehlgeschlagen“ | `Result<_, String>` → Hinweis |
| `R:engine/originals.rs` | „Original behalten: kein Papierkorb auf diesem Laufwerk“ · „Original behalten: zu groß für den Papierkorb“ · „Original behalten: Papierkorb nicht möglich ({reason})“ mit Gründen: „Keine normale Datei“, „Interner Fehler“, „Datei ist schreibgeschützt“, „Datei nicht gefunden“, „Datei nicht lesbar“, „Vorgang abgebrochen“, „Datei ist noch vorhanden“, „Ergebnis nicht prüfbar“, „Zugriff verweigert“, „Datei wird von einem anderen Programm verwendet“, „Windows-Fehler 0x{:08X}“ · `FakeRecycleBin` „Testfehler“ | `handle_original -> Option<String>`, `RecycleBin::recycle -> Result<(), String>` |
| `R:engine/preview.rs` | „Vorschau nicht möglich“ · „Vorschau nicht möglich: Bild nicht unterstützt oder zu groß“ · „Vorschau nicht möglich: Datei passt nicht zum Modus“ | Befehlsfehler (Oberfläche zeigt eigenen Text) |
| `R:engine/tools.rs` | „{name} startet nicht: {e}“ · „{name} antwortet nicht.“ · „{name} liefert keine Versionsangabe.“ (mit `" "` verbunden) | `ToolStatus.message` |
| `R:engine/report.rs` | Kopf „JPEG XL Konverter – Protokoll“ · „In JPEG XL umwandeln“/„Aus JPEG XL zurückwandeln“ · „Modus: …“ · „Einstellungen: …“ · „Summe: {done} fertig, {skipped} übersprungen, {errors} Fehler, {in} Bytes → {out} Bytes“ · „ja“/„nein“ · „Distanz …, Aufwand …, JPEG verlustfrei umpacken: …“ · „Automatisch“ · „Format …, JPEG-Qualität …, auf 8 Bit reduzieren: …“ · „neben dem Original“ · „…, Metadaten behalten: …, Originale in den Papierkorb: …, Ziel: …“ · Statuswörter „Übersprungen“, „Fehler“, „Prüfung fehlgeschlagen“, „Fertig mit Hinweis“ | Protokolldatei |

Nie sichtbar (dürfen bleiben, nur intern): `R:engine/verify.rs:67,70`, `R:engine/jxl_container.rs:85`, deutsche Namen von Arbeitsdateien (`quelle.png`, `pruefung.*`, `zwischen.png`, `vorschau…`).

**Tests, die heute deutsche Texte prüfen (51 Stellen):** `#[cfg(test)]` in `tools.rs` (1), `verify.rs` (3, dürfen bleiben), `originals.rs` (9), `report.rs` (6), `queue.rs` (3); `src-tauri/tests/convert_io.rs` (19), `decode_roundtrip.rs` (3), `encode_roundtrip.rs` (7).

**Träger-Typen:** `Outcome.note`, `ItemEvent.note` (`R:engine/types.rs:105-122`), `ToolStatus.message` (`types.rs:142-149`), `Stop { Skipped(&'static str), Error(String), VerifyFailed(&'static str), Cancelled }` (`pipeline_util.rs:55-60`), `finished(target, notes: Vec<String>)` (`pipeline_util.rs:211-222`), `RunReport.problems: Vec<(ItemStatus, PathBuf, String)>` (`report.rs:18`), `RecycleBin::recycle`, `handle_original`, Befehle `Result<_, String>`.

## B. TypeScript: sichtbare deutsche Texte (ohne Tests)

| Datei | Texte |
|---|---|
| `src/App.tsx` | „Wirklich beenden?“ · „Laufende Umwandlungen werden abgebrochen.“ · „Beenden“ · „Weiterarbeiten“ |
| `src/components/TitleBar.tsx` | `alt` „JPEG XL Konverter“ · `aria-label` „Richtung“ · „In JPEG XL“ · „Aus JPEG XL“ · „Dunkel“/„Hell“ · „Experte“ · `aria-label` „Minimieren“, „Maximieren“, „Schließen“ |
| `src/components/ToolsBanner.tsx` | „cjxl.exe oder djxl.exe fehlt neben dem Programm.“ (Kopie des Rust-Texts) |
| `src/components/QueuePanel.tsx` | „JPEG, PNG, GIF, TIFF, WebP, BMP“ · „.jxl-Dateien“ · „Warteschlange“ · „1 Datei“/„{n} Dateien“ · „Dateien“ · „Ordner hinzufügen“ · „Leeren“ · Spalten „Datei“, „Original“, „Ergebnis“, „Ersparnis“/„Änderung“, „Status“ · „Bilder oder Ordner hierher ziehen“ |
| `src/components/QueueRow.tsx` | „Wartet“ · „{n} %“ · „Fertig“ · „Fertig mit Hinweis“ · „Übersprungen“ · „Fehler“ · „Prüfung fehlgeschlagen“ |
| `src/components/RunFooter.tsx` | `aria-label` „Gesamtfortschritt“ · „Kein Platz mehr auf Laufwerk {drive}“ · „Protokoll öffnen“ · **Logik hängt an Text:** `label === "Alles erledigt"` (deaktiviert), `saved.startsWith("−")` (Farbe) |
| `src/components/SettingsPanel.tsx` | „Einstellungen“ · „Kodierung · nach JPEG XL“/„Dekodierung · aus JPEG XL“ |
| `src/components/EncodeSettings.tsx` | „empfohlen · pixelgenau“ · „d 0.5/1.2/2.6“ · „Qualität“ · „{q} · Distanz {d}“ · „Effort“ · „Verlustfrei“ · „Pixelgenau, ignoriert Qualität“ · „JPEGs verlustfrei transkodieren“ · „~20 % kleiner, bit-genau zurückwandelbar“ |
| `src/components/DecodeSettings.tsx` | „verlustfrei“, „kompatibel“, „Web“, „Druck“ · `aria-label` „Ausgabeformat“ · „Zielformat“ · „Automatisch“ · „Original-JPEG, sonst PNG“ · „Aus JPEG transkodierte Dateien werden bit-genau als Original-JPEG wiederhergestellt.“ · „JPEG-Qualität“ · „Bittiefe“ · „{d} Bit“ |
| `src/components/CommonSettings.tsx` | „Metadaten behalten“ · „EXIF, XMP, Farbprofil“ · „Ausgabeordner · klicken zum Ändern“ · „Neben der Originaldatei“ · „Ordner wählen …“ · „Originale nach Prüfung in den Papierkorb“ · „Nur bei verlustfreiem Ergebnis“ |
| `src/components/CompareSlider.tsx` | `auto: "Original"` · „JXL · verlustfrei“ · „Vorher / Nachher“ · „Keine Datei ausgewählt“ · `alt` „Vorher“/„Nachher“ · „Vorschau nicht möglich“ · `aria-label` „Trennlinie“ |
| `src/state/settings.ts` | `PRESETS[*].label` „Verlustfrei“, „Hohe Qualität“, „Ausgewogen“, „Klein“ (EFFORT_NAMES sind englische Codec-Namen, bleiben) |
| `src/state/store.ts` | „1 Datei passt nicht zu diesem Modus und wurde nicht aufgenommen.“ · „{n} Dateien passen nicht zu diesem Modus und wurden nicht aufgenommen.“ · `errorMessage(e)` zeigt Backend-Fehlertexte roh als `notice` |
| `src/api/tauriBackend.ts` | Dialogfilter „Bilder“ · kein Dialogtitel gesetzt |
| `src/api/mockBackend.ts` | Hinweis „Zieldatei existiert bereits“ |
| `src/lib/format.ts` | `Intl.NumberFormat("de-DE")` · Dezimalkomma in `formatSize` · `summaryLine` „{n} fertig · {n} übersprungen · {n} Fehler · {in} → {out}“ · `footerHint` „Ersparnis erscheint nach der ersten fertigen Datei“ / „{n} von {total} fertig · …“ · `ctaLabel` „Anhalten“, „Fortsetzen“, „Fehlgeschlagene erneut versuchen“, „Alles erledigt“, „{n} Datei(en) in JPEG XL konvertieren“, „… zurückkonvertieren“ |
| `index.html` | `lang="de"` · `<title>JPEG XL Konverter</title>` |

Englische Vorlagen: `docs/design/jpeg-xl-konverter.dc.html`, Objekt `I18N` (Zeilen ~196–206).

## C. Pfade und Fenster heute

- Einstellungen: `R:lib.rs:41` `app_config_dir()` (`%APPDATA%\de.bassboost1981.jpegxlkonverter\settings.json`), `R:settings_store.rs`.
- Fensterzustand: Plugin `tauri-plugin-window-state` (`R:lib.rs:26`), Datei `.window-state.json` im selben Ordner; Berechtigung `window-state:default`.
- Protokolle: `R:lib.rs:15-20` `%LOCALAPPDATA%\JPEG XL Konverter\Protokolle`; `open_log` erlaubt nur diesen Ordner.
- Arbeitsordner: `R:lib.rs:28-31` `%TEMP%\jpeg-xl-konverter\<pid>`, gelöscht bei `RunEvent::Exit` nach `cancel_and_wait(3 s)`.
- WebView2-Daten: nicht gesetzt → `%LOCALAPPDATA%\<identifier>\EBWebView`.
- Fenster: `src-tauri/tauri.conf.json` `app.windows[0]` (Titel „JPEG XL Konverter“, 1200×760, min 1000×640, `decorations:false`). `productName` „JPEG XL Konverter“.
- Berechtigungen (`src-tauri/capabilities/default.json`): `core:default`, `core:window:allow-minimize`, `allow-toggle-maximize`, `allow-close`, `allow-destroy`, `allow-start-dragging`, `dialog:default`, `window-state:default`.
