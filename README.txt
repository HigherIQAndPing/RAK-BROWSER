RAK - Render Accelerated Kinetic
================================
R = Render, A = Accelerated, K = Kinetic

Ein schlanker, schneller Mini-Browser in Rust (tao + wry).

So bekommst du die .exe (einmalig):
  1. Rust installieren: https://rustup.rs  (Visual Studio C++ Build Tools zulassen)
  2. Alle Terminalfenster schliessen.
  3. ZIP entpacken (Rechtsklick -> Alle extrahieren), NICHT aus der ZIP heraus starten.
  4. build-exe.bat doppelklicken. Danach liegt RAK.exe im selben Ordner.

Danach reicht ein Doppelklick auf RAK.exe (oder start.bat).
Die RAK.exe kannst du frei verschieben, z.B. auf den Desktop.

FUNKTIONEN
----------
Tabs            + = neuer Tab, Masken-Symbol = privater Tab, Limit 12 Tabs
Adressleiste    Adresse oder Suchbegriff. Suchanbieter waehlen: Google,
                DuckDuckGo, Ecosia oder Bing. Spezialadressen:
                rak://verlauf, rak://lesezeichen und rak://einstellungen
Lesezeichen     Stern in der Adressleiste = Seite merken (Name und Ordner waehlbar).
                Die Lesezeichenleiste zeigt Ordner und Lesezeichen; Ordner anklicken
                zum Oeffnen, "Leiste" bringt dich zurueck. Mit dem x am Lesezeichen
                (beim Darueberfahren) entfernst du es. Ordner anlegen, umbenennen,
                loeschen und Lesezeichen verschieben: Ordner-Symbol rechts oben.
Verlauf         Uhr-Symbol rechts oben: durchsuchbar, nach Tagen sortiert, einzelne
                Eintraege loeschbar oder alles auf einmal.
Privat          Private Tabs (violett, "PRIVAT") kommen nie in den Verlauf und
                speichern keine Cookies/Cache dauerhaft. Lesezeichen setzt du dort
                nur, wenn du es selbst willst.
Seitenleiste    Spotify, Instagram und X. Klick oeffnet ein Panel neben der Seite.
                Breite: Griff zwischen Panel und Seite ziehen, oder + / - in der
                Seitenleiste; Doppelklick auf den Griff = Standardbreite.
                Stift-Symbol unten: Bearbeiten-Modus. Rotes x entfernt eine App
                (die Seite wird beendet), gruenes + fuegt sie wieder hinzu.
                Angemeldet bleibst du in den Apps nach dem ersten Login.
Erweiterungen   Puzzle-Symbol rechts oben (oder rak://erweiterungen). Adresse einer
                Erweiterung aus dem Chrome Web Store einfuegen -> "Installieren".
                RAK laedt sie herunter, entpackt sie und laedt sie beim naechsten
                Start. Dafuer gibt es einen Knopf "RAK jetzt neu starten".
                WICHTIG: Funktioniert nur unter Windows (WebView2). Es gibt KEINE
                Erweiterungsleiste: Symbole und Popups von Erweiterungen sind nicht
                bedienbar. Erweiterungen, die direkt auf Webseiten arbeiten
                (Werbeblocker, Dunkelmodus, Skripte) laufen oft, aber nicht alle
                Chrome-Funktionen sind verfuegbar. Private Tabs nutzen keine
                Erweiterungen. Ordner: %APPDATA%\RAK\extensions
                (Der Download nutzt curl.exe, das ab Windows 10 mitgeliefert wird.)
Einstellungen  Zahnrad rechts oben: Suchmaschine waehlen, automatische
                Uebersetzung ein-/ausschalten und Browserfarben per Farbwahler
                fuer Tab-Leiste, Tabs, Werkzeugleiste, Lesezeichenleiste,
                Seitenleiste, Akzent und Text anpassen.
Uebersetzen     Schaltflaeche „文“ oben rechts uebersetzt die aktuelle Seite.
                Automatische Uebersetzung nutzt Google Uebersetzer und benoetigt
                eine Internetverbindung.

Gespeichert wird in deinem Benutzerordner unter %APPDATA%\RAK
(bookmarks.json, settings.json, history.jsonl). Zum Zuruecksetzen den Ordner loeschen.
