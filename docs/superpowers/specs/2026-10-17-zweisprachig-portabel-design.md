# JPEG XL Converter – Zweisprachigkeit, Programm-Einstellungen und USB-Portabilität

**Datum:** 2026-10-17
**Status:** Entwurf zur Durchsicht
**Baut auf:** `docs/superpowers/specs/2026-10-16-jpeg-xl-konverter-design.md` (gilt weiter, soweit hier nichts anderes steht)
**Oberflächenvorlage:** `JPEG XL Konverter UI.zip` → `docs/design/jpeg-xl-konverter.dc.html` (Stand 2026-10-17, enthält das Klappfenster „Programm-Einstellungen“ und die komplette Textliste `I18N` für Deutsch und Englisch)

Markierungen: **[Nutzer]** = so vom Nutzer entschieden · **[Annahme bestätigt]** = als Annahme vorgelegt und vom Nutzer bestätigt · **[Meine Entscheidung]** = vom Planer festgelegt, bei der Durchsicht prüfen · **[Builder]** = dem Umsetzenden überlassen.

## 1. Absicht

Die App soll auch für englischsprachige Menschen nutzbar sein, und das Projekt soll sich auf GitHub in erster Linie auf Englisch präsentieren, mit Deutsch als zweiter Sprache. Gleichzeitig wird die Titelleiste aufgeräumt: Die Schalter „Hell/Dunkel“ und „Experte“ wandern zusammen mit der neuen Sprachwahl in ein Klappfenster „Programm-Einstellungen“. Und die App soll **komplett lokal** bleiben und **USB-portabel** werden, also von einem USB-Stick laufen und ihre Einstellungen und Protokolle dort mitnehmen, ohne Spuren im Benutzerprofil des PCs. [Nutzer]

## 2. Ziele

1. **Oberfläche komplett auf Deutsch und Englisch**, jederzeit umschaltbar. Kein Bildschirm zeigt eine Mischung beider Sprachen. [Nutzer]
2. **Klappfenster „Programm-Einstellungen“** genau wie in der ZIP. [Nutzer]
3. **Alle sichtbaren Texte werden übersetzt**, auch die Hinweise und Fehlermeldungen an den Einträgen, die heute fest auf Deutsch vom Rust-Hintergrund kommen (rund 80 Stellen in `src-tauri/src/engine/*`, `commands.rs`). [Annahme bestätigt]
4. **Programmname nach außen „JPEG XL Converter“.** Im Fenstertitel steht je nach Sprache „JPEG XL Converter“ oder „JPEG XL Konverter“. [Annahme bestätigt]
5. **GitHub englisch zuerst, Deutsch daneben.** [Nutzer]
6. **Komplett lokal und USB-portabel.** [Nutzer]

## 3. Nicht-Ziele

- Weitere Sprachen. Die Struktur soll aber eine dritte Sprache ohne Umbau erlauben. [Meine Entscheidung]
- Eine Option „System“ beim Erscheinungsbild (die ZIP hat nur Hell/Dunkel). [Nutzer, über die ZIP]
- Übersetzung von Spezifikation, Plan und `docs/libjxl-notes.md`, denn das sind interne Arbeitsunterlagen. [Annahme bestätigt]
- Ein Installer. Die App bleibt ein portabler Ordner. [Spezifikation 2026-10-16]
- Mitliefern einer festen WebView2-Laufzeit. Windows 11 bringt WebView2 mit (siehe 7.4). [Meine Entscheidung]

## 4. Anti-Ziele (wäre ein Fehlschlag, selbst wenn es technisch läuft)

- Ein englischer Bildschirm mit einem deutschen Hinweis darin, oder umgekehrt.
- Die Sicherheitslogik (Prüfung, Papierkorb-Regeln) ändert durch den Umbau der Meldungen ihr Verhalten. Hier ändern sich nur **Texte**, keine Entscheidungen. Alle bestehenden Tests müssen unverändert grün bleiben, abgesehen von Stellen, die wörtlich einen deutschen Text erwartet haben.
- Die App schreibt bei Nutzung vom USB-Stick Einstellungen, Protokolle oder WebView-Daten in das Benutzerprofil des PCs (siehe 7.3 für die einzige Ausnahme).
- Die App nimmt Netzwerkverbindungen auf (das gilt schon heute, CSP `connect-src ipc:`).
- Vorhandene Einstellungen gehen beim Update verloren. Eine `settings.json` ohne Sprachfeld wird wie ein erster Start behandelt (Sprache aus Windows), alles andere bleibt erhalten.

## 5. Programm-Einstellungen (Klappfenster)

Vorlage: ZIP, Block `prefsOpen` und `prefRows`. [Nutzer]

- **Titelleiste:** Die Schalter „Hell/Dunkel“ und „Experte“ entfallen. An ihrer Stelle steht **ein Knopf mit Schieberegler-Symbol** (38 × 38 px, Radius 10, SVG mit drei Linien und Knöpfen wie in der ZIP) links von den Fensterknöpfen. Tooltip: „Programm-Einstellungen“ / „App settings“. Ist das Klappfenster offen, wird der Knopf dunkler hinterlegt (`d200` statt `d100`).
- **Klappfenster:** Es öffnet sich direkt unter dem Knopf, rechtsbündig dazu (in der ZIP `top:58px; right:146px` im 1200 px breiten Fenster). Breite 300 px, Radius 14, Rand `d200`, Schatten `0 12px 40px rgba(0,0,0,.18)`, Innenabstand 16 px, ohne Abdunkeln. Ein Klick irgendwo daneben schließt es, **Esc** ebenfalls (Esc ist [Meine Entscheidung]). Einen „Fertig“-Knopf gibt es nicht.
- **Inhalt:** Titel „Programm-Einstellungen“ / „App settings“, darunter drei Zeilen, jede mit kleiner grauer Beschriftung und einem Umschalter aus zwei gleich breiten Knöpfen:

  | Beschriftung DE / EN | Knöpfe DE / EN |
  |---|---|
  | Erscheinungsbild / Appearance | Hell · Dunkel / Light · Dark |
  | Modus / Mode | Normal · Experte / Normal · Expert |
  | Sprache / Language | Deutsch · English (immer so, in beiden Sprachen) |

- **Verhalten:** Jede Änderung wirkt sofort und wird gespeichert. Während ein Durchlauf läuft oder angehalten ist, ist „Modus“ gesperrt (wie die heutigen Einstellungen). Erscheinungsbild und Sprache bleiben umschaltbar. [Annahme bestätigt]
- Beim Wechsel auf „Experte“ übernehmen die Regler die Werte der gewählten Stufe (`enterExpert`, wie heute).
- Bedienbar per Tastatur (Tab, Enter/Leertaste, Esc). [Meine Entscheidung]

## 6. Sprache

### 6.1 Erster Start und Speichern

- `UiSettings` erhält `lang: "de" | "en" | null`. `null` bedeutet „noch nicht gewählt“.
- Bei `null` richtet sich die App nach der **Windows-Anzeigesprache**: beginnt sie mit `de`, gilt Deutsch, sonst Englisch. Kann sie nicht ermittelt werden, gilt Englisch. [Annahme bestätigt]
- Die Wahl im Klappfenster wird gespeichert (dann nie wieder `null`). Bei USB-Nutzung liegt sie auf dem Stick (siehe 7) und gewinnt damit auf jedem PC gegen die Windows-Sprache. [Meine Entscheidung, folgt aus „USB-portabel“]
- Wie die Windows-Sprache ermittelt wird (zum Beispiel `navigator.language` im WebView2 oder `GetUserDefaultUILanguage` in Rust): [Builder]. Es muss nur ein Test die Zuordnung `de`, `de-DE`, `de-AT` → Deutsch und `en-US`, `fr-FR`, leer → Englisch abdecken.
- `restoreSettings` prüft `lang` wie die übrigen Felder; ein ungültiger Wert wird zu `null`.

### 6.2 Texte

- **Eine Quelle für alle Texte. [Meine Entscheidung]** Es gibt zwei Katalogdateien `src/i18n/de.json` und `src/i18n/en.json` mit identischen Schlüsseln. Die Oberfläche liest sie direkt. Der Rust-Hintergrund bindet dieselben Dateien beim Kompilieren ein (z. B. `include_str!`) und nutzt daraus nur, was er selbst schreiben muss (Protokolldatei). So gibt es keine zweite, auseinanderlaufende Textliste. Die Einteilung in Abschnitte (UI, Hinweise, Fehler, Protokoll …) ist [Builder].
- **Startwerte der Texte:** die `I18N`-Liste aus der ZIP. Ausnahmen, bei denen die bereits beschlossenen Texte gelten (die ZIP ist dort älter):

  | Stelle | Deutsch (gilt) | Englisch (neu) |
  |---|---|---|
  | Ablagehinweis „In JPEG XL“ | JPEG, PNG, GIF, TIFF, WebP, BMP | JPEG, PNG, GIF, TIFF, WebP, BMP |
  | Untertitel Papierkorb | Nur bei verlustfreiem Ergebnis | Only if the result is lossless |
  | Start-Knopf nach Lauf mit Fehlern | Fehlgeschlagene erneut versuchen | Retry failed |
  | Start-Knopf nach Lauf ohne Fehler | Alles erledigt | All done |
  | Singular | 1 Datei in JPEG XL konvertieren / 1 Datei zurückkonvertieren | Convert 1 file to JPEG XL / Convert 1 file back |

  Alle weiteren Texte, die die ZIP nicht enthält (Statuschips, Hinweise, Fehler, Rückfrage beim Schließen, Zusammenfassung, Hinweisbalken, Ausgabeordner-Menü, Dialogtitel, Expertenregler, „Keine Datei ausgewählt“, „Vorschau nicht möglich“ usw.), übersetzt der Builder ins Englische. Dabei gelten die Begriffe der ZIP: Queue, Lossless, Recycle Bin, Output folder, Keep metadata, Savings, Change, Done, Queued, Stop. [Builder]
- **Statuswörter Englisch:** Queued, Running (`N %`), Done, Done with note, Skipped, Error, Check failed. [Meine Entscheidung]
- **Ein Test stellt sicher**, dass beide Kataloge exakt dieselben Schlüssel haben und kein Wert leer ist. Ein fehlender Schlüssel im Code soll beim Kompilieren (TypeScript) bzw. in einem Test (Rust) auffallen.

### 6.3 Meldungen aus dem Rust-Hintergrund

- **Hinweise und Fehler werden zu Kennungen. [Annahme bestätigt]** Statt deutscher Sätze liefert der Hintergrund eine Kennung plus Parameter (z. B. Laufwerksbuchstabe, Grund). Die Oberfläche setzt den Text in der aktuellen Sprache zusammen. Wechselt die Sprache, erscheinen auch schon angezeigte Hinweise sofort in der neuen Sprache.
- Betroffen sind mindestens `Outcome.note` bzw. `ItemEvent.note` (heute ein String, mehrere Hinweise mit „ · “ verbunden), Fehlergründe der Pipelines, `ToolStatus.message` und die Fehlertexte der Tauri-Befehle. Die genaue Form (z. B. `notes: Vec<Message>` mit `{ code, params }`) ist [Builder]. Die Oberfläche verbindet mehrere Hinweise weiterhin mit „ · “.
- **Verhalten unverändert:** Wann welcher Hinweis entsteht und welche Entscheidung (Papierkorb ja/nein, Status) daraus folgt, bleibt exakt wie heute. Tests, die heute deutsche Texte prüfen, prüfen künftig die Kennung.
- Die Serde-Verträge (`src-tauri/src/engine/types.rs` ↔ `src/api/types.ts`) werden angepasst. Der bestehende JSON-Formtest wird entsprechend erweitert.

### 6.4 Protokolldatei

- Sie wird in der Sprache geschrieben, die **beim Start des Durchlaufs** eingestellt war. Dafür bekommt `start_run` (und `retry_failed`) die Sprache mit. [Annahme bestätigt]
- Inhalt und Aufbau wie heute, nur übersetzt: Einstellungen, Summenzeile, je Problemfall `Status<TAB>Pfad<TAB>Grund`.
- Dateiname bleibt `JJJJ-MM-TT_HH-MM-SS.txt`. Speicherort siehe 7.

### 6.5 Zahlen und Formate

- Deutsch wie bisher: „6,80 MB“, „9.812 fertig“, „−20 %“.
- Englisch: „6.80 MB“, „9,812 done“, „−20 %“. Distanzwerte („d 1.2“, „Distance 1.18“) haben in beiden Sprachen einen Punkt.
- `src/lib/format.ts` bekommt die Sprache als Parameter (oder liest sie aus dem Store). Die bestehenden deutschen Erwartungswerte bleiben gültig, englische kommen dazu.

### 6.6 Native Dialoge und Fenstertitel

- Titel der Datei- und Ordnerauswahl in der aktuellen Sprache (das Dialog-Plugin nimmt einen Titel an).
- Fenstertitel: Deutsch „JPEG XL Konverter“, Englisch „JPEG XL Converter“. Er wird beim Sprachwechsel zur Laufzeit gesetzt. Die dafür nötige Tauri-Berechtigung (`core:window:allow-set-title`) kommt in `capabilities/default.json`. **Keine andere Berechtigung entfernen** (siehe Folgefehler beim letzten Mal: `core:window:allow-destroy` wird zum Schließen gebraucht).
- Die Schaltflächen der Windows-Dialoge selbst („Öffnen“, „Abbrechen“) kommen von Windows und folgen der Windows-Sprache. Das ist so in Ordnung.

## 7. Komplett lokal und USB-portabel

### 7.1 Datenordner neben dem Programm [Meine Entscheidung]

Alles, was die App dauerhaft speichert, liegt in einem Ordner **`data\` neben `JPEG XL Converter.exe`**:

| Inhalt | heute | künftig |
|---|---|---|
| Einstellungen (`settings.json`) | App-Konfigurationsordner unter `%APPDATA%` | `data\settings.json` |
| Fenstergröße/-position (window-state) | App-Konfigurationsordner unter `%APPDATA%` | `data\` (eigene Speicherung oder Plugin-Pfad umgelenkt, [Builder]) |
| Protokolle | `%LOCALAPPDATA%\JPEG XL Konverter\Protokolle` | `data\logs\` |
| WebView2-Benutzerdaten (Cache, LocalStorage) | `%LOCALAPPDATA%\<identifier>\EBWebView` | `data\webview\` (Tauri: Datenverzeichnis des Webviews setzen) |

`open_log` erlaubt künftig nur Pfade innerhalb von `data\logs\` (bisher der alte Protokollordner).

### 7.2 Nicht beschreibbarer Programmordner [Meine Entscheidung]

Liegt das Programm an einem schreibgeschützten Ort (z. B. schreibgeschützter Stick oder `C:\Program Files`), weicht die App auf `%LOCALAPPDATA%\JPEG XL Converter\` mit denselben Unterordnern aus und zeigt einmalig einen Hinweis im Hinweisbalken: „Programmordner ist schreibgeschützt – Einstellungen werden auf diesem PC gespeichert.“ / „App folder is read-only – settings are stored on this PC.“

### 7.3 Was auf dem PC bleibt

- **Zwischendateien** liegen weiterhin im Temp-Ordner des PCs (`%TEMP%\jpeg-xl-konverter\<pid>`), weil das auf einem USB-Stick deutlich langsamer wäre. Sie werden beim Beenden gelöscht, wie heute. [Meine Entscheidung]
- Die umgewandelten Dateien landen dort, wo der Nutzer es einstellt (neben dem Original oder im gewählten Ausgabeordner).

### 7.4 Voraussetzungen

- Windows 11 (x64). WebView2 ist dort fest eingebaut, deshalb wird keine eigene Laufzeit mitgeliefert. Fehlt WebView2 doch, zeigt Windows/Tauri seine Standardmeldung. Das wird in der README erwähnt.
- Keine Installation, keine Registry-Einträge der App selbst (das Lesen der Papierkorb-Einstellungen bleibt wie heute).
- Alte Daten im Benutzerprofil (bisherige `settings.json`, alte Protokolle) werden **nicht** übernommen oder gelöscht. Beim ersten Start der neuen Version gelten Standardwerte, die Sprache kommt aus Windows. [Meine Entscheidung; der Nutzer hat die App bisher nur testweise benutzt]

## 8. Programmname

- `productName` und der Name der `.exe` im portablen Ordner: **„JPEG XL Converter“** → `JPEG XL Converter.exe`, Ordner `dist-portable\JPEG XL Converter\`. [Annahme bestätigt]
- Der Tauri-`identifier` bleibt `de.bassboost1981.jpegxlkonverter`. Wegen 7.1 hängt daran ohnehin kein Speicherort mehr. [Meine Entscheidung]
- Der Cargo-Paketname und der Lib-Name bleiben, um den Umbau klein zu halten. [Meine Entscheidung]
- `scripts/package-portable.ps1` erzeugt den neuen Ordner- und Exe-Namen und legt **keinen** `data\`-Ordner an (der entsteht beim ersten Start).

## 9. GitHub und Unterlagen

- `README.md` wird **Englisch** (Startseite), der heutige deutsche Inhalt zieht nach **`README.de.md`**. Oben in beiden steht ein Sprachumschalter „**English** | [Deutsch](README.de.md)“ bzw. „[English](README.md) | **Deutsch**“. Inhaltlich sind beide gleich und an die neue Version angepasst: Programmname, USB-Portabilität, `data\`-Ordner, Sprachwahl im Klappfenster. [Annahme bestätigt]
- **Screenshots je Sprache**, hell und dunkel (über `<picture>` passend zum GitHub-Design): `docs/screenshots/app-en-light.png`, `app-en-dark.png`, `app-de-hell.png`, `app-de-dunkel.png`. Die bisherigen zwei Dateien werden ersetzt. Aufgenommen wie zuletzt aus der Oberfläche im Demo-Modus mit den Beispieldaten, 1200 × 760 in doppelter Auflösung. Zusätzlich ein Screenshot mit **geöffnetem Klappfenster** je Sprache (hell) für den Abschnitt „Settings/Einstellungen“ der README. [Meine Entscheidung]
- Das Skript, mit dem die Screenshots entstehen (Edge ohne Fenster plus DevTools-Protokoll), kommt nach `scripts/readme-screenshots.mjs`, damit man sie bei Änderungen neu erzeugen kann. [Meine Entscheidung]
- **Repo-Beschreibung** auf Englisch: „Windows app to convert images to JPEG XL and back – JPEGs losslessly transcoded and verified byte for byte; originals only go to the Recycle Bin after a passed check. Portable, fully offline.“ [Annahme bestätigt, Zusatz „Portable, fully offline“ ist meine Entscheidung]
- **Abnahme:** `docs/ABNAHME.md` bleibt deutsch, `docs/ACCEPTANCE.md` kommt als englische Fassung dazu. Beide werden an den neuen Programmnamen, den `data\`-Ordner und einen zusätzlichen Schritt „von einem USB-Stick starten, Einstellungen bleiben auf dem Stick, auf dem PC entsteht nichts im Benutzerprofil“ angepasst.
- `docs/design/` enthält den neuen Entwurfsstand (bereits auf diesem Branch).

## 10. Ablauf

- Arbeit auf dem Branch `feature/zweisprachig`. Am Ende wird in `main` zusammengeführt und der Branch auf GitHub und lokal gelöscht, sodass auf GitHub nur `main` bleibt. [Nutzer]
- Hochladen und Zusammenführen erst nach Zustimmung des Nutzers.

## 11. Tests

- Alle bestehenden Tests bleiben grün. Angepasst werden nur Stellen, die wörtlich deutsche Texte erwarten (künftig: Kennung oder Text aus dem Katalog).
- Neu:
  - Beide Kataloge haben dieselben Schlüssel, kein Wert ist leer (TS und Rust).
  - Jede Hinweis- und Fehlerkennung, die der Hintergrund erzeugen kann, hat in beiden Sprachen einen Text.
  - Sprachermittlung beim ersten Start (siehe 6.1), `restoreSettings` mit `lang`.
  - Oberfläche in Englisch: Stichproben in jeder Komponente (Titel, Knöpfe, Statuschips, Hinweise, Start-Knopf-Texte, Zahlenformat).
  - Sprachwechsel übersetzt bereits angezeigte Hinweise.
  - Klappfenster: öffnet/schließt (Knopf, Klick daneben, Esc), drei Zeilen, Modus gesperrt während Lauf, Fenstertitel wird gesetzt.
  - Protokolldatei auf Englisch, wenn der Lauf auf Englisch gestartet wurde.
  - Datenordner: `data\` neben der Exe wird benutzt; bei nicht beschreibbarem Programmordner wird auf `%LOCALAPPDATA%\JPEG XL Converter\` ausgewichen.
- Am Ende: portabler Build, einmal von einem anderen Laufwerk starten und prüfen, dass `data\` neben der Exe entsteht und im Benutzerprofil kein neuer App-Ordner angelegt wurde (abgesehen von Temp). Das Fenster muss sich schließen lassen.

## 12. Dem Builder überlassen

- Wie die Kataloge geladen und typisiert werden, und die Gliederung der Schlüssel.
- Die genaue Rust-Form der Meldungen (Kennung + Parameter).
- Die Ermittlung der Windows-Sprache.
- Wie window-state nach `data\` umgelenkt wird (Plugin-Option oder eigene kleine Speicherung).
- Englische Formulierungen für Texte, die die ZIP nicht enthält, im Ton der ZIP.
