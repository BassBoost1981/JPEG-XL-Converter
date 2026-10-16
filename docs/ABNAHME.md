# Abnahme: JPEG XL Konverter

Schritt für Schritt, mit Haken. **Arbeite zuerst nur an Kopien.** Das Programm löscht nie
etwas endgültig; mit dem Papierkorb-Schalter schiebt es Originale nach erfolgreicher Prüfung
in den Windows-Papierkorb. Trotzdem gilt: Erst alles an einer Kopie prüfen, dann am echten Archiv.

Das Programm liegt portabel in `dist-portable\JPEG XL Konverter\` (`JPEG XL Konverter.exe`,
`cjxl.exe`, `djxl.exe` und `LICENSES\` müssen im selben Ordner bleiben). Erzeugt wird der Ordner
mit `.\scripts\package-portable.ps1`.

## 0. Vorbereitung

- [ ] `JPEG XL Konverter.exe` aus dem Ordner starten. Es erscheint **kein** Hinweisbalken
      „cjxl/djxl fehlen“ und der Start-Knopf ist nicht gesperrt.
- [ ] Der Schalter „Originale nach Prüfung in den Papierkorb“ ist beim Start **aus**
      (auch nach einem Neustart, selbst wenn er vorher an war).

## 1. Kopie eines Foto-Ordners anlegen

- [ ] Einen echten Foto-Ordner mit ein paar hundert Bildern (am besten gemischt: JPEG, PNG,
      TIFF, einige mit Umlauten im Namen, einige in Unterordnern) **kopieren**, z. B. nach
      `D:\Test\Fotos-Kopie`. Alles Weitere passiert nur an dieser Kopie.
- [ ] Ein zweites Mal kopieren, z. B. nach `D:\Test\Fotos-Kopie2` (für Schritt 3).
- [ ] Hash-Liste der Originale in der Kopie merken:

  ```powershell
  Get-ChildItem D:\Test\Fotos-Kopie -Recurse -File |
      Get-FileHash | Export-Csv D:\Test\hashes-vorher.csv -NoTypeInformation
  ```

## 2. Durchlauf ohne Papierkorb

- [ ] Modus „In JPEG XL“, Stufe „Ausgewogen“, „Metadaten behalten“ an, Papierkorb-Schalter **aus**.
- [ ] Den Ordner `Fotos-Kopie` per Drag-and-drop in das Fenster ziehen. Die Liste scrollt
      flüssig, die Anzahl stimmt mit dem Ordner überein.
- [ ] Umwandeln starten. Zwischendurch „Anhalten“ und „Fortsetzen“ ausprobieren; laufende
      Dateien werden fertig, es startet nichts Neues, nach „Fortsetzen“ geht es weiter.
- [ ] Am Ende: Zusammenfassung zeigt Fertig / Übersprungen / Fehler und die Größen.
- [ ] Neben jedem Bild liegt jetzt eine `.jxl`; **alle Originale sind noch da**.
- [ ] Die Summen stimmen:

  ```powershell
  (Get-ChildItem D:\Test\Fotos-Kopie -Recurse -Filter *.jxl | Measure-Object Length -Sum).Sum
  ```

  Das Ergebnis entspricht der „nachher“-Größe in der Zusammenfassung.
- [ ] Einen Eintrag mit „Fertig“ aufklappen / den Vorher-Nachher-Vergleich ansehen: kein
      sichtbarer Unterschied bei „Hohe Qualität“.
- [ ] Es liegen keine `*.tmp`-Reste in den Ordnern.
- [ ] Den Durchlauf mit denselben Dateien nochmals starten (Liste leeren, Ordner erneut
      hineinziehen): Alles ist „Übersprungen“ (bestehende Ziele werden nie überschrieben).

## 3. Durchlauf mit Papierkorb

- [ ] Auf `Fotos-Kopie2`: Papierkorb-Schalter **an**, Stufe „Verlustfrei“ (verlustfrei bzw.
      verlustfrei umgepackte JPEGs sind die Fälle, bei denen Originale in den Papierkorb dürfen).
- [ ] Ordner hineinziehen, umwandeln.
- [ ] Die verlustfrei umgewandelten Originale liegen im **Windows-Papierkorb** (aus dem
      Papierkorb wiederherstellbar), die `.jxl` liegt am alten Platz.
- [ ] Dateien, die nicht verlustfrei umgewandelt wurden oder deren Metadaten nicht
      vollständig mitkamen, bleiben liegen und sind „Fertig mit Hinweis“.
- [ ] Datum: `.jxl` hat dasselbe Erstellungs- und Änderungsdatum wie das Original
      (Eigenschaften im Explorer oder `Get-Item`).
- [ ] **Falls Windows jemals** ein Fenster „Datei endgültig löschen?“ zeigt:
      mit **„Nein“** antworten. Das bedeutet, dass der Papierkorb die Datei nicht aufnehmen kann.
      Das Programm sollte das nie auslösen; bitte melden, wenn es doch passiert.

## 4. Rückwandlung „Automatisch“ und Byte-Vergleich

- [ ] Modus „Aus JPEG XL“, Ausgabeformat „Automatisch“.
- [ ] In `Fotos-Kopie` (aus Schritt 2) die vom Programm erzeugten `.jxl` hineinziehen. Zuvor
      die ursprünglichen JPEGs aus der Kopie in einen anderen Ordner verschieben
      (z. B. `D:\Test\Originale-JPG`), damit nichts „Übersprungen“ wird.
      Alternativ als Ausgabeziel „Ordner wählen …“ nutzen, z. B. `D:\Test\Zurück`.
- [ ] Umwandeln. Für aus JPEG umgepackte Dateien erscheint der Hinweis, dass sie bit-genau
      als Original-JPEG wiederhergestellt werden.
- [ ] Byte-Vergleich, JPEG-Dateien vorher und nachher:

  ```powershell
  $vorher  = Get-ChildItem D:\Test\Originale-JPG -Recurse -Filter *.jpg | Get-FileHash
  $nachher = Get-ChildItem D:\Test\Zurück       -Recurse -Filter *.jpg | Get-FileHash
  Compare-Object $vorher $nachher -Property Hash   # keine Ausgabe = alle identisch
  ```

  Es darf **kein** Unterschied erscheinen (Dateinamen und Anzahl gleich).
- [ ] PNG/TIFF, verlustfrei umgewandelt und zurück: Bilder sehen gleich aus und haben gleiche
      Pixelmaße (die Bytes können abweichen, die Pixel nicht).

## 5. USB-Stick-Test (kein Papierkorb)

- [ ] Eine Kopie von ein paar Fotos auf einen USB-Stick legen.
- [ ] Papierkorb-Schalter **an**, Ordner vom Stick hineinziehen, umwandeln.
- [ ] Die Originale **bleiben auf dem Stick**, die `.jxl` liegt daneben.
- [ ] Die Einträge sind „Fertig mit Hinweis“ mit dem Text „Original behalten: kein Papierkorb
      auf diesem Laufwerk“.
- [ ] Nichts wurde endgültig gelöscht; der Stick enthält Originale und `.jxl`.

## 6. Kein Netzwerkzugriff

- [ ] Ressourcenmonitor öffnen (`resmon`), Reiter „Netzwerk“.
- [ ] Programm starten und einen Durchlauf machen. Unter „Prozesse mit Netzwerkaktivität“ und
      „TCP-Verbindungen“ tauchen weder `JPEG XL Konverter.exe` noch `cjxl.exe`/`djxl.exe` auf.
- [ ] Optional: Netzwerk trennen (Flugmodus), alles funktioniert und die Schrift ist Lexend.

## 7. Zum Schluss: das echte Archiv

Erst wenn **alle** Haken oben gesetzt sind:

- [ ] Backup des Archivs vorhanden (oder zumindest Papierkorb-Schalter noch aus).
- [ ] Zuerst einen einzelnen echten Unterordner umwandeln, Ergebnis prüfen.
- [ ] Dann das ganze Archiv, zunächst ohne Papierkorb, danach (wenn alles passt) mit Papierkorb.
- [ ] Papierkorb nicht sofort leeren; erst nach einigen Tagen, wenn die `.jxl` sich im Alltag
      bewährt haben.

## Bekannte Einschränkungen

- Keine EXR-Dateien, keine CMYK-JPEGs und keine Fließkomma-Bilder (HDR/float): sie werden
  nicht oder mit Fehler/Hinweis abgelehnt und bleiben unberührt.
- Mehrseitige TIFFs und Animationen (animierte GIFs): Es wird die erste Seite bzw. das erste
  Bild geprüft, deshalb bleiben **die Originale immer liegen**, auch bei eingeschaltetem
  Papierkorb.
- „Metadaten behalten“ aus: Auch die EXIF-Ausrichtung (Orientation) wird verworfen. Bilder,
  die nur über diese Angabe gedreht angezeigt wurden, erscheinen danach evtl. gedreht.
- Nur Windows 11 (x64). Vorschaubilder für `.jxl` im Explorer liefert Windows nur mit
  Microsofts „JPEG XL Image Extension“.
