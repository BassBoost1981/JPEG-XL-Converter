# Abgleich App ↔ Entwurf

Paare `app-*.jpg` / `design-*.jpg` (Screenshots des Browser-Panes, JPEG, ca. 800 px breit; das Werkzeug liefert keine PNG-Dateien). App bei 1200 × 760 mit simuliertem Backend, Entwurf `jpeg-xl-konverter.dc.html` Variante „2a HeroUI“.

| Zustand | Dateien |
|---|---|
| hell · einfach · Enc · 6 Dateien | `*-hell-enc` |
| dunkel · einfach · Enc | `*-dunkel-enc` |
| dunkel · Experte an | `*-dunkel-enc-experte` |
| dunkel · Dec · Automatisch | `*-dunkel-dec-auto` |
| dunkel · leere Liste (Dec) | `*-dunkel-leer` |
| hell · laufender Durchlauf | `*-hell-enc-laeuft` |

Gemessene Werte (getComputedStyle) stimmen für Titelleiste, Karten, Voreinstellungen, Tabellenkopf, Zeilen, Chips, Schalter und Start-Knopf überein (Position ±1 px, Schriftgröße/-gewicht, Farben, Radien, Schatten, Hell und Dunkel).

Bewusste Abweichungen: Logo (Fold), Spezifikation 3.4 (Texte, Papierkorb-Schalter, Ablage-Hinweis, Ausgabeordner-Menü, echte Größen „—“ bis fertig, 16 Bit), echte Vorschaubilder statt Streifen, „Keine Datei ausgewählt“ statt Streifen-Platzhalter in der leeren Vorschau, Einstellungen und Start-Knopf werden gesperrt/abgedunkelt (während des Laufs bzw. ohne Dateien; Qualität bei „Verlustfrei“), „Distanz 0“ statt „0.00“, rechte Vorschau-Marke „Original“ statt der Zielendung bei Dec · Automatisch.
