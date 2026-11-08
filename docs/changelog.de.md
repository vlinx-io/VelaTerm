## v0.3.0 — 2026-11-08

- 🔏 Die macOS-Version wird jetzt mit dem Zertifikat von Vlinx Software, Inc. statt mit dem bisherigen signiert. Nach dem Update müssen Sie einige macOS-Berechtigungen möglicherweise erneut erteilen, und im Schlüsselbund gespeicherte Einträge wie Passwörter für Remote-Verbindungen müssen noch einmal freigegeben werden: Geben Sie bei der Abfrage Ihr Anmeldepasswort ein und wählen Sie „Immer erlauben“.
- 🗂️ Sitzungen gibt es jetzt in drei Typen – Aufgabe, Projekt und Chat –, die Sie oben auf der Seite „Neue Sitzung“ auswählen, und die linke Seitenleiste ist in die festen Sammlungen Projekte, Terminals, Aufgaben und Chats gegliedert, wobei Terminals alle laufenden Terminals und die darin erkannten Agenten auflistet.
- 🧭 Über eine neue Aktivitätsleiste am linken Rand wechseln Sie zwischen den Bereichen Projekte, Terminals, Aufgaben, Chats, Archivierte Sitzungen und Wissensbasis, und unten finden Sie Einstellungen, Konto, Feedback, Teilen und Spielecenter.
- 📦 Sammlungen lassen sich über „Sammlung archivieren“ im Kontextmenü vollständig archivieren; wird eine ihrer Sitzungen wiederhergestellt, ist auch die Sammlung wieder da.
- 🆕 Auf der Seite „Neue Sitzung“ wählen Sie Projekt, Agent, Modell, Denkaufwand und Berechtigungen in einer Einstellungskarte über dem Eingabefeld, und über die Projektliste können Sie ein Projekt erstellen, importieren oder klonen, ohne die Seite zu verlassen.
- 🩺 Beim Öffnen eines Gesprächs wird geprüft, ob dessen Agent installiert und angemeldet ist: Falls nicht, erscheint ein Installationshinweis oder ein Anmeldebereich, und „Senden“ bleibt deaktiviert, bis der Agent bereit ist.
- 🔧 Ein Agent gilt nur dann als installiert, wenn er tatsächlich läuft: Eine abgebrochene Installation oder ein fehlerhaftes Upgrade zeigt daher die Installationsanleitung an, Ein-Klick-Installationen gelten erst als abgeschlossen, wenn der Agent startet, und ein Update, nach dem der Agent nicht mehr startet, wird als unvollständig gemeldet.
- 🪟 Der neue Befehl `vartifact <file|URL>` öffnet eine Datei in einem Bereich neben dem Gespräch, mit Live-Vorschau für HTML und bearbeitbaren Ansichten für Markdown, Bilder und Code, die Sie mit ⌘S speichern, während `vopen` jetzt immer einen Tab öffnet.
- 💤 Abgeblendete Inline-Vorschläge im Terminal sind jetzt standardmäßig aus, die Vervollständigung steht wieder auf „Nur Tab“, und eine bestehende Einstellung „Inline + Tab“ wird mit diesem Update einmalig auf „Nur Tab“ umgestellt; in den Einstellungen lassen sich die Vorschläge wieder einschalten.
- ⌨️ Im Modus „Inline + Tab“ können Sie über die neue Einstellung „Tab-Taste“ festlegen, dass Tab den abgeblendeten Vorschlag übernimmt; ein zweites Tab macht dies rückgängig und listet die Treffer auf, und die Titelleiste des Terminals zeigt, welche Tasten verfügbar sind, solange ein Vorschlag sichtbar ist.
- 🎨 Beim Bearbeiten einer Zsh-Befehlszeile wird bereits eingegebener Text nicht mehr grau dargestellt, und die Beschreibung des Vorschlagsmodus in den Einstellungen entspricht dem ausgewählten Modus.
- 📡 Die Tab-Vervollständigung über Remote-Verbindungen antwortet nach einem einzigen Hin- und Rückweg.
- 💬 In ZCode-Sitzungen können Sie mit Option/Alt+Enter eine Ergänzung senden, die nach dem nächsten Werkzeugaufruf in den laufenden Durchgang einfließt.
- 🔑 Ab ZCode 3.15 ist die Anmeldung per OAuth möglich, und ein erworbener GLM Coding Plan wird nicht mehr als fehlend oder aufgebraucht gemeldet.
- 📥 Sitzungen aus Pi, OMP, ZCode und DeepSeek Harness lassen sich importieren, und der Dialog „Sitzungen importieren“ kann nach Agent filtern.
- 🔤 Die Oberfläche verwendet durchgehend Inter, während Code, Werkzeugausgaben, Pfade und Terminals in JetBrains Mono bleiben, und die Einstellung für die Schriftart der Oberfläche entfällt.
- 📚 Die Wissensbasis zeigt Einträge in einer zentrierten Spalte an, listet archivierte Sitzungen als Baum aus Projekten, Gruppen und übergeordneten Sitzungen mit einer Ansicht „Nach Datum“ auf, und die lokale Wissensbasis lässt sich zwischen Karten- und Listenansicht umschalten.
- 🔔 Unter macOS und Windows holt ein Klick auf eine Benachrichtigung das zugehörige Fenster in den Vordergrund, sodass eine Benachrichtigung aus einem Remote-Fenster nicht mehr das Hauptfenster davor schiebt.
- 📋 Unter macOS friert das Einfügen die App nicht mehr ein, wenn die Quell-App der Zwischenablage nicht mehr reagiert: VelaTerm bricht nach 5 Sekunden ab und weist Sie darauf hin.
- 📝 „Alles auswählen“ im macOS-Menü oder im Kontextmenü des Quelltexteditors markiert die ganze Datei, sodass beim Kopieren keine Zeilen außerhalb des sichtbaren Bereichs mehr fehlen.
- 📏 Das Eingabefeld wird nach dem Senden einer Nachricht nicht mehr erst größer und dann wieder kleiner.
- 📂 Die Sammlungen Terminals, Aufgaben und Chats zeigen ihre Inhalte aufgeklappt an, unabhängig davon, was unter Projekte eingeklappt ist.
- 🔄 Neue Sitzungen scheitern nicht mehr mit `cursor_expired` an der Synchronisierung, nachdem Sie das Gespräch einer Teilaufgabe geöffnet haben.
- 🧯 Nach einer langen Verbindungsunterbrechung mit großem Rückstand bleibt das Gespräch beim Wiederverbinden nicht mehr mit deaktiviertem „Senden“ hängen.
- 🪞 Remote-Verbindungen zu Windows-Hosts mit Cygwin oder Git Bash nutzen die dort bereits laufende VelaTerm-Desktop-App, sodass das Senden einer Nachricht nicht mehr fehlschlägt, während diese App die Sitzung ausführt.
- ⚡ Das Indizieren langer Codex-Gespräche benötigt nur noch etwa halb so viel CPU-Zeit.
- 🔗 Links zu Claude-Artefakten öffnen sich in Ihrem Standardbrowser.
- 🗄️ Die Schaltfläche „Archivierte Sitzungen“ ist von der Tab-Leiste in die Aktivitätsleiste umgezogen.

## v0.2.15 — 2026-11-03

- 🆕 Im neuen Tab „Neue Sitzung“ wählen Sie rund um ein zentriertes Eingabefeld Projekt, Agent, Modell, Denkaufwand, Berechtigungen, Ansicht und einen separaten Worktree aus und starten die Sitzung mit Ihrer ersten Aufgabe; Sie öffnen ihn über „Neue Sitzung…“ in den Menüs für neue Sitzungen, über die Stern-Schaltfläche in der Tab-Leiste oder über den leeren mittleren Bereich.
- 🐋 DeepSeek Harness (`dsh`) kommt als Agent für die Gesprächsansicht hinzu, mit Werkzeugkarten, Denkprozess, Berechtigungsabfragen, Wechsel von Modell und Denkaufwand, Kontextauslastung und einem Verlauf, der nach dem erneuten Öffnen fortgesetzt wird.
- 🧩 Copilot, Cursor, Kimi und Grok lassen sich jetzt in der Gesprächsansicht ausführen.
- 🤖 Zhipu ZCode kommt als Agent für die Gesprächsansicht hinzu: Ist die ZCode-Desktop-App installiert, wird deren integrierter Agent verwendet, und Sie können sich mit Z.ai oder Zhipu BigModel anmelden oder zwischen beiden wechseln, ohne das Gespräch zu verlassen.
- 📊 ZCode-Sitzungen zeigen im Infobereich die Tokens, die Cache-Trefferrate und die Generierungsgeschwindigkeit des aktuellen Durchgangs, die Kontextauslastung sowie die 5-Stunden- und Wochennutzung des GLM Coding Plan.
- 📦 ZCode (unter macOS und Windows) und DeepSeek Harness lassen sich direkt im Gespräch mit „Jetzt installieren“ installieren, und die fehlgeschlagene Nachricht wird nach Abschluss der Installation erneut gesendet.
- ⬆️ Ein Klick auf „Update“ in der Versionsanzeige eines Agenten öffnet einen Dialog, der den eigenen Update-Befehl des Agenten mit Live-Ausgabe ausführt und „Agent neu starten“ anbietet, damit das Gespräch mit der neuen Version weitergeht.
- 🔌 Remote-Verbindungen mit vielen ausgelasteten Terminals brechen nach dem Wiederverbinden nicht mehr immer wieder ab.
- 🔄 Nach einem Neustart des Remote-Servers bleiben Sitzungen nicht mehr in der Synchronisierung hängen, während ihre Terminals starten.
- 🔁 Schlägt die Synchronisierung eines Gesprächs fehl, wird sie automatisch wiederholt, und unter dem Eingabefeld erscheint der Status mit der Schaltfläche „Erneut versuchen“.
- 🐢 SSH-Verbindungen zu einem stark ausgelasteten Remote-Host brechen nicht mehr wegen Zeitüberschreitung ab, während sie auf die Antwort des Servers warten.
- 🔗 „Jetzt neu verbinden“ in einem Remote-Fenster baut den SSH-Tunnel sofort neu auf, statt auf den nächsten automatischen Wiederholungsversuch zu warten.
- 🪟 Unter macOS und Windows fragt das Schließen des Hauptfensters nach einer Bestätigung und lässt Remote-Fenster geöffnet; unter macOS bleibt die App im Dock, und beim erneuten Öffnen des Hauptfensters wird Ihr gespeicherter Arbeitsbereich wiederhergestellt.
- 🧾 Geänderte Dateien in der Liste „This turn“ und im Git-Panel öffnen ihre Diffs in einem Tab statt in einem Dialog.
- ⌨️ Die Befehlsvorschläge im Terminal erscheinen nicht mehr während der Eingabe: Zsh zeigt den neuesten passenden Befehl aus dem Verlauf als grauen Inline-Text an, den Sie mit → oder Ende übernehmen, und die Liste erscheint erst, wenn Sie Tab drücken; in den Einstellungen wählen Sie „Inline + Tab“, „Nur Tab“ oder „Aus“.
- 💬 In Artifact-Vorschauen können Sie Seitenelemente anklicken, um Kommentare für die Sitzung zu hinterlassen, und auf der Seite markierte Optionen werden über der Vorschau aufgelistet, sodass Sie zwischen ihnen springen und eine davon auswählen können.
- 🔍 Die Suche in einem Gespräch arbeitet als Volltextsuche, findet alle Ihre Wörter in beliebiger Reihenfolge und zeigt „Keine Treffer“ statt „Unterhaltung wird geladen…“, wenn nichts gefunden wird.
- 🧰 Mitgelieferte Skills wie vspawn und vrefer werden jetzt im gemeinsamen Ordner `~/.agents/skills` installiert, sodass Pi, ZCode und DeepSeek Harness sie nutzen können, und Codex führt nicht mehr jeden Skill doppelt auf.
- 📋 Ergebnisse von `/code-review` erscheinen in der Gesprächsansicht als Liste von Befunden mit anklickbaren Dateipositionen.
- 🌐 Ist die Übersetzung von Antworten für eine Sitzung deaktiviert, bieten Fragekarten die Schaltfläche „Übersetzung“, die nur die jeweilige Karte übersetzt.
- ✂️ Fragen und Optionen auf Fragekarten lassen sich markieren und kopieren.
- 🌳 Der Sitzungsbaum ordnet Gruppen und übergeordnete Sitzungen, die zu einem Worktree gehören – auch Sitzungen, die direkt in einem Worktree-Ordner gestartet wurden –, unter der Zeile dieses Worktrees ein.
- 🌲 Verschachtelte Gruppen verwenden für neue Sitzungen, Git-Aktionen und Merges den Worktree der nächstgelegenen übergeordneten Gruppe, die einen Worktree hat.
- 🔎 Wird ein Element umbenannt, während der Sitzungsbaum gefiltert ist, verschwindet es nicht mehr aus den Ergebnissen.
- ⏳ Erreicht OpenCode ein Nutzungslimit, nennt eine einzige Fehlerzeile das betreffende Limit, und das Gespräch wird nach dem Zurücksetzen automatisch fortgesetzt, sofern OpenCode den Zeitpunkt der Zurücksetzung meldet.
- 📸 Die Bildschirmfoto-Überlagerung erscheint schneller, nachdem Sie das Tastenkürzel gedrückt haben.
- 🚀 Fenster zeigen beim Öffnen sofort einen Umriss des Layouts und laden beim Start weniger als halb so viel Code.
- ⚡ `vspawn --yes` erstellt die Untersitzung direkt, ohne die Startkarte anzuzeigen.
- 🛡️ Codeprüfungen verwenden Codex Security 0.8.2 und lassen sich mit DeepSeek ausführen, und die Formulare für die Codeprüfung und „Zur Wissensdatenbank hinzufügen“ bieten dieselben Felder für Agent, Modell und Denkaufwand wie neue Sitzungen und akzeptieren auch Modelle, die nicht in der Liste stehen.
- 🌗 Folgt das Theme unter macOS dem System, wird die Titelleiste zusammen mit dem übrigen Fenster dunkel.
- 📥 Ein-Klick-Installationen melden den Abschluss erst, wenn der Installationsbefehl beendet ist, und eine unvollständige Installation von Zoo Code gilt als nicht installiert, statt beim Start zum Absturz zu führen.
- 🖥️ Das Umschalten einer Sitzung in die Terminalansicht schlägt nicht mehr mit „The initial task was already claimed“ fehl.
- 🧟 Sitzungen melden nicht mehr „The previous agent is still running“, wenn die Prozess-ID des alten Agenten inzwischen von einem anderen Programm verwendet wird.
- 🧯 Nachdem sich der Browser neu verbunden hat, behalten Gespräche keine TransportError-Zeile aus der Verbindungsunterbrechung mehr.
- 🪞 Bei aktiviertem Spiegelmodus werden Sitzungen in der Gesprächsansicht beim Öffnen und Schließen von Tabs nicht mehr zu Platzhaltern mit „Aus dem gespeicherten Arbeitsbereich wiederhergestellt“.
- 📜 Claude-Gespräche, die eine verworfene Antwort enthalten, laden ihren Verlauf wieder normal.
- 🧠 Integrierte Claude-Modelle, die im Modellkatalog der Website fehlen, erscheinen weiterhin in den Dialogen für neue Sitzungen und in den Startdialogen.

## v0.2.14 — 2026-10-23

- 🔌 Remote-Verbindungen brechen nicht mehr jede Sekunde ab und verbinden sich neu, wenn mehrere lange Gespräche geöffnet sind; diese Gespräche lassen sich wieder normal öffnen.
- ⏹️ SSH- und WSL-Verbindungen zeigen während des Verbindungsaufbaus die Schaltfläche „Abbrechen“, mit der Sie in jeder Phase abbrechen können, auch während der Server vorbereitet wird; während einer laufenden Verbindung bricht auch „Abbrechen“ im Passwortdialog sie ab.
- 🌿 Sitzungsbaum und Branch-Anzeigen häufen in großen Repositorys keine Git-Prozesse mehr an, und Git-Lesevorgänge im Hintergrund werden nach 30 Sekunden beendet, statt hängen zu bleiben.
- 🧩 Das Git-Panel aktualisiert sich in Repositorys mit Submodulen deutlich schneller: Änderungen innerhalb eines Submoduls führen nicht mehr dazu, dass es im übergeordneten Repository aufgeführt wird; ein Submodul, das auf einen neuen Commit zeigt, wird weiterhin aufgeführt.
- 📁 Gruppen lassen sich in andere Projekte ziehen und nehmen ihre verschachtelten Gruppen und Sitzungen mit; diese Sitzungen starten danach im Ordner des neuen Projekts.
- ⏳ Beim Öffnen eines langen Gesprächs erscheint eine kleine Anzeige „Unterhaltung wird geladen…“ statt „Der Verlauf wird noch indexiert“.
- 🖱️ Das Kontextmenü einer Sitzung zeigt „Prozess beenden“, solange der Prozess der Sitzung läuft – auch wenn er von einem anderen Client gestartet wurde –, und blendet den Eintrag aus, sobald der Prozess beendet ist.

## v0.2.13 — 2026-10-22

- 📱 Auf dem Smartphone öffnen sich Agenten in der Terminalansicht direkt im Terminal, mit einem schwebenden Eingabefeld, in dem Sie ganze Nachrichten schreiben oder diktieren können; kleine Schaltflächen „Transkript“ und „Terminal“ in der Kopfzeile wechseln zwischen beiden, und das Transkript schlägt nicht mehr den Wechsel zur Gesprächsansicht vor.
- 🔑 Über geteilte Verbindungen kann der Besitzer des Hosts das Terminal von Agenten in der Terminalansicht öffnen und darin tippen, und die Sitzungsliste zeigt auch Agenten ohne Gesprächsansicht wie Kiro; eingeladene Konten sehen weiterhin ein schreibgeschütztes Transkript.
- 🌐 Wenn Sie eine Antwort in einer inaktiven Sitzung manuell übersetzen, erscheint die Übersetzung statt „Die Übersetzung ist nicht verfügbar“.
- 📜 Während Sie in einem langen Gespräch nach oben scrollen, landen neue Antworten nicht mehr hinter „Neuere Nachrichten laden“, und beim Herunterscrollen aus älterem Verlauf werden neuere Nachrichten automatisch geladen.
- 🧭 Die Smartphone-Ansicht bietet „Zur Desktop-Version wechseln“ nicht mehr an, und Smartphones, die früher gewechselt haben, kehren automatisch zur Smartphone-Ansicht zurück.
- 🧹 Laufaufzeichnungen von `vrun` werden 24 Stunden nach dem Ende der Aufgabe automatisch gelöscht.

## v0.2.12 — 2026-10-20

- 🛑 Das Stoppen eines Claude-Durchgangs in der Gesprächsansicht wirkt sofort, und Ihre nächste Nachricht wird direkt gesendet – auch wenn der Computer stark ausgelastet ist.
- 🪟 Menükürzel wie Cmd+D und Klicks auf Benachrichtigungen wirken nur im zugehörigen Fenster, sodass Remote-Fenster nicht mehr zusammen mit dem Hauptfenster geteilt werden.
- 🔎 Die globale Suche liefert bereits während der Indizierung Ergebnisse und zeigt an, wie viele Sitzungen noch indiziert werden.
- ⚡ Die globale Suche indiziert Claude-, Codex-, Pi- und OMP-Gespräche direkt aus ihren Dateien – deutlich schneller und ohne den Verlaufscache der Gespräche zu füllen –, und ein geöffnetes Ergebnis springt genau zur passenden Nachricht.
- 🧠 Der Gesprächsverlauf wird direkt aus den Originalaufzeichnungen der Agenten gelesen statt aus einer separaten Kopie auf dem Datenträger; beim Start werden die Ordner `chat-index` und `chat-spool` entfernt, um Speicherplatz freizugeben.
- 📱 Auf dem Smartphone schlagen Sitzungen in der Terminalansicht den Wechsel zur Gesprächsansicht vor; nach Ihrer Bestätigung wechselt auch die Desktop-App die Sitzung.
- 👀 Geteilte Verbindungen listen Sitzungen in der Terminalansicht schreibgeschützt auf, und nur der Besitzer des Hosts kann sie auf die Gesprächsansicht umstellen.
- 🌐 Die Übersetzungseinstellungen enthalten neu „Übersetzung standardmäßig aktivieren“, das den anfänglichen Übersetzungsstatus von Sitzungen ohne gespeicherte Auswahl festlegt.
- 🌳 Der Sitzungsbaum fasst Sitzungen, die denselben Worktree verwenden, unter einer gemeinsamen Worktree-Zeile zusammen.
- 📄 Kurze Antworten in der Gesprächsansicht, auch Nachrichten mit Bildern, zeigen keinen Link „Vollständigen Inhalt lesen“ mehr.
- 🔕 Die Schaltfläche zum Löschen aller Benachrichtigungsmarkierungen befindet sich jetzt am linken Ende der unteren Leiste des Sitzungsbaums.
- 🤖 Die Dialoge für neue und fortgesetzte Sitzungen zeigen Agentensymbole in der Liste der Agententypen, und die Berechtigungsliste öffnet sich nach oben, wenn darunter nicht genug Platz ist.
- 🩹 Die Gesprächsansicht meldet bei stark beanspruchtem Speicher keine fehlerhaften Einträge im Verlauf mehr und lädt kurz darauf weiter.
- 📝 Im Markdown-Editor entfernen Rücktaste und Umschalt+Tab die Nummerierung eines leeren Listeneintrags oder eines nicht nummerierten Absatzes innerhalb einer Liste.
- 🔵 Der Chip „Aufgaben“ in der Gesprächsansicht wird cyanfarben – in der Farbe, die der Sitzungsbaum für Hintergrundarbeit verwendet –, solange noch Hintergrundaufgaben laufen; laufende Aufgaben erscheinen in der Aufgabenliste ebenfalls in Cyan und mit dem Hinweis „Läuft“.
- 🗑️ Beim Löschen einer Sitzung wird nicht mehr angeboten, Git-Worktrees zu entfernen; dafür gibt es „Delete worktree…“.
- 🔁 Der Übersetzungsstatus wird bei jeder Änderung aktualisiert statt alle 10 Sekunden, und die Übersetzungseinstellungen prüfen die installierten Agenten bei jedem Öffnen erneut.
- 💤 Der Infobereich unterbricht die Erfassung der Prozessressourcen, solange das Fenster ausgeblendet ist.
- ⏱️ Arbeitsanzeige und Durchgangstimer der Gesprächsansicht fehlen nicht mehr und bleiben nicht mehr stehen, wenn ein Durchgang beginnt oder endet, während Sie die Sitzung wechseln.
- 🔗 `vrefer` nummeriert Nachrichten genauso wie `vsearch`, und bei abgezweigten Codex-Sitzungen enthalten `vrefer` und die globale Suche auch den Verlauf des übergeordneten Gesprächs.

## v0.2.11 — 2026-10-18

- 🧪 Experimentelle HTML-Vorschauen öffnen sich neben Unterhaltungen oder in Tabs, aktualisieren sich bei Dateiänderungen und bieten Quelltext, Konsole und Versionsfunktionen; die native und plattformübergreifende Abnahme steht noch aus.
- 📜 Lange Unterhaltungen laden ältere Nachrichten seitenweise und lange Inhalte bei Bedarf, während Claude und Codex Nachrichten und Werkzeuge im laufenden Gespräch ohne Duplikate anzeigen.
- 🔎 Die globale Suche aktualisiert geänderte Unterhaltungen schneller, setzt die Indizierung nach Unterbrechungen fort und überspringt unlesbare Unterhaltungen, solange deren Dateien unverändert bleiben.
- 📊 Die Anzahl geänderter Dateien im Infobereich öffnet eine Liste der in diesem Durchgang bearbeiteten Dateien mit verfügbaren Zahlen hinzugefügter und entfernter Zeilen.
- 🗂️ Archivierte Unterhaltungen erscheinen in kompakten Zeilen entsprechend ihren ursprünglichen Projekten, Gruppen und übergeordneten Sitzungen, mit einklappbaren Projekten und Funktionen zum vollständigen Ein- und Ausklappen.
- 🧩 Plugin-Skills des Codex-Kontos erscheinen in den Vervollständigungsvorschlägen, deren Menüs sich mit den Pfeiltasten bedienen lassen.
- ⌨️ Unter Windows und Linux kopiert Strg+Umschalt+C markierten Terminaltext; das Kürzel lässt sich unter Einstellungen ▸ Tastenkürzel ändern.
- 🧹 Nach `/clear` im Claude-Terminal wird die gelöschte Unterhaltung beim erneuten Öffnen oder beim Wechsel zur Gesprächsansicht nicht mehr fortgesetzt.
- 📏 Der Infobereich zeigt bei OpenCode-Unterhaltungen das Kontextlimit des Modells.
- 🌐 Die Übersetzung bietet Bing Translator ohne API-Schlüssel, kompakte Spracheinstellungen und die Übersetzung bearbeiteter Claude- und Codex-Nachrichten vor dem erneuten Senden.
- 🔤 Vorhandene Übersetzungen bleiben nach einem Wechsel von Übersetzungsdienst oder Modell sichtbar und können erneut übersetzt werden.
- ⏱️ `vrun` meldet alle 180 Sekunden den Fortschritt mit Exit-Code 75, setzt das Warten mit `--wait` fort und schützt laufende Aufgaben vor der Wiederverwendung desselben Namens.
- 🧵 Claude-Antworten nach Abschluss einer Hintergrundaufgabe erhalten eine eigene Aufgabenübersicht und Gesprächsgrenze, auch im Verlauf.
- ⚠️ Beim Wechsel zwischen Terminal- und Gesprächsansicht wird um Bestätigung gebeten, wenn Hintergrundaufgaben laufen.
- 🏠 Sitzungen ohne Arbeitsverzeichnis oder mit nicht mehr vorhandenem Verzeichnis starten in beiden Ansichten im persönlichen Ordner.
- 🪟 Unter Windows fordern weder der Start von VelaTerm noch das Verbindungsfenster zur WSL-Installation auf, wenn keine Distribution registriert ist.
- 🔐 Fehlgeschlagene Fernanmeldungen zählen auch dann zum Versuchslimit, wenn die Verbindung während der Passwortprüfung geschlossen wird.
- 🏷️ Versionsanzeigen der Agenten zeigen bei einer neueren CLI-Version einen ausdrücklichen Link `Update: vX`.
- 🔵 Anzeigen für Hintergrundaufgaben leuchten dauerhaft cyan, und Statuspunkte bleiben in Tabs und Seitenleiste statt in den Überschriften der Teilfenster.
- ✍️ Das Eingabefeld für Unterhaltungen hat einen deutlicheren Rand mit äußerer Fokuslinie, und Werte im dunklen Infobereich sind so hell wie ihre Beschriftungen.
- 🌿 Schaltflächen zum Klonen von Projekten verwenden ein Branch-Symbol mit Projektmarkierung.

## v0.2.10 — 2026-10-13

- 🧩 OpenCode v2 wird zusätzlich zu v1 unterstützt, und die Modellliste lädt auch mit OpenCode v2.0.26 korrekt.
- 🏷️ Der Sitzungskopf zeigt die Version der Agent-CLI; ist eine neuere Version verfügbar, führt die Versionsnummer zur offiziellen Upgrade-Anleitung.
- 🗂️ Das Git-Panel zeigt bis zu 1.000 geänderte Dateien und bietet an, große Ordner wie `node_modules` zu `.gitignore` hinzuzufügen.
- 🔤 Das Git-Panel zeigt Dateinamen mit Nicht-ASCII-Zeichen korrekt an, Stagen, Unstagen und Verwerfen wirken genau auf die ausgewählten Dateien, und Konflikte stehen in einer eigenen Gruppe.
- 📜 Der Git-Verlauf behält beim Aktualisieren den geladenen Bereich, aufgeklappte Einträge und die Scrollposition.
- 🌿 Der im rechten Panel, in der Statusleiste, im Terminaltitel und in der Seitenleiste angezeigte Branch aktualisiert sich sofort nach einem Branch-Wechsel, ohne Neustart.
- 📁 Der Dateibaum aktualisiert geöffnete Ordner automatisch und markiert Ordner, die Git-Änderungen enthalten.
- 🧹 Windows: Selbstaktualisierungen hinterlassen keine Installationsprogramme mehr im Temp-Ordner, und Reste früherer Updates werden entfernt.
- 🪝 Windows: Codex, Crush, Kiro, Cursor, Kimi, Antigravity, Copilot und Grok melden den Sitzungsstatus direkt an VelaTerm, ohne PowerShell-Skripte.
- 🖱️ Windows: Vollbild-Terminalprogramme wie vim empfangen Mausklicks und Scrollen; das Mausrad wird nicht mehr in Pfeiltasten umgewandelt.
- 🕘 zsh-Terminals lesen und schreiben wieder Ihre eigene `~/.zsh_history`, sodass `history` und die Pfeil-nach-oben-Taste auch Befehle aus anderen Terminals zeigen.
- 🖥️ Neue Claude- und Codex-Sitzungen öffnen sich in der Konversationsansicht; andere Agenten öffnen sich standardmäßig im Terminal.
- 🚀 `vspawn` akzeptiert `--pi` und `--agent <name>` und kann damit jeden Agenten starten, der Aufgaben übernimmt.
- ⏱️ Wird die Modellliste nicht innerhalb von 30 Sekunden geladen, erscheint ein Fehler mit Retry.
- 🗑️ Gespeicherte Voreinstellungen lassen sich unter Einstellungen ▸ Agenten löschen.
- ⌨️ Der Tooltip der Screenshot-Schaltfläche zeigt das aktuelle Tastenkürzel, und das Symbol lässt sich leichter von Teilen unterscheiden.
- 🧭 Das rechte Panel wechselt nach dem Öffnen von Wissen wieder zu Info und anderen Tabs.
- ✍️ macOS: Die Eingabe von `/` im Terminal mit einer Eingabemethode erscheint nicht mehr doppelt.

## v0.2.9 — 2026-10-07

- ✨ Beim Umbenennen mit KI werden das gewählte Modell und die Denkintensität für jeden Agenten gespeichert.
- ⛔ Die KI-Titelgenerierung lässt sich mit Cancel, Esc oder einem Klick außerhalb des Dialogs abbrechen und anschließend erneut starten.
- 🧭 Bei neuen Plan/Execute-Workflows ist die unabhängige Review-Rolle standardmäßig deaktiviert und kann bei Bedarf im Startdialog oder mit `--review` aktiviert werden.
- 🤖 Die Chat-Modellauswahl zeigt Lade-, Fehler- und Leerzustände mit Retry an, und die angebotenen Codex-Modelle entsprechen dem Konto und der Konfiguration der aktuellen Sitzung.
- 📊 Das Info-Panel zeigt eine Schätzung der durchschnittlichen Ausgabegeschwindigkeit einschließlich gemeldeter Reasoning-Tokens an, oder einen Strich, wenn keine Schätzung verfügbar ist.
- 🩺 In den erweiterten Einstellungen ist eine optionale Diagnose der Eingabelatenz mit einstellbarem Schwellenwert verfügbar, ohne den eingegebenen Text aufzuzeichnen.
- ✂️ Das experimentelle Screenshot-Werkzeug hat auf unterstützten Tauri-Desktopplattformen eine Schaltfläche in der Titelleiste, die auch bei deaktiviertem Tastenkürzel verfügbar ist.
- 🌬️ Die türkisfarbenen Anzeigen für Hintergrundaufgaben verwenden im Sitzungsbaum, in den Statusfiltern und in der Statusleiste dieselbe pulsierende Animation.
- 📁 Bei aktivem Statusfilter neu angelegte Gruppen bleiben bis zur Statusaktualisierung oder einer Filteränderung sichtbar, sofern sie auch zur Namenssuche passen.
- 🎨 Protokolle von Hintergrundbefehlen zeigen Farben, Textformatierungen und die letzte Fortschrittszeile an, und die Ausführungsleiste verdeckt die Fokusanzeige des Bereichs nicht mehr.

## v0.2.8 — 2026-10-06

- 🪟 Windows: Beim Zurückkehren zum Fenster bleibt der Tastaturfokus unverändert, wenn er sich bereits in der Seite befindet.

- 🛡️ Windows: Der Virenscanner Huorong stuft VelaTerm und vela-server nicht mehr als `Trojan/MSIL.ShellLoader.q` ein. Es handelte sich um einen Fehlalarm: Um Shell-Befehle aus der Gesprächsansicht auszuführen, erstellte VelaTerm jeden Prozess im angehaltenen Zustand und setzte ihn anschließend fort – eine Abfolge von Aufrufen, die auch Schadsoftware zum Einschleusen von Code nutzt. Prozesse werden jetzt normal gestartet, und der Code, der die Erkennung auslöste, wurde entfernt.

- 🔕 Windows: Beim Anlegen neuer Sitzungen und beim Aktualisieren der Modellliste wird PowerShell nicht mehr im Hintergrund ausgeführt; der Status von Cursor-Sitzungen wird nun anhand der Terminalausgabe ermittelt und kann etwas ungenauer sein.

- ⛔ Windows: In der Gesprächsansicht werden Eingaben, die mit `!` beginnen, nicht mehr als Shell-Befehl ausgeführt; stattdessen erscheint ein Hinweis und die Eingabe bleibt erhalten, während frühere Befehle im Verlauf weiterhin angezeigt werden.

- 🔐 SSH-Verbindungen zu Windows-Hosts mit dem integrierten OpenSSH-Server starten PowerShell nicht mehr mit `-EncodedCommand`.

- 🚀 TUI-Agentensitzungen starten auch bei langem PATH korrekt, ohne abgeschnittenen Startbefehl im Terminal.

- ⬇️ TUI-Agentensitzungen springen ans Ende, wenn ein Durchgang endet oder der Agent eine Freigabe anfordert, sofern Sie nicht gerade frühere Ausgaben lesen.

- 💾 Die experimentelle Gesprächswiederherstellung aus v0.2.7 wurde entfernt: Chat zeigt keine doppelten Saved-submission-Nachrichten oder Unterbrechungshinweise mehr, und nicht gesendete Nachrichten in der Warteschlange bleiben nach einem Neustart nicht erhalten.

- 🔁 Planungs- und Ausführungs-Workflows bieten eine optionale, unabhängige Review-Rolle, die bei neuen Workflows standardmäßig aktiv ist und eigenen Agenten, eigenes Modell und eigene Denkstufe hat; `vspawn --plan-execute` akzeptiert `--review`, `--no-review`, `--review-agent`, `--review-model` und `--review-effort`.

- 🏷️ „Mit KI umbenennen…“ fragt zuerst nach einer Bestätigung und lässt Agent, Modell und Denkstufe für diese Umbenennung wählen; der Dialog erscheint bereits während des Ladens der Optionen zentriert.

- 🔍 Sitzungen, Gruppen und Projekte lassen sich auch bei aktiver Namenssuche, Statusfilterung oder Markierungsfilterung ziehen.

- ➕ Bei aktiver Statusfilterung bleibt eine neue Sitzung unter ihrer Gruppe oder übergeordneten Sitzung, bis Sie den Status aktualisieren oder den Filter ändern.

- 🪟 App-Fenster öffnen sich standardmäßig größer, passend zum verfügbaren Bereich des Bildschirms, und Verbindungsfenster öffnen sich zentriert auf dem Bildschirm des Hauptfensters.

- 🎨 Sammlungen haben ein neues Symbol mit gestapelten Ebenen, die Symbole der dunklen Titelleiste sind etwas gedämpfter, die vier Schaltflächen der leeren Seitenleiste sind gleich breit, und Formulardialoge verwenden einen einheitlichen Beschriftungsstil.

---

## v0.2.7 — 2026-10-05

- 🪐 Antigravity ist als experimenteller Agent in der Gesprächsansicht verfügbar und unterstützt Text, Werkzeuge, Nachrichtenwarteschlangen und den nativen Verlauf; Bilder, zusätzliche Anweisungen während einer Antwort, interaktive Freigaben, Verzweigen und Zurückspulen werden noch nicht unterstützt, und die vollständige plattformübergreifende Prüfung der Wiederherstellung steht noch aus.

- 🗂️ Sammlungen können Untersammlungen, Projekte und Sitzungsgruppen enthalten und im Arbeitsbereich erstellt oder importiert werden, wobei Projekte und archivierte Sitzungen beim Löschen einer Sammlung erhalten bleiben.

- ⚠️ Frühere Versionen dürfen eine von Ordnern auf Sammlungen migrierte Datenbank nicht verwenden; für ein Downgrade muss eine Sicherung von vor der Migration wiederhergestellt werden.

- 🏷️ Intelligentes Umbenennen benennt eine Sitzung anhand ihres Gesprächs mit den Einstellungen des aktuellen Agenten und bietet eine Agentenauswahl an, wenn diese Einstellungen nicht verwendet werden können.

- 🌱 Neue Sitzungen verwenden die aktuelle Shell-Umgebung und neu installierte Agenten, ohne VelaTerm neu starten zu müssen.

- 💾 Die experimentelle Gesprächswiederherstellung stellt gespeicherte Nachrichten, Bilder und wartende Eingaben nach einem Neustart wieder her und pausiert unterbrochene Arbeit bis zur ausdrücklichen Fortsetzung; die vollständige Abnahme in nativen Umgebungen und auf allen Plattformen steht noch aus.

- 📸 Die experimentelle Bildschirmaufnahme in den Tauri-Desktop-Apps für macOS und Windows ermöglicht Bereichsauswahl, Anmerkungen, Kopieren und Speichern der ursprünglichen PNG-Datei; die native Desktop-Abnahme steht noch aus, und Electron, Browser und Remote-Ansichten werden nicht unterstützt.

- 🧵 Die Tabs für Claude-Hintergrundaufgaben zeigen den vollständigen nativen Gesprächsverlauf einschließlich paralleler Werkzeugaufrufe.

- 🖼️ Eine Gesprächsnachricht kann bis zu 20 Bilder enthalten, mit maximal 5 MiB pro Bild.

- 📍 Nachrichtenmarkierungen zeigen Vorschauen und führen direkt zu früheren Benutzernachrichten.

- 🧭 Die Verzeichnisauswahl für Projekte, Klonen und Speichern unter bietet einheitlich bearbeitbare Pfade und die Vervollständigung von Speicherorten.

- 🪪 `vself` liest gespeicherte Sitzungseinstellungen und Eltern-Kind-Beziehungen, während `vflow list` zugehörige Planungs- und Ausführungsworkflows auflistet, ohne sie zu verändern.

- ⌨️ Alles auswählen im Terminal lässt sich in den Tastenkürzeln konfigurieren, verwendet standardmäßig Cmd+A unter macOS und Ctrl+Shift+A auf anderen Plattformen und gilt nur bei fokussiertem Terminal.

- 🐚 Die Bash-Vervollständigung unterstützt Pfade mit `~` am Anfang, und ältere Bash-Versionen öffnen Sitzungen ohne PS0-Fehler.

- 🛑 Hooks von untergeordneten Aufgaben verändern den Gesprächsstatus ihrer übergeordneten Sitzung nicht mehr, und das Schließen eines Gesprächstabs beendet dessen aktiven Chat-Prozess.

- 🪟 Windows stellt beim Zurückkehren zum Fenster den Tastaturfokus des Terminals wieder her und zeigt bei der chinesischen Texteingabe keine blaue Linie mehr.

- 🎨 Klassisch Dunkel bietet klarere Kontraste, Desktop-Symbole folgen dem Systemdesign, Projektsymbole sind blau und Sammlungszeilen zeigen keine Projektanzahl mehr.

- 📚 Die Menüs für Sitzungen und Sammlungen verwenden eine klarere Beschriftung für das Hinzufügen zur Wissensdatenbank.

- 🌐 Der Browserzugriff auf VelaTerm über unverschlüsseltes HTTP im lokalen Netzwerk öffnet die Seite korrekt.

- 🌍 Update-Prüfungen übermitteln die Oberflächensprache für passende Versionshinweise, und der Update-Dienst akzeptiert den Linux-Electron-Updater.

- 🧰 Die Agentennamen Kimi Code und Grok Build erscheinen ohne Modellversionsnummern.

---

## v0.2.6 — 2026-09-30

- 🐧 Die Linux-Version basiert jetzt auf Electron. Das AppImage behält seinen Dateinamen und Ihre Daten, benötigt kein WebKitGTK und kein libfuse2 mehr, und bestehende Installationen lassen sich über die integrierte Aktualisierung darauf umstellen.

- 🛰️ Rechner, die per SSH nicht erreichbar sind, etwa WSL auf einem anderen Computer oder ein Docker-Container, lassen sich nach der Installation von `vela-server` mit einem einzigen Befehl mit Ihrem Konto verknüpfen und erscheinen dann in der Remote-Liste für KI-Unterhaltungen. Sobald der Besitzer auf dem Host vollen Zugriff gewährt, kann die Desktop-App auch das Terminal, die Dateien und das Git-Panel dieses Hosts nutzen.

- 📂 Ordner, die aus dem Finder, dem Datei-Explorer oder einem Linux-Dateimanager in die Seitenleiste gezogen werden, werden als Projekte hinzugefügt.

- 📝 Der Markdown-Editor bietet jetzt Tastenkürzel wie in Typora für Überschriften, Listen, Codeblöcke und Tabellen, automatisches Schließen von Klammern und Anführungszeichen, Cmd/Strg+Klick zum Öffnen von Links, einen Fokus- und einen Schreibmaschinenmodus sowie eine Wort- und Zeichenzählung in der Statusleiste. YAML-Front-Matter erscheint in einem eigenen Feld über dem Dokument und bleibt unverändert, solange Sie es nicht bearbeiten.

- 🗃️ Ist ein Ordner selbst kein Git-Repository, enthält aber mehrere, bietet das Git-Panel eine Repository-Auswahl und arbeitet mit dem ausgewählten Repository.

- ⏳ In der Unterhaltungsansicht zeigt eine Sitzung, deren Antwort abgeschlossen ist, während noch Hintergrundaufgaben laufen, den neuen Status „Aufgaben laufen“ mit einem cyanfarbenen Punkt, statt weiterhin „Aktiv“ anzuzeigen. Sitzungen, die lange Aufgaben mit `vrun` ausführen, zeigen denselben Status.

- 🍴 In der Unterhaltungsansicht beginnt eine abgezweigte Sitzung beim ersten Start eine eigene Unterhaltung, und die Ursprungssitzung erhält ihre Nachrichten nicht mehr.

- 🛡️ In der Unterhaltungsansicht folgt die Modusbeschriftung einer Claude-Sitzung dem Berechtigungsmodus, den Claude tatsächlich verwendet, etwa nach dem Wechsel in den Plan-Modus, und Berechtigungskarten zeigen den Grund, den Claude für die Nachfrage angibt.

- 📏 Die Höhe des Eingabefelds in der Unterhaltungsansicht lässt sich durch Ziehen am oberen Rand anpassen; ein Doppelklick auf den Rand stellt die Standardhöhe wieder her. Die Höhe gilt für alle Bereiche und bleibt nach einem Neustart erhalten.

- 🗂️ Sind in der Seitenleiste mehrere Ordner ausgewählt, lassen sie sich über das Kontextmenü zusammen mit ihren Sitzungen archivieren.

- 📚 Sammlungen stehen in der Seitenleiste immer über den Projekten.

- 🧠 Hat ein Verarbeitungsprotokoll der Wissensbasis nur einen Eintrag erzeugt, öffnet sein Titel diesen Eintrag direkt; die Eintragsschaltflächen zeigen die Titel der Einträge statt Nummern.

- 📊 Das Info-Panel zeigt für neuere Claude-Modelle wie Opus 5.5 das korrekte Kontextlimit statt 200k.

- 🎨 Kleinere Änderungen: Die drei Optionen zum geteilten Öffnen im Kontextmenü einer Sitzung sind im Untermenü „Im geteilten Bereich öffnen“ zusammengefasst, die Beenden-Bestätigung weist darauf hin, dass geöffnete Remote-Fenster ebenfalls geschlossen werden, und Auswahllisten in Formularen schließen sich sofort nach der Auswahl.

---

## v0.2.5 — 2026-09-28

- 🐧 In der Windows-Desktop-App lassen sich separate Arbeitsbereiche in WSL1 und WSL2 öffnen, die die Agenten, Dateien und den Sitzungsverlauf der ausgewählten Linux-Distribution verwenden.

- 🔄 WSL-Arbeitsbereiche lassen sich nach dem Beenden des Servers erneut verbinden; beim Schließen eines Fensters können Sie den Server beenden oder die Sitzungen weiterlaufen lassen.

- 🌐 Claude-Sitzungen haben im Eingabebereich die Option „Chrome“, mit der sich Claude in Chrome ein- und ausschalten lässt, ohne die Unterhaltung neu zu starten. Jede Sitzung speichert ihre eigene Auswahl, Sitzungen ohne Auswahl folgen der Standardeinstellung.

- 📟 Der Tab einer Shell-Aufgabe im Hintergrund zeigt den ausgeführten Befehl und die neueste Ausgabe, die während der Ausführung laufend aktualisiert wird.

- 🧩 Sind die Vela Skills nicht installiert, bietet die Statusleiste die Installation an. Der Dialog beschreibt die einzelnen Skills, installiert sie direkt und lässt sich so einstellen, dass er nicht mehr erinnert.

- 🪪 Mit `vspawn` erstellte untergeordnete Sitzungen erhalten die ID ihrer übergeordneten Sitzung in `VLX_PARENT_SESSION_ID`, sodass ein Agent sie mit `vrefer` und `vtell` erreichen kann. Der neue Befehl `vself` zeigt die aktuelle Sitzung und die darüberliegenden Sitzungen.

- 🖥️ Das Linux-AppImage öffnet auf neueren Distributionen wie Fedora 44 kein leeres Fenster mehr.

- 🎨 Kleinere Änderungen: „Hintergrundaufgaben“ erscheint standardmäßig unter dem Eingabefeld und bleibt im Menü „Mehr“, wenn Sie es dorthin zurückverschieben; lässt sich ein Link nicht öffnen, weist eine Meldung darauf hin, dass die Adresse per Rechtsklick kopiert werden kann.

---

## v0.2.4 — 2026-09-26

- 📋 Kopieren aus der Unterhaltung liefert den Text so, wie er auf dem Bildschirm steht: keine Backticks um Inline-Code, keine Sternchen um Hervorhebungen, Links nur als ihr Text, Codeblöcke ohne Begrenzungszeilen, Tabellenzellen durch Tabulatoren getrennt und Listen mit den sichtbaren Aufzählungszeichen. Die formatierte Fassung landet weiterhin zusätzlich in der Zwischenablage, und das Kontextmenü bietet nun „Als Markdown kopieren“ für den Markdown-Quelltext der Auswahl.

- ⏳ Das automatische Fortsetzen nach dem Zurücksetzen eines Nutzungslimits ist jetzt standardmäßig eingeschaltet. Wer die Einstellung selbst geändert hat, behält seine Wahl.

- ↩️ Das Zurücksetzen einer Unterhaltung wird nicht mehr von einer bereits beendeten Aufgabe blockiert: Eine Aufgabe im Vordergrund gilt mit dem Ende ihrer Runde als beendet, und ein danach eintreffender Fortschritt markiert sie nicht erneut als laufend. Läuft tatsächlich noch eine Hintergrundaufgabe, nennt die Meldung ihren Namen.

- 🧹 Die Subagenten-Threads von Codex erscheinen nicht mehr in der Liste des Sitzungsverlaufs – genau wie Claudes Sidechains.

- ⌨️ Terminal-Vervollständigung: Nachdem Sie mit den Pfeiltasten durch die Vorschläge gegangen sind, übernimmt die Eingabetaste den markierten Vorschlag ebenso wie die Tabulatortaste. An die Shell geht die Eingabetaste weiterhin, wenn Sie die Auswahl nicht bewegt haben, nach dem Auswählen weitergetippt haben oder eine Modifikatortaste gedrückt halten.

- 📱 Android wird in zwei Kanälen gebaut. Der Standard-Build enthält die Push-Kanäle von Getui sowie Huawei, Xiaomi, OPPO, vivo, Meizu und Honor; der Play-Build kommt ohne sie aus und weist Aufgabenbenachrichtigungen als ohne konfigurierten Push-Kanal aus.

- 🎨 Kleinere Änderungen an der Oberfläche: Aufgabenlisten zeigen das Kontrollkästchen nicht mehr doppelt, die Benachrichtigungsleiste und der Hinweis zum automatischen Fortsetzen sitzen zwischen Unterhaltung und Eingabefeld in der Breite des Nachrichtentexts, und auf dem Telefon bleibt die Senden-Schaltfläche neben dem Eingabefeld, wenn die Optionen eingeklappt sind.

---

## v0.2.3 — 2026-09-24

- 🪟 Bestehende Sitzungen lassen sich in Teilungen verschieben. Über das Kontextmenü der Seitenleiste öffnet eine Sitzung in einer rechten Teilung, einer unteren Teilung oder im fokussierten Bereich; zieht man sie aus der Seitenleiste an den Rand eines Bereichs, wird in diese Richtung geteilt, und ein Ablegen in der Mitte ersetzt die dort gezeigte Sitzung. Zwei bis vier ausgewählte Sitzungen lassen sich in einem gleichmäßig geteilten Tab nebeneinander anordnen, und Sitzungen in den übrigen Bereichen des aktuellen Tabs sind in der Seitenleiste gekennzeichnet.

- 🗂️ Hintergrundaufgaben, die ein Agent startet, öffnen in eigenen Tabs neben der Unterhaltung – mit Status, verstrichener Zeit, Tokens, Werkzeugaufrufen, dem zuletzt gemeldeten Werkzeug und den Phasen jedes Agenten. Eine Aufgabe hat eine eigene Adresse, und beim Verlassen kehrt die Ansicht in den Bereich zurück, aus dem sie geöffnet wurde.

- 💬 In der Unterhaltungsansicht wird eine Nachricht, die mit `!` beginnt, in der Shell der Sitzung ausgeführt. Die Ausgabe erscheint laufend, der Exit-Code wird angezeigt, der Befehl lässt sich währenddessen abbrechen, und er bleibt im Leseverlauf von Claude-, Codex-, OpenCode-, Pi- und OMP-Sitzungen erhalten.

- ⏱️ Der neue Befehl `vrun` startet einen lang laufenden Befehl und wartet in einem einzigen Aufruf auf ihn, sodass ein Agent erfährt, wann die Arbeit tatsächlich beendet ist. So gestartete Befehle stehen über dem Terminal, zusammen mit ihrer Laufzeit, einem Protokollfenster und einer Schaltfläche zum Beenden, die eine Bestätigung verlangt.

- ⌨️ Neue Agentensitzungen haben eine eigene Seite und ein eigenes Tastenkürzel: Agenten und Voreinstellungen durchsuchen, die zuletzt verwendete übernehmen und festlegen, ob die Sitzung neben der aktuellen oder darunter entsteht.

- 🧰 Die Eingabe-Symbolleiste lässt sich in den Einstellungen zusammenstellen: Sie wählen, welche Elemente neben der Nachricht stehen und in welcher Reihenfolge. Abgeschaltete Elemente und solche, die nicht in die Breite passen, bleiben über das Menü „Mehr“ erreichbar.

- 📥 Beim Herunterladen einer Datei in einem URL- oder SSH-Verbindungsfenster wird zuerst auf diesem Rechner der Speicherort abgefragt, danach zeigt die Oberfläche den Fortschritt und eine Schaltfläche zum Abbrechen.

- 🗃️ Kiro-Sitzungen lassen sich in ein Projekt importieren und lesen: Der Import ordnet Sitzungen über das Arbeitsverzeichnis zu, und jede öffnet in einer schreibgeschützten Verlaufsansicht mit Suche. Derzeit werden reine Textaufzeichnungen unterstützt.

- 🧠 Das Ordnen der Wissensdatenbank funktioniert neben Claude und Codex auch mit Grok, OpenCode, Pi und OMP, und der Agent wird über dieselbe Art von Auswahlliste gewählt wie an den übrigen Stellen der Anwendung.

- 📱 iOS und Android: Die Startseite bewahrt Ihre SSH- und URL-Verbindungen auf, meldet Sie an einem VelaTerm-Konto an und listet die Geräte auf, die darüber teilen. Der Fingerabdruck eines Hosts wird einmal bestätigt und gemerkt, ein QR-Code füllt die Dienstadresse aus, und sämtliche nativen Bildschirme und Systemmeldungen liegen in allen 11 Oberflächensprachen vor.

- 🔐 Claude-Sitzungen starten in dem gewählten Berechtigungsmodus: Zusätzliche Startargumente behalten ihre vorgesehene Rangfolge, der Modus wird beim Start mit der Meldung des CLI abgeglichen, und eine ohne Rückfragen gestartete Sitzung bleibt entsprechend gekennzeichnet.

- 🧩 Das Modellmenü führt Opus 5.5 auf und ergänzt die zusätzlichen Modelle, die das gewählte CLI meldet, ohne die eingebaute Reihenfolge zu verändern; ein Modell, das mit einem CLI-Update hinzukommt, erscheint ohne Neustart der Anwendung.

- 🌱 Anfragen für untergeordnete Sitzungen überstehen Unterbrechungen: Eine Anfrage, deren Antwort verloren ging, lässt sich nach dem erneuten Verbinden wiederherstellen, ein bereits bestätigter Start verwendet bei einem erneuten Versuch dieselbe Sitzung und dieselben Einstellungen, und ein Arbeitsbaum, der nicht angelegt werden konnte, wird zurückgenommen, ohne zuvor Vorhandenes anzutasten.

- ⚡ Die Anwendung startet schneller – der beim Start geladene Code ist rund halb so groß wie zuvor, und die Seiten für Wissensdatenbank, Sicherheitsaudit, Sitzungsimport und geteilte Projekte werden erst beim Öffnen geladen.

- 🖼️ In eine Nachricht eingefügte oder gezogene Bilder werden vor dem Senden auf 1568 Pixel an der langen Seite verkleinert.

- 🐚 Bash-Sitzungen laden die Shell-Vervollständigung aus einer Startdatei, sodass eine neue Bash-Sitzung nicht mehr mit einem bereits eingetippten Befehl öffnet. Die Anmeldeprofile werden weiterhin in der von Bash vorgesehenen Reihenfolge gelesen.

- ✍️ Markdown: Eine einzelne Tilde streicht den Rest der Zeile nicht mehr durch, sodass eine eingefügte Shell-Eingabeaufforderung lesbar bleibt, und Fett- oder Kursivschrift, die direkt nach chinesischen, japanischen oder koreanischen Zeichen endet, wird korrekt geschlossen, statt Sternchen auf dem Bildschirm zu hinterlassen.

- 📨 `vtell --steer` stellt eine Nachricht in die gerade laufende Runde des Empfängers zu, statt deren Ende abzuwarten. Steht der Empfänger bei einer Rückfrage, lautet die Rückmeldung blocked, denn die Nachricht wird erst gelesen, wenn diese Rückfrage beantwortet ist.

- 🔁 In der Unterhaltungsansicht endet der Status einer Sitzung zugleich mit ihrer Runde, die Ungelesen-Markierung verschwindet, sobald die Arbeit tatsächlich beginnt, und eine Sitzung mit weiterlaufender Hintergrundarbeit bleibt als laufend gekennzeichnet.

- 🪟 Windows: Cursor-Hooks starten korrekt, und die Fensterschicht wurde für die nach einer RDP-Neuverbindung oder einem Wechsel des virtuellen Desktops gemeldeten Tastatureingabeprobleme aktualisiert.

- 🛡️ Sicherheitsaudit: Modellliste und Agentennamen stammen aus demselben Startkatalog wie im Rest der Anwendung, sodass frühere Durchläufe den aktuellen Namen jedes Agenten zeigen.

- 🩹 Kleinere Korrekturen: Fehlermeldungen in der Unterhaltung richten sich an der mittleren Spalte aus; beide Startdialoge lassen sich jederzeit schließen, und ein bestätigter, aber fehlgeschlagener Start kann abgebrochen werden; Nachrichten und Nachrichten in der Warteschlange zeigen ihren Absender; eine lange Warteschlange scrollt innerhalb einer festen Höhe; leere HTML-Anker zeigen beim Bearbeiten eines Dokuments keine Markierung mehr; und der Statusfilter auf dem Telefon nimmt Sitzungen auf, die erst später dazu passen.

---

## v0.2.2 — 2026-09-15

- ⏳ Automatische Fortsetzung nach Nutzungslimits, standardmäßig in den Einstellungen deaktiviert: Wenn Claude oder Codex an einem 5-Stunden- oder Wochenlimit stoppt, setzt die Sitzung von selbst fort, sobald das Limit zurückgesetzt ist. Über der Eingabe erscheint ein Banner mit der Rücksetzzeit und einer Schaltfläche „Abbrechen“; die Wartezeit übersteht einen App-Neustart und endet, sobald du eine Nachricht sendest, zurücksetzt, die Sitzung leerst oder die Einstellung ausschaltest.

- 🪟 Windows: One-Click-Installationen von OpenCode, Grok und Crush übergeben `--allow-scripts`, sodass npm die ausführbare Datei erzeugt; Cursor, OMP und Antigravity werden in ihrem tatsächlichen Installationsordner unter `%LOCALAPPDATA%` gefunden, und OMP beachtet zusätzlich `PI_INSTALL_DIR`.

- 🪟 Windows: Dateien, die noch heruntergeladen oder kopiert werden, gelten nicht mehr als installiert; die Installationskarte wartet mit der Erfolgsmeldung, bis die Installation wirklich beendet ist, und ein veralteter gespeicherter Pfad wird durch den neu gefundenen ersetzt.

- 🧩 Konversationsansicht: Denken, Werkzeugaufrufe und Zwischenantworten einer Runde lassen sich mit „Zwischenschritte ausblenden“ ausblenden, sodass nur die endgültige Antwort bleibt; die Werkzeugleiste blendet alle Runden auf einmal aus oder ein, und die Suche klappt eine ausgeblendete Runde zum Auffinden eines Treffers auf.

- 🔐 Die Berechtigungsschaltfläche zeigt jetzt den Modus, mit dem eine Sitzung gestartet wurde, statt „Aktuelle Berechtigungen unbestätigt“; die doppelte Zeile „Starteinstellung“ ist entfernt, und eine Sitzung, die bereits mit übersprungenen Bestätigungen gestartet wurde, wird in der Statusleiste wieder hervorgehoben.

- 🔐 Eine Sitzung ohne eigene Berechtigungseinstellung folgt in beiden Ansichten dem globalen Standard ihres Agententyps, und beim Bearbeiten einer Sitzung wird der geerbte Wert nicht mehr als fester Wert der Sitzung gespeichert.

- ↩️ Die Seiten der Wissensdatenbank erhalten eine Schaltfläche „Eine Ebene nach oben“: Ein Eintrag führt zurück zu seiner Sitzung, dann zum Projekt und zur Startseite; archivierte Sitzungen, Notizen und Ordner steigen auf dieselbe Weise auf.

- 🌱 Eine Untersitzung öffnet sich in derselben Ansicht wie ihre übergeordnete Sitzung: Aus einer Sitzung in der Konversationsansicht kommt die Aufgabe als erste Nachricht an und Bilder werden zu Anhängen; aus einer Terminalsitzung startet sie im Terminal, wobei die Aufgabe als Startargument übergeben wird.

- 🎨 macOS: Fenster und Titelleisten-Streifen erhalten vor dem Erscheinen des Fensters die Farben des Themes, und ein Themenwechsel zur Laufzeit zeichnet sie sofort neu.

- 🗜️ Claude-Sitzungen behalten die Runden vor einer Kompaktierung im Leseverlauf; ein Zurücksetzen entfernt weiterhin den verworfenen Zweig.

- 🔎 Die Suche in der Konversation hält den ausgewählten Treffer fest, während älterer Verlauf geladen wird; Enter springt zum vorherigen Treffer und Umschalt+Enter zum nächsten.

- 🩹 Die Rückkehr zu einer Konversation, die mitten in ihrem Verlauf stand, stellt die Leseposition wieder her, statt einen leeren Bereich anzuzeigen.

- 🧭 Diagnoseinformationen zum Modellkatalog erscheinen nur beim Öffnen des Modellmenüs mit gedrückter Option-Taste; Fehlerzeilen richten sich am Nachrichtentext aus, und die englische Wissensdatenbank zeigt „Archived Sessions“.

---

## v0.2.1 — 2026-09-14

- 🔎 Die Startseite der Wissensdatenbank erhält ein Suchfeld: Eine Abfrage durchsucht Sitzungswissen und lokale Notizen zugleich und gruppiert die Treffer nach Quelle. Genaue Treffer stehen zuerst; eine unscharfe Suche (Abkürzungen, Teilfolgen und Tippfehler) greift nur, wenn nichts genau passt, und chinesische Anfragen bleiben Teilstring-basiert.

- 🗂️ Archivierte Sitzungen ziehen in die Wissensdatenbank um. Die Schaltfläche in der Seitenleiste springt dorthin statt ein eigenes Fenster zu öffnen, der Baum erhält eine nach dem ursprünglichen Projekt gruppierte Wurzel „Archivierte Sitzungen“, und der Hauptbereich listet alle archivierten Sitzungen mit Wiederherstellen, Neuordnen, Exportieren und Löschen an Ort und Stelle.

- 🔍 Archivierte Sitzungen lassen sich volltextdurchsuchen: Treffer werden je Sitzung mit Anzahl gruppiert, und die Vorschau springt jeden Treffer mit derselben Hervorhebung wie die globale Suche an. Eine geöffnete Sitzung wechselt zwischen ihrem Verlauf und ihren Wissenstreffern.

- 🤖 Konversationsansicht: Fehlt die ausführbare Datei eines Agenten, erscheint unter der Nachricht eine Installationsanleitung statt des rohen Startfehlers; „Jetzt installieren“ wechselt in die Terminalansicht, die den empfohlenen Befehl ausführt. Liegt der Agent außerhalb des PATH, nimmt die Karte direkt einen Pfad zur ausführbaren Datei entgegen, am Desktop mit System-Dateiauswahl.

- 🩹 Installationen, die vorhanden aussehen, aber defekt sind, werden erkannt: Ein globaler npm-Wrapper, dessen Ziel entfernt oder ersetzt wurde, gilt als nicht installiert, und ein konfigurierter Pfad zu einem solchen Wrapper öffnet die Installationsanleitung, ohne deine Einstellung zu überschreiben.

- 🔐 Der Eintrag für den Berechtigungsmodus zeigt direkt den gewählten Modus, und das Menü vermerkt an jeder Zeile, ob die Wahl bereits läuft oder auf den nächsten Zug wartet.

- 🔽 Alle Auswahlmenüs nutzen jetzt die eingebaute Select-Komponente statt des nativen Steuerelements und sehen damit unter macOS 15 und 26 gleich aus, ohne die Überlagerung des Systemelements.

- ℹ️ Projekte und Sammlungen haben einen Info-Dialog im Kontextmenü.

- 🪟 Windows: Von der Konversationsansicht gestartete Unterprozesse (Agenten, Modellkatalog und git-Prüfungen) blitzen nicht mehr als Konsolenfenster auf.

- 💡 Die Feedback-Schaltfläche in der Titelleiste steht jetzt nach der Teilen-Schaltfläche.

---

## v0.2.0 — 2026-09-13

- 📱 VelaTerm für iOS und Android (frühe Version): Verbinde dich per SSH oder URL mit einem Rechner und bestätige beim ersten Verbinden den Fingerabdruck des Hosts; lade die vollständige entfernte Oberfläche direkt in der App, fülle Verbindungsdaten per QR-Code aus und melde dich bei einem entfernten Konto an.

- 🛡️ Experimentelle Codeprüfung: Starte im Kontextmenü eines Projekts mit Codex oder Claude Code eine Prüfung des gesamten Repositorys, eines Verzeichnisses oder einer Datei oder der nicht committeten Änderungen im Arbeitsverzeichnis; prüfe Befunde gegen den Quellcode und exportiere einen Markdown- oder JSON-Bericht.

- 📓 Lokale Wissensdatenbank: Öffne im rechten Bereich einen Ordner mit Markdown-Notizen, bearbeite Notizen im WYSIWYG-Editor, suche in Pfaden und im Volltext, verwalte Tags und Favoriten und stelle gelöschte Notizen aus dem Papierkorb wieder her. Agenten können Notizen mit `vkb` abfragen; Ordnerimporte laufen im Hintergrund und hinterlassen einen aufklappbaren, erneut aufrufbaren und abbrechbaren Verlauf.

- 🤖 Planungs- und Ausführungssitzungen: `vspawn --plan-execute` öffnet eine Planungssitzung, die die Aufgabe in Ausführungssitzungen aufteilt; `vflow` schlägt die Aufteilung vor, `vtell --report` meldet das Ergebnis jeder Ausführung zur Abnahme zurück, und bei einer Überarbeitung wird die ursprüngliche Ausführungssitzung wiederverwendet. Worktrees können von allen Rollen geteilt oder pro Sitzung erstellt werden.

- 🔗 Geteilte Projekte und Sitzungen laden jetzt die echte Oberfläche des Hosts hinter der geteilten URL, übertragen über einen ausgehenden Tunnel und beschränkt auf das freigegebene Projekt oder die freigegebene Sitzung. Geräte und Freigaben lassen sich auf den Kontoseiten verwalten.

- 🧠 Die Sitzungswissensdatenbank unterstützt Gruppen für Projekte, Sitzungen und Einträge mit Drag-and-drop, Umbenennen und Löschen; eine erneute Organisation derselben Sitzung ersetzt die wartende Aufgabe.

- 💬 Gesprächsansicht: Lange Verläufe laden seitenweise bis zur ersten Nachricht, jeder gesprächsfähige Agent (einschließlich OMP) öffnet neue Sitzungen standardmäßig in der Gesprächsansicht, und aufeinanderfolgende Runden desselben Agenten teilen eine Autorenzeile.

- ⌨️ Terminal: Shell-eigene Vorschläge bieten Tab-Vervollständigung für zsh, bash, fish und PowerShell unter macOS und Linux, die Pfeiltasten rufen weiterhin frühere Befehle ab, während die Liste geöffnet ist, und der Terminalkern wechselt auf xterm 6.

- 📊 Das Info-Panel zeigt Statistiken der aktuellen Runde für Claude, Codex, Grok, OpenCode, Pi und OMP: Eingabe- und Ausgabe-Token, Cache-Trefferrate, Generierungsgeschwindigkeit, Werkzeugaufrufe und erfasste Dateiänderungen. Seltener genutzte Bedienelemente der Eingabe wandern unter „Mehr“.

- 🔐 Die Eingabe zeigt den konfigurierten, den aktuell wirksamen und den ausstehenden Berechtigungsmodus und fragt nach Bestätigung, wenn ein Neustart zum Anwenden nötig ist.

- 🔔 Benachrichtigungen zeigen den Sitzungsnamen und eine kurze Vorschau; ein Klick öffnet die Sitzung, und die Mobil-App kann sie über den Push-Dienst des Systems empfangen.

- 🌐 Claudes Modellliste stammt jetzt aus dem veröffentlichten Modellkatalog, wird lokal zwischengespeichert und alle sechs Stunden aktualisiert und mit den von der CLI gemeldeten Modellen zusammengeführt.

- 🧵 Mit den Pfeiltasten auf und ab ruft die Eingabe frühere Nachrichten ab, auch wartende und noch unbestätigte, und stellt am Ende den ungesendeten Entwurf wieder her.

- ↩️ Beim Zurücksetzen einer Nachricht kehren ihre Bilder in die Eingabe zurück und können erneut gesendet werden.

- 🔑 Unter macOS gestartete Agenten übernehmen die vollständige Umgebung der Login-Shell, sodass auch Werkzeuge außerhalb des Standard-PATH gefunden werden.

- 💡 Ein Feedback-Eintrag in der Titelleiste öffnet die Feedback-Seite.

- 🕹️ Ein Spielecenter-Eintrag in der Tab-Leiste öffnet das Spielecenter der Website (PIXEL WING); am Desktop öffnet sich das Spiel im integrierten Browser.

---

## v0.1.108 — 2026-09-08

- 💬 Experimentelle Gesprächsansicht für Claude, Codex und OpenCode mit gestreamten Antworten, Denkprozess, Werkzeugdetails, Berechtigungen und Frageformularen. Neue Konfigurationen verwenden weiterhin standardmäßig die Terminalansicht.

- 🎛️ Die Gesprächssteuerung bietet je nach Engine Modelleinstellungen, Nachrichtenwarteschlangen, Zwischenmeldungen, Dateivervollständigung und Bildanhänge.

- 🔎 Gesprächssuche, Dateilinks, Bildaktionen und separate Gesprächsschriften erleichtern das Lesen langer Sitzungen. Entfernte Verläufe werden schrittweise geladen, Werkzeugdetails bei Bedarf.

- 📨 Übermittlungsbelege helfen, Ergebnisse nach Verbindungsabbrüchen abzugleichen. Ungewisse Übermittlungen warten auf Bestätigung und werden nicht automatisch erneut gesendet.

- 🤝 Die neuen Befehle `vrefer` und `vsearch` lesen und durchsuchen andere Sitzungen; `vrefer --ask` überträgt das Lesen an einen Agenten. `vorch` und `vstat` ergänzen die Koordination mehrerer Agenten und Statusabfragen.

- 🧠 Speichereinträge unterstützen Tags, direkte Textbearbeitung, Schutz ungespeicherter Änderungen und Organisationseinstellungen. Filter und Auswahl beim Verlaufsimport sowie die Graphnavigation wurden verbessert.

- 🌐 Der Client für öffentliche Freigaben unterstützt Konto-/Gerätebindung, begrenzten Zugriff auf AI-Sitzungen und verschlüsselte Relay-Verbindungen. Bilduploads durch Gäste und komplexe MCP-Formulare werden noch nicht unterstützt.

- 🔄 Codex-Fortsetzungs-IDs werden mit dem gespeicherten Verlauf abgeglichen. Fehlt dieser nachweislich, erscheint ein eindeutiger Fehler statt einer leeren Sitzung. Laufende Terminals bleiben erneut erreichbar.

- ↩️ Das Zurücksetzen richtet sich nach der Engine: Codex setzt nur das Gespräch zurück und stellt keine Dateien wieder her. Die Bereichsbehandlung und Verzeichnisvorprüfung in OpenCode haben weiterhin bekannte Einschränkungen.

- 📦 Versionsänderungen für Releases erhalten gesperrte Abhängigkeitsversionen. Plattformübergreifende Installations-/Upgradeprüfungen und die Abnahme mit echten AI-Aufrufen stehen noch aus; automatisierte Prüfungen ersetzen sie nicht.

---

## v0.1.107 — 2026-09-05

- 🧠 Globales Gedächtnis (experimentell): Claude oder Codex fasst Ihre Unterhaltungen zu einem nach Themen geordneten, gemeinsamen Wiki zusammen
- 🕸️ Code-Graph (experimentell): Arbeitsverzeichnis indizieren, Beziehungen zwischen Symbolen durchsuchen und mit Gedächtniseinträgen verknüpfen
- 🔎 Agenten können Code und Gedächtnis während einer Sitzung mit `vknowledge` abfragen
- 🖥️ SSH kann die entfernte Desktop-App spiegeln: gleiche Tabs, gleiche Teilungen und dieselbe aktive Sitzung auf beiden Rechnern
- 🪟 SSH-Gegenstellen dürfen jetzt auch Windows-Rechner sein
- 📥 Vorhandene Codex-, Claude- und OpenCode-Sitzungen aus einem Projektordner importieren
- 🤖 Neuer Agent: OMP
- 🎚️ `vspawn` kann Modell und Denkaufwand einer untergeordneten Sitzung festlegen
- 🌿 `vspawn-tree` funktioniert auch in Sammlungen, und laufende Worktree-Sitzungen zeigen ein Branch-Symbol
- 🔤 Die Suche findet zuerst ganze Wörter und hebt genau das hervor, was sie gefunden hat
- 🖱️ Ein Klick mit der mittleren Maustaste schließt einen Tab
- ⌨️ macOS-Browser: ⌘D und ⌘⇧D teilen den Bereich
- 💬 Die Eingabe per IME verschiebt die Ansicht nicht mehr, und der Cursor bleibt während der Komposition sichtbar
- 🪓 Eine Teilung aus dem Menü wirkt nur auf das fokussierte Fenster, und jede Teilung wird in `logs/split.log` protokolliert
- 📁 Die Ordnerauswahl für Remote-Verbindungen behält den eingegebenen Pfad bei
- ℹ️ Info-Panel: Startzeit und Laufzeit in einer Zeile, unter macOS zusätzlich die Systemlast
- 🔑 Passwortfelder zeigen nicht mehr die browsereigene Anzeigen-Schaltfläche

---

## v0.1.106 — 2026-09-02

### Arbeitsbereich

- **Die Seitenleiste bietet Sammlungen: Container auf oberster Ebene, die an keinen Ordner auf dem Datenträger gebunden sind.** Nicht jede Sitzung gehört zu einem Repository – etwa Remote-Sitzungen, Browserseiten oder ein Terminal, das man nur kurz zum Ausprobieren öffnet. Bisher ließen sich solche Sitzungen nur innerhalb eines Projekts ablegen, wo sie deplatziert wirkten. Eine Sammlung ist ein eigenständiger Eintrag mit einem Namen, aber ohne festen Ordner auf der Festplatte. Da es kein Projekt-Stammverzeichnis gibt, werden unpassende Aktionen direkt ausgeblendet, anstatt erst beim Anklicken fehlzuschlagen: „Neue Worktree-Sitzung“, „In Worktree verschieben…“ sowie die Worktree-Auswahl im Dialog für neue Sitzungen sind bei Sammlungen unsichtbar, statt einen Fehler auszulösen. Stattdessen bietet der Dialog für neue Sitzungen ein Feld für das Arbeitsverzeichnis, das standardmäßig auf das Home-Verzeichnis verweist – inklusive nativer Ordnerauswahl auf dem Desktop. Sitzungen, die ohne Pfadangabe gestartet werden, öffnen sich nun direkt im Home-Verzeichnis, anstatt den Startpfad der App zu erben (unter macOS war dies `/`).

- **Das Ressourcen-Panel zeigt das gesamte System an, nicht nur die aktuelle Sitzung.** Bisher zeigte das Panel CPU und Arbeitsspeicher nur für den Prozessbaum der jeweiligen Sitzung an. Dadurch ließ sich schwer erkennen, ob die Sitzung selbst alle Ressourcen beanspruchte oder das Gesamtsystem ohnehin überlastet war. Das Panel ist nun in die Bereiche „DIESE SITZUNG“ und „SYSTEM“ unterteilt. Der Bereich „SYSTEM“ zeigt Auslastungsbalken für CPU, Arbeitsspeicher und Swap (die sich ab 70 % bernsteinfarben und ab 90 % rot färben) sowie eine plattformspezifische Zeile: Speicherdruck (Memory Pressure) unter macOS, auf die Kernanzahl normierte Lastdurchschnitte (Load Averages) unter Linux und keinen dieser Werte unter Windows (wo es keine vergleichbare Metrik gibt). Wer sich nur für die eigene Sitzung interessiert, kann „system“ in der Kopfzeile des Panels einfach abwählen – die Systemzeilen werden dann ausgeblendet und das Hintergrund-Sampling wird komplett gestoppt. Diese Einstellung wird gespeichert und gilt für das Panel jeder Sitzung.

- **Windows und Linux erhalten eine Menüleiste, die über die Alt-Taste aufgerufen wird.** Während macOS native Systemmenüs nutzt, waren Einstellungen, Update-Prüfungen und geteilte Bereiche (Split Panes) unter Windows und Linux bisher nur über das Symbol in der Titelleiste zugänglich. Durch einmaliges Drücken und Loslassen der Alt-Taste lässt sich nun eine Menüleiste ein- und ausblenden. Tastenkombinationen mit Alt bleiben davon unberührt: Da Alt im Terminal als Meta-Präfix dient, werden Kombinationen wie Alt+B und Alt+F weiterhin wie gewohnt an die Shell weitergeleitet – nur das isolierte Drücken von Alt, das keine Eingabe an das Terminal sendet, öffnet das Menü. Die Leiste umfasst die Menüs Datei (Einstellungen, Nach Updates suchen), Terminal (Neues Terminal, Rechts teilen, Unten teilen) und Hilfe (Website, Feedback, Teilen) und zeigt Tastenkürzel passend zur eigenen Tastenbelegung an. Sie lässt sich sowohl per Maus als auch per Tastatur bedienen: Ein Druck auf Esc schließt das geöffnete Menü, blendet die Leiste wieder aus und setzt den Fokus direkt zurück ins Terminal.

- **„Benachrichtigungs-Badges löschen“ im Menü entfernt Badges, die sich sonst nicht zurücksetzen ließen.** Das Dock-Badge zählt ungelesene Sitzungen sowie unbeantwortete Bestätigungskarten für neue Prozesse (Spawn Confirmation Cards). Die Schaltfläche zum Leeren in der Seitenleiste erschien bisher jedoch nur bei ungelesenen Sitzungen und löschte auch nur diese. Blieb eine unbeantwortete Abfrage zurück – etwa aus einem geschlossenen Fenster oder einer gelöschten Sitzung, in der die Karte nicht mehr gerendert werden konnte –, zeigte das Dock-Badge dauerhaft eine 1 an, ohne dass man sie wegbekam. Der neue Menüeintrag markiert nun alle Sitzungen als gelesen, lehnt alle ausstehenden Spawn-Anfragen ab und setzt den Zähler im Betriebssystem direkt auf null zurück. Das ist besonders unter macOS wichtig, da das Dock den Badge-Zähler selbst nach einem Neustart der App beibehält.

- **Das Fenster friert nicht mehr ein, während Befehle Dateien lesen oder auf das System warten.** Synchrone Tauri-Befehle laufen im Haupt-Thread und blockieren während ihrer gesamten Ausführung die Benutzeroberfläche. Eine Überprüfung aller 43 Befehle zeigte, dass 13 davon I/O-Operationen im Haupt-Thread ausführten: Die Abfrage von Benachrichtigungsberechtigungen unter macOS konnte bis zu eine Minute lang auf die Bestätigung warten und dabei die UI blockieren; das Laden von Sitzungsaufzeichnungen langlebiger Sitzungen verarbeitete dutzende Megabyte in 64-KB-Blöcken; das Einfügen von Screenshots schrieb direkt auf den Datenträger; und das Ermitteln des Arbeitsverzeichnisses rief synchron `lsof` auf. Alle 13 Befehle wurden nun aus dem Haupt-Thread ausgelagert. Die Verarbeitung von Tastaturanschlägen verbleibt unverändert auf dem Fast-Path.

### Fernzugriff

- **Hosts zeigen jetzt an, welche Remote-Clients verbunden sind.** Da die Sitzungsspiegelung bidirektional funktioniert, kann ein Remote-Client auch das Layout des Hosts verändern. Bisher konnte der Host jedoch weder erkennen, ob überhaupt jemand verbunden war, geschweige denn wer. Die Titelleiste zeigt nun den Badge „Gespiegelt von N“ an; ein Klick darauf öffnet eine Liste aller verbundenen Clients samt Namen, IP-Adressen und Verbindungszeitpunkt. Der Client-Name wird beim verschlüsselten Handshake übertragen (mit Rückgriff auf „Unnamed“, falls kein Name angegeben wurde).

### KI-Agenten

- **Die Account-Nutzung wird jetzt einmal pro Rechner statt einmal pro Sitzung abgerufen.** Nutzungslimits gelten für den gesamten Account, bisher rief das Info-Panel jeder Sitzung diese Werte jedoch separat ab. Bei zehn offenen Sitzungen führte das zu zehn doppelten Anfragen für dieselben Daten – was schnell die strengen Rate-Limits von Claude auslöste und das Panel leer bleiben ließ. Ein zentraler Backend-Dienst verwaltet nun einen einzelnen Snapshot, aus dem alle Sitzungen lesen, und verteilt Aktualisierungen, sobald sich die Daten ändern. Dabei werden nur die Anbieter abgefragt, die auch aktiv genutzt werden – erkannt am Vorhandensein von `~/.claude`, `~/.codex` oder `~/.grok/auth.json` –, ohne den System-Schlüsselbund anzurühren, um unnötige Berechtigungsdialoge zu vermeiden. Snapshots werden lokal gespeichert, sodass das Panel nach einem Neustart sofort die zwischengespeicherten Werte anzeigt, statt erst die erste Abfrage abzuwarten. Auch beim Aufwecken aus dem Ruhezustand werden die Daten direkt aktualisiert, ohne auf das nächste reguläre Intervall zu warten. Bei aufeinanderfolgenden Fehlern greift ein Backoff von bis zu 8x. In den Einstellungen hat das Aktualisierungsintervall nun einen eigenen Ein-/Aus-Schalter neben dem Eingabefeld, der die bisherige, missverständliche 0-Sekunden-Einstellung ersetzt.

- **Fehlgeschlagene Nutzungsaktualisierungen werden jetzt deutlich als veraltet markiert, statt als aktuell zu erscheinen.** Schlug eine Aktualisierung bisher fehl, lieferte der Client stillschweigend seine zwischengespeicherte Kopie zurück und verbuchte dies als Erfolg – wodurch der Zeitstempel vorrückte, Fehler gelöscht wurden und der Backoff-Timer zurückgesetzt wurde. Beim Erreichen von Rate-Limits sah das Panel dadurch intakt aus, obwohl es alle fünf Minuten vergeblich neue Abfragen sendete. Fehlgeschlagene Anfragen behalten nun die zwischengespeicherten Werte bei, zeigen aber ein bernsteinfarbenes `stale`-Badge neben dem Anbieter. Fährt man mit der Maus über das Badge, erscheint ein Tooltip mit dem Zeitpunkt des Fehlers, der Ursache und dem Alter des angezeigten Werts. Der Zeitstempel „Aktualisiert“ spiegelt die letzte erfolgreiche Synchronisierung wider, und `Retry-After`-Vorgaben von Servern mit Rate-Limit werden respektiert; manuelle Aktualisierungen lassen sich trotzdem jederzeit sofort anstoßen.

- **Modellspezifische Wochenkontingente werden im Nutzungspanel angezeigt.** Neben den accountweiten 5-Stunden- und 7-Tage-Limits liefert die Schnittstelle auch modellspezifische Wochenquoten. Bisher wurden diese ignoriert: Wer Modelle wie Fable nutzte, sah nur die Gesamtnutzung des Accounts, nicht aber das noch verbleibende modellspezifische Kontingent. Diese aufgeschlüsselten Zeilen werden nun unterhalb der Gesamtsummen aufgeführt, übersichtlich gruppiert nach Modellfamilie.

- **Das erneute Öffnen einer OpenCode-Sitzung stellt den Chatverlauf wieder her.** Bisher prüfte die App per `opencode session list`, ob eine Sitzung noch existierte – dieser Befehl fragt jedoch nur Sitzungen im aktuellen Arbeitsverzeichnis ab. Da die Desktop-App aus ihrem Startverzeichnis heraus läuft (unter macOS `/`), wurden vorhandene Sitzungs-IDs nie gefunden, sodass beim Wiederaufnehmen stillschweigend leere Sitzungen geöffnet wurden. Die Prüfung fragt die jeweilige Sitzungs-ID nun direkt ab und stuft sie nur dann als gelöscht ein, wenn der Befehl dies ausdrücklich bestätigt. Andere Fehler werden als unklar gewertet und die App setzt die Wiederaufnahme trotzdem fort – so geht kein Sitzungsverlauf mehr versehentlich verloren.

### Oberfläche

- **Die Texteingabe über einen Input Method Editor (IME) zeigt nun den Kompositionstext an.** Über einen IME eingegebener Text (wie Pinyin oder Kana) war auf allen drei Plattformen bisher unsichtbar, bis man ihn mit Enter bestätigte. Ursache war eine eigene CSS-Regel: xterm rendert den Kompositionstext in einem Overlay-Container ohne Größenbegrenzung. Durch eine von uns hinzugefügte `right`-Einschränkung (die lange Terminalzeilen umbrechen sollte) schrumpfte dessen Breite auf null, während `overflow: hidden` den Text komplett abschnitt. Diese Regel wurde entfernt; als einzige Anpassung überschreiben wir jetzt nur noch das standardmäßige Schwarz-auf-Weiß-Farbschema des Overlays, das zu keinem Theme passte. Ein verwandtes Problem wurde gleich mit behoben: xterm vergrößerte seine versteckte Textarea passend zur Overlay-Größe, um IME-Vorschläge zu positionieren, verkleinerte sie danach aber nicht mehr. Das hinterließ ein unsichtbares Overlay, das Mausklicks und die Textauswahl in dieser Zeile blockierte.

- **Unter macOS lösen Ctrl-Tastenkombinationen keine Command-Befehle mehr aus.** Bei der Prüfung von Modifikatortasten wurden Cmd und Ctrl unter macOS bisher gleichgesetzt. Dadurch kollidierten gewohnte Terminal-Tastenkürzel mit den Shortcuts der App: Ctrl+D (EOF) teilte Bereiche, Ctrl+W (Wort löschen) schloss Bereiche und Tastenkürzel wie Ctrl+F, Ctrl+T und Ctrl+O wurden von der App abgefangen. Weil neue Bereiche geräuschlos öffnen, sah es oft so aus, als würden geteilte Fenster aus dem Nichts auftauchen. Die Desktop-App für macOS verwendet für Shortcuts nun ausschließlich Cmd, während andere Plattformen (sowie mit einem Mac verbundene Webbrowser) weiterhin Ctrl nutzen.

- **Dropdown-Menüs öffnen sich nach Auswahl einer Option nicht mehr sofort wieder.** Ein umschließendes `<label>`-Element leitete Klicks aus der gesamten Zeile an die Auslöser-Schaltfläche weiter. Dadurch öffnete sich das Dropdown-Menü direkt nach der Auswahl eines Eintrags sofort wieder.

### Fehlerbehebungen

- **Linux: Das AppImage startet wieder.** Das AppImage von Version 0.1.105 stürzte unter Ubuntu 22.04 ab, noch bevor ein Fenster erschien, da die WebKit-Hilfsprozesse nicht starten konnten. Packaging-Tools für AppImages schreiben feste `/usr`-Pfade in Binärdateien zu `././` um (um die String-Länge für In-Place-Patches beizubehalten). Daher muss das Starter-Skript das Arbeitsverzeichnis nach `$APPDIR/usr` wechseln, damit diese Pfade aufgelöst werden können. In Version 0.1.105 vergaß unser überarbeitetes Starter-Skript – das eigentlich verhindern sollte, dass Umgebungsvariablen in untergeordnete Shells durchsickern – genau diesen Verzeichniswechsel. Der Starter wechselt das Verzeichnis nun ordnungsgemäß, abgesichert durch eine Build-Time-Assertion zur Vermeidung künftiger Regressionen. Version 0.1.104 und ältere Versionen waren davon nicht betroffen.

- **Windows: Benachrichtigungen spielen wieder einen Ton ab, und ein Klick darauf springt direkt zur jeweiligen Sitzung.** Windows-Benachrichtigungen übermittelten zuvor eine macOS-Tonkennung an das Benachrichtigungs-Plugin. Da Windows diesen Namen nicht verarbeiten konnte, blieb die Ausgabe stumm – Windows-Benachrichtigungen gaben dadurch unabhängig von den Einstellungen nie einen Ton wieder. Der Umweg über das Plugin war vor zwei Versionen eingebaut worden, um ein Problem zu umgehen, das in derselben Version behoben wurde. Dieser Workaround wurde nun rückgängig gemacht: Benachrichtigungen laufen wieder über den nativen Kanal, was sowohl die Tonausgabe als auch den direkten Sprung zur Sitzung per Klick wiederherstellt.

- **Windows: Rahmen- und Blockgrafikzeichen schließen nahtlos aneinander an.** Terminal-Rahmen wurden zuvor mit einer proportionalen Fallback-Schriftart gerendert, was zu unsauberen Darstellungen führte. Selbst nachdem dies behoben wurde, wiesen Blockzeichen (wie sie in Fortschrittsbalken und beim Claude-Code-Maskottchen genutzt werden) noch immer eine haardünne vertikale Lücke am rechten Rand jeder Zelle auf, da die Fallback-Schriftart geringfügig schmaler war als die primäre Terminal-Schriftart. Beide Zeichenbereiche werden nun über ein mitgeliefertes Subset von JetBrains Mono bereitgestellt, was eine exakte Übereinstimmung der Glyphenbreite gewährleistet.

- **Windows: `git` funktioniert in der vollständigen Git-Bash-Variante.** Obwohl die Installation der vollständigen Git Bash erfolgreich war, meldete die Ausführung von `git` im Terminal „command not found“ und forderte Nutzer wiederholt auf, die vollständige Git Bash zu installieren. Git Bash fügt `mingw64/bin` nur dann zu `PATH` hinzu, wenn ihr mitgeteilt wird, in welchem Verzeichnisbaum sie ausgeführt wird. Da unser Shell-Starter diesen Parameter ausließ, konnten Binärdateien, die sich ausschließlich in diesem Verzeichnis befinden – wie etwa `git` und `curl` –, nicht gefunden werden. Zudem wählt die Download-Aufforderung nun automatisch das Installationsprogramm passend zur Architektur des Hostsystems, statt standardmäßig 64-Bit vorzugeben.

- **Windows: Das Entfernen von Worktrees funktioniert, und die Neuinstallation von Kiro-Hooks dupliziert diese nicht mehr.** Befehle zum Entfernen von Worktrees wurden zuvor innerhalb des Ziel-Worktree-Verzeichnisses ausgeführt. Da Windows das Löschen des aktuellen Arbeitsverzeichnisses eines aktiven Prozesses verhindert, schlug der Vorgang stets mit einem Berechtigungsfehler fehl. Git-Befehle werden nun aus dem Hauptverzeichnis des Repositorys ausgeführt. Zudem verglich die Hook-Erkennungslogik Pfade mit Schrägstrichen (Forward Slashes) mit Windows-Backslash-Pfaden, wodurch die Prüfung fehlschlug und bei jeder Neuinstallation doppelte Hook-Einträge angehängt wurden.

---

## v0.1.105 — 2026-08-26

### Arbeitsbereich

- **Sitzungsstatus, Ungelesen-Markierungen und die Frage, ob eine Sitzung noch läuft, entscheidet jetzt das Backend, und alle Clients sehen dieselbe Antwort.** Bisher rechnete sich das jeder Client aus dem zusammen, was er zufällig mitbekam, „ungelesen“ hieß also in Wahrheit „in diesem Fenster nicht gelesen“: Wer eine Sitzung im Browser las, ließ die Kopie auf dem Desktop ungelesen, und der dynamische Statusfilter schob diese Zeile immer wieder in die gemeinsame Liste zurück, wo selbst „Status aktualisieren“ sie nicht mehr herausbekam. Statusereignisse wurden außerdem erst registriert, sobald ein Client selbst eine Sitzung gestartet hatte, sodass ein frisch verbundener Browser nur für die von ihm geöffneten Sitzungen Punkte zeigte und für alle übrigen gar nichts. Das Backend führt jetzt pro Sitzung einen maßgeblichen Datensatz — den Agenten, seinen Zustand, ob sein Prozess lebt, und das Ungelesen-Kennzeichen —, beantwortet beim Verbinden oder Wiederverbinden eine Sammelabfrage und verteilt jede Änderung an alle. Die Clients melden, was sie beobachten; die Schlüsse zieht das Backend. Wer eine Sitzung auf dem Telefon liest, löscht damit die Markierung auf dem Desktop, ein frisch verbundener Browser zeigt die richtigen Punkte auch für Sitzungen, die er nie geöffnet hat, und die Schlichtungsregeln, die bisher im Frontend lagen, sind mitsamt ihren Begründungen und ihren Tests mit umgezogen: Ein hook, der einmal gemeldet hat, sperrt alles aus, was aus roher Ausgabe erschlossen wurde; fortlaufende Ausgabe bedeutet nur dann „arbeitet“, wenn die Sitzung einen Agenten hat, nicht Codex ist und noch keine maßgebliche Meldung vorliegt; und eine Sperrfrist von 1200 ms verhindert, dass eine gerade gestartete Aufgabe von einem unmittelbar hinterherkommenden Abschlussereignis wieder abgeräumt wird. Die Ausnahme ist das Auslesen des Bildschirms, denn dafür braucht es das gerenderte Zeichenraster, und das gibt es nur in einem Client: Derjenige Client, dem die Terminalgröße gehört, meldet, was er gelesen hat, und das Backend entscheidet, ob es die Meldung annimmt — von jedem anderen Client wird sie abgelehnt. Geht dabei etwas schief, gibt `vlx-arbitration` in localStorage auf `frontend` gesetzt die Schlichtung an die alte Frontend-Kette zurück.

- **Der Neustart einer Sitzung schließt ihren Tab auf dem anderen Client nicht mehr.** `pty://killed` trug keinerlei Nutzdaten, deshalb konnte die Gegenseite einen Neustart nicht von einem Schließen unterscheiden: Sie behandelte alles als Schließen, entfernte den Bereich und spiegelte dieses Layout dann zurück. Das Ereignis sagt jetzt, welcher Client den Prozess beendet hat und warum, sodass ein Neustart den Bereich stehen lässt und auf den neuen Prozess wartet. Ein fehlender oder unbekannter Grund gilt weiterhin als Schließen — einen Bereich für eine Sitzung offen zu lassen, die nie wiederkommt, hinterlässt ein totes Terminal auf dem Bildschirm, und das ist schlimmer, als einen Bereich zu schließen, der gerade neu starten wollte.

- **Ein Browser, der sich mit einem Desktop verbindet, der gerade seinen Arbeitsbereich wiederhergestellt hat, startet nicht mehr jede Sitzung wirklich.** Beim Wiederherstellen eines Arbeitsbereichs werden Platzhalterkarten gezeichnet, statt Prozesse zu starten, doch diese Entscheidung existierte nur auf dem Desktop; der Browser folgte dem gespiegelten Layout, konnte „läuft nicht“ nicht von „läuft, wurde hier nur nie geöffnet“ unterscheiden und hängte die Terminals ein — und ein Terminal einzuhängen heißt, es zu starten. Ein Blatt, das mit dem Layout eines anderen ankommt, zeigt jetzt eine Platzhalterkarte, wenn das Backend sagt, dass kein Prozess dahintersteht. Eine Sitzung selbst zu öffnen bleibt die Absicht, sie zu starten, und ein Terminal, auf das Sie gerade schauen, wird nie durch eine Karte ersetzt, wenn sein Prozess endet — Sie wollen vielleicht noch lesen, was es ausgegeben hat.

- **Eine geänderte Einstellung erreicht die anderen Clients sofort.** Die maßgeblichen Einstellungen lagen immer schon im Backend, aber es änderte sie, ohne jemandem Bescheid zu sagen, sodass der andere Client es erst beim nächsten Start erfuhr. Über eine Fernverbindung ist das mehr als eine unterschiedliche Anzeige: „Statusfilter dynamisch erweitern“ auf dem einen Client abzuschalten brachte gar nichts, solange ein anderer weiter Zeilen in die gemeinsame Liste schob, und der Client mit der kleineren Grenze für gleichzeitig aktive Tabs räumte allen anderen die Hintergrund-Tabs weg. Beim Schreiben einer Einstellung wird jetzt verteilt, welcher Schlüssel sich geändert hat, und jeder Client liest ihn über denselben Weg neu ein, den er auch beim Start benutzt — die Regeln, die geschützte Werte vor Fernaufrufern verbergen, gelten also weiterhin, und die Verteilung trägt ausschließlich Schlüsselnamen, niemals Werte. Telefone, die bisher weder Einstellungen sendeten noch empfingen, machen jetzt ebenfalls mit.

- **Ein Browser wartet auf das gespiegelte Layout, bevor er sein eigenes wiederherstellt.** Ein Fernfenster hat zwei Layout-Quellen — das in seinem eigenen localStorage und das, was der Host im Spiegelmodus schickt —, und was zuerst ankam, wurde vom jeweils anderen überschrieben. Das lokale zuerst aufzubauen kostete mehr als ein Flackern: Ein Terminal-Blatt einzuhängen startet einen echten Prozess, und bei einer Sitzung, deren Prozess längst weg war, hieß das, eine Shell zu starten, die nie jemand ansehen würde — und die dann auch blieb, denn ein Browser hängt seine Terminals nur ab, statt sie zu beenden. Der Browser wartet jetzt höchstens zwei Sekunden darauf, dass sich der erste Abgleich setzt, bevor er überhaupt etwas wiederherstellt; ist das Backend langsam oder nicht erreichbar, fällt er auf das lokale Layout zurück, statt vor einem leeren Fenster zu sitzen. Telefone und jeder Client mit abgeschaltetem Spiegeln werden sofort durchgelassen.

### Fernzugriff

- **Eine SSH-Verbindung kann den Spiegelmodus für den Dienst einschalten, den sie startet.** Der Schalter sitzt im Fernzugriffs-Panel, aber SSH startet auf dem entfernten Rechner einen Dienst ohne Oberfläche, und dort gibt es kein Panel zum Anklicken. Wer beim Klick auf „Fernverbindung herstellen“ die Wahltaste hält, bekommt im SSH-Formular jetzt ein Kästchen „Oberfläche über alle Clients spiegeln“, standardmäßig aus. Der Wert reist mit der Verbindung mit und lebt nur im Speicher des Dienstes, statt in die Datenbank des entfernten Rechners geschrieben zu werden: Wer zusätzlich die Datenbank des entfernten Desktops mitbenutzt, soll mit einer SSH-Verbindung nicht stillschweigend einen Schalter in fremden Panels umlegen. Die Wahl wird pro Host gemerkt, wer denselben Rechner also aus der Liste holt, bekommt sie zurück. Einen bereits laufenden Dienst weiterzuverwenden setzt jetzt voraus, dass Version, Datenmodus und Spiegelmodus alle übereinstimmen — weicht eines der drei ab, wird der alte Dienst ersetzt, was die darauf laufenden Sitzungen beendet; deshalb liegt diese Option zusammen mit dem Datenbankschalter hinter der Wahltaste.

- **Ein Client, der gespiegelt wird, sagt das auch.** Tabs und Teilungen ordneten sich auf einem folgenden Client bisher neu, ohne dass irgendetwas auf dem Bildschirm erklärt hätte, woher die Änderung kam. Die Titelleiste trägt jetzt ein Badge „Gespiegelt“, mit einer Erklärung beim Überfahren mit der Maus. Der Host bekommt keines — er hat ja den Schalter.

- **Dateien lassen sich zwischen Ihrem Rechner und dem Rechner mit dem Terminal übertragen.** Der Fernzugriff zeigte die Dateien der Gegenseite, bot aber keinen Weg, eine davon zu holen oder eine hinzulegen — es blieb nur ein Shell-Befehl im Terminal. Das Dateien-Panel bietet nun „Download“ im Kontextmenü einer Datei und „Upload“ in der Kopfzeile; wer Dateien vom Schreibtisch auf eine Ordnerzeile zieht, schickt sie ebenfalls dorthin. Beide Richtungen laufen über dieselbe authentifizierte Verbindung wie alles andere, funktionieren also im Browser im lokalen Netz, auf dem Telefon und im Fernverbindungsfenster gleichermaßen. Übertragen wird in Blöcken, mit einer Fortschrittsliste unter dem Baum, die auch dann weiterläuft, wenn Sie ein anderes Panel ansehen. „Download“ ist ein gewöhnlicher Download-Link, um den sich der Download-Manager des Browsers kümmert: Er schreibt fortlaufend auf die Festplatte, zeigt Tempo und Restzeit an und lässt sich anhalten und fortsetzen — bei jeder Dateigröße und in jedem Browser, auch auf dem Telefon. Der Link trägt ein Ticket, das für genau diese eine Datei ausgestellt wurde und nach wenigen Minuten verfällt, denn dieser Server hält Anmeldedaten in einem Header, und ein Browser, der einen Link abruft, sendet keinen. Ein Upload schreibt zunächst unter einem temporären Namen und wird erst nach vollständiger Übertragung umbenannt — eine abgebrochene Übertragung hinterlässt also nie eine halb geschriebene Datei dort, wo eine ganze stehen sollte; ein bereits vergebener Name wird abgelehnt, bevor etwas übertragen wird. Uploads zeigen Tempo und Restzeit an und überstehen einen Verbindungsabbruch: Ein fehlgeschlagener Block wird rund eine Minute lang mit wachsenden Pausen erneut versucht, wobei jedes Mal beim Server nachgefragt wird, wie weit die temporäre Datei tatsächlich gekommen ist, statt einen möglicherweise längst angekommenen Block noch einmal zu senden. Auch beim Aufgeben bleiben diese Bytes erhalten: Dieselbe Datei erneut auf denselben Ordner gezogen macht dort weiter, wo es aufgehört hat, sogar nach einem Neuladen — nur Abbrechen verwirft die Teildatei.

### KI-Agenten

- **Antigravity- und Copilot-Sitzungen werden nach ihrer ersten Nachricht benannt.** Beide fehlten in der automatischen Umbenennung, sodass in der Seitenleiste Reihen von „Antigravity 1, 2, 3“ standen. Antigravitys hook-Ereignisse enthalten überhaupt keinen Nutzertext, nur eine Unterhaltungs-ID und einen Pfad zum Transkript, deshalb wird die erste Nachricht stattdessen aus dem Transkript gelesen; der Metadatenblock, der darauf folgt, bleibt aus dem Titel heraus. Copilots Ereignisse enthalten ebenso wenig einen Ereignisnamen und werden an ihrer Form auseinandergehalten, deshalb gilt ein Rumpf mit einem prompt und ohne Werkzeugnamen jetzt als Absenden — womit der prompt, mit dem eine Sitzung gestartet wird, und Werkzeugaufrufe korrekt draußen bleiben.

- **Eine Antigravity-Sitzung öffnet sich wieder mit ihrer Historie.** Zum Fortsetzen braucht es die Unterhaltungs-ID, und der Parser, der eine Sitzungs-ID aus den Startargumenten herausliest, kannte Antigravitys Schreibweise dafür nicht — `--conversation=<id>` hatte also nie einen Anker, auf den es hätte zeigen können, und jede wieder geöffnete Sitzung kam leer zurück.

- **`vspawn --yes` startet eine Unteraufgabe ohne die Bestätigungskarte.** Wer „vor dem Starten bestätigen“ eingeschaltet lässt, musste sich in einem Durchlauf durch die Karte jeder einzelnen Untersitzung klicken. Das Flag — auch als `-y` oder `--no-confirm` zu schreiben — überspringt die Karte für diesen einen Aufruf und startet die Sitzung mit den Standardeinstellungen. An der Einstellung selbst ändert es nichts, der nächste Start ohne das Flag fragt also wieder nach.

- **Das Modellfeld auf der Karte für Unteraufgaben nimmt alles an, was Sie eintippen.** Es war ein reines Aufklappmenü, es ließen sich also nur die Modelle aus der Liste wählen — und ein Agent versteht weit mehr Bezeichner als die: datierte Namen wie `claude-opus-4-6`, Namen mit Herstellerpräfix, lokal eingerichtete Aliasse. Jetzt ist es ein Textfeld, an dem die bekannten Modelle als Abkürzung in einem Aufklappmenü daneben hängen. Die Liste ist ein Vorschlag, keine Positivliste: Übergeben wird, was Sie eintippen, ein leeres Feld heißt gar kein `--model`, und die Liste filtert beim Tippen mit und klappt weg, sobald ein eigener Bezeichner zu nichts mehr passt.

### Oberfläche

- **Ein Aufklappmenü, überall dasselbe.** Die Aufklappmenüs der App waren mehr als ein Dutzend Mal aus demselben Code kopiert worden und dann auseinandergelaufen: drei Panel-Hintergründe, vier Schatten, drei Hover-Farben, Auslöser mit 26, 28 und 32 Pixeln Höhe und Haken an ausgewählten Zeilen, die nur eine Mehrfachauswahl braucht. Eine einzige Komponente trägt jetzt die Branch-Auswahl beim Mergen, die Auswahl von Sprache, Standard-Shell und Schrift in den Einstellungen, die Agentenauswahl, die Worktree-Dialoge, den Agententyp bei neuen und wiederhergestellten Sitzungen und das letzte native Auswahlfeld im Formulardialog. Sie bringt außerdem eine Tastaturbedienung mit, die keines von ihnen hatte: Pfeiltasten zum Bewegen, Enter zum Auswählen, Escape zum Schließen, ohne den Dialog dahinter mitzuschließen, Pos1 und Ende zum Springen. Die beiden Menüs in der Statusleiste verhalten sich wie bisher, und der Statusfilter in der Seitenleiste behält seine Haken, denn er ist wirklich eine Mehrfachauswahl.

- **Das Passwortfeld im Fernzugriffs-Panel lässt sich anzeigen.** Es war ein nacktes Passwortfeld, man konnte also nicht sehen, was man getippt hatte; den Augen-Knopf gab es zwar, aber nur innerhalb der Datei des Verbindungs-Panels. Beide teilen sich jetzt eine Komponente, und der Anzeigezustand wird zurückgesetzt, sobald das Panel schließt.

- **Die IP-Auswahl sieht nicht mehr wie ein Bedienelement des Systems aus.** Sie war ein natives Auswahlfeld, und WKWebView steckt so etwas in Systemoptik, die auf einem dunklen Panel schlecht sitzt — dieselbe Klage wie bei den oben ersetzten Aufklappmenüs. Sie verwendet jetzt die gemeinsame Komponente, und ihre Beschriftung ist auf „IP“ verkürzt, weil der Text daneben ohnehin sagt, wofür sie da ist.

### Fehlerbehebungen

- **Die Suche nach Updates fragt jetzt jedes Mal wirklich beim Server nach.** Ein durchlaufender Client war auf die erste Version festgenagelt, die er je gesehen hatte: Wer 0.1.101 gefunden hatte, bekam auch nach dem Erscheinen von 0.1.104 weiterhin 0.1.101 angeboten, und „Nach Updates suchen“ öffnete nur denselben Dialog erneut, weil der alte Code sofort zurückkehrte, sobald ein Hinweis bereits anstand. Jede Prüfung ist jetzt eine echte Anfrage. Eine neuere Version ersetzt den Hinweis auf dem Bildschirm, dieselbe Version oder ein laufender Download lassen ihn in Ruhe, und ein Server, der überhaupt kein Update meldet, nimmt einen veraltet gewordenen Hinweis wieder herunter — die Version wurde zurückgezogen, oder Sie haben sie zwischenzeitlich selbst installiert. Der Knopf „Manuell herunterladen“ öffnet jetzt die Download-Seite auf der Website; bisher reichte er das Paket des Updaters selbst heraus, das sich an Ort und Stelle auspackt und sich von Hand gar nicht installieren lässt.

- **Beim schnellen Abbauen von Terminals erscheint kein bildschirmfüllendes Fehler-Overlay mehr.** Der Viewport von xterm plant beim Erzeugen einen Abgleich des Scrollbereichs ein und beim Zurücksetzen noch einen, bricht aber beim Verwerfen keinen von beiden ab. Ein Terminal, das innerhalb derselben Aufgabe geöffnet und wieder geschlossen wird — und genau das passiert, wenn der Sitzungsbaum während einer Fernverbindung neu aufgebaut wird —, führte diese Callbacks trotzdem aus, fand einen bereits geleerten Renderer vor und warf einen Fehler. Der Fehler kommt aus einem Timer, wo weder try/catch noch eine Error Boundary hinreichen, deshalb wird er global abgefangen und eng abgeglichen: Nur ein Stack oder eine Meldung, die genau diesen Abgleich benennt und dazu den Renderer oder seine Abmessungen erwähnt, wird als harmlos geschluckt und ins Anfrageprotokoll geschrieben. Echte Abstürze lassen das Overlay weiterhin erscheinen.

---

## v0.1.104 — 2026-08-25

### KI-Agenten

- **Die Karte für Unteraufgaben bietet jetzt die echten Modelle jedes Agenten an und dasjenige Aufwands-Flag, das der Agent auch wirklich versteht.** Die Karte baute ihre Startargumente für alle Agenten mit `--model` und `--effort` zusammen, doch nur Claude, Kiro und Antigravity schreiben den Denkaufwand so — Grok und Zoo nennen ihn `--reasoning-effort`, Cline nennt ihn `--thinking`. Wer bei einem der übrigen eine Aufwandsstufe wählte, übergab der CLI ein Flag, von dem sie noch nie gehört hatte, und die Sitzung startete nicht. Jeder Agent steuert jetzt seine eigenen Flag-Namen und seine eigenen Werte bei. Die Modellauswahl richtet sich danach, was die jeweilige CLI mitteilen kann: Wer seinen Katalog auflisten kann (OpenCode, Grok, Crush, Antigravity, Cursor, pi, Kiro), wird danach gefragt und bietet die echte Liste an; wer eine feste Auswahl hat (Claude, Codex, Kimi Code), bietet genau diese an; der Rest zeigt ein Textfeld mit einem Beispiel für die erwartete Schreibweise. Ein Agent, der nicht installiert oder nicht angemeldet ist, sagt das, statt endlos zu drehen. Die Wahl von „Standard“ löscht jetzt eine geerbte Überschreibung, statt den alten Wert stehen zu lassen, und die Wahl einer Aufwandsstufe verwirft nicht mehr das von der übergeordneten Sitzung geerbte Modell.

- **Eine aus einer Kimi-Code-Sitzung gestartete Unteraufgabe bleibt Kimi Code.** Kimi Code fehlte in der Liste, aus der der Startpfad den Agenten der übergeordneten Sitzung übernimmt, sodass die Untersitzungen stillschweigend mit dem Standardagenten zurückkamen.

### Arbeitsbereich

- **Eine Gruppe lässt sich auch nachträglich in einen Worktree verschieben.** Der Worktree wurde beim Anlegen der Gruppe gewählt und stand von da an fest; wer es sich anders überlegte, musste die Gruppe löschen und neu aufbauen. Ein Rechtsklick auf eine Gruppe und „Move to Worktree…“ legt einen neuen Worktree an, bindet einen vorhandenen ein oder richtet eine bereits gebundene Gruppe neu aus. Nur die Gruppe selbst ändert sich: Sitzungen, die schon darin liegen, behalten das Verzeichnis, mit dem sie angelegt wurden — eine laufende Sitzung kann nicht in ein anderes Verzeichnis unterhalb ihrer selbst verschoben werden —, während später angelegte Sitzungen im Worktree starten.

- **Der Spiegelmodus umfasst jetzt den gesamten Baum der Seitenleiste.** Bisher teilte er die Auswahl und die eingeklappten Bereiche; das Suchfeld sowie die Status- und Markierungsfilter blieben lokal, in der Annahme, ihre Synchronisierung störe denjenigen, der gerade etwas nachschlägt. Diese Überlegung war verkehrt herum: Spiegeln heißt, dass beide Fenster denselben Zustand halten, und nicht, dass eines die Tastenanschläge des anderen nachspielt — ein Filter, der hier an ist, ist auch dort an. Was wirklich stört, sind zwei Seiten, die verschiedene Bäume zeigen. Jetzt reist jede Projektion der Seitenleiste mit: das aufgeteilte Layout, der Name jeder Projektion, ihr Suchtext, ihre Status- und Markierungsfilter und ihr eigener Einklappzustand. Das Format der Momentaufnahme steht jetzt auf Version 2, und ein Client mit einer älteren Version hört auf zu spiegeln, statt ein Bild nur halb anzuwenden — laden Sie also jedes Fenster neu, das Sie über das Update hinweg offen gelassen haben.

### Oberfläche

- **Das Schließen des Fensters unter macOS stellt jetzt dieselbe Frage wie das Beenden.** ⌘Q und der Menüeintrag liefen über die eigene Rückfrage der App, doch der rote Schließen-Knopf zerstörte das Fenster auf der Stelle — und genau dieses Fenster enthält die Webview, in der der Bestätigungsdialog lebt. Man bekam also entweder gar keine Rückfrage oder den abgespeckten nativen Ersatz, ohne das Kästchen „Arbeitsbereich sichern“ und mit unübersetztem Text. Auf allen drei Plattformen bleibt das Fenster jetzt offen, bis Sie geantwortet haben.

- **„Arbeitsbereich sichern“ ist von vornherein angehakt und bleibt so, wie Sie es einstellen.** Ein verlorenes Layout kostet mehr als eine ungewollte Momentaufnahme, deshalb startet das Kästchen angehakt. Bisher vergaß es die Einstellung außerdem: Sie wurde mit 400 ms Verzögerung in die Datenbank geschrieben, und das Beenden tötete den Prozess noch innerhalb dieses Zeitfensters, sodass der nächste Start gegen den alten Wert abglich und Ihre Änderung zurücknahm. Der Schreibvorgang wird jetzt vor dem Beenden herausgeschrieben, mit einer Obergrenze von 600 ms, damit ein hängendes Backend den Bestätigungsknopf nicht ewig drehen lässt. (Beigetragen von FarhadGSRX.)

- **Passwörter im Fernverbindungs-Panel lassen sich anzeigen.** Sowohl das URL-Passwort als auch das SSH-Passwort haben einen Augen-Knopf, der zwischen verdeckter Darstellung und Klartext umschaltet. Der Anzeigezustand gilt nur für das jeweilige Feld und wird zurückgesetzt, sobald das Panel schließt, sodass nie ein Passwort offen auf dem Bildschirm stehen bleibt.

- **Das Filter-Badge in der Seitenleiste zählt jeden aktiven Filter.** Ein Markierungsfilter ließ die Schaltfläche nur aufleuchten, ohne etwas dazuzusagen, sodass dort 1 stehen konnte, während zwei Filter aktiv waren. Es zählt jetzt Status und Markierungen zusammen und stimmt mit den Haken im Aufklappmenü überein; ein einzelner Statusfilter behält seinen farbigen Punkt.

### Fehlerbehebungen

- **Automatische Updates unter macOS funktionieren wieder.** Die Pakete der v0.1.103 enthielten AppleDouble-Begleiteinträge (`._VelaTerm.app`); der Updater entfernt von jedem Pfad die erste Komponente — womit hier ein leerer Pfad übrig blieb — und weigerte sich daraufhin, das Archiv überhaupt zu entpacken. Betroffen waren beide Architekturen, alle macOS-Nutzer auf v0.1.103 saßen also fest. Beim Packen entstehen diese Einträge nicht mehr.

- **Native Bedienelemente folgen dem Thema der App, wenn es vom System abweicht.** Ein Themenwechsel setzte zwar die Farben der App selbst, aktualisierte aber nie `color-scheme`: Dieser Wert wurde einmal beim Start aus der Systemeinstellung übernommen und danach nie wieder angefasst, sodass Kontrollkästchen, Aufklappmenüs und Bildlaufleisten unter einer hellen App auf einem dunklen System dunkel blieben. (Beigetragen von FarhadGSRX.)

- **Ein frisch geklontes Repository lässt sich wieder bauen.** Die Rust-Crate bettet `../dist` beim Kompilieren ein, und der Dev-Befehl erzeugt dieses Verzeichnis nicht — ein frisch geklontes Repository scheiterte deshalb schon beim Kompilieren, bevor es überhaupt laufen konnte. Das Build-Skript legt das Verzeichnis jetzt an, wenn es fehlt. (Beigetragen von FarhadGSRX.)

---

## v0.1.103 — 2026-08-24

### Fehlerbehebungen

- **Codex-Sitzungen unter Windows verweigern nicht mehr den Start.** Jede Codex-Sitzung schlug sofort mit `unexpected argument '--codex-hook'` fehl, da die TOML-Tabelle der Lifecycle-Hooks über die Kommandozeile mit Leerzeichen und Anführungszeichen übergeben wurde und das per npm installierte `codex.cmd` dies über cmd.exe erneut verarbeitet — cmd.exe entfernt die Anführungszeichen und teilt den Wert an Leerzeichen in mehrere Argumente auf. Die Hook-Injektion wird unter Windows nun übersprungen; die Statuserkennung fällt auf die bestehenden Heuristiken notify / screen / busy zurück, die weiterhin Leerlauf- und Beschäftigungszustände melden, wenn auch weniger präzise als Hooks. macOS und Linux sind nicht betroffen und verwenden weiterhin Hooks.

- **Die Windows-IME-Voreditierkorrektur aus v0.1.102 wurde zurückgenommen.** Die Korrektur, die das Kompositions-Overlay für die Eingabe von Chinesisch, Japanisch und Koreanisch wiederherstellte, fügte ihm auch eine Hintergrundfarbe, einen 1px-Rahmen und abgerundete Ecken hinzu, wodurch beim Tippen ein kleines Kästchen um den Voreditiertext im Terminal erschien — etwas, das dort nicht erscheinen sollte. Da die Overlay-Dimensionierung, die Helper-Container-Geometrie und die Textarea-Bereinigung voneinander abhingen, musste die gesamte Änderung zurückgenommen werden. Das zugrundeliegende Problem — blindes Tippen von CJK unter Windows — bleibt offen und wird in Issue #6 verfolgt.

---

## v0.1.102 — 2026-08-23

### KI-Agenten

- **Agenten-Vorlagen: mehrere kompatible CLIs nebeneinander betreiben.** Jeder Agententyp war fest mit genau einer ausführbaren Datei verdrahtet; ein Fork, ein Nightly-Build oder eine zweite CLI, die dasselbe Protokoll spricht, kamen deshalb gar nicht erst hinein – man änderte die Startargumente eines vorhandenen Typs und verlor damit das Original. Eine Vorlage benennt jetzt ihre eigene ausführbare Datei, ihr eigenes Symbol und ihre eigenen Startargumente und erscheint im Menü für neue Sitzungen neben den eingebauten Typen. Sitzungen merken sich, aus welcher Vorlage sie entstanden sind, sodass beim Forken dieselbe ausführbare Datei erhalten bleibt; und eine auf dem Desktop angelegte Vorlage taucht samt Symbol auch auf gekoppelten Browsern und Fernclients auf, weil das Symbol als Daten mitreist und nicht als Pfad auf einem bestimmten Rechner. Bestehende Sitzungen bleiben unberührt: Eine Datenbank aus einer früheren Version startet genau so wie zuvor.

- **Die Karte für Unteraufgaben wählt Modell und Denkaufwand, und eine Antwort erledigt sie überall.** Wenn ein Agent eine Unteraufgabe starten möchte, bietet die Bestätigungskarte jetzt das Modell an und – sofern der Agent das unterstützt – den Denkaufwand, beides vorausgefüllt aus den Startargumenten der übergeordneten Sitzung, sodass im Normalfall ein Klick genügt. Stellt man die Karte auf einen anderen Agenten um, werden beide Werte neu ermittelt; ein Modellname aus der einen CLI kann also nicht mehr auf der Kommandozeile einer anderen landen. Die Karte erscheint auf allen verbundenen Clients, und wer sie auf einem beantwortet, lässt sie jetzt auf den übrigen verschwinden; die erste Antwort beansprucht die Aufgabe außerdem auf dem Server, sodass eine Bestätigung auf dem Telefon und auf dem Desktop in derselben Sekunde genau einen Worktree und eine Untersitzung erzeugt statt zwei.

### Arbeitsbereich

- **Spiegelmodus: ein gemeinsames Layout auf allen Clients.** Der Terminalstrom war immer schon gemeinsam – ein PTY, ein Bytestrom –, aber die Anordnung darum herum lag nur im Browserspeicher des jeweiligen Clients: Ein über das LAN geöffneter Browser zeigte seine eigenen Tabs und Teilungen, und wer den einen Bildschirm umräumte, änderte am anderen nichts. Ist der Spiegelmodus an, werden Tabs, Teilungen, die aktive Sitzung, die Auswahl in der Seitenleiste und die eingeklappten Bereiche an alle Clients verteilt und von allen übernommen. Den Schalter dafür bedient der Host im Fernzugriffs-Panel. Umräumen auf der einen Seite wirkt auf der anderen; eine Sitzung, die das Layout dieses Fensters verlässt, wird nur abgehängt und nicht beendet, sodass das Folgen eines Gegenübers nie jemandem den Prozess abwürgt; und das Übernehmen eines fremden Layouts nimmt demjenigen, der gerade lokal tippt, nicht die Tastatur weg. Telefone bleiben außen vor – die zweistufige Telefonnavigation ist eine andere Art von Oberfläche, und ein Desktop-Teilungsbaum darauf hilft niemandem.

- **Der Git-Tab in der rechten Seitenleiste ist ein brauchbarer Git-Client geworden.** Bisher listete er nur die geänderten Dateien auf. Jetzt lassen sich einzelne Dateien oder ganze Gruppen zum Commit vormerken und wieder herausnehmen, Änderungen verwerfen, ein Commit schreiben (auch als amend) und die Commit-Historie mit den Dateien und Diffs jedes Commits ansehen – gruppiert in vorgemerkt, geändert, unverfolgt und committet, jeweils zum Zuklappen. Pfade werden vom Wurzelverzeichnis des Repositorys aus behandelt, sodass eine in einem Unterverzeichnis geöffnete Sitzung genau die Dateien anfasst, die sie anzeigt, und ein losgelöster HEAD wird auch so benannt, statt als Branch namens HEAD zu erscheinen.

### Oberfläche

- **⌘Q stellt jetzt dieselbe Frage wie das Schließen des Fensters.** Der Eintrag „Beenden“ im Anwendungsmenü war der des Systems und beendet den Prozess auf der Stelle: ⌘Q übersprang damit die Rückfrage zum Sichern des Arbeitsbereichs, die beim Klick auf Schließen erscheint – dieselbe Absicht verhielt sich also unterschiedlich, je nachdem, wie man sie ausdrückte. Beide Wege laufen jetzt über eine einzige Rückfrage. Hat das Fenster, das sie anzeigt, sich zwischenzeitlich neu geladen oder ist es abgestürzt, fragt ein erneutes ⌘Q noch einmal nach und weicht auf einen nativen Dialog aus, statt die App unbeendbar zu lassen.

- **Die Kurzbefehl-Hinweise zeigen die Tasten, die wirklich funktionieren.** Die Voreinstellungen unterscheiden sich je nach Plattform, und ein Browser behält die Kombinationen aus ⌘/Ctrl und einem Buchstaben für sich – ⌘D setzt ein Lesezeichen, ⌘T öffnet einen Tab –, sodass die ⌘-Kurzbefehle der App unter macOS nie bei der Seite ankamen, wenn VelaTerm über eine URL geöffnet wurde. Gewöhnliche Browser-Tabs verwenden jetzt auf jedem Betriebssystem die Ctrl+Alt-Belegungen, während Desktop-Apps und Fernverbindungsfenster bei ⌘ bleiben. Tooltips und der Hinweis im leeren Tab zeigen die tatsächlich geltende Belegung an – auch eine selbst geänderte – statt einer fest eingebauten ⌘-Kombination; und das Terminal blockiert genau die Kombinationen, die die App für sich beansprucht hat, sodass eine neu belegte Aktion ihre Taste mitnimmt.

- **Die Schriftvorlagen decken Nerd Fonts und CJK ab, und eine eigene Schrift, die nicht installiert ist, sagt es.** Die Liste der Vorlagen enthält jetzt die gängigen Nerd-Font- und CJK-Familien, und eine von Hand eingetippte Schrift wird angezeigt und geprüft: Hat das System sie nicht, sagt die Einstellungsseite das, statt stillschweigend auf eine Standardschrift zurückzufallen, die mit der gewünschten nichts zu tun hat.

- **Textfelder unter macOS schreiben nichts mehr groß und korrigieren nichts mehr.** Die automatische Großschreibung, die Autokorrektur und die Rechtschreibprüfung des Systems griffen in jedem Eingabefeld der App, auch bei Sitzungsnamen und Befehlsfeldern, wo aus „npm“ ein „Npm“ wurde. Sie sind jetzt überall abgeschaltet.

### Windows

- **Beim Tippen auf Chinesisch, Japanisch oder Koreanisch erscheinen der Kompositionstext und das Kandidatenfenster wieder.** Beide waren unsichtbar – man tippte blind und sah das Ergebnis erst nach der Eingabetaste. Schuld waren zwei eigene CSS-Regeln: Der Container mit dem Kompositions-Overlay fiel auf die Breite null zusammen, und der `right`-Abstand des Overlays löste sich darin zu gar nichts auf. Das Kandidatenfenster folgte dem, denn das Betriebssystem platziert es anhand des Rechtecks dieses Overlays. Das Overlay wird wieder gezeichnet und in den Farben der App dargestellt, und das unsichtbare Eingabeelement, auf dem es sitzt, gibt seine Geometrie frei, sobald die Komposition endet – Klicken und Ziehen über dieser Stelle erreicht also das Terminal und nicht mehr ein leeres Element, das sie bisher zudeckte.

- **Die native Titelleiste folgt der Hell-/Dunkel-Einstellung.** Die App behält die Titelleiste des Systems, und Windows zeichnet sie hell, solange ihm niemand etwas anderes sagt – über einer dunklen Oberfläche saß deshalb ein weißer Streifen. Jetzt passt sie zur App, auch bei später geöffneten Fenstern wie SSH- und Fernverbindungsfenstern. Wer „Systemeinstellung folgen“ wählt, gibt die Kontrolle wieder an das Betriebssystem ab, statt einen Wert festzuschreiben.

- **Das verirrte Quadrat beim Kaltstart ist weg.** Das Single-Instance-Plugin legt ein verstecktes Nachrichtenfenster an und gab ihm nie die Transparenz, die sein eigener Stil versprach; Windows vergrößerte das nulldimensionale Fenster deshalb gelegentlich auf seine Mindestgröße und malte während des Starts ein kleines Quadrat. Es ist jetzt richtig transparent; am Single-Instance-Verhalten ändert sich nichts.

### Leistung

- **Der Fernzugriff lädt bis zum ersten Bild deutlich weniger.** Statische Dateien werden jetzt bei Bedarf komprimiert und mit Cache-Kennungen ausgeliefert, sodass ein zweiter Besuch nur noch nachfragt, statt erneut herunterzuladen; und die Sprachpakete sowie die optionalen Renderer des Terminals werden erst geladen, wenn sie jemand braucht, statt zur ersten Übertragung zu gehören. Zusammen sinkt die anfängliche Übertragung auf ungefähr ein Fünftel.

### Fehlerbehebungen

- **Eine gestartete Untersitzung beginnt jetzt so wie die Sitzung, die sie angefordert hat.** Untersitzungen erbten weder den Berechtigungsmodus noch die Startargumente der übergeordneten Sitzung: Die Untersitzung einer Sitzung, die Bestätigungen überspringt, fragte wieder nach, und ein auf der übergeordneten Sitzung festgelegtes Modell fiel weg. Beides wird jetzt vererbt, mit Rückfall auf die globalen Vorgaben des Agententyps – dieselben, die auch das Menü „Neue Agentensitzung“ verwendet.

- **Ein Fernfenster meldet beim erneuten Verbinden keine falsche „Authentifizierung fehlgeschlagen“ mehr.** Beim Wiederverbinden lieferten sich der neue WebSocket und der abgelöste ein Rennen; der Abbau des Verlierers wurde als fehlgeschlagene Authentifizierung gemeldet, und das Banner warf einer völlig gültigen Kopplung vor, abgewiesen worden zu sein.

- **Eine Gruppe lässt sich nicht mehr in den eigenen Unterbaum ziehen.** Wer eine Gruppe auf einen ihrer eigenen Nachfahren fallen ließ, hängte damit den ganzen Zweig vom Baum ab, und die Sitzungen darin verschwanden aus der Seitenleiste, bis die Datenbank von Hand repariert wurde. Dieser Zug wird jetzt abgelehnt.

- **Die Ausgabe von Agenten behält ihre Farben, wenn VelaTerm aus einem anderen Werkzeug heraus gestartet wird.** Ein Terminal erbt die Umgebung dessen, was es gestartet hat: Ein Start aus einer IDE oder aus einem Agenten-Harness, das `NO_COLOR`, `CI` oder `FORCE_COLOR=0` exportiert, ließ deshalb jede Agenten-TUI in VelaTerm einfarbig erscheinen, obwohl das Terminal volle Farbunterstützung meldet. Diese geerbten Werte werden beim Start einer Sitzung verworfen; dieselben Variablen aus dem eigenen Shell-Profil gelten weiterhin, denn dieses Profil läuft innerhalb der Sitzung.

## v0.1.101 — 2026-08-15

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
