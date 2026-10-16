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
- Das Archiv ist mit **Deflate64** gepackt. Weder `Expand-Archive` (.NET Framework) noch
  `tar.exe` können das entpacken. Das Skript nimmt deshalb den ZIP-Ordner der Windows-Shell
  (`Shell.Application`) und holt nur diese drei Einträge heraus. Für `cjxl.exe` ist geprüft,
  dass das Ergebnis byte-gleich mit einer Entpackung durch 7-Zip ist.
- Ohne die Dateien in `src-tauri/binaries` bricht `cargo build` ab, weil `tauri-build` die
  Sidecars aus `bundle.externalBin` sucht. Also zuerst das Skript ausführen.
  `tauri-build` kopiert die beiden Programme bei jedem Build als `cjxl.exe`/`djxl.exe`
  nach `target/debug` bzw. `target/release`.
- Im Archiv liegen unter `licenses/` noch weitere Lizenzen der fest eingebundenen
  Bibliotheken: brotli, giflib, highway, libjpeg-turbo, libpng, libwebp, sjpeg, skcms, zlib.

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

Die Erwartungswerte der Tests in `args.rs` stimmen mit der Hilfe überein und wurden nicht
geändert. Für das Entfernen der Metadaten erzeugt `cjxl_args` die Paare
`-x strip=exif` und `-x strip=xmp`.

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

- **EXR wird nicht unterstützt.** Laut Hilfe nimmt `cjxl` JXL, PPM, PNM, PFM, PAM, PGX, PNG,
  APNG, GIF und JPEG an. Eine EXR-Datei (Float, 64 × 48, mit dem `image`-Crate erzeugt) endet
  mit „Getting pixel data failed.“ und Exit-Code 1. Dieser Build enthält kein OpenEXR, und
  `djxl` kann auch nicht nach `.exr` schreiben. Das widerspricht der Spezifikation
  („EXR direkt per `cjxl`“).
- **djxl wählt das Ausgabeformat nach der Dateiendung.** Bei `x.png.tmp` kommt
  „can't decode to the file extension '.tmp'.“, bei `--reconstruct_jpeg` nach `x.jpg.tmp`
  kommt „--reconstruct_jpeg requires output format to be JPEG.“. Mit
  `--output_format=png` bzw. `--output_format=jpg` klappt es. Wer djxl in eine `.tmp`-Datei
  schreiben lässt, muss diese Option ergänzen. `djxl_args` enthält sie nicht. `cjxl` schreibt
  dagegen immer JXL, egal welche Endung.
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
