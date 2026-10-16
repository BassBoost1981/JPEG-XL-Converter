# JPEG XL Konverter – Design-Spezifikation

**Datum:** 2026-10-16
**Status:** Entwurf zur Durchsicht
**Plattform:** Windows 11 (x64)

## 1. Ziel und Zweck

Ein Windows-Programm mit Fenster, das Bilddateien nach JPEG XL (`.jxl`) umwandelt und JPEG XL wieder zurück in andere Formate.

Zwei Einsatzzwecke:

1. **Foto-Archiv verkleinern:** Große Mengen JPEGs werden verlustfrei nach JPEG XL umgepackt. Die Dateien werden etwa 20 % kleiner, und jedes Original-JPEG lässt sich jederzeit Byte für Byte wiederherstellen.
2. **Gelegentliche Einzelumwandlungen:** Einzelne Bilder werden in JPEG XL umgewandelt oder zurück, zum Beispiel zum Verschicken.

**Oberste Regel:** Es darf nie ein Bild verloren gehen. Originale werden nur angefasst, wenn die neue Datei geprüft ist und das Ergebnis verlustfrei ist.

## 2. Getroffene Entscheidungen

| Thema | Entscheidung |
|---|---|
| Bedienung | Programmfenster mit Drag & Drop für Dateien und Ordner (inkl. Unterordner) und Fortschritt pro Datei |
| JPEG → JXL | Verlustfrei umpacken (Schalter „Transkodieren“, standardmäßig an), Prüfung Byte für Byte |
| Originale | Einstellung „Originale nach Prüfung in den Papierkorb“, standardmäßig **aus**. Niemals endgültig löschen |
| Andere Formate → JXL | Qualität wählbar, Voreinstellung **Verlustfrei** |
| JXL → zurück | **Automatisch** (Voreinstellung): Original-JPEG wenn möglich, sonst PNG. Außerdem PNG, JPEG, WebP, TIFF |
| Zieldatei existiert | Überspringen, nie überschreiben |
| Metadaten | EXIF, XMP und Farbprofil bleiben erhalten (Schalter, Standard an). Erstellungs- und Änderungsdatum werden immer übernommen |
| Technik | Tauri 2 + React + TypeScript + HeroUI v3 + Tailwind CSS v4, Hintergrund in Rust |
| Schrift | Lexend als Standardschrift der ganzen App, lokal eingebunden (funktioniert offline) |
| Umwandlungs-Engine | Offizielle libjxl-Programme `cjxl.exe` und `djxl.exe` (v0.12.0, Windows-x64-Static-Build), als Tauri-Sidecars |
| Oberflächensprache | Deutsch |
| Auslieferung | Portabler Ordner: Programm-`.exe` + `cjxl.exe` + `djxl.exe`, kein Installer |
| Protokoll | Pro Durchlauf eine Textdatei mit allen Problemfällen unter `%LOCALAPPDATA%\JPEG XL Konverter\Protokolle\` |

## 3. Oberfläche

### 3.1 Vorlage

Grundlage ist der Claude-Design-Entwurf `JPEG XL Konverter UI.zip` im Projektordner (Datei `JPEG XL Konverter.dc.html`, Variante **„2a HeroUI · Hell / Dunkel“**). Er gilt für Aufbau, Abstände, Farben und Texte, soweit diese Spezifikation nichts anderes sagt.

- Akzentfarbe `#006FEE`, helles und dunkles Farbschema wie im Entwurf
- Schrift Lexend; Zahlen in Tabellen und Summen mit gleich breiten Ziffern (`tabular-nums`)
- Fenstergröße zu Beginn 1200 × 760, Mindestgröße 1000 × 640
- Eigene Titelleiste wie im Entwurf (Logo „XL“, Modus-Umschalter, Schalter Hell/Dunkel und Experte, Fensterknöpfe); das Fenster lässt sich an der Titelleiste verschieben

Alle Bedienelemente, die es in HeroUI v3 gibt, werden mit HeroUI gebaut (u. a. Tabs, Switch, Slider, Button, Card, Chip, ProgressBar, Tooltip). Selbst gebaut im gleichen Stil werden nur der **Vorher/Nachher-Schieber** und die **Titelleiste**.

### 3.2 Bereiche

1. **Titelleiste:** Umschalter „In JPEG XL / Aus JPEG XL“, Schalter Hell/Dunkel, Schalter Experte, Fensterknöpfe (Minimieren, Maximieren, Schließen).
2. **Vorher/Nachher:** 100-%-Ausschnitt der ausgewählten Datei mit verschiebbarer Trennlinie; Beschriftung links (Ausgangsformat, Größe) und rechts (Zielformat, Qualitätsangabe, Größe erst nach der Umwandlung).
3. **Warteschlange:** Knöpfe „Dateien“, „Ordner hinzufügen“, „Leeren“; Ablagefläche „Bilder oder Ordner hierher ziehen“, solange die Liste leer ist. Spalten: Vorschaubild, Datei (Name + Abmessungen), Original, Ergebnis, Ersparnis bzw. Änderung, Status mit Fortschrittsbalken.
4. **Einstellungen** (rechts, je nach Modus, siehe 3.3).
5. **Fußbereich:** Summenzeile, Gesamtfortschritt, Start-Knopf (Text wechselt zwischen „N Dateien in JPEG XL konvertieren“ / „N Dateien zurückkonvertieren“, „Anhalten“, „Fortsetzen“, „Erneut konvertieren“).

### 3.3 Einstellungen

**Modus „In JPEG XL“, einfach:** Qualitätsstufen

| Stufe | Untertitel | `cjxl`-Werte |
|---|---|---|
| Verlustfrei (Voreinstellung) | empfohlen · pixelgenau | Distanz 0, Effort 7 |
| Hohe Qualität | d 0.5 | Distanz 0.5, Effort 7 |
| Ausgewogen | d 1.2 | Distanz 1.2, Effort 7 |
| Klein | d 2.6 | Distanz 2.6, Effort 8 |

**Modus „In JPEG XL“, Experte:**
- Regler Qualität 30–100; die angezeigte Distanz folgt der libjxl-Formel `d = 0,1 + (100 − q) · 0,09`, bei q = 100 ist sie 0 (verlustfrei)
- Regler Effort 1–9 mit den libjxl-Namen (Lightning … Tortoise)
- Schalter Verlustfrei (setzt Distanz 0, Qualitätsregler wird ignoriert)
- Beim Einschalten des Expertenmodus übernehmen die Regler die Werte der gerade gewählten Stufe

**In beiden Ansichten:**
- Schalter „JPEGs verlustfrei transkodieren“ (Standard an). Aus: JPEGs werden mit der gewählten Qualität neu kodiert und sind dann nicht mehr bit-genau herstellbar.

**Modus „Aus JPEG XL“:**
- Zielformat: **Automatisch** (Voreinstellung, Untertitel „Original-JPEG, sonst PNG“, als breite Schaltfläche über den anderen), PNG, JPEG, WebP, TIFF
- Hinweis bei Automatisch und JPEG: „Aus JPEG transkodierte Dateien werden bit-genau als Original-JPEG wiederhergestellt.“
- Experte: Regler JPEG-Qualität 50–100 (Standard 92) für JPEG-Ausgabe, die nicht aus einem Original-JPEG stammt; Bittiefe für PNG und TIFF mit „8 Bit“ (auf 8 Bit reduzieren) und „16 Bit“ (Bittiefe des Bildes behalten, höchstens 16; Standard)
- WebP-Ausgabe immer mit Qualität 90, kein eigener Regler

**In beiden Modi:**
- Schalter „Metadaten behalten“ (EXIF, XMP, Farbprofil), Standard an
- Schalter „Originale nach Prüfung in den Papierkorb“, Standard aus. Untertitel: „Nur bei verlustfreiem Ergebnis“
- Ausgabeordner: „Neben der Originaldatei“ (Standard) oder ein gewählter Ordner (Ordnerauswahl-Dialog)

### 3.4 Änderungen gegenüber dem Entwurf

| Stelle im Entwurf | Umsetzung |
|---|---|
| Automatisch: „Ursprüngliches Format wiederherstellen“ | „Original-JPEG, sonst PNG“ |
| Papierkorb: „Erst wenn die neue Datei erfolgreich gelesen wurde“ | „Nur bei verlustfreiem Ergebnis“ |
| Ablage-Hinweis „JPEG, PNG, TIFF, GIF, EXR“ | „JPEG, PNG, GIF, TIFF, WebP, BMP, EXR“ |
| Knöpfe „Dateien“ und „Ordner hinzufügen“ machen im Entwurf dasselbe | „Dateien“ öffnet eine Dateiauswahl, „Ordner hinzufügen“ eine Ordnerauswahl |
| Ausgabeordner „klicken zum Ändern“ wechselt im Entwurf reihum | Menü mit „Neben der Originaldatei“ und „Ordner wählen …“ |
| Größen in „Ergebnis“ und Summen sind im Entwurf geschätzt | Echte Werte, sobald eine Datei fertig ist; bis dahin „—“ |
| Leere Liste | Ablagefläche; Drag & Drop funktioniert aber auf dem ganzen Fenster |
| Bittiefe ist im Entwurf auf 8 Bit voreingestellt | Voreinstellung 16 Bit (= Bittiefe des Bildes behalten), damit das Einschalten des Expertenmodus nichts unbemerkt verschlechtert |
| Start-Knopf „Erneut konvertieren“ nach einem Durchlauf | „Fehlgeschlagene erneut versuchen“, wenn es Einträge mit Fehler gibt (verarbeitet nur diese); sonst „Alles erledigt“ (gesperrt). Neue Dateien machen ihn wieder zum Start-Knopf |

### 3.5 Annahmen (bitte bei der Durchsicht bestätigen)

- **Hell/Dunkel** richtet sich beim ersten Start nach der Windows-Einstellung; danach gilt die Wahl am Schalter.
- **Einstellungen** (Modus, Qualität, Experte, Zielformat, Schalter, Ausgabeordner, Hell/Dunkel, Fenstergröße) werden gespeichert und beim nächsten Start wiederhergestellt. Ausnahme: Der Papierkorb-Schalter startet immer **aus**, damit er nie versehentlich von einer früheren Sitzung aktiv ist.
- **Leeren** entfernt alle Einträge der Warteschlange; während eines Durchlaufs ist der Knopf gesperrt.
- **Jeder Modus hat seine eigene Warteschlange** (wie im Entwurf); beim Umschalten bleibt die andere erhalten.
- **Unpassende Dateien** (z. B. `.jxl` im Modus „In JPEG XL“ oder nicht unterstützte Formate) werden nicht aufgenommen; ein kurzer Hinweis nennt ihre Anzahl.
- **Während eines Durchlaufs** können weitere Dateien hinzugefügt werden; sie werden hinten angestellt und im selben Durchlauf verarbeitet.

## 4. Aufbau

### 4.1 Teile

```
JPEG XL/
├─ src/                    Oberfläche (React + HeroUI + Tailwind)
├─ src-tauri/              Hintergrund (Rust, Tauri 2)
│  └─ binaries/            cjxl / djxl als Sidecars (werden per Skript geladen, nicht eingecheckt)
└─ docs/                   Spezifikation, Pläne
```

**Oberfläche:** zeigt an, nimmt Eingaben und Drag & Drop entgegen, rechnet selbst nichts. Spricht ausschließlich über eine feste Schnittstelle (4.3) mit dem Hintergrund. Für Entwicklung und Tests gibt es eine **simulierte Schnittstelle**, mit der die Oberfläche auch im normalen Browser läuft.

**Hintergrund (Rust):** kleine Bausteine mit je einer Aufgabe:

| Baustein | Aufgabe |
|---|---|
| `scan` | Pfade einsammeln, Ordner rekursiv durchsuchen, nur unterstützte Endungen übernehmen, Abmessungen lesen |
| `tools` | `cjxl`/`djxl` aufrufen, Ausgaben auswerten, beim Start Vorhandensein und Version prüfen |
| `encode` | Datei → JXL (siehe 5.1) |
| `decode` | JXL → Zielformat (siehe 5.2) |
| `convert_io` | TIFF/WebP/BMP lesen und schreiben inkl. Metadaten-Übergabe über PNG |
| `verify` | Prüfungen (siehe 5.3) |
| `originals` | Papierkorb-Regeln, Papierkorb-Erkennung pro Laufwerk, Datumsübernahme |
| `queue` | Warteschlange, parallele Verarbeitung, Anhalten/Fortsetzen/Abbrechen, Ereignisse an die Oberfläche |
| `preview` | Vorher/Nachher-Ausschnitt und kleine Vorschaubilder |
| `report` | Zusammenfassung und Protokolldatei |
| `settings` | Einstellungen speichern und laden; Abbildung der Oberflächen-Einstellungen auf `cjxl`/`djxl`-Parameter |

### 4.2 Unterstützte Formate

| Richtung | Formate |
|---|---|
| Eingang → JXL | JPEG (`.jpg`, `.jpeg`), PNG, GIF, EXR direkt per `cjxl`; TIFF (`.tif`, `.tiff`), WebP, BMP über PNG-Zwischenschritt |
| JXL → Ausgang | Automatisch, PNG, JPEG, WebP, TIFF |

### 4.3 Schnittstelle Oberfläche ↔ Hintergrund

Befehle:

| Befehl | Zweck |
|---|---|
| `check_tools()` | Prüft `cjxl`/`djxl`; liefert Status und Versionen |
| `add_paths(paths, mode)` | Sammelt Dateien ein, liefert neue Einträge der Warteschlange |
| `remove_all()` | Leert die Warteschlange |
| `start(settings)` / `pause()` / `resume()` / `cancel()` | Steuert den Durchlauf |
| `preview(item_id, settings)` | Liefert Vorher- und Nachher-Ausschnitt als Bild |
| `thumbnail(item_id)` | Liefert ein kleines Vorschaubild |
| `load_settings()` / `save_settings(settings)` | Einstellungen |
| `open_log()` | Öffnet die Protokolldatei des letzten Durchlaufs |

Ereignisse an die Oberfläche:

| Ereignis | Inhalt |
|---|---|
| `item-progress` | Eintrag, Status (Wartet, Läuft, Fertig, Übersprungen, Fehler, Prüfung fehlgeschlagen), Fortschritt in %, Ergebnisgröße, Hinweistext |
| `run-finished` | Zahlen für die Zusammenfassung, Pfad der Protokolldatei |
| `run-paused` | Grund (vom Nutzer angehalten, Laufwerk voll) |

## 5. Ablauf

### 5.1 In JPEG XL (pro Datei)

1. **Ziel bestimmen:** neben dem Original (`foto.jpg` → `foto.jxl`) oder im gewählten Ausgabeordner. Bei hinzugefügten Ordnern wird die Unterordner-Struktur ab dem hinzugefügten Ordner nachgebaut. Existiert das Ziel schon → **Übersprungen**.
2. **Umwandeln** in die Zwischendatei `<ziel>.tmp`:
   - JPEG mit Schalter „Transkodieren“ an: verlustfreies Umpacken mit eingestelltem Effort. Ist das nicht möglich (z. B. CMYK-JPEG), wird die Datei **übersprungen**, nicht neu kodiert.
   - JPEG mit Schalter aus, PNG, GIF, EXR: `cjxl` mit eingestellter Distanz und eingestelltem Effort.
   - TIFF, WebP, BMP: einlesen, als PNG-Zwischendatei schreiben (Bittiefe, Transparenz, Farbprofil und EXIF werden übernommen, soweit das Ausgangsformat sie enthält), dann wie PNG.
3. **Prüfen** (5.3). Bei Fehlschlag: Zwischendatei löschen, Status **Prüfung fehlgeschlagen**, Original unverändert.
4. **Fertigstellen:** Zwischendatei in den endgültigen Namen umbenennen, Erstellungs- und Änderungsdatum des Originals übernehmen.
5. **Original:** behalten oder in den Papierkorb (5.4).

### 5.2 Aus JPEG XL (pro Datei)

1. Ziel bestimmen wie in 5.1 (Endung je nach Zielformat).
2. Umwandeln in die Zwischendatei:
   - **Automatisch:** enthält die Datei Daten zur JPEG-Wiederherstellung → Original-JPEG bit-genau wiederherstellen; sonst PNG.
   - **JPEG:** wenn möglich Original-JPEG wiederherstellen, sonst aus Pixeln kodieren mit JPEG-Qualität (Standard 92).
   - **PNG:** Bittiefe des Bildes (höchstens 16); im Expertenmodus auf Wunsch auf 8 Bit reduziert.
   - **TIFF, WebP:** über PNG-Zwischendatei; WebP mit Qualität 90, TIFF verlustfrei.
3. Prüfen (5.3), fertigstellen und Datum übernehmen wie in 5.1.
4. `.jxl`-Datei behalten oder in den Papierkorb (5.4).

### 5.3 Prüfung

| Fall | Prüfung |
|---|---|
| JPEG umgepackt | Original aus der neuen `.jxl` wiederherstellen und Byte für Byte mit der Ausgangsdatei vergleichen |
| Verlustfrei (PNG, GIF, TIFF, WebP, BMP → JXL) | `.jxl` dekodieren, Pixel mit dem Ausgangsbild vergleichen (gleiche Abmessungen, gleiche Kanäle, gleiche Werte) |
| Verlustfrei aus EXR (Gleitkomma) | `.jxl` dekodiert fehlerfrei, Abmessungen stimmen |
| Verlustbehaftet | `.jxl` dekodiert fehlerfrei, Abmessungen stimmen |
| Rückwandlung | Ergebnisdatei lässt sich öffnen, Abmessungen stimmen; bei wiederhergestelltem JPEG zusätzlich: Wiederherstellung von `djxl` ohne Fehler beendet |

### 5.4 Papierkorb-Regeln

Das Original bzw. die `.jxl` wandert nur dann in den Papierkorb, wenn **alle** Bedingungen erfüllt sind:

1. Der Schalter „Originale nach Prüfung in den Papierkorb“ ist an.
2. Die Prüfung ist bestanden.
3. Das Ergebnis ist verlustfrei:
   - Richtung „In JPEG XL“: JPEG umgepackt oder Distanz 0.
   - Richtung „Aus JPEG XL“: Original-JPEG wiederhergestellt, oder PNG/TIFF ohne Reduzierung der Bittiefe.
4. Die Metadaten sind vollständig übernommen: Schalter „Metadaten behalten“ an (bei umgepackten JPEGs immer erfüllt), und der Eintrag hat keinen Metadaten-Hinweis.
5. Das Laufwerk hat einen Papierkorb, und die Datei passt hinein. Das wird vor dem Verschieben geprüft. Gibt es keinen Papierkorb (z. B. USB-Stick, Netzlaufwerk) oder ist die Datei zu groß, bleibt das Original liegen und der Eintrag bekommt einen Hinweis.

Es gibt **keinen** Weg, auf dem das Programm Dateien endgültig löscht. Gelöscht werden nur eigene Zwischendateien (`*.tmp`).

### 5.5 Metadaten

| Weg | Verhalten |
|---|---|
| JPEG umpacken | Metadaten bleiben immer erhalten, unabhängig vom Schalter (sonst keine bit-genaue Wiederherstellung) |
| PNG, GIF, EXR ↔ JXL | Übernahme durch `cjxl`/`djxl`; Schalter aus → EXIF und XMP werden entfernt, das Farbprofil bleibt (sonst falsche Farben) |
| TIFF, WebP, BMP ↔ JXL | EXIF und Farbprofil werden über die PNG-Zwischendatei weitergereicht; gelingt das nicht, Hinweis am Eintrag |
| Dateidatum | Erstellungs- und Änderungsdatum werden immer übernommen |

### 5.6 Vorschau

- **Vorher/Nachher:** Beim Auswählen einer Datei schneidet der Hintergrund einen 100-%-Ausschnitt aus der Bildmitte (passend zur Größe des Vorschaufelds), wandelt ihn mit den aktuellen Einstellungen probehalber um und liefert beide Ausschnitte. Bei Änderungen an den Einstellungen wird neu berechnet (verzögert, damit Reglerbewegungen nicht jedes Mal eine Berechnung starten). Bei umgepackten JPEGs und Distanz 0 sind beide Seiten pixelgleich, die rechte Seite zeigt „verlustfrei“.
- **Vorschaubilder** in der Warteschlange werden nur für sichtbare Zeilen erzeugt, im Hintergrund mit niedriger Priorität, und zwischengespeichert.
- Die Warteschlange zeichnet nur sichtbare Zeilen, damit auch tausende Einträge flüssig bleiben.

### 5.7 Parallelität

Mehrere Dateien werden gleichzeitig verarbeitet. Startwert: die Hälfte der logischen Prozessorkerne als parallele Dateien. Der endgültige Wert wird im Belastungstest so eingestellt, dass der Prozessor ausgelastet ist und die Oberfläche flüssig bleibt.

## 6. Fehlerbehandlung

**Grundregel:** Ein Fehler betrifft nur die eine Datei. Zwischendatei löschen, Original unverändert lassen, Grund am Eintrag anzeigen (Tooltip), mit der nächsten Datei weitermachen.

| Status | Farbe | Auslöser |
|---|---|---|
| Fertig | grün | Umwandlung und Prüfung erfolgreich |
| Fertig mit Hinweis | grün mit Info-Symbol | Erfolgreich, aber z. B. Original wegen fehlendem Papierkorb behalten oder Metadaten nicht übernehmbar |
| Übersprungen | grau | Ziel existiert; JPEG nicht verlustfrei umpackbar |
| Fehler | rot | Datei defekt/unlesbar; Format nicht unterstützt; `cjxl`/`djxl` meldet Fehler |
| Prüfung fehlgeschlagen | rot | Prüfung 5.3 nicht bestanden |

Programmweit:

| Situation | Verhalten |
|---|---|
| `cjxl`/`djxl` fehlen oder starten nicht | Hinweisbalken oben, Start-Knopf gesperrt |
| Laufwerk voll | Warteschlange hält an, Meldung „Kein Platz mehr auf Laufwerk X“, weiter mit „Fortsetzen“ |
| Anhalten | Keine neuen Dateien starten, laufende werden fertig |
| Fenster schließen während eines Durchlaufs | Rückfrage „Wirklich beenden?“; bei Ja laufende Umwandlungen abbrechen, Zwischendateien löschen |
| Reste früherer Abbrüche | Vorhandene Zwischendatei für ein Ziel wird vor dem Umwandeln gelöscht |
| Umlaute, Emojis, Pfade über 260 Zeichen | Müssen funktionieren. Falls `cjxl`/`djxl` mit solchen Pfaden nicht umgehen können, arbeitet das Programm mit Zwischendateien mit einfachen Namen in einem eigenen Temp-Ordner auf demselben Laufwerk |

**Nach dem Durchlauf:** Summenzeile, z. B. „9.812 fertig · 14 übersprungen · 2 Fehler · 41,2 GB → 33,0 GB“, Knopf „Protokoll öffnen“. Einträge mit Problemen bleiben in der Liste.

**Protokolldatei:** `%LOCALAPPDATA%\JPEG XL Konverter\Protokolle\JJJJ-MM-TT_HH-MM-SS.txt` mit Einstellungen des Durchlaufs, Zusammenfassung und je Problemfall Pfad, Status und Grund.

## 7. Tests

### 7.1 Automatische Tests (Rust)

Rundreise-Tests mit den echten `cjxl`/`djxl`; Testbilder werden von den Tests selbst erzeugt.

| Test | Erwartung |
|---|---|
| JPEG (baseline, progressiv, mit EXIF) → JXL → JPEG | Byte-identisch |
| PNG 8 Bit, 16 Bit, mit Transparenz → JXL verlustfrei → PNG | Pixelgleich |
| TIFF, WebP, BMP, GIF → JXL | Umwandlung erfolgreich, EXIF erhalten (wo vorhanden) |
| CMYK-JPEG | Übersprungen |
| Defekte Datei, falsche Endung | Fehler nur für diese Datei |
| Ziel existiert | Übersprungen, Ziel unverändert |
| Umlaute, Emojis, Pfad über 260 Zeichen | Erfolgreich |
| Papierkorb-Regeln (5.4) | Mit einer Papierkorb-Attrappe: verlustbehaftet → Original bleibt; Bittiefe reduziert → bleibt; Metadaten nicht übernommen → bleibt; kein Papierkorb → bleibt; alle Bedingungen erfüllt → verschoben |
| Datumsübernahme | Erstellungs- und Änderungsdatum gleich dem Original |
| Einstellungs-Abbildung | Jede Stufe und jeder Expertenwert ergibt die richtigen `cjxl`/`djxl`-Parameter |

### 7.2 Oberfläche

- Läuft im Browser mit simulierter Schnittstelle.
- Bildschirmvergleich mit dem Entwurf: hell, dunkel, Experte an/aus, beide Modi, leere Liste, laufender Durchlauf.
- Tests für die Einstellungslogik (Stufen, Experte übernimmt Werte, Papierkorb startet aus).

### 7.3 Belastungstest

Ordner mit etwa 1.000 Bildern: Liste bleibt flüssig, Anhalten/Fortsetzen funktioniert, Summen stimmen.

### 7.4 Abnahme durch den Nutzer

An einer **Kopie** eines echten Foto-Ordners: ein Durchlauf ohne und einer mit Papierkorb, ein Test auf einem USB-Stick ohne Papierkorb. Erst danach am echten Archiv.

## 8. Nicht enthalten

- Rechtsklick-Menü im Explorer (kann später ergänzt werden)
- Installer und automatische Updates
- Eingangsformate HEIC und AVIF
- Eigener Qualitätsregler für WebP-Ausgabe
- Mehrseitige TIFFs (nur die erste Seite) und besondere Behandlung von Animationen (animierte GIFs werden so umgewandelt, wie `cjxl`/`djxl` es von sich aus tun)
- macOS und Linux
- Vorschaubilder für `.jxl` im Windows-Explorer (dafür gibt es Microsofts „JPEG XL Image Extension“)

## 9. Bei der Umsetzung zu klären

Diese Punkte hängen an Details der eingesetzten Programme und Bibliotheken und werden gleich zu Beginn der Umsetzung mit kleinen Versuchen geklärt:

1. Genaue `cjxl`-Optionen zum Entfernen von EXIF/XMP und zum Erzwingen/Abschalten des JPEG-Umpackens in v0.12.0.
2. Ob `cjxl`/`djxl` Pfade mit Emojis und über 260 Zeichen verarbeiten (sonst Ausweichweg aus Abschnitt 6).
3. Wie zuverlässig sich EXIF und Farbprofil aus TIFF/WebP/BMP lesen und über PNG weiterreichen lassen.
4. Wie sich pro Laufwerk zuverlässig erkennen lässt, ob ein Papierkorb existiert und eine Datei hineinpasst, ohne dass Windows im Zweifel endgültig löscht.
5. Ob die HeroUI-v3-Tabelle das Zeichnen nur sichtbarer Zeilen unterstützt oder ob dafür eine eigene Liste nötig ist.

## 10. Lizenzen

`cjxl`/`djxl` (libjxl, BSD-3-Clause) und Lexend (SIL Open Font License) werden mit ihren Lizenztexten ausgeliefert. HeroUI, Tauri, React und Tailwind stehen unter MIT- bzw. MIT/Apache-Lizenzen.
