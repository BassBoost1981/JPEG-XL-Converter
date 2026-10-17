# Abnahme: JPEG XL Converter

Schritt für Schritt, mit Haken. **Arbeite zuerst nur an Kopien.** Das Programm löscht nie
etwas endgültig; mit dem Papierkorb-Schalter schiebt es Originale nach erfolgreicher Prüfung
in den Windows-Papierkorb. Trotzdem gilt: Erst alles an einer Kopie prüfen, dann am echten Archiv.

Das Programm liegt portabel in `dist-portable\JPEG XL Converter\` (`JPEG XL Converter.exe`,
`cjxl.exe`, `djxl.exe` und `LICENSES\` müssen im selben Ordner bleiben). Erzeugt wird der Ordner
mit `.\scripts\package-portable.ps1`. Der Ordner `data\` ist nicht dabei, er entsteht beim ersten
Start neben der Exe (Einstellungen, Fensterposition, Protokolle).

Die Bezeichnungen unten gelten für die **deutsche** Oberfläche. Stelle die Sprache dafür über den
Knopf mit den Reglern in der Titelleiste (Programm-Einstellungen → Sprache) auf „Deutsch“. Die
englische Fassung dieser Liste ist [`ACCEPTANCE.md`](ACCEPTANCE.md).

## 0. Vorbereitung

- [ ] `JPEG XL Converter.exe` aus dem Ordner starten. Es erscheint **kein** Hinweisbalken
      „cjxl/djxl fehlen“ und der Start-Knopf ist nicht gesperrt.
- [ ] Neben der Exe ist ein Ordner `data\` entstanden.
- [ ] Der Schalter „Originale nach Prüfung in den Papierkorb“ ist beim Start **aus**
      (auch nach einem Neustart, selbst wenn er vorher an war).

## 1. Kopie eines Foto-Ordners anlegen

- [ ] Einen echten Foto-Ordner mit ein paar hundert Bildern (am besten gemischt: JPEG, PNG,
      TIFF, einige mit Umlauten im Namen, einige in Unterordnern) **kopieren**, z. B. nach
      `D:\Test\Fotos-Kopie`. Alles Weitere passiert nur an dieser Kopie.
- [ ] Ein zweites Mal kopieren, z. B. nach `D:\Test\Fotos-Kopie2` (für Schritt 3).
- [ ] Hash-Liste aller JPEG-Originale der Kopie (`*.jpg` und `*.jpeg`, auch in Unterordnern)
      mit relativem Pfad in `D:\Test\hashes-vorher.csv` ablegen. PowerShell (Windows PowerShell 5.1
      genügt), Pfad `$quelle` anpassen:

  ```powershell
  function Get-Hashliste($ordner, $endungen) {
      Push-Location -LiteralPath $ordner
      try {
          Get-ChildItem -Recurse -File | Where-Object { $_.Extension -in $endungen } | ForEach-Object {
              [pscustomobject]@{
                  Pfad = (Resolve-Path -LiteralPath $_.FullName -Relative).Substring(2)
                  Hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
              }
          }
      } finally { Pop-Location }
  }
  $quelle = "D:\Test\Fotos-Kopie"
  Get-Hashliste $quelle ".jpg", ".jpeg" | Export-Csv "D:\Test\hashes-vorher.csv" -NoTypeInformation -Encoding UTF8
  "{0} Originale erfasst" -f @(Import-Csv "D:\Test\hashes-vorher.csv").Count
  ```

  Die Anzahl muss der Zahl der JPEGs im Ordner entsprechen. Die CSV wird in Schritt 4 gebraucht.

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
- [ ] Einen Eintrag mit „Fertig“ auswählen und den Vorher-Nachher-Vergleich ansehen: kein
      sichtbarer Unterschied bei „Ausgewogen“. Hinweise und Fehlergründe zeigt der Tooltip, wenn
      man mit der Maus über den Status fährt.
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

Die Rückwandlung läuft in einen **eigenen Ausgabeordner**, damit nichts „Übersprungen“ wird und
der Vergleich eindeutig ist. Grundlage ist `Fotos-Kopie` aus Schritt 2 (dort liegen die `.jxl`)
und die CSV aus Schritt 1.

- [ ] Papierkorb-Schalter **aus** (er ist aus Schritt 3 noch an).
- [ ] Modus „Aus JPEG XL“, Ausgabeformat „Automatisch“. Ausgabeordner: „Ordner wählen …“ →
      `D:\Test\Zurück` (neuer, leerer Ordner).
- [ ] `Fotos-Kopie` ins Fenster ziehen (aufgenommen werden nur die `.jxl`), umwandeln.
      Für aus JPEG umgepackte Dateien erscheint der Hinweis, dass sie bit-genau als Original-JPEG
      wiederhergestellt werden. Das Programm legt die Ordnerstruktur samt Namen des gezogenen
      Ordners an: `D:\Test\Zurück\Fotos-Kopie…`.
- [ ] Byte-Vergleich gegen die CSV (relativer Pfad ohne Endung + Hash; ein Original `.jpeg`
      kommt als `.jpg` zurück und gilt trotzdem als gleich). `Get-Hashliste` aus Schritt 1
      muss in dieser PowerShell-Sitzung noch definiert sein, sonst den Funktionsblock aus Schritt 1
      erneut einfügen:

  ```powershell
  $vorher  = Import-Csv "D:\Test\hashes-vorher.csv"
  $nachher = Get-Hashliste "D:\Test\Zurück\Fotos-Kopie" ".jpg", ".jpeg"
  $a = @{}; foreach ($e in $vorher)  { $a[[IO.Path]::ChangeExtension($e.Pfad, $null).TrimEnd(".").ToLowerInvariant()] = $e.Hash }
  $b = @{}; foreach ($e in $nachher) { $b[[IO.Path]::ChangeExtension($e.Pfad, $null).TrimEnd(".").ToLowerInvariant()] = $e.Hash }
  "Vorher: $($a.Count) Dateien, nachher: $($b.Count) Dateien"
  $unterschiede = foreach ($k in ($a.Keys + $b.Keys | Sort-Object -Unique)) {
      if (-not $b.ContainsKey($k))     { "FEHLT nachher:   $k" }
      elseif (-not $a.ContainsKey($k)) { "ZUVIEL nachher:  $k" }
      elseif ($a[$k] -ne $b[$k])       { "UNTERSCHIEDLICH: $k" }
  }
  if ($unterschiede) { $unterschiede } else { "alles gleich" }
  ```

  Erwartet: gleiche Anzahl auf beiden Seiten und die Ausgabe `alles gleich`. Jede andere Zeile
  (`FEHLT`, `ZUVIEL`, `UNTERSCHIEDLICH`) ist ein Fehler, der gemeldet werden muss. Die Zahl bei
  „Vorher“ muss außerdem der in Schritt 1 erfassten Anzahl entsprechen; ist sie kleiner, liegen
  in einem Ordner zwei JPEGs mit gleichem Namen und anderer Endung (z. B. `foto.jpg` und
  `foto.jpeg`), von denen nur eines umgewandelt wurde (das andere ist „Übersprungen“).
- [ ] PNG/TIFF, verlustfrei umgewandelt und zurück: Bilder sehen gleich aus und haben gleiche
      Pixelmaße (die Bytes können abweichen, die Pixel nicht).

## 5. USB-Stick-Test (kein Papierkorb)

- [ ] Eine Kopie von ein paar Fotos auf einen USB-Stick legen.
- [ ] Papierkorb-Schalter **an**, Ordner vom Stick hineinziehen, umwandeln.
- [ ] Die Originale **bleiben auf dem Stick**, die `.jxl` liegt daneben.
- [ ] Die Einträge sind „Fertig mit Hinweis“ mit dem Text „Original behalten: kein Papierkorb
      auf diesem Laufwerk“.
- [ ] Nichts wurde endgültig gelöscht; der Stick enthält Originale und `.jxl`.

## 6. Portabel vom USB-Stick

Der Programmordner `JPEG XL Converter\` (mit den drei `.exe` und `LICENSES\`, **ohne** `data\`)
liegt auf einem USB-Stick, die Prüfung läuft von dort aus.

- [ ] Vorher notieren, welche Ordner es unter `%APPDATA%` und `%LOCALAPPDATA%` gibt
      (zum Beispiel die Namen in beiden Ordnern aufschreiben oder die Ordnerlisten speichern).
- [ ] `JPEG XL Converter.exe` **vom Stick** starten. Neben der Exe entsteht `data\`.
- [ ] In den Programm-Einstellungen (Knopf mit den Reglern in der Titelleiste) **Sprache** auf
      „English“ und **Erscheinungsbild** auf die jeweils andere Einstellung (Hell ↔ Dunkel) umstellen.
      Die Oberfläche wechselt sofort komplett die Sprache, auch der Fenstertitel.
- [ ] Das Fenster mit dem Schließen-Knopf **schließen**. Es schließt sich und der Prozess endet.
      `%TEMP%\jpeg-xl-konverter\<pid>` samt `webview` ist danach weg.
- [ ] Programm **erneut vom Stick starten**: Sprache und Erscheinungsbild sind noch so, wie du sie
      gesetzt hast (die Einstellungen liegen in `data\settings.json`, die Fensterposition in
      `data\window.json`).
- [ ] Einen kurzen Durchlauf machen. Das Protokoll liegt in `data\logs\` auf dem Stick;
      „Protokoll öffnen“ öffnet es.
- [ ] **Unter `%APPDATA%` und `%LOCALAPPDATA%` ist kein App-Ordner entstanden** (kein
      `de.bassboost1981.jpegxlkonverter`, kein `JPEG XL Konverter`, kein `JPEG XL Converter`).
      Die Listen gleichen der Notiz von oben. Ordner einer früheren Version können dort
      liegen; die App übernimmt sie nicht und löscht sie nicht (du kannst sie von Hand löschen).
- [ ] Optional, Nur-Lesen-Modus: Den Stick mit dem Schreibschutzschalter sperren (oder den
      Programmordner an einen Ort ohne Schreibrecht legen) und das Programm starten. Eine vorhandene `data\settings.json` wird
      gelesen (Sprache und Erscheinungsbild stimmen), einmal pro Start erscheint der Hinweis
      „Programmordner ist schreibgeschützt – Einstellungen werden nicht gespeichert, Protokolle
      liegen im Temp-Ordner.“, Änderungen wirken, werden aber nicht gespeichert, und das Protokoll
      liegt in `%TEMP%\jpeg-xl-konverter\logs\`.

## 7. Kein Netzwerkzugriff

- [ ] Ressourcenmonitor öffnen (`resmon`), Reiter „Netzwerk“.
- [ ] Programm starten und einen Durchlauf machen. Unter „Prozesse mit Netzwerkaktivität“ und
      „TCP-Verbindungen“ tauchen weder `JPEG XL Converter.exe` noch `cjxl.exe`/`djxl.exe` auf.
- [ ] Optional: Netzwerk trennen (Flugmodus), alles funktioniert und die Schrift ist Lexend.

## 8. Zum Schluss: das echte Archiv

Erst wenn **alle** Haken oben gesetzt sind:

- [ ] Ein Backup des Archivs ist vorhanden (echte, getrennte Kopie, z. B. auf einer externen Platte).
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
- Rückwandlung von Gleitkomma-JPEG-XL (HDR), Bildern mit mehr als 16 Bit oder mit
  Zusatzkanälen außer Alpha: PNG/TIFF speichern höchstens 16 Bit ganzzahlig, Werte über 1,0
  und Zusatzkanäle gehen verloren. Die `.jxl` bleibt deshalb **immer liegen** (Hinweis
  „Gleitkomma- oder Mehrkanal-Bild – .jxl behalten“).
- „Metadaten behalten“ aus: Auch die EXIF-Ausrichtung (Orientation) wird verworfen. Bilder,
  die nur über diese Angabe gedreht angezeigt wurden, erscheinen danach evtl. gedreht.
- Nur Windows 11 (x64). Vorschaubilder für `.jxl` im Explorer liefert Windows nur mit
  Microsofts „JPEG XL Image Extension“.
