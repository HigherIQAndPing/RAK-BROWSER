RAK - Render Accelerated Kinetic
================================
R = Render, A = Accelerated, K = Kinetic
Ein nativer Desktop-Browser in Rust (tao + wry) mit Tab-Inseln.

WINDOWS: STARTEN OHNE RUST
-------------------------
1. https://github.com/HigherIQAndPing/RAK-BROWSER/releases oeffnen.
   Das Repository ist privat: Mit einem GitHub-Konto anmelden, das Zugriff hat.
2. Im gewuenschten Release unter "Assets" RAK-windows-x64.zip herunterladen.
3. ZIP vollstaendig entpacken (Rechtsklick -> Alle extrahieren).
   Nicht aus der ZIP-Vorschau heraus starten.
4. RAK.exe oder start.bat doppelklicken. Fertig.

Das Paket enthaelt RAK.exe, start.bat, README.txt und setup-webview2.bat.
Rust, Cargo und Visual Studio sind zum Benutzen NICHT erforderlich.
Die MSVC-C-Laufzeit wird statisch eingebunden: Ein separates Visual C++
Redistributable ist fuer den mitgelieferten Build nicht erforderlich.
Das entpackte Paket laesst sich ohne RAK-Installer verschieben.
Benutzerdaten werden trotzdem im Windows-Benutzerprofil gespeichert,
nicht neben der EXE. "Portabel" bezieht sich hier auf das Programm.

Voraussetzung ist ein Windows-x64-System mit Microsoft Edge WebView2 Runtime
(Evergreen). Die Runtime ist NICHT im ZIP enthalten. Unter Windows 11 ist sie
normalerweise vorhanden, ebenso auf vielen Windows-10-Systemen. Der installierte
Edge-Browser allein ist kein verlaesslicher Ersatz fuer die WebView2 Runtime.

Falls RAK wegen einer fehlenden WebView2 Runtime nicht startet, die optionale
setup-webview2.bat oeffnen oder diese offizielle Microsoft-Seite besuchen:
https://developer.microsoft.com/en-us/microsoft-edge/webview2/#download-section
Dort "Evergreen Standalone Installer", Architektur x64, waehlen; alternativ
ist der "Evergreen Bootstrapper" verfuegbar. Die Installation selbst erfolgt
nur nach deiner Entscheidung. setup-webview2.bat oeffnet lediglich die Seite.
Dokumentation der Voraussetzung:
https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution

start.bat sucht zuerst RAK.exe neben dem Skript und danach fuer lokale
Entwickler-Builds target\release\rak.exe. Falls die EXE fehlt, zeigt es
Downloadhinweise an, oeffnet die Release-Seite und beendet sich mit Fehler.
start.bat installiert weder Rust noch WebView2 und kompiliert nichts.
Ein Internetzugang wird erst fuer Webseiten und Onlinefunktionen benoetigt.
Nicht signierte Builds koennen einen Windows-SmartScreen-Hinweis ausloesen;
nur Dateien aus einer Quelle starten, der du vertraust.

Falls noch kein Release existiert, sind aktuelle Builds unter
https://github.com/HigherIQAndPing/RAK-BROWSER/actions verfuegbar:
Den erfolgreichen Lauf "Windows portable" oeffnen und unter "Artifacts"
RAK-windows-x64 herunterladen. GitHub liefert dabei RAK-windows-x64.zip mit
allen Startdateien direkt im ZIP. Anmeldung und Repository-Zugriff sind noetig.
Build-Artefakte werden 30 Tage aufbewahrt; Release-Assets sind fuer dauerhafte
Versionen gedacht. Auch ein erfolgreicher manueller Lauf erstellt nur ein
Artefakt, nicht automatisch ein Release.

PATCHNOTES
----------
Jedes Windows-Release bekommt eigene deutsche Patchnotes. Die Release-Automatik
ordnet die konkreten Aenderungen der jeweiligen Versionsnummer zu; wiederholte
Standardtexte werden nicht erneut verwendet. Die Notes enthalten neben den
Versionsaenderungen weiterhin die gemeinsame Anleitung zum Entpacken und Starten.

TAB-INSELN
----------
Zusammengehoerige Tabs lassen sich wie bei Opera in Inseln organisieren.
Links, die eine Webseite in einem neuen Tab oeffnet, bilden automatisch mit
ihrem Ursprungstab eine Insel beziehungsweise treten dessen Insel bei.

Mit Strg + Klick mehrere Tabs auswaehlen; Umschalt + Klick waehlt einen
zusammenhaengenden Bereich mit gleichem Privatmodus. Dann rechtsklicken oder
den Insel-Knopf neben + nutzen und unten "Gruppieren" anklicken. Dort lassen
sich Tabs auch in eine vorhandene Insel verschieben oder aus einer Insel loesen.
Farbig markierte Insel-Griffe helfen beim Wiederfinden. Per Klick lassen sich
Inseln einklappen und wieder aufklappen; Tabs bleiben dabei geoeffnet.
Rechtsklick auf den Insel-Griff bietet Name/Farbe, + Tab, Aufloesen (Tabs
behalten) und Insel schliessen (alle ihre Tabs schliessen). Die Aktionen werden
in der Lesezeichenzeile gezeigt; Escape oder x schliesst diese Aktionsleiste.
Per Drag-and-drop einen Tab auf einen anderen Tab ziehen, um zu gruppieren,
oder auf einen Insel-Griff ziehen, um der Insel beizutreten.
Private und normale Tabs bleiben in getrennten Inseln; eine Gruppe mischt
keine privaten und normalen Browsersitzungen.
Inseln und Tabs gelten fuer die aktuelle Sitzung; sie werden nicht nach einem
Neustart wiederhergestellt. Das bestehende Limit von insgesamt 12 Tabs bleibt.
Viele Tabs sind per Mausrad ueber der Tab-Leiste oder mit den Pfeilen erreichbar.

STARTSEITE
----------
Beim Start und sobald der letzte Tab geschlossen wird, bleibt RAK geoeffnet und
zeigt eine interne Startseite mit RAK-Logo, Suche, Schnellzugriffen und Buttons
fuer neue beziehungsweise private Tabs. Die Schnellzugriffe werden als
Navigation aus Rust an einen normalen Tab uebergeben; die Startseite selbst
laedt keine externe Webseite. Die Home-Schaltflaeche oben oeffnet sie jederzeit
wieder. Startseiten-Tabs werden wie Verlauf, Lesezeichen und Einstellungen als
interne Browser-Seiten behandelt.

FENSTERLEISTE
-------------
RAK verwendet eine eigene, schmale Leiste ganz oben anstelle der nativen
Titelleiste. Links stehen das RAK-Zeichen und der Name, rechts liegen
Minimieren, Maximieren/Wiederherstellen und Schliessen. Der freie Bereich ist
ziehbar, ein Doppelklick darauf schaltet zwischen maximiert und normal um. Die
gesamte Browseroberflaeche passt sich beim Verkleinern und Maximieren an; die
Startseite wechselt dabei ihr Raster automatisch von vier auf zwei Spalten.

Tastenkuerzel, solange die Browser-Oberflaeche oder Adressleiste den Fokus hat:
Strg+T neuer Tab, Strg+Umschalt+N privater Tab, Strg+W aktiven Tab schliessen,
Strg+Tab / Strg+Umschalt+Tab wechseln, Alt+T neuer Tab in der aktiven Insel.
Strg+T, Strg+Umschalt+N und Strg+W werden jetzt direkt vom Rust-Fenster
verarbeitet und funktionieren deshalb auch bei fokussierter Webseite. F11
schaltet den Browser in den Vollbildmodus und wieder zurueck.
Die Befehle werden zusaetzlich in jede WebView injiziert, damit WebView2 die
Tastendruecke nicht verschluckt; externe Webseiten erhalten dabei keine freie
Browser-IPC-Schnittstelle, sondern nur diese vier fest erlaubten Befehle.
F11 ist echter Vollbildmodus: Die obere RAK-Fensterleiste wird ausgeblendet
und auch Tab-Leiste, Adress-/Werkzeugleiste sowie Lesezeichenleiste werden
ausgeblendet. Im Vollbild bleibt nur der Webseiteninhalt sichtbar.

Der Homescreen ist ein vollflaechiges, responsives Raster. Karten, Schrift,
Abstaende und Innenabstaende skalieren gemeinsam mit Breite und Hoehe des
Fensters; beim Verkleinern wechselt das Raster auf zwei Spalten, ohne dass
horizontal und vertikal unproportionale Streckungen entstehen.

WEITERE FUNKTIONEN
------------------
Tabs            + = neuer Tab, Masken-Symbol = privater Tab.
Adressleiste    Adresse oder Suchbegriff. Suchanbieter: Google, DuckDuckGo,
                Ecosia oder Bing. Spezialadressen: rak://verlauf,
                rak://lesezeichen, rak://einstellungen und rak://erweiterungen.
Lesezeichen     Stern in der Adressleiste = Seite merken. Name und Ordner sind
                waehlbar. Die Lesezeichenleiste zeigt Ordner und Lesezeichen;
                das x entfernt Eintraege. Ordner und Lesezeichen lassen sich
                ueber das Ordner-Symbol verwalten.
Verlauf         Uhr-Symbol: durchsuchbarer Verlauf, nach Tagen sortiert;
                einzelne Eintraege oder den gesamten Verlauf loeschen.
Privat          Private Tabs werden nicht im lokalen Verlauf gespeichert und
                verwenden eine private WebView-Sitzung. Private und normale
                Tabs bleiben getrennt. Manuell gesetzte Lesezeichen koennen
                auch aus privaten Tabs dauerhaft gespeichert werden.
                Privatmodus macht dich NICHT anonym gegenueber Webseiten,
                Netzwerkbetreibern oder angemeldeten Konten.
Seitenleiste    Spotify, Instagram, X, TikTok und ChatGPT als Panel neben der
                Seite. Breite am Griff oder ueber + / - anpassen; Doppelklick
                auf den Griff setzt die Standardbreite. Stift-Symbol = Apps
                bearbeiten. ChatGPT wird als Web-App geoeffnet; ein API-Schluessel
                wird von RAK nicht benoetigt.
Erweiterungen   Puzzle-Symbol oder rak://erweiterungen. Erweiterungsadresse
                aus dem Chrome Web Store einfuegen und installieren.
                Nach dem Download und Entpacken ist ein Neustart noetig.
                Nur Windows/WebView2. Keine bedienbare Erweiterungsleiste;
                nicht alle Chrome-Funktionen oder Erweiterungen funktionieren.
                Private Tabs nutzen keine Erweiterungen.
                Ablage: %APPDATA%\RAK\extensions. Der Download nutzt curl.exe.
Import           In Einstellungen koennen kompatible Erweiterungen aus Chrome,
                 Edge, Brave und Opera/Opera GX aus den lokalen Profilen
                 importiert werden. Firefox wird durchsucht; inkompatible XPI-
                 Erweiterungen werden uebersprungen. Danach RAK neu starten.
Einstellungen   Zahnrad: Suchmaschine, automatische Uebersetzung und Farben
                fuer Tab-Leiste, Tabs, Werkzeugleiste, Lesezeichenleiste,
                Seitenleiste, Akzent und Text anpassen.
Uebersetzen     Uebersetzung der aktuellen Seite; automatische Uebersetzung
                nutzt Google Uebersetzer und benoetigt eine Internetverbindung.

Benutzerdaten liegen unter %APPDATA%\RAK, unter anderem bookmarks.json,
settings.json, history.jsonl und extensions. Zum Zuruecksetzen RAK schliessen
und diesen Ordner loeschen; dabei gehen dort gespeicherte Daten verloren.

NUR FUER ENTWICKLER: SELBST BAUEN
--------------------------------
Diesen Abschnitt ueberspringen, wenn du nur den Browser benutzen willst.
Ein eigener Windows-x64-Build benoetigt Rust (MSVC-Toolchain), Visual Studio
C++ Build Tools fuer x64/x86 und ein Windows SDK. Rust: https://rustup.rs
Danach ein neues Terminal oeffnen. Es wird nichts durch start.bat installiert.

build-exe.bat prueft die Windows-x64-MSVC-Toolchain, baut mit
cargo build --release --locked und kopiert die EXE neben start.bat.
Es verwendet explizit das x64-MSVC-Ziel und den lokalen target-Ordner.
Alternativ im Repository mit einer x64-MSVC-Toolchain:
  cargo test --locked
  cargo build --release --locked
  start.bat
Der direkte Cargo-Build liegt normalerweise in target\release\rak.exe.
Die Datei .cargo\config.toml aktiviert crt-static nur fuer Windows/MSVC.
Eigene RUSTFLAGS koennen diese Projektkonfiguration ueberschreiben.
Cargo.lock wird eingecheckt; --locked verhindert unbemerkte Lockfile-Aenderungen.

Optionales lokales Packen mit PowerShell und installierten MSVC-Build-Tools:
  .\packaging\package-windows.ps1
Nach build-exe.bat kann die kopierte EXE als Eingabe dienen:
  .\packaging\package-windows.ps1 -Executable .\RAK.exe
Die Pakethilfe prueft x64 und unerwuenschte dynamische CRT-/Loader-Abhaengigkeiten.
Ergebnis: dist\RAK-windows-x64.zip; WebView2 selbst wird nicht gebuendelt.

GITHUB ACTIONS UND RELEASES
--------------------------
Der Workflow .github/workflows/windows-portable.yml laeuft auf windows-latest:
- Push auf main: cargo test --locked, Release-Build und Download-Artefakt.
- "Actions -> Windows portable -> Run workflow": manueller Artefakt-Build.
- Push eines Tags v*: gleicher Build und danach ein GitHub Release mit dem ZIP.
  Beispiel fuer berechtigte Maintainer, erst nach einem Commit der Aenderungen:
    git tag v0.2.0
    git push origin v0.2.0

Ein manuell auf einem Tag gestarteter Lauf erstellt bewusst kein Release.
Zum Wiederholen eines Tag-Releases den urspruenglichen Tag-Push-Lauf erneut
starten. Vorhandene Release-Assets werden fuer diesen Tag ersetzt.
Versionsnummer in Cargo.toml/Cargo.lock und Tag muessen vor dem Tag-Push passen.
Die Release-Erstellung verwendet ausschliesslich GITHUB_TOKEN mit contents:write;
das Build-Job hat nur contents:read. Kein persoenlicher Token ist erforderlich.
Repository-/Organisationsregeln muessen Actions und Release-Schreibzugriff
zulassen; private Repositorys benoetigen verfuegbare Actions-Minuten.
Releases erben die private Repository-Sichtbarkeit. Der Workflow aendert diese
nicht und enthaelt keine geplanten Dienste oder wiederkehrenden Aufgaben.

HOMESCREEN-DESIGN
-----------------
Der Homescreen-Header mit RAK-Logo und Startseite ist zentriert und skaliert
mit dem Fenster. Das Logo nutzt einen farbigen, modernen Verlauf. Die
Schnellzugriff-Icons stehen mittig in ihren Karten, sind groesser und skalieren
proportional mit Breite und Hoehe. Auch der Platzhalter "Suchen oder Adresse
eingegeben" passt seine Schriftgroesse an die Fensterhoehe an.

Die Startseite wurde in Version 0.9.2 auf das ruhigere Layout aus Version 0.6
zurueckgesetzt. Darunter zeigt der neue Bereich "Zuletzt geoeffnet" bis zu
acht zuletzt besuchte Webadressen aus dem lokalen Verlauf. Ein Klick oeffnet
den jeweiligen Eintrag erneut; wenn noch kein Verlauf vorhanden ist, erscheint
ein entsprechender leerer Zustand.
Die Einträge werden seit Version 0.9.3 fest in zwei Spalten und bis zu vier
Zeilen dargestellt. Die Karten, Icons und Texte wurden dafür vergrößert.

AUTOMATISCHE UPDATES
--------------------
RAK sucht kurz nach dem Start und danach alle 4 Stunden nach einer neuen Version (Releases dieses
Repositories) und installiert sie im Hintergrund. Danach erscheint in der Leiste "Update - Neustart".
Akzeptiert werden nur Updates mit gueltiger Ed25519-Signatur (oeffentlicher Schluessel ist in
src/update.rs eingebaut), passender SHA-256-Pruefsumme und HOEHERER Versionsnummer.
Voraussetzungen: Das Repository (bzw. REPO in src/update.rs) muss oeffentlich sein, und RAK.exe muss in
einem beschreibbaren Ordner liegen. Veroeffentlichen: Secret RAK_SIGNING_KEY setzen, Version in
Cargo.toml erhoehen, dann "git tag vX.Y.Z" und "git push --tags".

SITZUNG WIEDERHERSTELLEN
------------------------
Beim naechsten Start oeffnet RAK die zuletzt offenen normalen Tabs wieder (Datei session.json im
RAK-Datenordner, %APPDATA%\RAK). Private Tabs und interne Seiten werden nie gespeichert.
