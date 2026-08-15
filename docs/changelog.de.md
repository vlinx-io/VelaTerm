## Unveröffentlicht

### Fernzugriff

- **Wählen Sie, welche Adresse der Freigabe-Link verwendet – Tailscale-Adressen erscheinen jetzt.** Die Adressliste akzeptierte nur die klassischen privaten IPv4-Bereiche, deshalb fielen VPN-Meshes wie Tailscale, die Adressen aus dem Carrier-Grade-NAT-Bereich (100.64.0.0/10) vergeben, stillschweigend aus dem Fernzugriffs-Panel und dem Kopplungslink – obwohl der Server über sie längst erreichbar war. Diese Adressen werden jetzt aufgeführt; VPN-Tunnel stehen am Ende der Liste, damit sie nie zum Standard werden. Eine neue IP-Auswahl im Panel – sichtbar vor dem Start und im laufenden Betrieb – zeigt jeden Kandidaten mit seinem Schnittstellennamen und markiert VPN-Tunnel; wer eine Adresse wählt, rückt ihre URL nach vorn und erzeugt den Kopplungslink neu mit genau diesem Host, sodass der kopierte Link auf einem Gerät funktioniert, das diesen Rechner nur über das VPN erreicht – ohne die URL von Hand zu bearbeiten. Ein QR-Code unter dem Kopplungslink lässt sich direkt mit dem Telefon scannen. Die Auswahl wird gemerkt; verschwindet die gewählte Schnittstelle, fällt das Panel auf „Automatisch“ zurück, ohne die Auswahl zu vergessen. Der Server selbst bleibt unverändert und lauscht weiterhin auf allen Schnittstellen. Wer eine Adresse wählt, die erst nach dem Serverstart aufgetaucht ist – etwa ein später verbundenes VPN –, sieht jetzt auch die kopierte URL und den QR-Code sofort aktualisiert, statt bis zum nächsten Neustart nur den Kopplungslink; VPN-Tunnel stehen auf jeder Plattform hinter den LAN-Adressen, eine bei gestopptem Server getroffene Auswahl bestimmt den allerersten Kopplungslink nach dem Start, und sich überschneidende Link-Erneuerungen können einen neueren Link nicht mehr mit einem älteren überschreiben.

- **Das Teilen überlebt jetzt einen Neustart.** Das Kopplungs-Token wurde bisher bei jedem Serverstart neu erzeugt: Wer VelaTerm beendete und wieder öffnete, machte damit stillschweigend jeden geteilten Link ungültig, und jedes Handy musste neu gekoppelt werden. Das Token, die gekoppelten Geräte und die Geräte-Sperrliste werden jetzt in einer nur für den Besitzer lesbaren Datei im Datenverzeichnis gespeichert: Ein bereits gekoppeltes Gerät verbindet sich nach einem Neustart mit seiner gespeicherten URL – das Zugangspasswort bleibt als zweiter Faktor Pflicht – und ein widerrufenes Gerät bleibt widerrufen. VelaTerm merkt sich außerdem, dass das Teilen aktiv war: Wer die App bei laufendem Server beendet, bekommt ihn beim nächsten Start automatisch auf demselben Port zurück, in der Desktop-App ebenso wie auf einem Server ohne Oberfläche mit `--serve`; wer ihn selbst stoppt, bei dem startet nichts automatisch. Schlägt der automatische Start fehl, etwa weil der Port belegt ist, startet die App normal, und das Fernzugriffs-Panel zeigt den Grund an. Das Port-Feld merkt sich jetzt den tatsächlich verwendeten Port, statt auf den Standardwert zurückzuspringen, und „Link neu erzeugen“ bleibt der ausdrückliche Notausschalter: Es erzeugt sofort ein neues Token, macht alle alten Links ungültig und überschreibt den gespeicherten Zustand. Das Zugangspasswort selbst wird nie auf die Festplatte geschrieben – gespeichert wird ausschließlich ein speicherharter Hash (Argon2id).

### Sicherheit

- **Ein gekoppeltes Gerät kann die Freigabe nicht mehr selbst verwalten.** Jeder gekoppelte Browser konnte dieselben Verwaltungsbefehle aufrufen wie die Desktop-App – einen neuen Kopplungslink erzeugen (was auch die Geräte-Sperrliste leert), andere Geräte auflisten und widerrufen oder den Server stoppen und umkonfigurieren – und der Einstellungsspeicher lieferte jedem Client die komplette Einstellungstabelle, einschließlich des speicherharten Hashes des Zugangspassworts und der Autostart-Einstellungen, die der nächste Start liest. Verwaltungsbefehle sind jetzt der Desktop-App und der Electron-Shell vorbehalten; die Einstellungs-API filtert die Fernzugriffs-Schlüssel und das Gitea-Token aus jedem Lesezugriff eines gekoppelten Geräts und lehnt Schreibzugriffe darauf ab. Ein gekoppeltes Gerät behält, wofür die Kopplung da ist – seine Terminal-Sitzungen samt vollem Shell-Zugriff –, kann aber weder den Passwort-Prüfwert lesen noch andere Geräte einladen oder hinauswerfen noch den Port umlenken, den der nächste Start verwendet. Befehle, die gespeicherte Geheimnisse lesen, schreiben oder löschen – das Gitea-Token und gemerkte Host-Passwörter –, werden für ein gekoppeltes Gerät jetzt ebenfalls abgewiesen, und die pfadnehmenden Befehle – Lesen, Vorschau, Schreiben, Anlegen, Umbenennen und Löschen, ebenso das Anzeigen des Git-Diffs einer Datei und die Wahl des Zielordners beim Klonen eines Repositorys – lösen symbolische Links zuerst auf und lehnen Pfade innerhalb von VelaTerms eigenem Datenverzeichnis ab, in dem Kopplungszustand und Schlüssel liegen – jeder andere Pfad funktioniert weiter, sodass das Durchsuchen und Bearbeiten von Dateien aus der Ferne intakt bleibt. Ein Test zählt jeden Fernbefehl auf, der einen Pfad entgegennimmt, damit ein neuer Befehl nicht unbemerkt an dieser Prüfung vorbeikommt. Weist eine dieser Schutzmaßnahmen eine Anfrage ab, zeigt der Browser jetzt eine ordentlich übersetzte Meldung statt eines rohen englischen Fehlers.

- **Ein Widerruf oder ein neu erzeugter Link überlebt jetzt auch das Doppel-Instanz-Setup.** Auf einem Server ohne Oberfläche (`--serve`) mit aktiviertem Automatikstart hielten zwei Server-Instanzen je eine eigene Kopie des gespeicherten Kopplungszustands und schrieben ihn komplett zurück – ein Widerruf oder ein frischer Kopplungslink über die eine Instanz konnte von der anderen stillschweigend rückgängig gemacht werden. Alle Instanzen eines Prozesses teilen sich jetzt einen einzigen Kopplungszustand pro Datenverzeichnis: Widerruf und Link-Erneuerung wirken sofort überall, und genau ein Schreiber persistiert die Datei, die über echte Neustarts hinweg die maßgebliche Quelle bleibt.

- **Wiederholt fehlgeschlagene Anmeldungen werden gebremst.** Die Prüfung des Zugangspassworts verwendet Argon2id und ist absichtlich teuer – versuchen kann sie jeder, der den Port erreicht. Nach fünf Fehlversuchen von einer Adresse werden weitere Versuche eine Minute lang abgewiesen, bevor irgendeine Hash-Arbeit anfällt, und das Hashing selbst läuft jetzt außerhalb der Ereignisschleife des Servers mit einer harten Obergrenze für gleichzeitige Prüfungen: Eine Flut falscher Passwörter kann den Server weder mit speicherhartem Hashing sättigen noch für bereits verbundene Geräte ausbremsen. Die Bremse lebt im Speicher und wird mit dem Server zurückgesetzt; Kopplungs-Token und Passwort bleiben die eigentliche Hürde. Das Limit teilen sich jetzt alle Server-Instanzen desselben Datenverzeichnisses – das Doppel-Instanz-Setup mit `--serve` verdoppelt das Versuchsbudget nicht mehr –, und ein Versuch wird reserviert, bevor die Passwortprüfung beginnt, damit parallele Anfragen derselben Adresse das Limit nicht unterlaufen können. Ein gebremster Browser sieht auf dem Anmeldebildschirm jetzt eine eigene Rate-Limit-Meldung, statt ein falsches Passwort gemeldet zu bekommen; die Bremse wird außerdem nicht mehr wie ein falsches Passwort gemerkt: Ist die Pause vorbei, geht der nächste Versuch wieder durch, ohne die Seite neu zu laden. Ein mittendrin abgebrochener Versuch – der Tab wurde geschlossen, während das Passwort noch geprüft wurde – gibt seinen reservierten Platz jetzt sofort frei, statt für den Rest der Minute gegen die Adresse zu zählen, und eine erfolgreiche Anmeldung gibt nur ihre eigene Reservierung frei, statt den gesamten Eintrag der Adresse zu löschen: Hinter einer geteilten Netzwerkadresse setzt eine korrekte Anmeldung nicht mehr das Versuchsbudget eines Angreifers zurück, und gezählte Fehlversuche verfallen erst mit ihrer Minute.

- **Geheimnisse auf der Festplatte und in Protokollen werden sorgfältiger behandelt.** Die Datei mit dem Kopplungszustand und der Ende-zu-Ende-Verschlüsselungsschlüssel werden jetzt von Anfang an nur für den Besitzer lesbar angelegt, statt erst nach dem ersten Schreiben eingeschränkt zu werden, und auch die Sitzungsdatenbank – sie enthält den Passwort-Hash – ist auf den Besitzer beschränkt. Ein Server ohne Oberfläche (`--serve`) druckt das langlebige Geheimnis des Kopplungslinks nicht mehr in Protokolle: Ist die Ausgabe kein Terminal, wird der Link zurückgehalten und stattdessen ein Hinweis ausgegeben; `--print-pairing` schaltet das ausdrücklich wieder ein. Das Geräteregister ist auf 32 Einträge mit längenbegrenzten Namen gedeckelt, damit ein gekoppelter Client die gespeicherte Datei nicht unbegrenzt wachsen lassen kann, und schlägt das Speichern eines Widerrufs oder eines neuen Kopplungslinks fehl, erreicht der Fehler jetzt den Aufrufer statt nur eine Protokollzeile. Der Automatikstart ersetzt keinen bereits von Hand gestarteten Server mehr, und eine veraltete Autostart-Fehlermeldung verschwindet, sobald Sie den Server selbst stoppen.

### Fehlerbehebungen

- **Die Kopplung lässt sich jetzt auch aus der Electron-Shell verwalten.** Das Erstellen eines Kopplungslinks, das Auflisten gekoppelter Geräte und das Widerrufen eines Geräts existierten nur als Desktop-Befehle (Tauri); der WebSocket-Dispatcher, den die Electron-Shell und Browser-Clients verwenden, antwortete mit „Unknown command“, sodass das Fernzugriffs-Panel dort nicht funktionierte. Alle drei Befehle laufen jetzt auf beiden Transportwegen durch dieselben Kernfunktionen, damit sie nicht auseinanderdriften können; Regressionstests decken die neuen Dispatch-Routen ab – einschließlich des Erstellens eines echten Kopplungslinks gegen einen laufenden lokalen Server.

## v0.1.100 — 2026-08-10

### KI-Agenten

- **Kiro CLI ist jetzt ein vollwertiger Sitzungstyp.** Kiro-Sitzungen erhalten einen eigenen Knoten im Baum, einen verlässlichen Arbeits- und Wartezustand aus Kiros eigenen lifecycle hooks, Benachrichtigungen am Ende eines Durchlaufs, das automatische Fortsetzen derselben Unterhaltung beim erneuten Öffnen des Knotens, Startargumente samt Schalter zum Überspringen von Bestätigungen sowie den Start über vspawn – alles, was die übrigen Agenten bereits hatten. VelaTerm kopiert Ihren Standard-Kiro-Agenten in einen eigenen `vlx-term`-Agenten, ergänzt die Kopie um rein beobachtende lifecycle hooks und startet diese – Ihre eigene Agentendatei wird nie verändert, und Ihr prompt, Ihre Werkzeuge und Ihre MCP-Server werden unverändert übernommen. Kiro besitzt keinen hook für Berechtigungsanfragen, deshalb bleibt der Statuspunkt auf „arbeitet“ stehen, während Kiro auf Ihre Bestätigung wartet.

### Fehlerbehebungen

- **Aus dem Terminal gestartete Programme erben nicht mehr die Umgebung des AppImage (Linux).** Der AppImage-Starter richtet `PYTHONHOME`, `PYTHONPATH`, `PERLLIB`, `QT_PLUGIN_PATH` und die GStreamer-Plugin-Pfade auf das temporäre Einhängeverzeichnis des Pakets aus und stellt dessen Verzeichnisse in `PATH` und `LD_LIBRARY_PATH` allem anderen voran. Ein Terminal gibt seine gesamte Umgebung an die gestartete Shell weiter, deshalb suchte das System-`python3` seine Standardbibliothek im Paket und startete überhaupt nicht mehr, und andere dynamisch gelinkte Programme luden die Bibliothekskopie aus dem Paket statt der des Systems. VelaTerm entfernt diese Paketpfade jetzt, bevor eine Shell oder ein externes Werkzeug gestartet wird, und lässt selbst gesetzte Werte unangetastet. `APPDIR` und `APPIMAGE` bleiben sichtbar, damit Programme, die prüfen, ob sie aus einem AppImage laufen, weiterhin ihre Antwort erhalten. Betroffen waren nur AppImage-Builds; das deb-Paket, macOS und Windows verhalten sich wie bisher.

## v0.1.99 — 2026-08-09

### Terminal

- **Shift+Enter fügt einen Zeilenumbruch ein, statt abzuschicken.** Terminals kennen keine Kodierung für Enter mit Zusatztaste, deshalb erhielten Agenten-CLIs wie Claude Code und Codex nur einen einfachen Wagenrücklauf und schickten die Eingabe ab, während man noch schrieb. VelaTerm sendet nun ESC+CR, also genau die Sequenz, die diese Werkzeuge von einer iTerm2-Tastenbelegung erwarten. Damit funktionieren mehrzeilige Eingaben – auch unter macOS, wo die eigene Tastenbehandlung bisher überhaupt nicht installiert war. Während der Eingabemethoden-Komposition bleibt alles unverändert, Enter bestätigt weiterhin den Kandidaten.

### Projekte und Organisation

- **Den Status einer einzelnen Sitzung aktualisieren.** Sitzungen in einem statusgefilterten Bereich erhalten die Aktion „Status aktualisieren“, die ausschließlich diese eine Sitzung anhand der Bedingungen des jeweiligen Bereichs neu bewertet und sie aufnimmt oder entfernt, während alle übrigen Sitzungen an ihrem Platz bleiben. Die Aktion gehört zu dem Bereich, aus dem das Menü geöffnet wurde, sodass verschachtelte Teilungen nie den Filter eines anderen Bereichs verwenden. Das Ergebnis wird pro Bereich gespeichert und nach einem Neustart wiederhergestellt.
- **Eine Markierung zu entfernen kostet einen Klick.** Wer das bereits gesetzte Emoji erneut auswählt, entfernt es damit; der eigene Eintrag zum Entfernen und seine Trennlinie entfallen. Auch das Emoji-Abzeichen an der Filterschaltfläche ist verschwunden: Die Hervorhebung zeigt bereits, dass ein Markierungsfilter aktiv ist, und welches Emoji es ist, steht im Menü.

### Fehlerbehebungen

- **Die Desktop-Integration des Linux-AppImage lässt sich auf jedem Rechner installieren.** Das mitgelieferte Symbol war ein symbolischer Link auf einen absoluten Pfad des Build-Rechners, weshalb Werkzeuge wie Gear Lever und AppImageLauncher es nicht extrahieren konnten, obwohl die Anwendung selbst normal lief. Der Link ist jetzt relativ. Außerdem wurde die angegebene glibc-Anforderung auf 2.35 korrigiert, nachdem nicht nur die ausführbare Datei, sondern auch die mitgelieferten Bibliotheken gemessen wurden. Damit ist Ubuntu 22.04 die älteste unterstützte Distribution für die Desktop-Anwendung.

## v0.1.98 — 2026-08-02

### KI-Agenten

- **Grok Build wird zu einem vollwertigen Agenten in VelaTerm.** Grok 4.5 lässt sich installieren, starten und fortsetzen – mit stabilen Sitzungs-IDs, offiziellen lifecycle hooks, präzisen Arbeits- und Berechtigungszuständen, zusammengeführten Transkripten, Nutzungsdetails und einem offiziellen, an das Theme angepassten Symbol in Desktop-, Browser- und Mobilansichten.

### Projekte und Organisation

- **Teilen Sie die Projekt-Seitenleiste in unabhängige Arbeitsansichten.** Jeder Baumbereich kann erneut nach unten geteilt werden und stellt nach einem Neustart seine eigene Suche, Status- und Emoji-Filter, den eingeklappten Zustand sowie das Größenverhältnis wieder her. Alle Bereiche bleiben Projektionen desselben, vom Backend verwalteten Projektbaums, sodass Änderungen ohne doppelte Geschäftsdaten synchron bleiben.
- **Markieren und filtern Sie Knoten, ohne den Kontext zu verlieren.** Projekte, Gruppen und Sitzungen können Emoji-Markierungen tragen. Ein markierter Container behält seinen vollständigen Unterbaum, die Statuszugehörigkeit bleibt während der Arbeit stabil, dynamisches Hinzufügen und manuelles Aktualisieren stehen beide bereit, und Status- sowie Emoji-Bedingungen werden als Vereinigungsmenge kombiniert.
- **Erstellen Sie direkt ein leeres Projekt.** Wählen Sie ein übergeordnetes Verzeichnis, prüfen Sie den Namen und erstellen und importieren Sie den Ordner in einem Ablauf. Bei einem Teilfehler wird nur der Import wiederholt, ohne doppelte Verzeichnisse anzulegen.

### Oberfläche

- **Teilen Sie VelaTerm dort, wo Ihre Community ist.** Der Teilen-Dialog unterstützt jetzt WeChat Moments, Weibo, Xiaohongshu, X, Reddit, Hacker News, LinkedIn, Facebook, Telegram und WhatsApp – einschließlich QR-Code-Ablauf für WeChat und einem Teilen-Hinweis im Aktualisierungsdialog.
- **Auch kleine Interaktionen wirken durchdachter.** Temporäre Terminal-Tabs lassen sich umbenennen, bevor sie zu gespeicherten Sitzungen werden. Normale Eingabefelder deaktivieren die automatische Großschreibung mobiler Tastaturen, ohne die Terminaleingabe zu verändern.

## v0.1.97 — 2026-07-25

### KI-Agenten

- **Sitzungen bleiben nicht mehr auf „arbeitet“ stehen.** Codex meldete Werkzeugaktivität und Durchlaufende über getrennte, kurzlebige Prozesse, deren Callbacks in vertauschter Reihenfolge eintreffen konnten, sodass ein bereits beendeter Durchlauf weiterhin als arbeitend angezeigt wurde. Zwischenmeldungen, die nach dem Ende ihres eigenen Durchlaufs eintreffen, werden nun verworfen, und ein zusätzlicher Sitzungsende-hook deckt Sitzungen ab, die ohne Abschlussereignis enden.
- **Abgebrochene Durchläufe klären sich in Sekunden.** Esc oder ein Stream-Fehler beendet einen Claude- oder Codex-Durchlauf ganz ohne Abschluss-Callback. Sechs Sekunden Stille im Terminal korrigieren eine solche Sitzung jetzt still auf wartend, ohne eine „hat geantwortet“-Benachrichtigung auszulösen.

### Oberfläche

- **Zuverlässige Teilen-Kurzbefehle unter macOS.** Rechts teilen (Cmd+D) und nach unten teilen (Cmd+Shift+D) sind nun auch als native Terminal-Menübefehle registriert, sodass macOS die Tastenkombination nicht mehr vor VelaTerm abfängt.
- **Pro Tastendruck genau ein Speichervorgang.** Cmd+S wurde sowohl vom globalen Kurzbefehl als auch vom fokussierten Editor verarbeitet und konnte dieselbe Datei bei einem einzigen Tastendruck zweimal schreiben.

## v0.1.96 — 2026-07-23

### KI-Agenten

- **Codex-Status aus lifecycle hooks statt Terminalvermutungen.** Aktuelle Codex-Sitzungen verwenden ausschließlich die offiziellen lifecycle hooks als Aktivitätsquelle. Ein `SessionStart`-Handshake prüft die Verbindung, fehlende Callbacks werden als „Status nicht verfügbar“ angezeigt und Terminaltext oder Ausgabeaktivität kann Arbeits-, Bestätigungs- oder Abschlusszustände nicht mehr überschreiben.
- **Aktuellere Codex-Nutzung nach jedem Durchlauf.** Der Info-Bereich zeigt sofort den lokalen rollout-Snapshot, gleicht ihn mit den Live-Limits ab, aktualisiert nach dem letzten von Codex geschriebenen token-Snapshot erneut und verwirft verspätete Antworten einer älteren Sitzung.

### Oberfläche

- **Zuverlässige Treffer im Projektbaum unter macOS.** Virtuelle Zeilen benötigen keine Compositor-transform mehr. Dadurch können veraltete WKWebView-Hit-Test-Koordinaten Hover-, Klick- oder Ziehaktionen nach dem Scrollen oder einer Baumaktualisierung nicht mehr an eine andere Zeile senden.

## v0.1.95 — 2026-07-21

### KI-Agenten

- **Kimi Code und Zoo Code sind jetzt im Sitzungsbaum verfügbar.** VelaTerm kann beide Agenten starten, fortsetzen, installieren und konfigurieren. Kimi meldet Arbeits-, Berechtigungs- und Wartezustände über offizielle lifecycle hooks; Zoo Code behält eine stabile Task-ID und nutzt bei fehlenden externen hooks die Terminalerkennung.
- **Codex-Nutzung live aktualisieren.** Der Info-Bereich fragt aktuelle Limits beim Codex app server ab und fällt kompatibel auf den lokalen rollout-Snapshot zurück.

### Projekte und Terminals

- **Projekte mit `vela <path>` öffnen.** Paketierte Builds können einen Shell-Befehl nach VS-Code-Vorbild installieren. Ein zweiter Aufruf übergibt das Projekt an das vorhandene VelaTerm-Fenster, statt eine doppelte Instanz zu öffnen.
- **Sichtbares, abbrechbares Git-Klonen.** Clone Project zeigt Git-Phase, Prozent und Laufzeit, warnt bei Stillstand und beendet beim Abbruch den gesamten Git-Prozessbaum ohne halbfertiges Ziel. Zugangsdaten und query tokens werden in Fehlern und Audit-Logs geschwärzt.
- **WSL-Terminals unter Windows.** Alle installierten WSL-Distributionen stehen neben PowerShell, cmd und Git Bash für normale Terminals bereit. Agent-Sitzungen bleiben im Windows-Host-Shell, damit hooks und Programmpfade zuverlässig funktionieren.

### Oberfläche und Zuverlässigkeit

- **Klarere Kontrolle über Hintergrundsitzungen.** Menüs zeigen den Live-Status jeder Sitzung; im Limitdialog lassen sich mehrere ausgewählte Tabs gleichzeitig beenden.
- **Sichererer App-Lebenszyklus und mehrsprachige Hinweise.** Vor dem Beenden aktiver Sitzungen wird bestätigt, die genaue Codex-lifecycle-Identität hat Vorrang vor mehrdeutigen rollout-Scans und Update-Hinweise unterstützen alle integrierten Sprachen.

## v0.1.94 — 2026-07-12

### Lokalisierung

- **Vietnamesische Benutzeroberfläche.** Tiếng Việt ist jetzt in der Sprachauswahl verfügbar und wird bei vietnamesischen Systemgebietsschemas automatisch ausgewählt.

### Browser

- **Schnellerer Start des integrierten Browsers.** Jeder Browser-Tab bietet jetzt Ein-Klick-Verknüpfungen für ChatGPT, Claude, Gemini und Google. Über die Kontextmenüs von Projekten und Gruppen lässt sich außerdem direkt an der entsprechenden Stelle im Sitzungsbaum eine dauerhafte Browserseite erstellen.

### Bilder und Dokumente

- **Zuverlässiges Einfügen von Bildpfaden unter macOS.** Wenn WebKit ein kopiertes Bild nicht als Datei bereitstellt, liest VelaTerm es jetzt aus der nativen Zwischenablage und lädt es weiterhin als Dateipfad hoch, statt unbemerkt auf den nativen Bildplatzhalter eines Agenten zurückzufallen. Remote-Fenster zeigen die Einstellung zum Einfügen von Bildern immer an, erklären, warum der Dateipfadmodus erforderlich ist, und deaktivieren die nicht verfügbare native Option.
- **Bilder in Quelldokumente einfügen.** Der Quelltexteditor akzeptiert jetzt Bilder aus der Zwischenablage. Gespeicherte Markdown-Dokumente legen sie neben dem Dokument unter `assets/` ab und fügen portable Markdown-Bildsyntax ein. Nicht gespeicherte Entwürfe betten die Bilddaten ein, damit sie beim Bereinigen temporärer Dateien nicht verloren gehen.

### Oberfläche

- **Kontextmenüs bleiben sichtbar und zielen auf das richtige Element.** Menüs am rechten Rand werden korrekt vermessen und verschoben. Ein Rechtsklick auf einen Baumknoten hebt jetzt nur das Menüziel hervor, ohne die bestehende Auswahl zu ändern. Gruppenmenüs enthalten außerdem ein Terminal für die jeweilige Gruppe.
- **Übersichtlichere Bearbeitung und Statusbeschriftungen.** Quelltext zeigt für Zeichenfolgen wie HTML-Kommentare keine pfeilförmigen Schriftligaturen mehr an. Nutzungsprozente sind ausdrücklich als verwendet gekennzeichnet, und das unbeteiligte native Kontextmenü des Host-WebView erscheint nicht mehr hinter den VelaTerm-Menüs.

### Fehlerbehebungen

- **Codex bleibt im normalen Terminalverlauf.** Von VelaTerm gestartete Codex-Sitzungen verwenden jetzt den Inline-Terminalmodus. Beim Drücken von Esc zum Unterbrechen oder Zurückgehen werden daher weder die Terminal-Bildschirmpuffer gewechselt noch die Scrollback-Ansicht an den Anfang versetzt. Ihre eigene Codex-Konfiguration bleibt unverändert.
