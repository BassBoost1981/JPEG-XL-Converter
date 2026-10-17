# libjxl v0.12.0: Notizen zu cjxl/djxl

Ergebnisse der Versuche aus Task 2 (Plan, Spezifikation Abschnitt 9, Punkte 1 und 2).
Geprüft am 2026-10-16 unter Windows 11 (ANSI-Codepage 1252, `LongPathsEnabled = 1`).

## Herkunft

- Datei: `jxl-x64-windows-static.zip` aus der GitHub-Release `v0.12.0` von `libjxl/libjxl`.
- SHA-256: `3025d7e308390796d20492322e606bc92decaee7b6bc99d3f7547870ae5db7de`.
  Das stimmt mit dem `digest`-Feld überein, das die GitHub-API für dieses Asset meldet.
- `scripts/fetch-libjxl.ps1` lädt das Archiv, prüft den Hash und legt die Dateien so ab:
  - `bin/cjxl.exe` → `src-tauri/binaries/cjxl-x86_64-pc-windows-msvc.exe`
  - `bin/djxl.exe` → `src-tauri/binaries/djxl-x86_64-pc-windows-msvc.exe`
  - `licenses/LICENSE.libjxl` → `src-tauri/binaries/LICENSE-libjxl`
  - alle Dateien aus `licenses/` → `src-tauri/binaries/licenses-libjxl/` (Ruling R10)
- Das Archiv ist mit **Deflate64** gepackt. Weder `Expand-Archive` (.NET Framework) noch
  `tar.exe` können das entpacken. Das Skript nimmt deshalb den ZIP-Ordner der Windows-Shell
  (`Shell.Application`) und holt nur die benötigten Einträge heraus. Für `cjxl.exe` ist
  geprüft, dass das Ergebnis byte-gleich mit einer Entpackung durch 7-Zip ist.
- Ohne die Dateien in `src-tauri/binaries` bricht `cargo build` ab, weil `tauri-build` die
  Sidecars aus `bundle.externalBin` sucht. Also zuerst das Skript ausführen.
  `tauri-build` kopiert die beiden Programme bei jedem Build als `cjxl.exe`/`djxl.exe`
  nach `target/debug` bzw. `target/release`.
- Die statischen Programme enthalten auch brotli, giflib, highway, libjpeg-turbo, libpng,
  libwebp, sjpeg, skcms und zlib. Deren Lizenztexte stehen im Archiv unter `licenses/` und
  werden mit allen anderen Dateien aus diesem Ordner nach `licenses-libjxl/` kopiert, damit
  sie mit ausgeliefert werden können. Der Ordner ist wie die Programme nicht eingecheckt
  (`.gitignore`).

## Versionsausgabe

`cjxl --version` schreibt auf stdout `cjxl v0.12.0 4128790 [_AVX2_,SSE4,SSE2] {Clang 22.1.3}`
und eine Copyright-Zeile. Bei `djxl` ist es genauso. `Tools::check` liest daraus `0.12.0`.

## Schreibweisen der Optionen (aus `--help -v -v -v -v`)

Aufruf immer `cjxl EINGABE AUSGABE [OPTIONEN...]` bzw. `djxl EINGABE AUSGABE [OPTIONEN...]`.

| Zweck | Option | Bemerkung |
|---|---|---|
| JPEG umpacken an/aus | `--lossless_jpeg=1` / `--lossless_jpeg=0` (kurz `-j 1`/`-j 0`) | Standard ist 1 |
| Distanz | `-d 1.2` (`--distance=1.2`) | 0 = mathematisch verlustfrei, Bereich 0 bis 25 |
| Effort | `-e 7` (`--effort=7`) | 1 bis 10, Standard 7 |
| Threads | `--num_threads=2` | bei cjxl und djxl gleich, −1 = Maschinenstandard |
| EXIF entfernen | `-x strip=exif` (`--dec-hints=strip=exif`) | |
| XMP entfernen | `-x strip=xmp` | |
| JPEG wiederherstellen (djxl) | `--reconstruct_jpeg` (`-J`) | Fehler, wenn die Datei keine Wiederherstellungsdaten hat |
| Pixel als JPEG (djxl) | `--pixels_to_jpeg` (`-j`) | |
| JPEG-Qualität (djxl) | `--jpeg_quality=92` (`-q 92`) | setzt `--pixels_to_jpeg` von selbst, Standard 95 |
| Bittiefe der Ausgabe (djxl) | `--bits_per_sample=8` | Standard −1: hängt von Format und Eingabe ab (bis 16 Bit bei PNG) |
| Ausgabeformat (djxl) | `--output_format=jpg` / `--output_format=png` | sonst entscheidet die Endung der Ausgabedatei |

Die Schreibweisen in den Erwartungswerten der Tests in `args.rs` stimmen mit der Hilfe überein.
Für das Entfernen der Metadaten erzeugt `cjxl_args` die Paare `-x strip=exif` und
`-x strip=xmp`. Nach Ruling R9 hängt `djxl_args` zusätzlich immer `--output_format=…` an.
Die Erwartungswerte in `djxl_targets` sind entsprechend erweitert.

## Versuche

### (a) JPEG-Eingabe mit gesetztem `-d`

Nein, `cjxl` packt dann **nicht** einfach um. Es bricht mit Exit-Code 1 und ohne Ausgabedatei ab:

```
Must not set non-zero distance in combination with --lossless_jpeg=1, which is set by default.
```

Bei `-q 90` erscheint dieselbe Meldung für die Qualität. `-d 0` ist zusammen mit dem
Umpacken erlaubt. Folgerung für `cjxl_args`:

- Umpacken: `--lossless_jpeg=1` und **kein** `-d`.
- Nicht umpacken: `--lossless_jpeg=0` und `-d <Distanz>`.

Gemessen mit einem JPEG, das EXIF enthält (640 × 480):

| Aufruf | Ergebnis |
|---|---|
| ohne Optionen / `--lossless_jpeg=1 -e 7` | umgepackt, `jbrd`-Box vorhanden, 48 523 Byte |
| `-d 1.2` oder `-d 1.2 --lossless_jpeg=1` | Fehler (siehe oben), Exit-Code 1 |
| `--lossless_jpeg=0 -d 2.6 -e 8` | neu kodiert, keine `jbrd`-Box, 26 633 Byte |
| umgepackt, dann `djxl x.jxl x.jpg` | byte-gleich mit dem Original |

Metadaten:

- Umpacken mit `-x strip=exif` bricht ab: „Cannot strip exif metadata, try setting
  `--allow_jpeg_reconstruction=0` …“. Deshalb wird beim Umpacken nie etwas entfernt.
- `--lossless_jpeg=0 -d 1.2 -x strip=exif -x strip=xmp` funktioniert. Die EXIF-Daten sind
  danach weg.
- Mit einem PNG, das EXIF und XMP enthält, ist geprüft: Ohne `strip` liefert `djxl` aus der
  JXL 38 Byte EXIF und 276 Byte XMP, mit `-x strip=exif -x strip=xmp` jeweils 0 Byte. Das
  Farbprofil ist davon nicht betroffen: `strip` gilt laut Hilfe nur für exif, xmp und jumbf.
- Vorsicht: Unbekannte Werte wie `-x strip=foo` nimmt `cjxl` ohne Fehler hin. Ein Tippfehler
  würde also nicht auffallen.

### (b) Pfade mit `Ü`, `📷` und über 260 Zeichen

- **Umlaute und Emoji funktionieren.** Beide Programme haben im Manifest
  `<activeCodePage>UTF-8</activeCodePage>`. Sie bekommen ihre Argumente also als UTF-8, auch
  wenn die Systemcodepage 1252 ist. Geprüft mit `…\Ünïcödé 📷\bild 📷.png` für cjxl und djxl.
- **Pfade über 260 Zeichen scheitern in normaler Form.** Die Programme sind nicht
  `longPathAware` (das Manifest enthält den Eintrag nicht). `cjxl` meldet dann
  „Reading image data failed.“, `djxl` meldet „couldn't load …“.
- **Mit dem Präfix `\\?\` funktionieren lange Pfade** bei beiden Programmen, auch beim Lesen
  und beim Schreiben. `djxl` erkennt das Ausgabeformat auch dann an der Endung.

### (c) `-` für stdin/stdout

Beide Programme können das:

- `cjxl - aus.jxl < ein.png` und `cjxl ein.png - > aus.jxl` funktionieren. Die Hilfe von
  cjxl erwähnt das nicht. Das Ergebnis ist byte-gleich mit dem Aufruf über Dateien, auch beim
  Umpacken eines JPEG über stdin.
- `djxl - aus.png < ein.jxl` funktioniert. Für stdout braucht `djxl` die Option
  `--output_format`, z. B. `djxl ein.jxl - --output_format png`. Auch
  `--reconstruct_jpeg --output_format jpg` nach stdout liefert das Original byte-gleich.

### Weitere Beobachtungen

- **EXR wird nicht unterstützt.** Das offizielle statische `cjxl` aus v0.12.0 kann EXR nicht
  lesen. Laut Hilfe nimmt es JXL, PPM, PNM, PFM, PAM, PGX, PNG, APNG, GIF und JPEG an. Eine
  EXR-Datei (Float, 64 × 48, mit dem `image`-Crate erzeugt) endet mit „Getting pixel data
  failed.“ und Exit-Code 1. Dieser Build enthält kein OpenEXR, und `djxl` kann auch nicht
  nach `.exr` schreiben. **Deshalb nimmt die App keine `.exr`-Dateien an** (Ruling R8). Es
  gibt in `InputKind` keine Variante `Exr` und auch keinen Umweg über eine andere Umwandlung.
  Damit ist die Spezifikation an dieser Stelle („EXR direkt per `cjxl`“) überholt.
- **djxl wählt das Ausgabeformat sonst nach der Dateiendung.** Bei `x.png.tmp` kommt
  „can't decode to the file extension '.tmp'.“, bei `--reconstruct_jpeg` nach `x.jpg.tmp`
  kommt „--reconstruct_jpeg requires output format to be JPEG.“. Mit `--output_format=png`
  bzw. `--output_format=jpg` (Schreibweise aus der djxl-Hilfe) klappt es. Deshalb hängt
  `djxl_args` das Format immer an (Ruling R9): `--output_format=jpg` bei `ReconstructJpeg`
  und `PixelsToJpeg`, `--output_format=png` bei `Png`. Der Test `djxl_writes_to_tmp_paths`
  prüft das in `.tmp`-Dateien. `--reconstruct_jpeg --output_format=jpg` stellt das JPEG
  weiterhin byte-gleich wieder her. `cjxl` schreibt dagegen immer JXL, egal welche Endung.
- `cjxl` überschreibt eine vorhandene Ausgabedatei ohne Rückfrage.
- Bei einem Fehler (z. B. abgeschnittenes JPEG: „EncodeImageJXL() failed.“) gibt es Exit-Code 1
  und keine Ausgabedatei. Bei Erfolg ist der Exit-Code 0.
- Die Fortschritts- und Fehlermeldungen kommen auf stderr. Bei Dateiausgabe bleibt stdout leer.
- Mit `--num_threads=2` meldet cjxl „2 threads“. Ohne die Option nimmt es alle Kerne (hier 24).

## Gewählte Pfadstrategie

Variante **(1) Pfade direkt übergeben**, jeweils in der Langform mit `\\?\`
(`Tools::run_cjxl`/`run_djxl` → `extended_length`):

- Der Pfad wird mit `std::path::absolute` absolut gemacht und normalisiert (`/` → `\`, `..` aufgelöst).
- `C:\…` wird zu `\\?\C:\…`, `\\server\freigabe\…` wird zu `\\?\UNC\server\freigabe\…`.
- Pfade, die schon mit `\\?\` oder `\\.\` beginnen, bleiben unverändert.

Begründung: Variante (1) besteht `unicode_and_long_paths_work`, also die erste Variante der
Reihenfolge, die funktioniert. In normaler Form scheitert nur der lange Pfad, das zeigt der
Testlauf ohne Präfix. Umlaute und Emoji gehen dank UTF-8-Manifest ohnehin. Die Langform hebt
die 260-Zeichen-Grenze auf, ohne Daten durch Rust zu leiten (Variante 2) oder Dateien zu
kopieren (Variante 3). Aufrufer übergeben ganz normale `PathBuf`s und merken davon nichts.

## Task 8: Ausrichtung und Metadaten beim Umwandeln nach JXL

Geprüft am 2026-10-16 mit PNG-Testbildern (64 × 48), `cjxl -d 0 -e 7`, dann `djxl` nach PNG.

### EXIF-Ausrichtung (Ruling R11)

- `djxl` hat **keine** Option, die Ausrichtung beim Dekodieren zu behalten
  (`--help -v -v -v -v` nennt nichts dazu).
- **Mit Metadaten:** `cjxl` liest die Ausrichtung aus dem `eXIf`-Chunk der PNG und schreibt
  sie in den JXL-Header. `djxl` dreht die Pixel beim Dekodieren entsprechend und setzt den
  Ausrichtungswert im ausgegebenen EXIF auf 1. Bei Ausrichtung 6 und 5 kommt aus einem
  64 × 48-Bild also ein 48 × 64-Bild. Die Pixel sind genau die des Ausgangsbilds, gedreht wie
  vom EXIF verlangt (geprüft für 1, 3, 5, 6 und 8).
- **Ohne Metadaten** (`-x strip=exif -x strip=xmp`): Die Ausrichtung geht mit dem EXIF
  verloren. `djxl` liefert die Pixel ungedreht, so wie sie in der PNG stehen.
- Folgerung für die Pixelprüfung in `encode.rs`: Das Vergleichsbild ist die `cjxl`-Eingabe,
  bei eingeschalteten Metadaten gedreht nach ihrer EXIF-Ausrichtung (`image`:
  `ImageDecoder::orientation` + `DynamicImage::apply_orientation`), sonst ungedreht. Die
  Maßprüfung bei Verlust nutzt `dims_match` (gleich oder vertauscht).
- Das `image`-Crate liest die Ausrichtung einer PNG nur aus `eXIf`. Steht sie nur in einem
  `zTXt`-Chunk „Raw profile type exif“, würde der Vergleich fehlschlagen
  (Prüfung fehlgeschlagen, Original bleibt). PNGs von `djxl` haben dort immer Ausrichtung 1.

### Was `cjxl` aus einer PNG übernimmt

| Chunk | Ergebnis nach `djxl` |
|---|---|
| `eXIf` | übernommen (als `zTXt` „Raw profile type exif“) |
| `iTXt` „XML:com.adobe.xmp“ | übernommen (als `zTXt` „Raw profile type xmp“) |
| `zTXt` „Raw profile type exif/xmp“ im Format von `djxl` | übernommen |
| `zTXt` „Raw profile type exif“ ohne Zeilenumbrüche im Hex-Text | **still verworfen** |
| `tEXt` „Title“, `pHYs`, `tIME`, `bKGD` | **verworfen** |

`djxl` schreibt immer `sRGB`, `gAMA` und `cHRM` (bzw. `iCCP`). Deshalb prüft `encode.rs`
nach dem Dekodieren, ob EXIF und XMP der Quelle auch im Ergebnis stehen, und zählt jeden
anderen Text-Chunk der Quelle (`tEXt`, `zTXt`, `iTXt`: Titel, Autor, Kommentar, IPTC …) als verloren
(`convert_io::PngMeta::other_metadata`). Dann bleibt das Original liegen und der Eintrag
bekommt den Hinweis „Metadaten nicht vollständig übernommen“. `pHYs`, `tIME`, `bKGD`, `sPLT`,
`hIST` und ähnliche Anzeige-Chunks gehen ebenfalls verloren, zählen aber nicht (Ruling R20).
Beim GIF zählen Kommentare und unbekannte Anwendungs-Erweiterungen (z. B. XMP), nicht aber
`NETSCAPE2.0`/`ANIMEXTS1.0` (Wiederholungen).

### Weitere Beobachtungen

- Ein CMYK-JPEG lässt sich nicht umpacken: `cjxl --lossless_jpeg=1` endet mit
  „EncodeImageJXL() failed.“ und Exit-Code 1.
- Eine PNG mit der Endung `.jpg` nimmt `cjxl` mit `--lossless_jpeg=1` ohne Fehler an und
  kodiert die Pixel (keine `jbrd`-Box). `encode.rs` prüft deshalb vorher am Dateiinhalt, ob
  JPEG, PNG und GIF wirklich das sind, was die Endung sagt.
- Ein GIF mit zwei Bildern wird zu einer animierten JXL. `djxl` schreibt daraus eine APNG,
  deren erstes Bild pixelgleich mit dem ersten Bild des GIF ist. Geprüft wird nur dieses
  erste Bild, deshalb wandert ein animiertes Original nie in den Papierkorb (Ruling R17).

## Belastungstest und Anzahl der Worker (Task 19)

Messung mit `scripts/make-stress-set.ps1` (Beispiele `make_stress_set` und `stress_run`): 1.000 Bilder
(820 JPEG, 120 PNG, 60 TIFF, 2–12 MP, 25 Ordner mit teils Unterordner, zusammen 3,55 GB),
Ausgewogen (d 1.2, e 7), JPEG-Umpacken an, Metadaten an, Papierkorb-Attrappe, Ausgabe in einen
anderen Ordner. Rechner: 24 logische Prozessoren, `cjxl`/`djxl` mit je `--num_threads=2`.

| Worker | Dauer | Dateien/s |
|---|---|---|
| 6 | 138,7 s | 7,2 |
| 12 (= `default_workers()`) | 105,4 s | 9,5 |
| 24 | 86,6 s | 11,5 |

Ergebnis in allen Läufen: 1.000 fertig, 0 Fehler, 3.545.709.562 → 822.334.671 Bytes (23,2 %);
`bytes_out` der Zusammenfassung ist gleich der Summe der erzeugten `.jxl`-Dateien. Der höchste
gleichzeitige Worker-Stand war jeweils genau die eingestellte Zahl.

Entscheidung: `default_workers()` bleibt bei **halben logischen Prozessoren**. 24 Worker sind nur
etwa 18 % schneller, belegen aber mit 48 Tool-Threads alle Kerne und lassen weder der Oberfläche
noch dem Rest des Rechners Luft (und vervielfachen den Speicherbedarf bei 12-MP-Bildern). 12 Worker
sind bereits ~32 % schneller als 6 und lassen den Rechner bedienbar. Ein klar besserer Wert
ergibt sich aus der Messung nicht.
