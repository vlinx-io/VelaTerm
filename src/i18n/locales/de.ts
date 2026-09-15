//! German dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const de: typeof en = {
  "tree.newPlanExecuteSession": "Neue Sitzung für Planung und Umsetzung…",
  "launch.splitTasks": "Automatisch in mehrere Aufgaben aufteilen",
  "launch.splitTasksHint": "Die Planung schlägt unabhängige Aufgaben vor. Prüfen Sie vor dem Start die Anweisungen, Agenten, Modelle und den Denkaufwand.",
  "launch.splitReview": "Ausführungsaufgaben prüfen",
  "launch.splitReviewHint": "Eine Planungssitzung empfängt alle Berichte und prüft jede Aufgabe einzeln. Die Ausführung beginnt erst nach Ihrer Bestätigung.",
  "launch.splitConfirmed": "Diese Aufgaben wurden bestätigt.",
  "launch.splitClosed": "Dieser Vorschlag wartet nicht mehr auf Bestätigung.",
  "launch.splitRetry": "Ausstehende Nachrichten erneut zustellen",
  "launch.splitSharedDirectory": "Alle Ausführungssitzungen verwenden das Arbeitsverzeichnis der Planungssitzung und gegebenenfalls deren Worktree.",
  "launch.createIn": "Erstellen in",
  "launch.workingDirectory": "Pfad zum Arbeitsverzeichnis",
  "launch.createAndStart": "Erstellen und starten",
  "launch.planExecuteTaskHint": "Beschreiben Sie die Aufgabe, Anforderungen und Abnahmekriterien für die Planung.",
  "launch.planExecuteResult": "Zuerst startet die Planungssitzung. Sobald der Plan steht, erstellt sie die Ausführungssitzung.",
  "launch.planExecuteWorktreeHint": "Neue Worktrees basieren auf dem aktuellen Commit und enthalten keine uncommitteten Änderungen. Schlägt die Erstellung fehl, startet die betroffene Sitzung nicht.",
  "launch.workflowDirectorySharedHint": "Die Planungssitzung und alle Ausführungssitzungen teilen sich ein neues Verzeichnis und einen Branch.",
  "launch.workflowDirectoryEachHint": "Die Planungssitzung und jede Ausführungssitzung erhalten einen eigenen Worktree und einen eigenen Branch.",
  "launch.planTitle": "Planung und Prüfung",
  "launch.execTitle": "Umsetzung",
  "launch.planExecuteIntro": "Eine separate Planungssitzung steuert die Umsetzung, prüft das Ergebnis und fordert Korrekturen an.",
  "chat.origin.plan": "Planung",
  "chat.origin.exec": "Umsetzung",

  // Project code intelligence and knowledge entry associations.
  "knowledge.callers": "Aufrufer",
  "knowledge.callees": "Aufgerufene Symbole",
  "knowledge.explore": "Code erkunden",
  "knowledge.exploreHint": "Eine Funktion oder einen Ablauf beschreiben oder eine Datei bzw. ein Symbol nennen…",
  "knowledge.impact": "Auswirkungsanalyse",
  "knowledge.path": "Aufrufpfad",
  "knowledge.target": "Zielsymbol suchen…",
  "knowledge.depth": "Suchtiefe",
  "knowledge.noPath": "Im Index wurde kein gerichteter Aufrufpfad gefunden.",
  "knowledge.watching": "Automatische Synchronisierung aktiv",
  "knowledge.onDemand": "Vor Abfragen synchronisieren",
  "knowledge.overview": "Übersicht",
  "knowledge.uncertain": "Abgeleitete Beziehung",
  "knowledge.kind": "Symboltyp",
  "knowledge.language": "Sprache",
  "knowledge.results": "Ergebnisse",
  "knowledge.resultLarge": "Das Ergebnis ist zu groß für die Anzeige. Grenzen Sie die Abfrage ein oder verringern Sie die Suchtiefe.",
  "knowledge.queryFailed": "Die Codeabfrage ist fehlgeschlagen. Erneut versuchen oder den Index synchronisieren.",
  "knowledge.liveHelp": "Dateiänderungen werden synchronisiert, solange der Abfrageprozess läuft. Nach seinem Leerlaufende werden Änderungen vor der nächsten Abfrage nachgeholt.",
  "knowledge.startHelp": "Mit aktivierter Indizierung können Sie Code suchen, Aufrufe verfolgen und die Auswirkungen einer Änderung analysieren. Die Analyse läuft im Backend ohne KI-Modell.",
  "knowledge.title": "Codegraph",
  "knowledge.intro": "Untersuchen Sie Codebeziehungen und verknüpfen Sie sie mit gespeicherten Entwurfsentscheidungen.",
  "knowledge.setup": "Installieren Sie CodeGraph auf diesem Backend, um Projekte zu indizieren.",
  "knowledge.downloadNotice": "Lädt die geprüfte CodeGraph-Laufzeit von GitHub herunter. Die Indizierung erfolgt auf diesem Rechner; Telemetrie und Updateprüfungen sind deaktiviert.",
  "knowledge.install": "CodeGraph herunterladen",
  "knowledge.installing": "Download und Installation…",
  "knowledge.directory": "Arbeitsverzeichnis",
  "knowledge.enable": "Indizierung aktivieren",
  "knowledge.disable": "Indizierung deaktivieren",
  "knowledge.sync": "Synchronisieren",
  "knowledge.ready": "Bereit",
  "knowledge.disabled": "Deaktiviert",
  "knowledge.indexing": "Indizierung läuft…",
  "knowledge.syncing": "Synchronisierung läuft…",
  "knowledge.failed": "Fehlgeschlagen",
  "knowledge.symbols": "Symbole",
  "knowledge.files": "Dateien",
  "knowledge.edges": "Beziehungen",
  "knowledge.search": "Symbole oder Dateipfade suchen…",
  "knowledge.searchButton": "Suchen",
  "knowledge.noResults": "Keine passenden Symbole.",
  "knowledge.selectSymbol": "Wählen Sie ein Symbol aus, um Quellcode, Beziehungen und verknüpfte Wissenseinträge anzuzeigen.",
  "knowledge.source": "Quellcode",
  "knowledge.incoming": "Eingehende Beziehungen",
  "knowledge.outgoing": "Ausgehende Beziehungen",
  "knowledge.noEdges": "Keine indizierten Beziehungen.",
  "knowledge.analysisNote": "Die Beziehungen stammen aus einer statischen Analyse und können unvollständig oder unsicher sein.",
  "knowledge.changed": "Die Datei wurde während der Abfrage geändert. Synchronisieren Sie erneut, bevor Sie Zeilennummern verwenden oder die Prüfung bestätigen.",
  "knowledge.truncated": "Diese Ansicht ist begrenzt. Einige Beziehungen oder Quellcodezeilen werden nicht angezeigt.",
  "knowledge.linkMemory": "Wissenseintrag verknüpfen",
  "knowledge.chooseMemory": "Wissenseintrag auswählen",
  "knowledge.noLinks": "Noch keine Codeverknüpfungen. Sie können einen Wissenseintrag in der Symbolansicht verknüpfen.",
  "knowledge.inspect": "Code und Eintrag prüfen",
  "knowledge.unlink": "Verknüpfung entfernen",
  "knowledge.codeReferences": "Codeverweise",
  "knowledge.refresh": "Aktualisieren",
  "knowledge.current": "Unverändert",
  "knowledge.review": "Prüfung erforderlich",
  "knowledge.unavailable": "Nicht verfügbar",
  "knowledge.reviewHelp": "Vergleichen Sie diesen Eintrag mit dem angezeigten Code. Die Bestätigung erfasst die aktuelle Dateiversion, ohne den Text des Eintrags zu ändern.",
  "knowledge.confirmReview": "Prüfung bestätigen",
  "knowledge.agentHint": "Agenten können in diesem Verzeichnis vkb search \"Thema\" verwenden. Abfragen synchronisieren aktivierte Indizes und liefern Code und Wissenseinträge getrennt zurück.",
  "knowledge.busy": "Eine Indizierungsaufgabe läuft. Sie können diese Seite schließen oder die Indizierung deaktivieren, um die Aufgabe zu stoppen.",
  "knowledge.disabledHelp": "Aktivieren Sie die Indizierung dieses Verzeichnisses, um Code abzufragen. Beim Deaktivieren bleiben Index und Verknüpfungen zu Wissenseinträgen erhalten.",
  "knowledge.conflict": "Code oder Eintrag wurden geändert. Laden Sie beide neu, bevor Sie diese Verknüpfung speichern.",
  "knowledge.symbolMissing": "Symbol oder Quellcode sind nicht mehr verfügbar. Synchronisieren Sie und suchen Sie erneut.",
  "knowledge.directoryMissing": "Dieses Arbeitsverzeichnis fehlt oder wurde geändert. Prüfen Sie die Projekt- und Sitzungspfade.",
  "knowledge.partial": "Der Index ist unvollständig. Synchronisieren Sie erneut und prüfen Sie, ob die Quelldateien lesbar sind.",
  "knowledge.interrupted": "Die vorherige Aufgabe wurde unterbrochen. Synchronisieren Sie, um sie erneut zu versuchen.",
  "knowledge.checksum": "Die Prüfsumme des Downloads stimmt nicht überein. Die Laufzeit wurde nicht installiert.",
  "knowledge.downloadFailed": "CodeGraph konnte nicht heruntergeladen werden. Prüfen Sie die Verbindung des Backends zu GitHub und versuchen Sie es erneut.",
  "knowledge.timeout": "Die Indizierung hat das Zeitlimit überschritten. Prüfen Sie die Repository-Größe und versuchen Sie es erneut.",
  "knowledge.error": "Der Vorgang ist fehlgeschlagen. Prüfen Sie die Verzeichnisberechtigungen und die Laufzeit auf dem Backend und versuchen Sie es erneut.",

  // Knowledge base: saved knowledge organized by project and session.
  "memory.hierarchy": "Projekte und Sitzungen",
  "memory.up": "Eine Ebene nach oben",
  "memory.manualGroup": "Manuell erstellt",
  "memory.legacyGroup": "Früher zusammengeführte Einträge",
  "memory.unknownProject": "Quellprojekt unbekannt",
  "nb.addLink": "Link einfügen",
  "nb.attach": "Datei anhängen",
  "nb.browse": "Durchsuchen",
  "nb.chooseNote": "Mit einer Notiz beginnen",
  "nb.closeHint": "Dieses Notizbuch aus der Liste entfernen. Die Dateien bleiben auf dem Datenträger erhalten.",
  "nb.closeVault": "Notizbuch schließen",
  "nb.conflict": "Die Datei wurde außerhalb dieses Editors geändert. Ihr Entwurf bleibt erhalten. Laden Sie die Datei neu oder speichern Sie den Entwurf als neue Notiz.",
  "nb.copyTo": "In ein lokales Notizbuch kopieren",
  "nb.createVault": "Wissensbasis erstellen",
  "nb.destination": "Zielpfad",
  "nb.download": "Herunterladen",
  "nb.downloadHint": "Laden Sie diesen Anhang herunter, um ihn in einer anderen Anwendung zu öffnen.",
  "nb.empty": "Öffnen Sie einen Ordner oder erstellen Sie ein Notizbuch, um mit dem Schreiben zu beginnen.",
  "nb.emptyImport": "Es wurden keine importierbaren Dateien ausgewählt.",
  "nb.emptyNotes": "Notizen werden als Markdown-Dateien gespeichert.",
  "nb.emptyOutline": "Die Überschriften des Dokuments erscheinen hier.",
  "nb.emptyTrash": "Der Papierkorb ist leer.",
  "nb.error": "Das Notizbuch ist nicht erreichbar. Prüfen Sie die Verbindung und den Ordner und versuchen Sie es erneut.",
  "nb.exists": "Das Ziel ist bereits vorhanden. Wählen Sie einen anderen Namen oder Ordner.",
  "nb.favorites": "Favoriten",
  "nb.files": "Dateien",
  "nb.folder": "Ordner",
  "nb.generatedHint": "Aus Ihren Sitzungen aufbereitetes Wissen mit Quellen und Versionsverlauf.",
  "nb.homeHint": "Durchsuchen Sie die Sitzungswissensdatenbank und lokale Wissensbasen.",
  "nb.homeSearch": "Sitzungswissen und lokale Notizen durchsuchen…",
  "nb.loadMore": "Mehr laden",
  "nb.import": "Importieren",
  "nb.importFiles": "Dateien auswählen",
  "nb.importFolder": "Ordner auswählen",
  "nb.importHint": "Dateien werden in den gewählten Ordner kopiert. Vorhandene Dateien werden nicht überschrieben; versteckte Konfigurationsordner werden übersprungen.",
  "nb.imported": "Importiert",
  "nb.imports": "Importverlauf",
  "nb.importsEmpty": "Noch keine Importe.",
  "nb.importRoot": "Stammverzeichnis der Wissensbasis",
  "nb.importBusy": "Für diese Wissensbasis läuft bereits ein Import.",
  "nb.importDelete": "Eintrag löschen",
  "nb.importDeleteConfirm": "Diesen Importeintrag löschen? Bereits importierte Dateien bleiben erhalten.",
  "nb.importDone": "Import abgeschlossen",
  "nb.importDuration": (seconds: string) => `${seconds} s`,
  "nb.importFailed": "Import fehlgeschlagen",
  "nb.importFilePending": "Nicht importiert",
  "nb.importHideFiles": "Dateien ausblenden",
  "nb.importInterruptedHint": "Der Import wurde vor dem Abschluss beendet.",
  "nb.importProgress": (done: string, total: string) => `${done} / ${total} Dateien`,
  "nb.importShowFiles": (count: string) => `Dateien (${count})`,
  "nb.importSkipHidden": "Versteckte Datei oder Ordner",
  "nb.importStatusCancelled": "Abgebrochen",
  "nb.importStatusCompleted": "Abgeschlossen",
  "nb.importStatusFailed": "Fehlgeschlagen",
  "nb.importStatusInterrupted": "Unterbrochen",
  "nb.importStatusRunning": "Wird importiert",
  "nb.incomplete": "Der Vorgang konnte nicht abgeschlossen werden. Prüfen Sie die Dateien und versuchen Sie es erneut.",
  "nb.info": "Notizdetails",
  "nb.invalid": "Der Name oder Pfad ist ungültig.",
  "nb.links": "Ausgehende Links",
  "nb.local": "Lokale Dateien",
  "nb.localVaults": "Lokale Wissensbasen",
  "nb.move": "Umbenennen oder verschieben",
  "nb.moveHint": "Geben Sie einen Pfad relativ zum Stammordner des Notizbuchs ein. Beim Verschieben werden vorhandene Notizlinks aktualisiert.",
  "nb.name": "Name",
  "nb.newFolder": "Neuer Ordner",
  "nb.newNote": "Neue Notiz",
  "nb.noLinks": "Noch keine verknüpften Notizen.",
  "nb.tags": "Tags",
  "nb.notes": "Notizen",
  "nb.openVault": "Wissensbasis öffnen",
  "nb.outline": "Gliederung",
  "nb.quickOpen": "Schnell öffnen",
  "nb.readOnly": "Diese Datei kann nicht als UTF-8-Markdown-Notiz bearbeitet werden.",
  "nb.recent": "Letzte Notizen",
  "nb.restore": "Wiederherstellen",
  "nb.reload": "Vom Datenträger neu laden",
  "nb.root": "Ordnerpfad",
  "nb.rootHint": "Wählen Sie einen Ordner auf dem verbundenen Computer. Vorhandene Markdown-Dateien und Anhänge bleiben an ihrem bisherigen Ort.",
  "nb.saveCopy": "Als neue Notiz speichern",
  "nb.saved": "Auf Datenträger gespeichert",
  "nb.saving": "Wird gespeichert…",
  "nb.search": "Notizen durchsuchen…",
  "nb.searchAllVaults": "Alle Wissensbasen",
  "nb.searchCount": (count: string) => `${count} Ergebnisse`,
  "nb.searchEmpty": "Keine Notiz passt zu dieser Suche.",
  "nb.searchEmptyAll": "Nichts passt zu dieser Suche.",
  "nb.searchFuzzy": "Keine genauen Treffer. Es werden ungefähre Ergebnisse angezeigt.",
  "nb.searchLine": (line: string) => `Zeile ${line}`,
  "nb.searchMatches": (count: string) => `${count} Treffer`,
  "nb.searchMore": "Es werden nur die ersten Ergebnisse aufgelistet. Grenzen Sie die Suche ein, um die übrigen zu sehen.",
  "nb.searchRelated": "Verknüpfte Notizen",
  "nb.searchResults": "Suchergebnisse",
  "nb.searchScope": "Suchbereich",
  "nb.searchThisVault": "Diese Wissensbasis",
  "nb.skipped": "Übersprungen",
  "nb.split": "Geteilte Ansicht",
  "nb.tooLarge": "Die Datei oder Auswahl überschreitet die Grenzen des Notizbuchs.",
  "nb.trash": "Papierkorb",
  "nb.trashHint": "Dieses Element in den Papierkorb des Notizbuchs verschieben. Es kann später wiederhergestellt werden.",
  "nb.unsaved": "Ungespeicherte Änderungen",
  "nb.vaults": "Wissensbasen",
  "nb.view": "Ansichtsmodus",
  "nb.welcome": "Ihre Notizbücher",
  "nb.welcomeText": "Schreiben Sie frei, verknüpfen Sie Gedanken und bewahren Sie Ihre Notizen in gewöhnlichen lokalen Dateien auf. Öffnen Sie einen vorhandenen Markdown-Ordner oder importieren Sie Dokumente in ein neues Notizbuch.",
  "memory.globalMemory": "Sitzungswissensdatenbank",
  "memory.collections": "Archivierte Sitzungen",
  "memory.collectionConversation": "Konversation",
  "memory.collectionEmptyEntries": "Für diese Konversation gibt es noch keine Wissenseinträge.",
  "memory.title": "Wissensdatenbank",
  "memory.add": "Für die Sitzungswissensdatenbank aufbereiten",
  "memory.intro": "Wissen nach Projekt und Sitzung organisieren. Gespeicherte Einträge bleiben unabhängig von ihren Quellen und können manuell bearbeitet werden.",
  "memory.entries": "Wissenseinträge",
  "memory.emptyJobs": "Noch keine Verarbeitungen.",
  "memory.jobs": "Verarbeitungsverlauf",
  "memory.search": "Titel und Inhalte durchsuchen…",
  "memory.empty": "Keine passenden Einträge. Erstellen Sie Wissenseinträge aus einer Sitzung oder legen Sie einen Eintrag manuell an.",
  "memory.emptyDetail": "Wählen Sie einen Eintrag, um Inhalte, Verknüpfungen und Quellen zu lesen.",
  "memory.new": "Neuer Eintrag",
  "memory.titleField": "Titel",
  "memory.summary": "Zusammenfassung",
  "memory.content": "Inhalt (Markdown)",
  "memory.tags": "Tags (durch Kommas getrennt)",
  "memory.related": "Verwandte Einträge",
  "memory.backlinks": "Verweise auf diesen Eintrag",
  "memory.sources": "Quellen",
  "memory.history": "Versionsverlauf",
  "memory.restore": "Diese Version wiederherstellen",
  "memory.restoreConfirm": "Diese Version als neue Fassung wiederherstellen? Die aktuelle Fassung bleibt im Verlauf erhalten.",
  "memory.deleteConfirm": "Diesen Eintrag samt Versionsverlauf löschen? Die Quellsitzungen bleiben erhalten.",
  "memory.groupDeleteConfirm": (count: string) => `Alle ${count} Wissenseinträge dieser Gruppe löschen? Das Projekt bzw. die Sitzung selbst bleibt erhalten.`,
  "memory.export": "Markdown exportieren",
  "memory.selectAgent": "Agent",
  "memory.model": "Modell (optional)",
  "memory.modelHint": "Leer lassen, um das im Agenten konfigurierte Modell zu verwenden.",
  "memory.compile": "Aufbereiten und speichern",
  "memory.compileHelp": "Der ausgewählte Agent bereitet diese Sitzung auf. Eine erneute Erstellung ersetzt die zuvor erzeugten Einträge dieser Sitzung, einschließlich manueller Änderungen. Der Sitzungstext wird über den konfigurierten Agenten an das Modell gesendet.",
  "memory.unavailable": "Nicht installiert oder konfiguriert",
  "memory.allTags": "Alle Tags",
  "memory.updated": "Zuletzt aktualisiert",
  "memory.titleSort": "Titel",
  "memory.sourceNote": "Dieser Stand bewahrt den für die Aufbereitung verwendeten Unterhaltungstext, auch wenn die ursprüngliche Sitzung gelöscht wird.",
  "memory.noKnowledge": "Es wurden keine wiederverwendbaren Erkenntnisse extrahiert. Die zuvor erzeugten Einträge dieser Sitzung wurden entfernt.",
  "memory.queued": "Wartet auf den Start",
  "memory.cancelling": "Wird abgebrochen",
  "memory.schedulingHint": "Verschiedene Sitzungen können parallel verarbeitet werden. Ein neuer Auftrag bricht einen noch nicht abgeschlossenen Auftrag für diese Sitzung ab und ersetzt ihn.",
  "memory.waitingHint": "Dieser Auftrag startet automatisch, sobald der vorherige Auftrag für diese Sitzung beendet wurde.",
  "memory.running": "In Bearbeitung",
  "memory.completed": "Abgeschlossen",
  "memory.failed": "Fehlgeschlagen",
  "memory.cancelled": "Abgebrochen",
  "memory.extract": "Themen werden extrahiert",
  "memory.merge": "Wissen wird zusammengeführt",
  "memory.commit": "Einträge werden gespeichert",
  "memory.done": "Gespeichert",
  "memory.closeHint": "Sie können dieses Fenster während der Verarbeitung schließen und den Fortschritt im Verarbeitungsverlauf verfolgen.",
  "memory.conflict": "Dieser Eintrag wurde während des Vorgangs geändert. Laden Sie ihn neu und versuchen Sie es erneut. Ihre Änderungen wurden nicht gespeichert.",
  "memory.duplicate": "Ein Eintrag mit diesem Titel existiert bereits. Öffnen Sie ihn, um die Inhalte zusammenzuführen.",
  "memory.notFound": "Dieser Eintrag, diese Quelle oder dieser Auftrag existiert nicht mehr.",
  "memory.noTranscript": "Für diese Sitzung ist keine lesbare Unterhaltung verfügbar.",
  "memory.agentUnavailable": "Der gewählte Agent ist nicht verfügbar. Prüfen Sie seinen Programmpfad in den Einstellungen.",
  "memory.invalid": "Einige Felder oder Verknüpfungen sind ungültig. Prüfen Sie Titel, Inhalt und verwandte Einträge.",
  "memory.processFailed": "Der Agent konnte den Vorgang nicht abschließen. Prüfen Sie Anmeldung, Modell und CLI-Einstellungen und versuchen Sie es erneut.",
  "memory.timeout": "Zeitüberschreitung beim Agenten. Versuchen Sie es mit einem verfügbaren Modell oder einer kürzeren Unterhaltung erneut.",
  "memory.interrupted": "Die Verarbeitung wurde unterbrochen. Sie können sie mit dem gespeicherten Quellstand erneut starten.",
  "memory.tooLarge": "Quelle, Kontext oder Ausgabe überschreitet die unterstützte Größe. Nichts wurde gekürzt oder gespeichert.",
  "memory.invalidOutput": "Der Agent hat ungültige strukturierte Daten zurückgegeben. Nichts wurde gespeichert. Versuchen Sie es erneut oder wählen Sie einen anderen Agenten.",
  "memory.loadError": "Die Wissensdatenbank konnte nicht geladen werden. Prüfen Sie die Verbindung und versuchen Sie es erneut.",
  "memory.unsaved": "Nicht gespeicherte Änderungen verwerfen?",
  "memory.source": "Gespeicherter Quellstand",

  // ── Common ──
  "common.cancel": "Abbrechen", // Cancel
  "common.confirm": "OK", // OK
  "common.delete": "Löschen", // Delete
  "common.save": "Speichern", // Save
  "common.create": "Erstellen", // Create
  "common.close": "Schließen", // Close
  "chat.imageViewOriginal": "Originalbild anzeigen",
  "chat.imageCopy": "Bild kopieren",
  "chat.imageSave": "Bild speichern",
  "chat.imageActionFailed": "Die Bildaktion konnte nicht ausgeführt werden. Bitte versuchen Sie es erneut.",
  "common.copy": "Kopieren", // Copy
  "common.cut": "Ausschneiden", // Cut
  "common.paste": "Einfügen", // Paste
  "common.selectAll": "Alles auswählen", // Select All
  "common.copied": "Kopiert", // Copied
  "common.copyFailed": "Kopieren fehlgeschlagen. Bitte versuchen Sie es erneut.",
  "chat.sync.loading": "Unterhaltung wird synchronisiert…",
  "chat.sync.failed": "Synchronisierung fehlgeschlagen. Bereits geladene Nachrichten bleiben verfügbar.",
  "chat.sync.history": "Ältere Nachrichten laden",
  "chat.submission.updateRequired": "Aktualisieren Sie den Server, bevor Sie mit diesem Client Nachrichten senden.",
  "chat.submission.sending": "Wird gesendet…",
  "chat.submission.sent": "Gesendet",
  "chat.submission.queued": "In der Warteschlange",
  "chat.submission.failed": "Senden fehlgeschlagen",
  "chat.submission.unknown": "Zustellung nicht bestätigt",
  "chat.submission.check": "Status prüfen",
  "common.retry": "Erneut versuchen", // Retry
  "common.experimental": "Experimentell",
  "common.refresh": "Aktualisieren", // Refresh
  "common.loading": "Lädt…", // Loading…
  "common.prev": "Zurück", // Previous
  "common.next": "Weiter", // Next
  "common.on": "An", // On
  "common.off": "Aus", // Off
  "common.gotIt": "Verstanden", // Got it
  "common.rename": "Umbenennen", // Rename
  "common.edit": "Bearbeiten", // Edit
  "common.open": "Öffnen", // Open
  "common.session": "Sitzung", // Session

  // ── Session types and status ──
  "kind.terminal": "Terminal", // Terminal
  "kind.browser": "Browser", // Browser
  "status.idle": "Inaktiv", // Idle
  "status.running": "Läuft", // Running
  "status.exited": "Beendet", // Exited
  "status.error": "Fehler", // Error
  "status.working": "Arbeitet", // Working
  "status.asking": "Bestätigung nötig", // Needs confirmation
  "status.waiting": "Gesehen", // Viewed
  "status.unavailable": "Status nicht verfügbar",
  "indicator.unread": "Ungelesen · ausstehend", // Unread · awaiting review

  // ── Title bar ──
  "titlebar.builtAt": (time) => `Build vom ${time}`, // Built at {time}
  "titlebar.versionMismatch": (frontend, backend) =>
    `Versionskonflikt: Frontend v${frontend} ≠ Backend v${backend} – neu bauen oder synchron bereitstellen.`, // Version mismatch

  "titlebar.hotReloadedAt": (time) => `Hot-Reload um ${time}`, // Hot reloaded at {time}
  "titlebar.themeSystem": (resolved) => `System folgen (aktuell ${resolved})`, // Follow system (currently {resolved})
  "titlebar.themeDark": "Dunkel", // Dark
  "titlebar.themeLight": "Hell", // Light
  "titlebar.gameCenter": "Spielecenter",
  "titlebar.browser": "Integrierter Browser", // Built-in Browser
  "titlebar.remoteAccess": "Fernzugriff (Browser)", // Remote Access (Browser)
  "titlebar.connectRemote": "Mit Remote-Server verbinden", // Connect to Remote Server
  "titlebar.mirrored": "Gespiegelt", // Mirrored
  "titlebar.mirroredHint":
    "Spiegelung ist aktiv: Tabs, Splits und die aktive Sitzung folgen dem Host. Der Schalter liegt auf dem Host.", // Mirroring is on: tabs, splits, and the active session follow the host. The switch is on the host.
  "titlebar.mirroredBy": (n: number) => `Gespiegelt von ${n}`, // Mirrored by {n}
  "titlebar.mirroredByHint": (n: number) =>
    `${n} Remote-Client${n === 1 ? " ist" : "s sind"} verbunden. Tabs, Splits und die aktive Sitzung sind gemeinsam, und beide Seiten können sie umstellen.`, // {n} remote clients are connected. Tabs, splits, and the active session are shared, and either side can rearrange them.
  "titlebar.clientsTitle": "Verbundene Clients", // Attached clients
  "titlebar.clientUnnamed": "Unbenannter Client", // Unnamed client
  "titlebar.clientSince": (time: string) => `seit ${time}`, // since {time}
  "titlebar.feedback": "Feedback", // Feedback
  "titlebar.share": "Teilen", // Share
  // ── Alt-triggered menu bar (Windows/Linux) ──
  "menubar.file": "Datei", // File
  "menubar.terminal": "Terminal",
  "menubar.help": "Hilfe", // Help
  "menubar.newTerminal": "Neues Terminal", // New Terminal
  "menubar.visitWebsite": "Website besuchen", // Visit Website
  "menubar.sendFeedback": "Feedback senden", // Send Feedback
  "menubar.clearBadges": "Benachrichtigungsmarkierungen löschen", // Clear Notification Badges
  "share.title": "VelaTerm teilen", // Share VelaTerm
  "share.subtitle":
    "Hinter VelaTerm steht ein kleines Team. Wenn es dir gefällt, teile VelaTerm bitte mit anderen. Dass dadurch mehr Menschen von uns erfahren, bedeutet unserem Team sehr viel. Danke für deine Unterstützung! ❤️", // We're a small team behind VelaTerm. If you enjoy it, please share VelaTerm with others…
  "share.copyLink": "Link kopieren", // Copy link
  "share.copied": "Kopiert!", // Copied!
  "share.wechatMoments": "WeChat Moments",
  "share.weibo": "Weibo",
  "share.xiaohongshu": "Xiaohongshu",
  "share.xiaohongshuAction":
    "Beitragstext und Link kopieren und das Xiaohongshu Creator Center öffnen",
  "share.wechatQrTitle": "In WeChat Moments teilen",
  "share.wechatQrHint":
    "Scanne den Code mit WeChat, öffne den Link und wähle dann „In Moments teilen“.",
  "share.backToPlatforms": "Zurück zu den Teilen-Optionen",
  "titlebar.appearance": "Darstellung", // Appearance
  "titlebar.showLeft": "Seitenleiste einblenden", // Show sidebar
  "titlebar.hideLeft": "Seitenleiste ausblenden", // Hide sidebar
  "titlebar.showRight": "Infopanel einblenden", // Show info panel
  "titlebar.hideRight": "Infopanel ausblenden", // Hide info panel

  // ── Settings ──
  "settings.title": "Einstellungen", // Settings
  "settings.catTerminal": "Terminal", // Terminal
  "settings.catBehavior": "Verhalten", // Behavior
  "settings.catAgents": "Agenten", // Agents
  "settings.agentDefaultsTitle": "Standardeinstellungen für neue Sitzungen",
  "settings.referSummaryTitle": "Kontext für Sitzungsverweise",
  "settings.referSummaryMode": "Kontextmodus",
  "settings.referSummaryFull": "Vollständiges Protokoll",
  "settings.referSummaryFirst": "Zuerst zusammenfassen",
  "settings.referSummaryAgent": "Agent für Zusammenfassungen",
  "settings.referSummaryHint":
    "Standardmäßig übergibt vrefer --ask das vollständige Protokoll an den antwortenden Agenten. „Zuerst zusammenfassen“ komprimiert es mit dem hier gewählten Agenten, Modell und Denkaufwand; die endgültige Antwort erhält zusätzlich relevante Auszüge aus dem Originaltext.",
  "settings.permDefault": "Standard", // Default
  "settings.permYolo": "YOLO", // YOLO
  "settings.yoloHint": (flag: string) =>
    `Startet mit ${flag}. Überspringt alle Berechtigungsabfragen — mit Vorsicht verwenden.`,
  "settings.permViaEnvHint":
    "Überspringt alle Berechtigungsabfragen via Konfig-Injektion (kein CLI-Flag). Gilt für diese Sitzung beim Start.", // YOLO flag hint
  "settings.catGeneral": "Allgemein", // General
  "settings.cliLabel": "Shell-Befehl",
  "settings.cliInstall": "‘vela’-Befehl installieren",
  "settings.cliUninstall": "‘vela’-Befehl deinstallieren",
  "settings.cliInstalledAt": (path: string) => `Installiert unter ${path}`,
  "settings.cliConflict": (path: string) =>
    `Unter ${path} existiert bereits ein anderer ‘vela’-Befehl. VelaTerm überschreibt ihn nicht.`,
  "settings.cliHint":
    "Fügt `vela <Projektpfad>` wie den VS-Code-Befehl `code` zum PATH hinzu.",
  "settings.agentArgsHint":
    "Standard-Startargumente für neue Sitzungen jedes Agententyps. Beim Erstellen oder Bearbeiten pro Sitzung gesetzte Argumente haben Vorrang. Leer lassen für keine.", // Agent default launch args hint
  "settings.agentPathLabel": "Pfad zur ausführbaren Datei (optional)", // Executable path (optional)
  "settings.agentPathPlaceholder":
    "z. B. ~/.local/bin/claude — leer = über PATH suchen", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "Wenn gesetzt, starten Sitzungen dieses Typs über diesen vollständigen Pfad, statt den Befehl im PATH zu suchen. Nützlich, wenn der Agent installiert ist, aber nicht im PATH der Shell liegt. Wird nach erfolgreicher Ein-Klick-Installation automatisch ausgefüllt, wenn der Ort erkannt wird.", // Agent executable path hint
  "settings.agentDefaultView": "Standardansicht", // Default view
  "settings.agentDefaultViewHint":
    "Die Ansicht, in der neue Sitzungen dieses Agenten geöffnet werden. Bestehende Sitzungen behalten die Ansicht, mit der sie erstellt wurden.", // Agent default view hint
  "settings.appearance": "Darstellung", // Appearance
  "settings.accent": "Akzent", // Accent
  "settings.accentAuto": "Thema folgen", // Follow theme
  "settings.density": "Dichte", // Density
  "settings.densityCompact": "Kompakt", // Compact
  "settings.densityRegular": "Normal", // Regular
  "settings.densityComfy": "Locker", // Comfy
  "settings.pane": "Bereiche", // Panes
  "settings.paneFlush": "Bündig", // Flush
  "settings.paneCard": "Karte", // Card
  "settings.divider": "Trennlinie", // Divider
  "settings.dividerSubtle": "Dezent", // Subtle
  "settings.dividerVisible": "Sichtbar", // Visible
  "settings.nav": "Seitenleiste", // Sidebar
  "settings.navTree": "Baum", // Tree
  "settings.navCompact": "Kompakt", // Compact
  "settings.tabs": "Tabs", // Tabs
  "settings.dynamicStatusFilter": "Dynamische Ergänzung des Statusfilters",
  "settings.tabSingle": "Einzeln", // Single
  "settings.tabMulti": "Mehrere", // Multi
  "settings.maxLiveTabs": "Background limit", // Background limit
  "settings.defaultShell": "Standard-Shell", // Default shell
  "settings.spawnConfirm": "Confirm before spawn", // Confirm before spawn
  "settings.usageAuto": "Usage auto-refresh", // Usage auto-refresh
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.autoContinue": "Nach Zurücksetzung fortsetzen", // Continue after limit resets
  "settings.autoContinueHint": "Wenn ein 5-Stunden- oder Wochen-Nutzungslimit Claude oder Codex unterbricht, wird die Aufgabe nach der Zurücksetzung des Limits automatisch fortgesetzt.", // When a 5-hour or weekly usage limit stops Claude or Codex, the task continues automatically after the limit resets.
  "settings.cleanImages": "Eingefügte Bilder automatisch bereinigen",
  "settings.cleanImagesHint":
    "In das Terminal eingefügte oder gezogene Bilder werden zunächst als temporäre Dateien gespeichert (der Pfad wird an den Agenten gesendet). Wenn aktiviert, werden die temporären Dateien dieser Sitzung beim Beenden gelöscht und Reste, die älter als 24 Std. sind, beim Start entfernt. Bilder in Dokumenten bleiben unberührt.",
  "settings.cleanImagesNow": "Jetzt bereinigen",
  "settings.cleanImagesResult": (n: number, size: string) =>
    `${n} temporäre Bilder bereinigt (${size} freigegeben).`,
  "settings.cleanImagesEmpty": "Keine temporären Bilder zu bereinigen.",
  "settings.imagePasteMode": "Bild einfügen",
  "settings.imagePasteUpload": "Dateipfad einfügen",
  "settings.imagePasteAgent": "Nativ einfügen",
  "settings.imagePasteHint":
    "Wählen Sie, was beim Einfügen eines Bildes eingesetzt wird (nur lokaler Desktop). Dateipfad einfügen: Das Bild wird temporär gespeichert und der Pfad in Claude oder Codex eingesetzt. Nativ einfügen: Claude oder Codex liest die System-Zwischenablage und zeigt den eigenen Bildplatzhalter.",
  "settings.imagePasteRemoteHint":
    "In Remotesitzungen wird immer der Dateipfad eingefügt, damit der Agent das Bild auf seinem Rechner lesen kann. Natives Einfügen ist nur lokal verfügbar.",
  "spawn.title": "Untersitzung starten",
  "spawn.fromSession": "Anfordernde Sitzung",
  "spawn.promptLabel": "Aufgabenbeschreibung",
  "spawn.agentLabel": "Sitzungstyp",
  "spawn.worktreeLabel": "Separater Worktree",
  "spawn.modelLabel": "Modell",
  "spawn.effortLabel": "Denkaufwand",
  "spawn.modelDefault": "Agent-Standard",
  "spawn.modelLoading": "Modelle werden geladen…",
  "spawn.modelListUnavailable": "Keine Modellliste verfügbar. Sie können eine Kennung eingeben.",
  "spawn.launch": "Sitzung starten",
  "spawn.remaining": (n: number) => `${n} weitere Anfragen prüfen`,
  "spawn.notifyTitle": "Untersitzung wartet auf Bestätigung",
  "orch.title": "Mehrere Sitzungen starten",
  "orch.notifyTitle": "Sitzungsstart wartet auf Bestätigung",
  "orch.coordinatorName": "Sitzungsstatus",
  "orch.sharedSettings": "Gemeinsame Einstellungen",
  "orch.agentLabel": "Agent",
  "orch.modelLabel": "Modell",
  "orch.effortLabel": "Denkaufwand",
  "orch.nameLabel": "Sitzungsname",
  "orch.promptLabel": "Aufgabenbeschreibung",
  "orch.worktreeLabel": "Git-Worktree",
  "orch.worktreeNone": "Aktuelles Verzeichnis",
  "orch.worktreeShared": "Gemeinsamer Worktree",
  "orch.worktreeEach": "Ein Worktree pro Sitzung",
  "orch.follow": "Gemeinsame Einstellungen verwenden",
  "orch.overridden": "Eigene Einstellungen",
  "orch.remove": "Aufgabe entfernen",
  "orch.launch": (n: number) => `${n} Sitzungen starten`,
  "orch.modelPlaceholder": "Agent-Standard",
  "orch.effortPlaceholder": "Agent-Standard",
  "launch.terminalHint": "Ein normales Terminal öffnet das Arbeitsverzeichnis. Die Aufgabenbeschreibung wird nicht automatisch ausgeführt.",
  "launch.optionsError": "Die Startoptionen konnten nicht geladen werden. Versuchen Sie es vor dem Start erneut.",
  "launch.singleIntro": "Prüfen Sie Aufgabe und Einstellungen, bevor Sie eine Untersitzung starten.",
  "launch.taskHint": "Diese Anweisungen werden als erste Nachricht an die Untersitzung gesendet.",
  "launch.runtime": "Ausführungseinstellungen",
  "launch.directory": "Arbeitsverzeichnis",
  "launch.directoryCurrentHint": "Die Sitzungen bearbeiten Dateien im ursprünglichen Verzeichnis.",
  "launch.directorySharedHint": "Alle Sitzungen nutzen dasselbe neue Verzeichnis und denselben Branch.",
  "launch.directoryEachHint": "Jede Sitzung erhält ein eigenes Verzeichnis und einen eigenen Branch.",
  "launch.worktreeHint": "Worktrees basieren auf dem aktuellen Commit, ohne uncommittete Änderungen. Schlägt die Erstellung fehl, wird das ursprüngliche Verzeichnis verwendet.",
  "launch.singleResult": "Die Untersitzung erscheint unter der übergeordneten Sitzung in der Seitenleiste.",
  "launch.startError": "Der Start ist fehlgeschlagen. Prüfen Sie die Einstellungen und versuchen Sie es erneut.",
  "launch.starting": "Wird gestartet…",
  "launch.batchIntro": "Prüfen Sie die gemeinsamen Einstellungen und wählen Sie dann die Aufgaben zum Bearbeiten aus.",
  "launch.sessionCount": (n: number) => `Sitzungen: ${n}`,
  "launch.batchName": "Name der Aufgabengruppe",
  "launch.sharedHint": "Gilt für Sitzungen ohne eigene Einstellungen.",
  "launch.tasks": "Aufgaben",
  "launch.incomplete": "Angaben fehlen",
  "launch.undoRemove": "Entfernen rückgängig machen",
  "launch.taskNumber": (n: number) => `Aufgabe ${n}`,
  "launch.taskSettings": "Einstellungen dieser Sitzung",
  "launch.taskAgent": "Agent dieser Sitzung",
  "launch.sharedDirectoryLocked": "Alle Sitzungen dieser Gruppe verwenden einen gemeinsamen Worktree.",
  "launch.resetSettings": "Gemeinsame Einstellungen wiederherstellen",
  "launch.monitorHint": "Ein Terminal namens „Sitzungsstatus“ zeigt, welche Sitzungen arbeiten oder auf Eingaben warten. Es zeigt keinen prozentualen Aufgabenfortschritt.",
  "launch.taskIncomplete": (n: number) => `Ergänzen Sie den Namen und die Beschreibung von Aufgabe ${n}.`,
  "launch.batchResult": "Jede Aufgabe startet in einer eigenen interaktiven Sitzung.",
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "Änderungen anzeigen…",
  "changes.title": "Änderungen",
  "changes.loading": "Wird geladen…",
  "changes.loadingDiff": "Diff wird geladen…",
  "changes.noChanges": "Keine Änderungen",
  "changes.refresh": "Aktualisieren",
  "changes.notRepo": "Kein Git-Repository",
  "changes.selectFile": "Datei zum Anzeigen auswählen",
  "changes.binary": "Binärdatei – Zeilen-Diff nicht verfügbar",
  "changes.commitTitle": (hash: string) => `Commit ${hash}`,

  "git.staged": "Bereitgestellt",
  "git.changes": "Änderungen",
  "git.untracked": "Nicht verfolgte Dateien",
  "git.committed": "Committete Änderungen",
  "git.stage": "Bereitstellen",
  "git.unstage": "Bereitstellung aufheben",
  "git.stageAll": "Alle bereitstellen",
  "git.unstageAll": "Alle zurücknehmen",
  "git.discard": "Verwerfen",
  "git.deleteFile": "Löschen",
  "git.viewAll": "Alle anzeigen",
  "git.detached": "(losgelöst)",
  "git.aheadBehind": "Commits vor und hinter dem Upstream-Branch",
  "git.commitPlaceholder": "Commit-Nachricht",
  "git.amend": "Letzten Commit ändern",
  "git.amendCommit": "Commit ändern",
  "git.commitCount": (n: number) =>
    n === 1 ? "1 Datei committen" : `${n} Dateien committen`,
  "git.commitNoFiles": "Keine Dateiänderungen in diesem Commit",
  "git.noCommits": "Noch keine Commits",
  "git.loadMore": "Mehr laden",
  "tree.merge": "Merge…", // TODO translate
  "tree.copyWorktreePath": "Copy worktree path",
  "tree.openWorktreeDir": "Open worktree folder",
  "tree.deleteWorktreeMenu": "Delete worktree…", // TODO translate
  "tree.deleteWorktreeTitle": "Delete worktree", // TODO translate
  "tree.deleteWorktreeBody":
    "Choose a worktree to remove. This deletes its working directory from disk.", // TODO translate
  "tree.deleteWorktreePlaceholder": "Select a worktree…", // TODO translate
  "tree.deleteWorktreeForce": "Force delete (discard uncommitted changes)", // TODO translate
  "tree.convertToNormalSession": "Convert to normal session", // TODO translate
  "tree.moveGroupToWorktree": "Zu Worktree verschieben…",
  "tree.convertToNormalGroup": "Convert to normal group", // TODO translate
  "merge.title": "Merge branches", // TODO translate
  "merge.desc":
    "Pick a source and a target branch; the source merges into the target.", // TODO translate
  "merge.notRepo": "This session's directory is not a git repository.", // TODO translate
  "merge.loadingBranches": "Loading branches…", // TODO translate
  "merge.loadingDiff": "Loading diff…", // TODO translate
  "merge.sourceLabel": "Source branch", // TODO translate
  "merge.targetLabel": "Target branch", // TODO translate
  "merge.selectBranch": "Select a branch…", // TODO translate
  "merge.swap": "Swap direction", // TODO translate
  "merge.pickHint":
    "Pick both branches to preview the changes this merge brings in.", // TODO translate
  "merge.changes": (target: string) => `Changes brought into "${target}"`, // TODO translate
  "merge.noChanges": "No file changes.", // TODO translate
  "merge.sameBranch": "Source and target are the same branch.", // TODO translate
  "merge.branchGone": "A selected branch no longer exists. Pick again.", // TODO translate
  "merge.upToDate":
    "The target branch already contains the source branch. Nothing to merge.", // TODO translate
  "merge.targetNotCheckedOut": (target: string) =>
    `Target branch "${target}" isn't checked out in any worktree, so a local merge can't run. Check it out first.`, // TODO translate
  "merge.targetDirty":
    "The target branch's working tree has uncommitted changes; the merge may be blocked.", // TODO translate
  "merge.sourceDirtyNote":
    "The source branch's working tree has uncommitted changes; they will be committed first.", // TODO translate
  "merge.commitMsgLabel": "Commit message", // TODO translate
  "merge.commitMsgPlaceholder":
    "Describe this change (used as the commit message)", // TODO translate
  "merge.apply": "Merge", // TODO translate
  "merge.commitAndApply": "Commit & merge", // TODO translate
  "merge.working": "Merging…", // TODO translate
  "merge.doneMsg": (source: string, target: string) =>
    `Merged "${source}" into "${target}".`, // TODO translate
  "merge.conflictMsg": (target: string) =>
    `Merge has conflicts. Resolve them in the terminal of "${target}"'s worktree, then commit:`, // TODO translate
  "merge.close": "Close", // TODO translate
  "gitea.title": "Gitea integration", // TODO translate
  "gitea.desc":
    "Configure a Gitea server to land worktrees by opening a pull request. The token is stored in your system keychain.", // TODO translate
  "gitea.baseUrl": "Base URL", // TODO translate
  "gitea.token": "Access token", // TODO translate
  "gitea.tokenSet": "Saved (leave blank to keep)", // TODO translate
  "gitea.tokenPlaceholder": "Personal access token", // TODO translate
  "gitea.test": "Test connection", // TODO translate
  "gitea.saved": "Saved.", // TODO translate
  "settings.renderer": "Terminal-Renderer", // Terminal renderer
  "settings.redrawOnReveal": "Beim Tabwechsel neu zeichnen", // Redraw on tab switch
  "settings.catAdvanced": "Erweitert", // Advanced
  "settings.outputScheduler": "Vordergrund-Ausgabe priorisieren", // Foreground-priority output
  "settings.recordSessions": "Sitzungsprotokolle aufzeichnen", // Record session logs
  "settings.recordSessionsHint":
    "Standardmäßig aus. Wenn aktiviert, wird die Terminalausgabe in einer Protokolldatei für Archiv-Wiedergabe und Suche gespeichert. Einfache Terminalsitzungen werden nie aufgezeichnet; Agent-Sitzungen lesen stattdessen ihr eigenes Transkript.", // Record session logs hint
  "settings.fonts": "Fonts", // TODO translate
  "settings.uiFont": "Interface font", // TODO translate
  "settings.uiFontSize": "Interface size", // TODO translate
  "settings.termFont": "Terminal font", // TODO translate
  "settings.termFontSize": "Terminal size", // TODO translate
  "settings.termLineHeight": "Zeilenhöhe im Terminal",
  "settings.chatTypography": "Unterhaltungsansicht",
  "settings.chatTypographyHint": "Diese Schrifteinstellungen sind unabhängig vom Terminal und werden sofort wirksam.",
  "settings.chatFont": "Schriftart der Unterhaltung",
  "settings.chatFontSize": "Schriftgröße der Unterhaltung",
  "settings.chatLineHeight": "Zeilenhöhe der Unterhaltung",
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
  "settings.fontListUnavailable": "Die Liste der Systemschriftarten ist nicht verfügbar. Sie können einen Schriftnamen manuell eingeben.",
  "settings.fontUnconfirmed": "Die Verfügbarkeit dieser Schriftart konnte nicht bestätigt werden.",
  "settings.fontAuto": "Auto", // TODO translate
  "settings.fontSmaller": "Smaller", // TODO translate
  "settings.fontLarger": "Larger", // TODO translate
  "settings.fontReset": "Reset", // TODO translate
  "settings.sound": "Benachrichtigungston", // Notification sound
  "settings.language": "Sprache", // Language
  "settings.langAuto": "Auto (System)", // Auto (system)
  "settings.skillLabel": "Vela Skills",
  "settings.skillInstall": "Install", // Install
  "settings.skillInstalled": "Reinstall", // Reinstall
  "settings.skillInvokeHint":
    "Claude: /vspawn <task> · Codex: $vspawn <task>. If Codex does not list it after installation, start a new Codex session.",
  // Notification permission guidance
  "settings.notify": "System notifications", // TODO translate
  "settings.notifyGranted": "Enabled", // TODO translate
  "settings.notifyAllow": "Allow notifications", // TODO translate
  "settings.notifyOffHint":
    "Allow VelaTerm to alert you when an agent needs your input or finishes a task.", // TODO translate
  "settings.notifyDeniedHint": "Notifications are blocked. To turn them on:", // TODO translate
  "settings.notifyStepsMac":
    "open System Settings ▸ Notifications ▸ VelaTerm and turn on Allow Notifications (Banners or Alerts recommended).", // TODO translate
  "settings.notifyStepsWin":
    "open Settings ▸ System ▸ Notifications, enable VelaTerm, and make sure Focus assist / Do not disturb isn't blocking it.", // TODO translate
  "settings.notifyStepsLinux":
    "open your desktop's Settings ▸ Notifications and allow VelaTerm.", // TODO translate
  "settings.notifyStepsBrowser":
    "click the site-permission icon in the address bar and set Notifications to Allow.", // TODO translate
  "settings.notifyUnsupported":
    "Notifications aren't available in this environment.", // TODO translate
  "settings.notifyOpenSettings": "Open System Settings", // TODO translate
  // Shortcut categories
  "settings.catShortcuts": "Tastenkürzel", // Shortcuts
  "settings.scOpenProject": "Projekt öffnen", // Open project
  "settings.scNewTab": "Neues Terminal", // New terminal
  "settings.scNewBrowserTab": "Neuer Browser-Tab", // New browser tab
  "settings.scClosePane": "Bereich / Tab schließen", // Close pane / tab
  "settings.scSplitRight": "Rechts teilen", // Split right
  "settings.scSplitDown": "Unten teilen", // Split down
  "settings.scSearch": "Im Terminal suchen", // Find in terminal
  "settings.scGlobalSearch": "Alle Sitzungen durchsuchen", // Search all sessions
  "settings.scSaveDoc": "Dokument speichern", // Save document
  "settings.scRecording": "Tasten drücken…", // Press keys…
  "settings.scHint":
    "Klicke ein Kürzel an und drücke eine neue Kombination (Cmd/Strg erforderlich).", // hint
  "settings.scReset": "Standard wiederherstellen", // Restore defaults
  "settings.scConflict": (label: string) => `Bereits belegt von „${label}“`, // conflict

  // ── Remote access panel ──
  "remote.title": "Fernzugriff (Browser)", // Remote Access (Browser)
  "remote.desc":
    "Nach dem Aktivieren können Geräte im selben LAN die folgende Adresse im Browser öffnen, das Passwort eingeben und erhalten dieselbe Oberfläche wie der Desktop.", // Once enabled, devices on the same LAN…
  "remote.needPassword": "Bitte zuerst ein Zugangspasswort festlegen", // Please set an access password first
  "remote.running": (port) => `Läuft · Port ${port}`, // Running · port {port}
  "remote.urlsHint":
    "Öffnen Sie die Adresse im selben WLAN / Subnetz wie Ihr Gerät (bei mehreren Netzwerkschnittstellen die passende wählen; VPN-/Tunnel-Adressen stehen am Ende und sind von anderen Geräten meist nicht erreichbar):", // Open the address on the same WiFi / subnet…
  "remote.copyUrl": "Klicken zum Kopieren der Adresse", // Click to copy address
  "remote.moreUrls": (n: number) => `${n} weitere Link${n > 1 ? "s" : ""}`, // N more urls
  "remote.lessUrls": "Einklappen", // Show less
  "remote.stop": "Server stoppen", // Stop Server
  "remote.passwordPlaceholder": "Zugangspasswort festlegen", // Set access password
  "remote.starting": "Startet…", // Starting…
  "remote.start": "Server starten", // Start Server
  "remote.portLabel": "Port", // Port
  "remote.portInvalid": "Port muss zwischen 1 und 65535 liegen", // Port must be between 1 and 65535
  "remote.ipLabel": "IP", // IP address
  "remote.ipAuto": "Automatisch (erste LAN-Adresse)", // Automatic (first LAN address)
  "remote.ipVpn": "VPN", // VPN
  "remote.qrHint":
    "Mit dem Handy scannen, um den Pairing-Link auf der gewählten Adresse zu öffnen.", // Scan with your phone to open the pairing link on the selected address.
  "remote.fingerprintLabel": "Zertifikat-Fingerabdruck (SHA-256)", // Certificate fingerprint (SHA-256)
  "remote.fingerprintHint":
    "Beim ersten Verbinden warnen Browser, dass das Zertifikat nicht vertrauenswürdig ist – bei einem selbstsignierten Zertifikat normal. Vergleichen Sie diesen Fingerabdruck, um sicherzugehen, dass es dieser Rechner ist.", // On first connect, browsers warn the certificate is untrusted…

  "remote.pairingCreate": "Kopplungslink erstellen", // Create pairing link
  "remote.pairingRegenerate": "Link neu erzeugen (trennt alle)", // Regenerate link (disconnects all)
  "remote.pairingCreating": "Wird erstellt…", // Generating…
  "remote.pairingHint":
    "Im Browser öffnen und das Passwort eingeben. Dieser Link enthält Zugangsdaten – nur mit eigenen Geräten teilen.", // Open in a browser, then enter the password…

  "remote.devicesLabel": "Gekoppelte Geräte", // Paired devices
  "remote.lastSeen": "Zuletzt verbunden", // Last seen
  "remote.revoke": "Widerrufen", // Revoke
  "remote.deviceBlock": "Sperren", // Block
  "remote.deviceBlockConfirm": "Sperren bestätigen", // Confirm block
  "remote.deviceBlockHint":
    "Gesperrte Geräte werden getrennt und können sich nicht erneut verbinden (ein neuer Kopplungslink ist nötig). Andere Geräte sind nicht betroffen.", // Block hint
  "remote.devicesEmpty": "Keine gekoppelten Geräte", // No paired devices yet
  "remote.autoRestartHint":
    "Der Fernzugriff startet beim erneuten Öffnen der App automatisch. „Server stoppen“ schaltet das ab.", // Remote access restarts automatically when the app is reopened. Stop Server turns this off.
  "remote.autostartFailed": "Automatischer Start fehlgeschlagen:", // Automatic start failed:
  "remote.mirror": "Layout auf allen Geräten spiegeln", // Mirror layout across devices
  "remote.mirrorHint":
    "Tabs, Splits und die aktive Sitzung bleiben auf allen verbundenen Geräten gleich. Der Tastaturfokus bleibt auf jedem Gerät, wo er ist.", // Tabs, splits, and the active session stay the same on every connected device. Keyboard focus stays put on each one.

  // ── Remote connection panel ──
  "connect.title": "Mit Remote-Server verbinden", // Connect to Remote Server
  "connect.pairingPlaceholder": "Kopplungslink einfügen", // Paste pairing link
  "connect.confirmConnect": "Fingerabdruck korrekt, verbinden", // Fingerprint matches, connect
  "connect.desc":
    "Adresse und Passwort eines entfernten VelaTerm eingeben, um es in einem neuen Fenster zu verbinden und zu steuern.", // Enter the address and password…
  "connect.addressPlaceholder": "IP-Adresse, z. B. 192.168.1.100", // IP address, e.g. 192.168.1.100
  "connect.portPlaceholder": "Port", // Port
  "connect.connecting": "Verbinde…", // Connecting…
  "connect.connect": "Verbinden", // Connect
  "connect.stagePreparing": "Server wird vorbereitet…",
  "connect.stageTransferring": "Server wird übertragen…",
  "connect.stageStarting": "Server wird gestartet…",
  "connect.sshFingerprintLabel": (kt: string) =>
    `SSH-Hostschlüssel-Fingerabdruck (${kt})`,
  "connect.sshHostNew":
    "Erste Verbindung zu diesem Host — überprüfen Sie den Fingerabdruck, bevor Sie fortfahren.",
  "connect.sshHostChanged":
    "⚠ Der Schlüssel dieses Hosts hat sich geändert — möglicherweise eine Neuinstallation des Servers oder ein Man-in-the-Middle-Angriff. Fahren Sie nur fort, wenn Sie sicher sind.",
  "connect.urlCertChanged":
    "⚠ Der Zertifikat-Fingerabdruck dieses Servers hat sich seit Ihrer letzten Bestätigung geändert — möglicherweise eine Neuinstallation des Servers oder ein Man-in-the-Middle-Angriff. Fahren Sie nur fort, wenn Sie sicher sind.",
  "connect.sshPasswordLabel": "SSH-Passwort",
  "connect.sshPasswordPlaceholder": "Kontopasswort",
  "connect.savedHosts": "Letzte Hosts",
  "connect.savedHostsAll": "Alle letzten Hosts",
  "connect.showAllHosts": (n: number) => `Alle anzeigen (${n})`,
  "connect.forgetHost": "Host entfernen",
  "connect.savedHasPassword": "Passwort gespeichert",
  "connect.rememberPassword": "Passwort merken",
  "connect.showPassword": "Passwort anzeigen",
  "connect.hidePassword": "Passwort verbergen",
  "connect.urlPasswordPlaceholder": "Anmeldepasswort",
  "connect.mirror": "Remote-Desktop-App spiegeln", // Mirror the remote desktop app
  "connect.mirrorHint":
    "Tabs, Splits und die aktive Sitzung entsprechen der Desktop-App auf dem Remote-Rechner; Änderungen auf einer Seite erscheinen auf beiden. Läuft die Desktop-App nicht, öffnet diese Verbindung direkt deren Datenbank oder, wenn keine vorhanden ist, eine eigene Datenbank.", // Same tabs, splits, and active session as the desktop app on the remote machine; changes on either side show on both. If the desktop app is not running, this connection opens its database directly, or a separate database when there is none.
  "connect.shareDesktopDb": "Datenbank der Remote-Desktop-App mitnutzen",
  "connect.shareDesktopDbHint":
    "Teilt sich eine Datenbank mit der Desktop-App des Remote-Rechners (am besten bei gleicher Version). Aus = eigene Datenbank.",

  // ── Sidebar ──
  "tree.newSession": "Neue Sitzung", // New Session
  "tree.newTerminalSession": "Neues Terminal", // New Terminal
  "tree.newBrowserPage": "Neue Browser-Seite", // New Browser Page
  "tree.newAgentSession": (agent) => `Neue ${agent}-Sitzung`, // New {agent} Session
  "tree.newAgentSessionGroup": "Weitere Agent-Sitzung", // More Agent Session
  "tree.newAgentSessionCustom": "Neu mit Startargumenten…", // New with launch args…
  "tree.resumeSession": "Sitzung fortsetzen…", // Resume Session…
  "tree.newGroup": "Neue Gruppe", // New Group
  "tree.newSubgroup": "Neue Untergruppe", // New Subgroup
  "tree.newChildSession": "Neue Untersitzung", // New Child Session
  "tree.openSelected": "Ausgewählte Sitzungen öffnen", // Open Selected Sessions
  "tree.archiveSelected": "Ausgewählte Sitzungen archivieren", // Archive Selected Sessions
  "tree.moveSelected": "Auswahl verschieben…", // Move Selected to…
  "tree.deleteSelected": (n) => `${n} ausgewählte Einträge löschen`, // Delete {n} Selected Items
  "tree.removeProject": "Projekt entfernen", // Remove Project
  "tree.deleteGroup": "Gruppe löschen", // Delete Group
  "tree.deleteSession": "Sitzung löschen", // Delete Session
  "tree.projectRoot": "Projektwurzel (ohne Gruppe)", // Project root (no group)
  "tree.moveToSession": "Unter eine Sitzung verschieben (als Kind)", // Move under a session (as child)
  "tree.moveTo": "Verschieben nach…", // Move to…
  "tree.openNewTab": "In neuem Tab öffnen", // Open in New Tab
  "tree.forkSession": "Sitzung forken", // Fork Session
  "tree.exportSession": "Sitzung exportieren…", // Export Session…
  "tree.sessionInfo": "Sitzungsinfo", // Session Info
  "tree.groupInfo": "Gruppeninfo", // Group Info
  "tree.collectionInfo": "Sammlungsinfo", // Collection Info
  "tree.projectInfo": "Projektinfo", // Project Info
  "info.branch": "Branch", // Branch
  "info.path": "Pfad", // Path
  "info.recentCommits": "Letzte Commits", // Recent Commits
  "info.noCommits": "Keine Commits", // No commits
  "tree.killProcess": "Prozess beenden", // Kill Process
  "tree.killProcessConfirm": (name: string) => `Den Prozess für „${name}“ beenden? Die aktuelle Aufgabe wird unterbrochen. Gespeicherte Gesprächsverläufe und Dateien bleiben erhalten.`,
  "tree.archiveSession": "Sitzung archivieren", // Archive Session
  "tree.archiveGroup": "Gruppe archivieren", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "temp", // scratch
  "tree.persistSession": "In dauerhafte Sitzung umwandeln…", // Make Permanent Session…
  "tree.persistDoc": "Auf Datenträger speichern…", // Save to Disk…
  "tree.closeScratch": "Entwurf schließen", // Close Scratch
  "tree.importProject": "Projekt importieren", // Import Project
  "tree.createProject": "Projekt erstellen",
  // New Collection / Collection name / research / Create Collection / No folder / Delete Collection
  "tree.newCollection": "Neue Sammlung",
  "tree.deleteCollection": "Sammlung löschen",
  "collection.title": "Neue Sammlung",
  "collection.name": "Name der Sammlung",
  "collection.namePlaceholder": "research",
  "collection.submit": "Sammlung erstellen",
  "collection.tag": "Kein Ordner",
  "collection.deleteTitle": "Sammlung löschen",
  "collection.deleteBody": (name) =>
    `Sammlung „${name}“ löschen? Alle enthaltenen Gruppen und Sitzungen werden ebenfalls gelöscht. Das lässt sich nicht rückgängig machen.`,
  "tree.cloneProject": "Von Git klonen", // Clone from Git
  "createProject.title": "Projekt erstellen",
  "createProject.name": "Projektname",
  "createProject.namePlaceholder": "mein-projekt",
  "createProject.into": "Erstellen in",
  "createProject.choose": "Auswählen…",
  "createProject.noParent": "Übergeordneten Ordner auswählen",
  "createProject.invalidName":
    "Geben Sie einen einzelnen Ordnernamen ohne / oder \\ ein.",
  "createProject.creating": "Wird erstellt…",
  "createProject.submit": "Projekt erstellen",
  "clone.title": "Git-Repository klonen", // Clone Git Repository
  "clone.url": "Repository-URL", // Repository URL
  "clone.urlPlaceholder": "https://… oder git@…",
  "clone.branch": "Branch (optional)", // Branch (optional)
  "clone.branchPlaceholder": "Leer = Standard-Branch", // Default branch if empty
  "clone.folder": "Ordnername", // Folder name
  "clone.folderPlaceholder": "Automatisch aus URL", // Auto from URL
  "clone.into": "Klonen nach", // Clone into
  "clone.choose": "Auswählen…", // Choose…
  "clone.noParent": "Übergeordneten Ordner wählen", // Choose a parent folder
  "clone.cloning": "Wird geklont…", // Cloning…
  "clone.cancelling": "Wird abgebrochen…",
  "clone.stageStarting": "Git wird gestartet…",
  "clone.stageConnecting": "Verbindung zum Repository…",
  "clone.stagePreparing": "Objekte werden vorbereitet…",
  "clone.stageReceiving": "Objekte werden empfangen…",
  "clone.stageResolving": "Deltas werden aufgelöst…",
  "clone.stageCheckout": "Dateien werden ausgecheckt…",
  "clone.stageFinalizing": "Wird abgeschlossen…",
  "clone.stageImporting": "Projekt wird importiert…",
  "clone.elapsed": (seconds: number) => `${seconds} s vergangen`,
  "clone.slowHint":
    "Seit 30 Sekunden kein Fortschritt. Prüfen Sie Netzwerk oder Proxy des Remote-Rechners; Sie können abbrechen und erneut versuchen.",
  "clone.submit": "Klonen", // Clone
  "tree.globalSearch": "Alle Sitzungen durchsuchen", // Search All Sessions
  "tree.archivedSessions": "Archivierte Sitzungen", // Archived Sessions
  "tree.searchPlaceholder": "Sitzungen / Gruppen suchen…", // Search sessions / groups…
  "tree.clearSearch": "Suche löschen", // Clear search
  "tree.filterWorking": "Aktiv", // Working
  "tree.filterAsking": "Ausstehend", // Pending
  "tree.filterWaiting": "Gesehen", // Viewed
  "tree.filterStatus": "Nach Status filtern", // Filter by status
  "tree.refreshStatusFilter": "Statusfilter aktualisieren",
  "tree.refreshStatusMatch": "Status aktualisieren",
  "tree.filterStatusSection": "Status", // Status
  "tree.filterMarkSection": "Markierung", // Mark
  "tree.viewMainName": "Hauptansicht",
  "tree.viewUntitled": "Unbenannte Ansicht",
  "tree.viewDefaultName": (n) => `Ansicht ${n}`,
  "tree.viewPrimary": "Hauptansicht",
  "tree.viewManage": "Ansicht verwalten",
  "tree.viewSetPrimary": "Als Hauptansicht festlegen",
  "tree.viewRename": "Ansicht umbenennen",
  "tree.viewName": "Ansichtsname",
  "tree.viewDelete": "Ansicht löschen",
  "tree.viewDeletePrimary": "Die Hauptansicht kann nicht gelöscht werden",
  "tree.viewDeleteTitle": "Baumansicht löschen",
  "tree.viewDeleteConfirm": (name) =>
    `„${name}“ löschen? Gespeicherte Such- und Filterbedingungen werden entfernt; Projekte und Sitzungen bleiben unverändert.`,
  "tree.viewSplitRight": "Baumansicht nach rechts teilen",
  "tree.viewSplitDown": "Baumansicht nach unten teilen",
  "tree.viewAdd": "Aktuelle Baumansicht in einen neuen Tab kopieren",
  "tree.viewCount": (n) => `${n} Baumansicht${n === 1 ? "" : "en"}`,
  "mark.menu": "Markierung", // Mark
  "mark.urgent": "Dringend", // Urgent
  "mark.important": "Wichtig", // Important
  "mark.bug": "Fehler", // Bug
  "mark.done": "Erledigt", // Done
  "mark.wip": "In Arbeit", // In progress
  "mark.pinned": "Angeheftet", // Pinned
  "mark.idea": "Idee", // Idea
  "mark.caution": "Achtung", // Caution
  "tree.clearAllNotifications":
    "Alle Benachrichtigungsmarken löschen (Sitzungspunkte und Dock-Badge)", // Clear all notification badges…
  "tree.noProjectsPre":
    "Noch keine Projekte. Klicken Sie auf das Ordnersymbol oder drücken Sie ", // No projects yet. Click the folder button, or press
  "tree.noProjectsPost": ", um ein Verzeichnis zu importieren.", // to import a directory.
  "tree.openProject": "Projekt öffnen", // Open Project
  "tree.noAttention": "Keine Sitzungen entsprechen dem Statusfilter", // No sessions match the status filter
  "tree.noMatch": "Keine Treffer", // No matches

  // Dialog fields
  "tree.groupName": "Gruppenname", // Group name
  "tree.sessionNameAuto": "Sitzungsname (leer = automatisch)", // Session name (leave empty to auto-name)
  "tree.editSession": "Sitzung bearbeiten", // Edit Session
  "tree.sessionName": "Sitzungsname", // Session name
  "tree.shellLabel": "Shell (leer = Systemstandard)", // Shell (leave empty for system default)
  "tree.shellMenu": "Shell",
  "tree.downloadFullGitbash": "Vollständige Git Bash herunterladen",
  "gitbash.title": "Git Bash",
  "gitbash.downloading": "Vollständige Git Bash wird heruntergeladen…",
  "gitbash.extracting": "Vollständige Git Bash wird entpackt…",
  "gitbash.done": "Vollständige Git Bash ist bereit.",
  "gitbash.failed": "Git Bash-Download fehlgeschlagen",
  "tree.shellSystemDefault": "Systemstandard", // System default
  "form.customOption": "Benutzerdefiniert…", // Custom…
  "tree.cwdLabel": "Arbeitsverzeichnis (leer = Projektwurzel)", // Working directory (leave empty for project root)
  "tree.initCmdLabel": "Startbefehl (optional)", // Startup command (optional)
  "tree.engineLabel": "Öffnet in",
  "tree.engineTui": "Terminalansicht",
  "tree.engineChat": "Gesprächsansicht",
  // The agent runs its own terminal interface.
  "tree.engineTuiHint": "Der Agent läuft in seiner eigenen Terminaloberfläche.",
  // Messages and tool cards, with buttons for permission questions.
  "tree.engineChatHint": "Darstellung als Nachrichten und Werkzeugkarten; Berechtigungsanfragen werden in der Oberfläche beantwortet.",
  "tree.agentArgsLabel": "Startargumente (optional)", // Launch args (optional)
  // Working directory / Leave empty for the default
  "tree.workingDirLabel": "Arbeitsverzeichnis",
  "tree.workingDirPlaceholder": "Leer lassen für das Standardverzeichnis",
  "preset.execPathLabel": "Programmdatei (optional)",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint":
    "Leer lassen, um den konfigurierten Befehl des Agenten zu verwenden. Mit Angabe läuft nur diese Sitzung mit einem kompatiblen Ersatz.",
  "preset.saveLabel": "Als Vorlage speichern",
  "preset.namePlaceholder": "Vorlage benennen",
  "preset.iconChoose": "Symbol wählen",
  "preset.iconClear": "Entfernen",
  "preset.iconHint":
    "Quadratische Bilder eignen sich am besten; andere werden zugeschnitten und auf 64x64 skaliert.",
  "tree.permissionSkipLabel": "Alle Berechtigungsabfragen überspringen", // Skip all permission confirmations
  "tree.permissionSkipHint":
    "Startet mit dem Bypass-Flag dieses Agenten (z. B. Claude --dangerously-skip-permissions; Codex deaktiviert zudem seine Sandbox). Gilt bei jedem Start – mit Vorsicht verwenden.",
  "tree.permissionUnsupported":
    "OpenCode steuert Berechtigungen über seine Konfigurationsdatei – kein Start-Flag, daher nicht anwendbar.",
  "tree.permissionUnsupportedPi":
    "Pi führt Tools bauartbedingt ohne Berechtigungsabfragen aus – daher nicht anwendbar.",

  // Dialog „Neue Agent-Sitzung“
  "newAgent.desc":
    "Optional können Sie die Sitzung benennen und eigene Startargumente angeben (an den Agent-Befehl übergeben, z. B. --model opus). Lassen Sie beide leer und drücken Sie Enter, um sie wie gewohnt zu starten.", // Optionally name the session and add custom launch args…

  // Delete confirmation
  "tree.batchDeleteTitle": "Stapellöschung", // Batch Delete
  "tree.deleteProjectTitle": "Projekt löschen", // Delete Project
  "tree.deleteGroupTitle": "Gruppe löschen", // Delete Group
  "tree.deleteSessionTitle": "Sitzung löschen", // Delete Session
  "tree.batchDeleteBody": (n) =>
    `${n} ausgewählte Einträge löschen (Projekte/Gruppen löschen ihre Untergruppen und Sitzungen kaskadiert mit). Dies kann nicht rückgängig gemacht werden.`, // Delete the {n} selected items…
  "tree.deleteProjectBody": (name) =>
    `Projekt „${name}“ löschen? Alle Untergruppen und Sitzungen werden ebenfalls gelöscht. Dies kann nicht rückgängig gemacht werden.`, // Delete project "{name}"?…
  "tree.deleteGroupBody": (name) =>
    `Gruppe „${name}“ löschen? Alle Untergruppen und Sitzungen werden ebenfalls gelöscht. Dies kann nicht rückgängig gemacht werden.`, // Delete group "{name}"?…
  "tree.deleteSessionBody": (name) =>
    `Sitzung „${name}“ (und alle Untersitzungen) löschen? Dies kann nicht rückgängig gemacht werden.`, // Delete session "{name}"…
  "tree.deleteWorktrees": (n) =>
    `Zugehörige Git-Worktrees mitlöschen (${n} insgesamt; bei Änderungen im Arbeitsbaum kann das Löschen fehlschlagen)`, // Also remove associated git worktrees…

  // Session information dialog
  "info.name": "Name", // Name
  "info.type": "Typ", // Type
  "info.status": "Status", // Status
  "info.notYetCaptured": "Noch nicht erzeugt (nach erstem Lauf erfasst)", // Not yet generated (captured after first run)
  "info.sessionId": "Sitzungs-ID", // Session ID
  "info.projectId": "Projekt-ID", // Project ID
  "info.cwd": "Arbeitsverz.", // Working dir
  "info.initCmd": "Startbefehl", // Startup cmd
  "info.agentArgs": "Startargumente", // Launch args
  "info.launchCmd": "Vollständiger Startbefehl", // Full launch command
  "info.permission": "Berechtigung", // Permission
  "info.permissionSkip": "Alle Abfragen überspringen", // Skip all confirmations
  "info.parentSessionId": "Eltern-ID", // Parent ID
  "info.termTitle": "Terminaltitel", // Terminal title
  "info.createdAt": "Erstellt am", // Created at

  // Resume-session dialog
  "importSessions.results": ({ count }: { count: number }) => `Ergebnisse: ${count}`,
  "importSessions.selected": ({ count }: { count: number }) => `Ausgewählt: ${count}`,
  "importSessions.clearSelection": "Auswahl aufheben",
  "importSessions.clearSearch": "Suche zurücksetzen",
  "importSessions.noHistory": "Für dieses Projektverzeichnis wurden keine bisherigen Sitzungen gefunden.",
  "importSessions.title": "Sitzungen importieren",
  "importSessions.description": "Suchen Sie bestehende Codex-, Claude- und OpenCode-Sitzungen mit dem Arbeitsverzeichnis dieses Projekts. Wählen Sie Sitzungen aus, um sie dem Projekt hinzuzufügen, und öffnen Sie anschließend eine Sitzung, um das Gespräch fortzusetzen.",
  "importSessions.search": "Nach Titel, Agent oder Sitzungs-ID suchen",
  "importSessions.empty": "Keine passenden Sitzungen gefunden.",
  "importSessions.imported": "Bereits importiert",
  "importSessions.confirm": ({ count }: { count: number }) => `Importieren (${count})`,
  "importSessions.success": ({ count }: { count: number }) => `Dem Projekt hinzugefügte Sitzungen: ${count}.`,
  "resume.title": "Sitzung fortsetzen", // Resume Session
  "resume.desc":
    "Agententyp wählen und die eigene Session-ID des Agenten eingeben; beim Öffnen wird das ursprüngliche Gespräch fortgesetzt.", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "Agententyp", // Agent type
  "resume.sessionIdPlaceholder": "Session-ID des Gesprächs", // Conversation session id
  "resume.confirm": "Fortsetzen & öffnen", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "Neue Worktree-Sitzung…", // New Worktree Session…
  "worktree.worktreeNameLabel": "Worktree-Name", // Worktree name
  "worktree.worktreeNameHint":
    "Wird als Worktree-Verzeichnis und Branch-Name verwendet.", // Used as the worktree directory and branch name.
  "worktree.createFailed": "Worktree konnte nicht erstellt werden", // Couldn't create the worktree
  "worktree.noRepoRoot":
    "Dieses Projekt hat keinen nutzbaren Git-Repository-Pfad.", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "Keiner", // None
  "worktreeSel.modeNew": "Neu", // New
  "worktreeSel.modeExisting": "Vorhanden", // Existing
  "worktreeSel.loading": "Worktrees werden geladen…", // Loading worktrees…
  "worktreeSel.empty": "Keine vorhandenen Worktrees in diesem Repository.", // No existing worktrees in this repository.
  "worktreeSel.loadFailed":
    "Worktrees konnten nicht aufgelistet werden (kein Git-Repository?).", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint":
    "In dieser Gruppe erstellte Sitzungen verwenden standardmäßig dieses Worktree.", // Sessions created in this group will use this worktree by default.
  "worktree.moveGroupTitle": "Gruppe zu Worktree verschieben",
  "worktree.moveGroupHint":
    "Ab jetzt in dieser Gruppe erstellte Sitzungen verwenden dieses Worktree. Bereits vorhandene behalten ihr aktuelles Verzeichnis.",

  // ── Archive panel ──
  "archive.title": "Archivierte Sitzungen", // Archived Sessions
  "archive.empty1": "Keine archivierten Sitzungen.", // No archived sessions.
  "archive.empty2":
    "Rechtsklick auf eine Sitzung in der Seitenleiste und „Sitzung archivieren“ wählen, um sie hier abzulegen.", // Right-click a session in the sidebar…
  "archive.restore": "Als normale Sitzung wiederherstellen", // Restore to normal session
  "archive.export": "Vollständigen Kontext als Markdown exportieren", // Export full context as Markdown
  "archive.deleteForever": "Endgültig löschen (samt Aufzeichnung)", // Delete permanently (with recording)
  "archive.pickOne":
    "Links eine archivierte Sitzung wählen, um das Transkript zu sehen", // Select an archived session on the left…
  "archive.recordingEnd": "--- Ende der Aufzeichnung ---", // --- End of recording ---
  "archive.readRecordingFailed": (err) =>
    `Aufzeichnung konnte nicht gelesen werden: ${err}`, // Failed to read recording: {err}
  "archive.searchRecording": "In Aufzeichnung suchen…", // Search in recording…
  "archive.searchTranscript": "Transkript durchsuchen…", // Search transcript…
  "archive.searchPlaceholder": "Archivierte Inhalte durchsuchen…", // Search archived content…
  "archive.msgCountAll": (n) => (n === 1 ? "1 Nachricht" : `${n} Nachrichten`), // {n} messages
  "archive.msgCountFiltered": (shown, total) =>
    `${shown} / ${total} Nachrichten`, // {shown} / {total} messages
  "archive.you": "Du", // You
  "archive.toolsUsed": (tools) => `Werkzeuge: ${tools}`, // Tools: {tools}
  "archive.noMatch": "Keine passenden Nachrichten", // No matching messages
  "archive.emptyTranscript": "Transkript ist leer", // Transcript is empty
  "archive.loadingTranscript": "Transkript wird geladen…", // Loading transcript…

  // ── Global session-content search ──
  "search.allPlaceholder": "Gesamten Sitzungsinhalt durchsuchen…", // Search across all session content…
  "search.hint":
    "Durchsuchen Sie Sitzungsinhalte. Archivierte Sitzungen sind standardmäßig ausgeschlossen — „Archivierte einbeziehen“ aktivieren, um sie hinzuzufügen.", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "Archivierte einbeziehen", // Include archived
  "search.includeArchivedHint":
    "Auch archivierte Sitzungen durchsuchen (standardmäßig aus)", // Also search archived sessions (off by default)
  "search.searching": "Suche…", // Searching…
  "search.noResults": "Keine Treffer gefunden", // No matches found
  "search.sessionCount": (n) => (n === 1 ? "1 Sitzung" : `${n} Sitzungen`), // n sessions
  "search.matchCount": (n) => (n === 1 ? "1 Treffer" : `${n} Treffer`), // n matches
  "search.pickSession":
    "Wählen Sie links eine Sitzung, um ihre Treffer zu sehen", // Select a session on the left to see its matches
  "search.openSession": "Sitzung öffnen", // Open session
  "search.backToResults": "Zurück zu den Ergebnissen", // Back to results
  "search.archivedBadge": "Archiviert", // Archived
  "search.summary": (m, s) =>
    `${m} Treffer · ${s} ${s === 1 ? "Sitzung" : "Sitzungen"}`, // X matches · N sessions
  "search.matchPosition": (n, total) => `${n} von ${total}`, // N of M
  "search.roleTerminal": "Terminal", // Terminal
  "search.collapseGroup": "Einklappen", // Collapse
  "search.expandGroup": "Ausklappen", // Expand
  "search.cappedNote": (l, total) => `${l} von ${total} lokalisierbar`, // L of total locatable

  // ── Center pane ──
  "center.noSession": "Keine Sitzung", // No session
  "center.noSessionHintPre": "Sitzung aus der Seitenleiste wählen oder ", // Pick a session from the sidebar, or press
  "center.noSessionHintPost": " drücken, um ein Terminal zu erstellen", // to create a terminal
  "center.createTerminal": "Terminal erstellen", // Create Terminal
  "tab.unsavedDot": "Ungespeicherte Änderungen", // Unsaved changes
  "tab.newTerminal": "Neues Terminal", // New terminal
  "tab.newDocument": "Neues Dokument", // New document
  "tab.bgTitle": (n) => `Hintergrund-Tabs: ${n} (Prozesse laufen weiter)`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `Hintergrund ${n}`, // Background {n}
  "tab.scratchFallback": "(temporäres Terminal)", // (scratch terminal)
  "tab.killBgTab": "Diesen Hintergrund-Tab beenden (Prozesse werden beendet)", // Kill this background tab…
  "tab.newBrowserTab": "Neuer Tab", // New Tab
  "tab.refreshFile": "Datei neu laden", // Refresh File
  "tab.closeOthers": "Andere Tabs schließen", // Close Other Tabs
  "tab.closeRight": "Tabs rechts schließen", // Close Tabs to the Right
  "tab.closeAll": "Alle Tabs schließen", // Close All Tabs
  "tab.sendToBackground": "In den Hintergrund verschieben", // Send to Background

  // ── Integrierter Browser ──
  "browser.back": "Zurück", // Back
  "browser.forward": "Vorwärts", // Forward
  "browser.reload": "Neu laden", // Reload
  "browser.desktopOnly":
    "Browser-Tabs lassen sich nur in der Desktop-App öffnen.", // Browser tabs open in the desktop app only.
  "browser.stop": "Laden abbrechen", // Stop loading
  "browser.openExternal": "Im System-Browser öffnen", // Open in system browser
  "browser.addressPlaceholder": "URL oder Suchbegriffe eingeben", // Enter URL or search terms
  "browser.quickAccess": "Schnellzugriff", // Quick access
  "browser.loading": "Lädt…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "VelaTerm beenden?", // Quit VelaTerm?
  "quit.body": "Alle laufenden Terminal- und Agent-Sitzungen werden beendet.", // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "Arbeitsbereich speichern", // Save workspace
  "quit.saveWorkspaceHint":
    "Beim nächsten Start dieselben Tabs und Teilungen öffnen. Terminals werden wiederhergestellt, aber nicht neu gestartet.", // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "Beenden", // Quit
  "dormant.body":
    "Aus dem gespeicherten Arbeitsbereich wiederhergestellt. Es läuft noch kein Prozess.", // Restored from your saved workspace. No process is running yet.
  "dormant.start": "Starten", // Start
  "overlimit.title": (max) => `Hintergrund-Limit überschritten (${max})`, // Background keep-alive over limit ({max})
  "overlimit.body":
    "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
  "overlimit.kill": "End Selected", // End Selected
  "overlimit.keep": "Keep for Now", // Keep for Now
  "overlimit.earliest": "earliest", // earliest
  "overlimit.statusWorking": "working", // working
  "overlimit.statusAsking": "awaiting reply", // awaiting reply
  "overlimit.statusWaiting": "waiting", // waiting

  // ── Terminal pane ──
  "term.paste": "Einfügen", // Paste
  "term.pasteUseShortcut": "Einfügen (⌘V drücken)", // Paste (press ⌘V)
  "term.selectAll": "Alles auswählen", // Select All
  "term.autoCopied": (n: number) => `${n} Zeichen autom. kopiert · ⌘V`,
  "term.clear": "Leeren", // Clear
  "term.searchMenu": "Suchen…", // Search…  ⌘F
  "term.splitRight": "Rechts teilen", // Split right (⌘D)
  "term.splitDown": "Unten teilen", // Split down (⌘⇧D)
  "term.closePane": "Teilung schließen", // Close split
  "term.redraw": "Neu zeichnen", // Redraw
  "term.mirrorTooltip":
    "Spiegelanzeige (Größe wird von einem anderen Client gesteuert). Klicken, um das PTY an dieses Fenster anzupassen", // Mirroring (size controlled by another client)…
  "term.mirrorBadge": (dims) => `⤢ Spiegel${dims} · klicken zum Anpassen`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) => `⤢ Spiegel${dims} · an Fenster anpassen`, // ⤢ Mirror{dims} · fit this window
  "term.imgUploadFailed": (n, lastError) =>
    `Bild-Upload für ${n} Bild${n === 1 ? "" : "er"} fehlgeschlagen${lastError ? `: ${lastError}` : ""}`, // Image upload failed for {n} images…
  "term.imgClipboardUnavailable":
    "Das Bild konnte nicht aus der Zwischenablage gelesen werden. Kopieren Sie es erneut.",
  "term.starting": (agent) => `${agent} wird gestartet…`, // Starting {agent}…
  "term.startFailed": (err) => `Start fehlgeschlagen: ${err}`, // Failed to start: {err}

  // ── Agent-Installationshinweis-Karte ──
  "agentInstall.title": (label) => `${label} ist nicht installiert`, // {label} is not installed
  "agentInstall.desc": (label) =>
    `VelaTerm hat ${label} nicht in Ihrem PATH gefunden. Installieren Sie es, um diese Sitzung zu starten.`, // couldn't find {label} on PATH
  "agentInstall.install": "Jetzt installieren", // Install now
  "agentInstall.retry": "Erneut starten", // Retry launch
  "agentInstall.dismiss": "Mache ich selbst", // I'll do it myself
  "agentInstall.docs": "Anleitung", // Install docs
  "agentInstall.needsNode": "Erfordert Node.js / npm", // Requires Node.js / npm
  "agentInstall.afterInstall": "Nach der Installation:", // After install:
  "agentInstall.pathSaved": (label: string) =>
    `Pfad zur ausführbaren Datei von ${label} in den Einstellungen gespeichert:`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} ist installiert`, // {label} is installed
  "agentInstall.doneDesc": "Starten Sie diese Sitzung neu, um loszulegen.", // Relaunch this session to start using it.
  "agentInstall.restartNow": "Jetzt neu starten", // Relaunch now
  "agentInstall.later": "Später", // Later
  "agentInstall.pathLabel": "Pfad zur ausführbaren Datei", // Executable path
  "agentInstall.pathPlaceholder": (bin: string) => `~/.local/bin/${bin}`,
  "agentInstall.pathHint": "Bereits außerhalb von PATH installiert? Geben Sie den vollständigen Pfad zur ausführbaren Datei ein.", // Already installed outside PATH?
  "agentInstall.pathSave": "Diesen Pfad verwenden", // Use this path
  "agentInstall.pathBrowse": "Durchsuchen…", // Browse…
  "search.placeholder": "Im Terminal suchen", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "WYSIWYG", // WYSIWYG
  "doc.visual": "Visuell",
  "doc.source": "Quelltext",
  "doc.compare": "Vergleich",
  "doc.editorLoadFailed": "Der Markdown-Editor konnte nicht geladen werden.",
  "doc.imageOnly": "Hier können nur Bilddateien eingefügt werden.",
  "doc.searchPlaceholder": "Suchen", // Find
  "doc.searchReplacePlaceholder": "Ersetzen", // Replace
  "doc.searchReplace": "Ersetzen", // Replace
  "doc.searchReplaceAll": "Alle", // All
  "doc.searchNoMatch": "Keine Treffer", // No results
  "doc.searchCaseSensitive": "Groß-/Kleinschreibung", // Match case
  "doc.searchToggleReplace": "Ersetzen umschalten", // Toggle replace
  "doc.fileTree": "Dateibaum", // File tree
  "doc.treeUp": "Übergeordneter Ordner", // Parent folder
  "doc.sidebar": "Seitenleiste", // Sidebar
  "doc.unsaved": "Nicht gespeichert", // Unsaved
  "doc.saveAsTitle": "Speichern unter", // Save As
  "doc.saveAsName": "Dateiname", // File name
  "doc.outline": "Gliederung", // Outline
  "doc.outlineEmpty": "Keine Überschriften", // No headings
  "doc.saving": "Speichert…", // Saving…
  "doc.overwriteConfirm":
    "Eine Datei mit diesem Namen existiert bereits. Zum Ersetzen auf „Überschreiben“ klicken.", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "Speichern", // Save
  "doc.externalChanged":
    "Die Datei wurde auf der Festplatte geändert (Sie haben ungespeicherte lokale Änderungen).", // The file was modified on disk…
  "doc.reloadDiscard": "Neu laden (meine Änderungen verwerfen)", // Reload (discard my changes)
  "doc.externalChangedClean": "Die Datei wurde auf der Festplatte geändert.", // The file was modified on disk.
  "doc.reload": "Neu laden", // Reload
  "doc.ignore": "Ignorieren", // Ignore
  "doc.loadingFile": (title) => `${title} wird geladen…`, // Loading {title}…
  "doc.closeTitle": "Dokument schließen", // Close Document
  "doc.unsavedBody": (title) => `„${title}“ hat ungespeicherte Änderungen.`, // "{title}" has unsaved changes.
  "doc.saveAndClose": "Speichern & schließen", // Save & Close
  "doc.closeNoSave": "Ohne Speichern schließen", // Close Without Saving
  "doc.conflictTitle": "Speicherkonflikt", // Save Conflict
  "doc.conflictBody":
    "Die Datei auf der Festplatte wurde extern geändert. Trotzdem mit dem aktuellen Inhalt überschreiben?", // The file on disk was modified externally…
  "doc.overwrite": "Überschreiben", // Overwrite
  "doc.saveFailed": (err) => `Speichern fehlgeschlagen: ${err}`, // Save failed: {err}
  "doc.closeTab": "Tab schließen", // Close Tab
  "doc.truncatedReadonly": (size: string) =>
    `Schreibgeschützt: zeigt die ersten 10 MB von ${size}. Speichern ist deaktiviert, um den Rest der Datei nicht zu überschreiben.`,
  "doc.imgLoading": (title, size) => `${title} (${size}) wird geladen…`, // Loading {title} ({size})…
  "doc.imgBeingWritten":
    "Die Datei wird gerade geschrieben; sie wird automatisch neu geladen, sobald sie stabil ist.", // The file is being written; it will reload automatically once it settles.
  "doc.imgDecodeFailed":
    "Dieses Bild kann nicht angezeigt werden (nicht unterstütztes oder beschädigtes Format).", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "Einpassen", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "Als PDF exportieren", // Export PDF
  "doc.diagramError": "Diagrammfehler", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "Keine Sitzung ausgewählt", // No session selected
  "panel.collapseSection": "Abschnitt einklappen", // Collapse section
  "panel.expandSection": "Abschnitt ausklappen", // Collapse section
  "panel.openInEditor": "Im Editor öffnen", // Open in Editor
  "panel.openInEditorTooltip":
    "Im Dokumenteditor des mittleren Bereichs öffnen (wie der view-Befehl)", // Open in the document editor…
  "panel.preview": "Vorschau", // Preview
  "panel.cantRead": "(Datei kann nicht gelesen werden)", // (cannot read this file)
  "panel.binary": "(Binärdatei, keine Vorschau)", // (binary file, no preview)
  "panel.truncated": "\n…(Inhalt gekürzt)", // …(content truncated)
  "panel.showHidden": "Versteckte Dateien anzeigen", // Show hidden files
  "panel.hideHidden": "Versteckte Dateien ausblenden", // Hide hidden files

  // ── File-tree actions (Files context menu and header add button) ──
  "files.newFile": "Neue Datei", // New File
  "files.newFolder": "Neuer Ordner", // New Folder
  "files.nameLabel": "Name", // Name
  "files.newTooltip": "Neue Datei oder Ordner", // New file or folder
  "files.openInTerminal": "Open in Terminal",
  "files.revealInFinder": "Show in File Manager",
  "files.copyPath": "Copy Path",
  "files.copyRelPath": "Copy Relative Path",
  "files.filterPlaceholder": "Filter files…",
  "files.dblClickOpen": "Zum Öffnen doppelklicken",
  "files.deleteConfirm": (name) =>
    `„${name}" löschen? Dies kann nicht rückgängig gemacht werden.`, // Delete "{name}"? This can't be undone.

  // ── File transfer (remote access) ──
  "transfer.uploadsTitle": "Uploads", // Uploads
  "transfer.download": "Herunterladen", // Download
  "transfer.upload": "Dateien hochladen…", // Upload Files…
  "transfer.uploadTooltip": "Dateien in diesen Ordner hochladen", // Upload files to this folder
  "transfer.clear": "Leeren", // Clear
  "transfer.cancelled": "Abgebrochen", // Cancelled
  "transfer.failed": "Fehlgeschlagen", // Failed
  "transfer.stalled": "Verbindung wird wiederhergestellt…", // Reconnecting…
  "transfer.foldersUnsupported": "Ordner können nicht hochgeladen werden.", // Folders can't be uploaded.

  // ── Status bar ──
  "statusbar.sessions": (n) => (n === 1 ? "1 Sitzung" : `${n} Sitzungen`), // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `Klicken, um in der Seitenleiste nur „${label}“-Sitzungen zu zeigen (erneut klicken zum Aufheben)`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `Hintergrund ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `Hintergrund-Tabs (Limit ${max}; bei Überschreitung wird der älteste inaktive Tab automatisch beendet)`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) =>
    `Hintergrund-Tab beendet: ${name} (Limit überschritten)`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) => `Browser-Fernzugriff aktiviert: ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "Rechte: Fragen", // Perms: Ask
  "statusbar.permSkip": "Rechte: Überspringen", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip":
    "Berechtigungsmodus dieser Sitzung · zum Ändern klicken (nur diese Sitzung)", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "Berechtigungen dieser Sitzung", // This session's permissions
  "statusbar.permOptAsk": "Jedes Mal fragen (Standard)", // Ask each time (default)
  "statusbar.permScopeHint":
    "Gilt nur für diese Sitzung. Für globale Standardwerte gehen Sie zu Einstellungen ▸ Agenten.", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg":
    "Berechtigung geändert. Die Sitzung muss neu gestartet werden, damit dies wirkt. Der Neustart setzt das aktuelle Gespräch fort, unterbricht aber laufende Aufgaben. Jetzt neu starten?", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "Jetzt neu starten", // Restart now
  "statusbar.permRestartLater": "Später", // Later
  "statusbar.permScopeTitle": "Anwenden auf?", // Apply to?
  "statusbar.permScopeSession": "Nur diese Sitzung", // This session only
  "statusbar.permScopeGlobal": "Globaler Standard", // Global default
  "statusbar.permScopeGlobalHint":
    "Gilt jetzt für diese Sitzung und wird zum Standard für künftige neue Sitzungen dieser Art (mit Einstellungen synchronisiert).", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

  // ── Store, notifications, and export ──
  "notify.working": "⏳ Arbeitet…", // ⏳ Working…
  "notify.asking": "❓ Ihre Bestätigung ist nötig", // ❓ Needs your confirmation
  "notify.waiting": "✅ Geantwortet", // ✅ Replied
  "store.subtask": "Teilaufgabe", // Subtask
  "store.splitPane": "Teilung", // Split
  "export.failedTitle": "Sitzungsexport fehlgeschlagen", // Failed to export session
  "export.contextSuffix": "Kontext", // context

  // ── Error panel ──
  "err.renderTitle": "Renderfehler", // Rendering Error
  "err.renderDesc":
    "Ein unerwarteter Fehler ist aufgetreten. Die folgenden Informationen helfen bei der Fehlersuche.", // An unexpected error occurred…
  "err.reload": "Neu laden", // Reload
  "err.uncaughtTitle": "Nicht abgefangener Fehler", // Uncaught Error
  "err.uncaughtDesc": "Die folgenden Informationen helfen bei der Fehlersuche.", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser":
    "Wiedergabe von Aufzeichnungen wird im Browser noch nicht unterstützt", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) =>
    `Bild-Upload fehlgeschlagen (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.showPassword": "Anzeigen",
  "login.hidePassword": "Ausblenden",
  "login.passwordSaveFailed": "Die Verbindung steht, aber das Passwort konnte nicht auf diesem Gerät gespeichert werden. Bitte erneut versuchen.",
  "login.connecting": "Verbinde…", // Connecting…
  "login.remoteAccess": "Fernzugriff", // Remote Access
  "login.desc":
    "Geben Sie das Zugangspasswort ein, um sich mit diesem Terminal zu verbinden.", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "Zugangspasswort", // Access password
  "login.connect": "Verbinden", // Connect
  "login.wrongPassword": "Falsches Passwort", // Wrong password
  "login.rateLimited":
    "Zu viele Versuche. Bitte eine Minute warten und erneut versuchen.", // Too many attempts. Please wait a minute and try again.
  "login.failed": "Anmeldung fehlgeschlagen, bitte erneut versuchen", // Login failed, please try again
  "login.pairingRequired":
    "Dieser Server erfordert einen Kopplungslink. Öffnen Sie den im Fernzugriff-Bereich der Desktop-App erzeugten Link.", // This server requires a pairing link
  "login.authFailed":
    "Authentifizierung fehlgeschlagen. Prüfen Sie das Zugangspasswort, oder öffnen Sie einen neuen Kopplungslink, falls der Link neu erstellt wurde.", // Authentication failed, check password or use a new pairing link
  "dir.title": "Projektverzeichnis wählen", // Choose Project Directory
  "dir.pathPlaceholder":
    "Suchen oder Pfad eingeben und Enter drücken (unterstützt ~)", // Search, or type a path and press Enter (supports ~)
  "dir.up": "Eine Ebene nach oben", // Up one level
  "dir.newFolder": "Neuer Ordner", // New Folder
  "dir.newFolderPlaceholder": "Ordnername", // Folder name
  "dir.goInput": "Zum eingegebenen Pfad", // Go to typed path
  "dir.noSubdirs": "(keine Unterverzeichnisse)", // (no subdirectories)
  "dir.empty": "(leerer Ordner)", // (empty folder)
  "dir.noMatch": "Keine passenden Elemente", // No matching items
  "dir.target": "Zielordner", // Target
  "dir.showHidden": "Versteckte Elemente anzeigen", // Show hidden items
  "dir.importing": "Importiere…", // Importing…
  "dir.choose": "Dieses Verzeichnis wählen", // Choose This Directory
  "conn.reconnecting": "Verbindung getrennt, verbinde erneut…", // Connection lost, reconnecting…
  "conn.reconnectNow": "Jetzt neu verbinden", // Reconnect now
  "conn.retrying": "Verbinde erneut…", // Reconnecting…
  "conn.sshReconnecting":
    "SSH-Verbindung unterbrochen, Tunnel wird neu aufgebaut…", // SSH link lost, rebuilding the tunnel…
  "conn.sshDown":
    "SSH-Verbindung getrennt — zum erneuten Versuch „Jetzt neu verbinden“ drücken", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "Anfrage fehlgeschlagen", // Request failed
  "reqerr.dismiss": "Schließen", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "Fehlerprotokoll", // Error Log
  "errlog.empty": "Keine Fehler aufgezeichnet.", // No errors recorded.
  "errlog.copyAll": "Alle kopieren", // Copy all
  "errlog.clear": "Löschen", // Clear
  "errlog.close": "Schließen", // Close

  // ── Mobile ──
  "mobile.backConnections": "Zurück zu den Verbindungen",
  "mobile.loadSlow": "Das Laden dauert länger als erwartet. Sie können es erneut versuchen oder zu Ihren Verbindungen zurückkehren.",
  "mobile.connectionUnavailable": "Verbindung nicht verfügbar",
  "mobile.pushTitle": "Aufgabenbenachrichtigungen",
  "mobile.pushHint": "Benachrichtigungen zeigen den Sitzungsnamen und eine kurze Antwortvorschau – auch im Hintergrund oder bei gesperrtem Bildschirm. Dieser Text wird an velaterm.com und den Push-Dienst übermittelt. Verbindungspasswörter und private SSH-Schlüssel werden nicht gesendet.",
  "mobile.pushEnable": "Benachrichtigungen aktivieren",
  "mobile.pushDisable": "Benachrichtigungen deaktivieren",
  "mobile.pushTest": "Testbenachrichtigung senden",
  "mobile.pushTestSent": "Die Testbenachrichtigung wurde eingereiht. Prüfen Sie die Mitteilungszentrale des Systems.",
  "mobile.pushDisabled": "Hintergrundbenachrichtigungen sind deaktiviert.",
  "mobile.pushEnabled": "Hintergrundbenachrichtigungen sind aktiviert.",
  "mobile.pushNotConfigured": "Für diesen Build ist kein Push-Dienst konfiguriert.",
  "mobile.pushDenied": "Erlauben Sie Benachrichtigungen in den Systemeinstellungen.",
  "mobile.pushRegistrationFailed": "Die Geräteregistrierung ist fehlgeschlagen. Bitte versuchen Sie es erneut.",
  "mobile.pushRelayUnavailable": "Der Benachrichtigungsdienst ist nicht verfügbar. Bitte versuchen Sie es erneut.",
  "mobile.pushHostUnavailable": "Der entfernte Host unterstützt noch keine Hintergrundbenachrichtigungen. Aktualisieren Sie ihn und stellen Sie die Verbindung erneut her.",
  "mobile.pushDisclosure": "Für Hintergrundbenachrichtigungen werden Getui und der Push-Dienst des Geräteherstellers verwendet. Zur Zustellung verarbeiten sie Gerätekennungen, Netzwerkinformationen, Sitzungsnamen und kurze Antwortvorschauen. Verbindungspasswörter und private SSH-Schlüssel werden nicht gesendet.",
  "mobile.pushConnectHint": "Öffnen Sie nach der Aktivierung jede gewünschte Verbindung einmal, um Benachrichtigungen zu abonnieren.",
  "mobile.pushTarget": "Verbindung testen",
  "mobile.copyConnection": "Kopieren und bearbeiten",
  "mobile.copyConnectionHint": "Bearbeiten Sie die Einstellungen auf Grundlage dieser Verbindung. Gespeicherte Zugangsdaten werden sicher übernommen. Die ursprüngliche Verbindung bleibt unverändert; bei identischen Einstellungen wird die vorhandene Verbindung beibehalten.",
  "mobile.copyConnectionReused": "Diese Einstellungen sind bereits gespeichert. Die vorhandene Verbindung wurde beibehalten.",
  "mobile.inputOptions": "Nachrichtenoptionen",
  "mobile.connections": "Verbindungen verwalten",
  "mobile.more": "Weitere Aktionen",
  "mobile.toDesktop": "Zur Desktop-Version wechseln", // Switch to desktop
  "mobile.empty1": "Keine Sitzungen.", // No sessions.
  "mobile.noMatch": "Keine passenden Sitzungen", // No matching sessions
  "mobile.empty2":
    "Erstellen Sie eine in der Desktop-App oder im Computer-Browser, sie erscheint hier automatisch.", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ Zurück", // ‹ Back
  "mobile.selCopy": "Kopieren", // Copy
  "mobile.selCancel": "Abbrechen", // Cancel

  // ── Mobile connection client (apps/mobile start page) ──
  "mobile.phaseConnecting": "SSH-Verbindung wird aufgebaut…", // Connecting over SSH…
  "mobile.phaseConfirming": "Host-Fingerabdruck bestätigen", // Confirm the host fingerprint
  "mobile.phasePreparing": "Remote-Dienst wird geprüft oder vorbereitet…", // Checking or preparing the remote service…
  "mobile.phaseForwarding": "SSH-Tunnel wird aufgebaut…", // Opening the SSH tunnel…
  "mobile.phaseReady": "Verbunden", // Connected
  "mobile.phaseDisconnected": "Verbindung getrennt", // Disconnected
  "mobile.phaseError": "Verbindung fehlgeschlagen", // Connection failed
  "mobile.accountAndLogin": "Konto und Anmeldung", // Account and sign-in
  "mobile.connectionService": "Verbindungsdienst nicht verfügbar", // Connection service unavailable
  "mobile.nativeOnly": "Verbindungen sind nur in der iOS- oder Android-App möglich. Der Browser dient lediglich der Vorschau der Oberfläche.", // Connecting is only available in the iOS or Android app. The browser is only for previewing the interface.
  "mobile.managedRemotely": "Projekte und Sitzungen werden vom Remote-Dienst verwaltet.", // Projects and sessions are managed by the remote service.
  "mobile.buildInfo": (version: string, time: string) => `App v${version} · Build vom ${time}`,
  "mobile.myDevices": "Meine Geräte", // My devices
  "mobile.account": "Konto", // Account
  "mobile.signedInHint": "Angemeldet. Sie können die Arbeitsbereiche, Projekte und Sitzungen sehen, die Geräte dieses Kontos freigeben.", // Signed in. You can view the workspaces, projects, and sessions shared by devices on this account.
  "mobile.manageAccount": "Konto verwalten", // Manage account
  "mobile.signOut": "Abmelden", // Sign out
  "mobile.viewMyDevices": "Meine Geräte anzeigen", // View my devices
  "mobile.noDevices": "Mit diesem Konto ist noch kein Gerät angemeldet.", // No devices are signed in to this account yet.
  "mobile.online": "Online", // Online
  "mobile.offline": "Offline", // Offline
  "mobile.deviceNotSharing": "Dieses Gerät gibt noch nichts frei.", // The device is not sharing anything yet.
  "mobile.scopeMachine": "Gesamter Arbeitsbereich", // Entire workspace
  "mobile.scopeProject": "Projekt", // Project
  "mobile.scopeSession": "Sitzung", // Session
  "mobile.sharingNotReady": "Die freigegebenen Inhalte sind noch nicht bereit. Prüfen Sie die Freigabeeinstellungen auf dem betreffenden Gerät.", // Shared content is not ready yet. Check the sharing settings on that device.
  "mobile.deviceOffline": "Das Gerät ist offline. Öffnen Sie VelaTerm auf dem betreffenden Gerät und halten Sie es mit dem Netzwerk verbunden.", // The device is offline. Open VelaTerm on that device and keep it connected to the network.
  "mobile.viewShared": "Freigegebene Inhalte anzeigen →", // View shared content →
  "mobile.devicesUnavailable": "Die Geräteliste konnte nicht geladen werden. Bitte versuchen Sie es erneut.", // Could not load the device list. Please try again.
  "mobile.accountUnavailable": "Der Kontostatus konnte nicht geladen werden. Prüfen Sie Ihre Netzwerkverbindung und versuchen Sie es erneut.", // Could not load the account status. Check your network and try again.
  "mobile.signInTitle": "Bei VelaTerm anmelden", // Sign in to VelaTerm
  "mobile.signInHint": "Melden Sie sich mit E-Mail und Passwort oder einem Drittanbieter-Konto an, um Ihre Geräte und freigegebenen Inhalte zu sehen.", // Sign in with your email and password or a third-party account to see your devices and shared content.
  "mobile.signIn": "Anmelden", // Sign in
  "mobile.checkSignIn": "Anmeldestatus prüfen", // Check sign-in status
  "mobile.waitingSignIn": "Warten auf Anmeldebestätigung…", // Waiting for sign-in confirmation…
  "mobile.workspaceTitle": "Ihr Arbeitsbereich", // Your workspace
  "mobile.workspaceHint": "Verbinden Sie sich mit einem Remote-Host und arbeiten Sie an Ihren Projekten weiter.", // Connect to a remote host and pick up where you left off.
  "mobile.newSsh": "+ SSH-Verbindung", // + SSH connection
  "mobile.newUrl": "+ URL-Verbindung", // + URL connection
  "mobile.remote": "Meine Geräte", // My devices
  "mobile.scanToConnect": "QR-Code scannen", // Scan QR code to connect
  "mobile.noConnections": "Noch keine Verbindungen gespeichert. Fügen Sie eine SSH- oder URL-Verbindung hinzu oder öffnen Sie Remote, um Inhalte zu sehen, die Geräte Ihres Kontos freigeben.", // No saved connections yet. Add an SSH or URL connection, or open Remote to see content shared by devices on your account.
  "mobile.tapToConnect": "Zum Verbinden antippen →", // Tap to connect →
  "mobile.webPasswordSaved": "Zugangspasswort gespeichert", // Access password saved
  "mobile.deleteConnectionTitle": "Verbindung löschen", // Delete connection
  "mobile.deleteConnectionConfirm": (name: string) => `„${name}“ und die gespeicherten Zugangsdaten löschen? Remote-Projekte werden nicht gelöscht.`,
  "mobile.connectionMissing": "Verbindung nicht gefunden", // Connection not found
  "mobile.editConnection": "Verbindung bearbeiten", // Edit connection
  "mobile.addSshHost": "SSH-Verbindung hinzufügen", // Add SSH connection
  "mobile.addUrlConnection": "URL-Verbindung hinzufügen", // Add URL connection
  "mobile.connectionName": "Verbindungsname", // Connection name
  "mobile.serviceUrl": "Dienstadresse", // Service address
  "mobile.scanToFill": "Per QR-Code ausfüllen", // Fill in from QR code
  "mobile.openingCamera": "Kamera wird geöffnet…", // Opening the camera…
  "mobile.scanCancelled": "Scan abgebrochen", // Scan cancelled
  "mobile.scanDone": "Dienstadresse erkannt. Prüfen Sie die Adresse, dann speichern und verbinden.", // Service address detected. Check it, then save and connect.
  "mobile.scanNativeOnly": "Das Scannen von QR-Codes ist nur in der iOS- oder Android-App möglich.", // QR scanning is only available in the iOS or Android app.
  "mobile.webPasswordOptional": "Zugangspasswort (optional)", // Access password (optional)
  "mobile.keepPassword": "Leer lassen, um das Passwort zu behalten", // Leave empty to keep the current password
  "mobile.webPasswordLater": "Auch nach dem Verbinden möglich", // You can also enter it after connecting
  "mobile.webPasswordSavedHint": "Das Zugangspasswort ist gespeichert und wird beim erneuten Verbinden automatisch verwendet. Ein leeres Feld behält das gespeicherte Passwort.", // The access password is saved and used automatically when you reconnect. Leaving the field empty keeps the saved password.
  "mobile.webPasswordStorageHint": "Das Passwort wird im sicheren Speicher des Smartphones abgelegt. Sie können es auch bei der Anmeldung speichern lassen.", // The password is kept in the phone’s secure storage. You can also choose to remember it when you enter it after connecting.
  "mobile.sshHost": "SSH-Host", // SSH host
  "mobile.sshHostPlaceholder": "Hostname oder IP-Adresse", // Hostname or IP address
  "mobile.sshPort": "SSH-Port", // SSH port
  "mobile.username": "Benutzername", // Username
  "mobile.authMethod": "Authentifizierung", // Authentication
  "mobile.authPassword": "Passwort", // Password
  "mobile.authKeyAndroid": "Privater Schlüssel (OpenSSH Ed25519 / RSA)", // Private key (OpenSSH Ed25519 / RSA)
  "mobile.authKey": "Privater Schlüssel (OpenSSH Ed25519)", // Private key (OpenSSH Ed25519)
  "mobile.sshPassword": "SSH-Passwort", // SSH password
  "mobile.privateKey": "Privater Schlüssel", // Private key
  "mobile.keepPrivateKey": "Leer lassen, um den gespeicherten Schlüssel zu behalten", // Leave empty to keep the saved private key
  "mobile.pastePrivateKey": "Privaten OpenSSH-Schlüssel einfügen", // Paste an OpenSSH private key
  "mobile.passphraseOptional": "Schlüssel-Passphrase (optional)", // Key passphrase (optional)
  "mobile.keepPassphrase": "Leer lassen, um die Passphrase zu behalten", // Leave empty to keep the current passphrase
  "mobile.sshSecretSavedHint": "Die SSH-Zugangsdaten sind im sicheren Speicher des Smartphones abgelegt. Lassen Sie die Felder beim Bearbeiten leer, um sie zu behalten.", // SSH credentials are kept in the phone’s secure storage. Leave the fields empty while editing to keep them.
  "mobile.remoteService": "Remote-Dienst", // Remote service
  "mobile.serviceAuto": "VelaTerm-Dienst automatisch suchen", // Find the VelaTerm service automatically
  "mobile.serviceManual": "Port eines vorhandenen Dienstes angeben", // Use an existing service port
  "mobile.remotePort": "Loopback-HTTP-Port des Remote-Dienstes", // Remote loopback HTTP port
  "mobile.webPasswordAutoHint": "Das Zugangspasswort ist gespeichert und wird beim erneuten Verbinden automatisch verwendet.", // The access password is saved and used automatically when you reconnect.
  "mobile.prepareService": "VelaTerm-Dienst herunterladen und starten, wenn keiner verfügbar ist", // Download and start the VelaTerm service when none is available
  "mobile.prepareServiceHint": "Die automatische Vorbereitung legt ein signaturgeprüftes Programm sowie Konfiguration und Protokolldateien unter ~/.velaterm/ auf dem Remote-Host ab und hält den Dienst am Laufen. Dafür sind Python 3 und ein OpenSSL mit Ed25519-Unterstützung nötig; bei einem vorhandenen Dienst oder einem festen Port entfällt das.", // Automatic preparation writes a signature-verified binary, configuration, and logs to ~/.velaterm/ on the remote host and keeps the service running. It needs Python 3 and an OpenSSL with Ed25519 support; reusing an existing service or specifying its port does not.
  "mobile.saveConnection": "Verbindung speichern", // Save connection
  "mobile.saveAndConnect": "Speichern und verbinden", // Save and connect
  "mobile.loginOpening": "Anmeldefenster wird geöffnet…", // Opening the sign-in page in your browser…
  "mobile.loginFinishInBrowser": "Schließen Sie die Anmeldung im Browserfenster ab und kehren Sie dann zur App zurück.", // Complete the sign-in in the browser window, then return to the app.
  "mobile.loginChecking": "Anmeldestatus wird geprüft…", // Checking sign-in status…
  "mobile.loginSuccess": "Angemeldet.", // Signed in.
  "mobile.loginWaiting": "Warten auf die Anmeldebestätigung. Konto und Geräteliste werden danach automatisch aktualisiert.", // Waiting for sign-in confirmation. Your account and device list update automatically once sign-in completes.
  "mobile.loginExpired": "Die Anmeldeanfrage ist abgelaufen. Bitte melden Sie sich erneut an.", // The sign-in request has expired. Please sign in again.
  "mobile.loginRetrying": "Der Kontodienst ist vorübergehend nicht erreichbar. Es wird erneut versucht; eine neue Anmeldung ist nicht nötig.", // The account service is temporarily unreachable. Retrying. You do not need to sign in again.

  // ── Other shared components ──
  "splitter.dragToResize": "Zum Anpassen ziehen", // Drag to resize
  "transport.wsDisconnected": "WebSocket getrennt", // WebSocket disconnected
  "transport.wsConnectFailed": "WebSocket-Verbindung fehlgeschlagen", // WebSocket connection failed
  "transport.cmdFailed": "Befehl fehlgeschlagen", // Command failed
  "transport.remoteCmdForbidden": (cmd: string) =>
    `Befehl für Remote-Clients nicht verfügbar: ${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) =>
    `Einstellungsschlüssel für Remote-Clients nicht schreibbar: ${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) =>
    `Remote-Clients können nicht auf Dateien im App-Datenverzeichnis zugreifen: ${path}`, // Remote clients cannot access files in the app data directory

  // ── Crepe（WYSIWYG-Editor）──
  "crepe.placeholder": "Text eingeben oder / für das Einfügemenü drücken", // Type text, or press / for the insert menu
  "crepe.textGroup": "Text", // Text
  "crepe.paragraph": "Text", // Text
  "crepe.h1": "Überschrift 1", // Heading 1
  "crepe.h2": "Überschrift 2", // Heading 2
  "crepe.h3": "Überschrift 3", // Heading 3
  "crepe.h4": "Überschrift 4", // Heading 4
  "crepe.h5": "Überschrift 5", // Heading 5
  "crepe.h6": "Überschrift 6", // Heading 6
  "crepe.quote": "Zitat", // Quote
  "crepe.divider": "Trennlinie", // Divider
  "crepe.listGroup": "Liste", // List
  "crepe.bulletList": "Aufzählung", // Bullet List
  "crepe.orderedList": "Nummerierte Liste", // Ordered List
  "crepe.taskList": "Aufgabenliste", // Task List
  "crepe.advancedGroup": "Einfügen", // Insert
  "crepe.image": "Bild", // Image
  "crepe.codeBlock": "Codeblock", // Code Block
  "crepe.table": "Tabelle", // Table
  "crepe.math": "Formel", // Math
  "crepe.linkPlaceholder": "Link einfügen oder eingeben…", // Paste or type a link…
  "crepe.upload": "Hochladen", // Upload
  "crepe.uploadImage": "Bild hochladen", // Upload Image
  "crepe.orPasteImageLink": "oder Bildlink einfügen", // or paste an image link
  "crepe.imageCaption": "Bildunterschrift", // Image caption
  "crepe.confirm": "Bestätigen", // Confirm
  "crepe.searchLanguage": "Sprache suchen", // Search language
  "crepe.noResult": "Keine Treffer", // No results
  "crepe.edit": "Bearbeiten", // Edit
  "crepe.collapse": "Einklappen", // Collapse
  // ── Rechtes Panel / untere Leiste ──
  "info.project": "Projekt", // Project
  "info.collection": "Sammlung", // Collection
  "panel.sessionInfo": "Sitzungsinfo", // Session info
  "panel.gitTitle": "Git-Status", // Git status
  "panel.gitProbing": "Wird geprüft…", // Checking…
  "panel.gitNotRepo": "Kein Git-Repository", // Not a Git repository
  "panel.gitBranch": "Branch", // Branch
  "panel.gitStaged": "Bereitgestellt", // Staged
  "panel.gitUnstaged": "Geändert", // Changed
  "panel.gitUntracked": "Unverfolgt", // Untracked
  "bottombar.running": "Läuft", // Running
  "bottombar.collapseTasks": "Aufgaben einklappen", // Collapse tasks
  "bottombar.expandTasks": "Aufgaben ausklappen", // Expand tasks
  "bottombar.sound": "🔔 Ton", // 🔔 Sound
  "bottombar.muted": "🔕 Stumm", // 🔕 Muted
  "bottombar.overview": "Sitzungsübersicht", // Sessions overview
  "bottombar.noSessions": "Keine Sitzungen", // No sessions
  "doc.pdfFilter": "PDF-Datei", // PDF file
  // ── auto update ──
  "updater.title": "Check for Updates", // TODO translate
  "updater.upToDate": "You're already on the latest version.", // TODO translate
  "updater.failed": (err) => `Update check failed: ${err}`, // TODO translate
  "updater.available": "Update available", // TODO translate
  "updater.versionLine": (version, current) =>
    `Version ${version} — you're on ${current}`, // TODO translate
  "updater.noNotes": "No release notes were published for this version.", // TODO translate
  "updater.updateNow": "Update now", // TODO translate
  "updater.later": "Later", // TODO translate
  "updater.skipVersion": "Skip this version", // TODO translate
  "updater.skipVersionHint":
    "Stop reminding me about this version. You can still install it later from Check for Updates.", // TODO translate
  "updater.downloadingPct": (pct) => `Downloading… ${pct}%`, // TODO translate
  "updater.downloadingBytes": (mb) => `Downloading… ${mb} MB`, // TODO translate
  "updater.installing": "Installing…", // TODO translate
  "updater.installed": "Update installed. Restart to finish.", // TODO translate
  "updater.restartNow": "Restart now", // TODO translate
  "updater.retry": "Try again", // TODO translate
  "updater.downloadFailed": (err) => `Update failed: ${err}`, // TODO translate
  "updater.hide": "Hide", // TODO translate
  "updater.hideHint":
    "Keep downloading in the background. Progress stays in the status bar.", // TODO translate
  "updater.downloadManually": "Download manually", // TODO translate
  "updater.downloadManuallyHint": "Open the download page in your browser.", // TODO translate
  "updater.windowsNotice":
    "VelaTerm will close while the installer runs, then reopen on its own.", // TODO translate
  "updater.installingWindows":
    "Installing… VelaTerm is about to close. The installer will finish the update and reopen it.", // TODO translate
  "statusbar.updateAvailable": (version) => `Update ${version}`, // TODO translate
  "statusbar.updateDownloading": (pct) => `Updating… ${pct}%`, // TODO translate
  "statusbar.updateInstalling": "Installing…", // TODO translate
  "statusbar.updateReady": "Restart to update", // TODO translate
  "statusbar.updateFailed": "Update failed", // TODO translate
  "statusbar.updateTooltip": "Click for details", // TODO translate

  // ── Gesprächsansicht (die Agenten-Sitzung als Unterhaltung gelesen) ──
  "session.showConversation": "Gesprächsansicht",
  "session.showTerminal": "Terminalansicht",
  "session.switchTitle": "Der Wechsel der Ansicht startet den Agenten neu",
  "session.switchBody": "Der laufende Zug wird abgebrochen. Das Gespräch bleibt erhalten.",
  "session.switchConfirm": "Wechseln",
  "session.terminalViewHint": "Klicken Sie hier, um zur Terminalansicht zurückzukehren.",
  "session.loading": "Gespräch wird gelesen…",
  "session.unavailable": "Dieses Gespräch lässt sich noch nicht lesen",
  "session.working": "Arbeitet…",
  "session.thinking": "Überlegung",
  "session.toolRunning": "läuft",
  "session.toolUnknown": "Werkzeug",
  "session.toolFailed": "Fehlgeschlagen",
  "session.toolNoDetail": "Mehr wurde nicht aufgezeichnet",
  "session.showMore": (n: number) => `${n} weitere Zeichen anzeigen`,
  "session.showLess": "Weniger anzeigen",
  "session.composerHint": "Nachricht an den Agenten · Enter sendet, Umschalt+Enter fügt eine Zeile ein",
  "session.send": "Senden",

  // ── Gesprächs-Engine (eine über Protokoll gesteuerte Sitzung) ──
  "chat.empty": "Beginnen Sie das Gespräch über das Eingabefeld unten.",
  "chat.interrupt": "Stoppen",
  "chat.interruptTooltip": "Anhalten · Esc",
  "chat.allow": "Erlauben",
  "chat.deny": "Ablehnen",
  "chat.permissionAsk": (tool: string) => `${tool} möchte ausgeführt werden`,
  "chat.exited": (code: number) => `Der Agent hat sich beendet (Code ${code})`,
  "chat.modeNextTurn": "Ab nächster Runde",
  "chat.modePendingHint": (current: string, next: string) =>
    `Aktuelle Berechtigungen: ${current}. ${next} gilt ab der nächsten Runde; die aktuelle Runde wird unverändert fortgesetzt.`,
  "chat.modeTooltip": "Berechtigungsmodus",
  "chat.collaborationModeTooltip": "Zusammenarbeitsmodus",
  "chat.collaborationMode.default": "Standard",
  "chat.collaborationMode.defaultHint":
    "Arbeitet direkt und fragt nur, wenn eine Entscheidung erforderlich ist",
  "chat.collaborationMode.plan": "Planung",
  "chat.collaborationMode.planHint":
    "Analysiert die Aufgabe und erstellt einen Plan; Fragen können als interaktive Karten erscheinen",
  "chat.moreOptions": "Mehr",
  "chat.modelTooltip": "Modell",
  "chat.keepChoice": "Als Standard",
  "chat.keepChoiceFor": (model) => `Als Standard für ${model}`,
  "chat.followModelDefault": (agent: string) => `${agent}-Standard verwenden`,
  "chat.followModelDefaultHint": "Verwendet das in der Agentenkonfiguration festgelegte Modell.",
  "chat.savedModelDefault": "App-Standard",
  "chat.catalogWebsite": "Modellkatalog der Website",
  "chat.catalogCache": "Gespeicherter Modellkatalog",
  "chat.catalogBundled": "Mitgelieferter Modellkatalog",
  "chat.catalogChecked": (time: string) => `Zuletzt geprüft: ${time}`,
  "chat.catalogFailed": "Aktualisierung fehlgeschlagen. Der bisherige Katalog bleibt verfügbar.",
  "chat.catalogRefresh": "Aktualisieren",
  "chat.modelDefault": "Standardmodell",
  "chat.mode.default": "Immer fragen",
  "chat.mode.agentDefault": "Agent-Standard",
  "chat.mode.acceptEdits": "Änderungen annehmen",
  "chat.mode.plan": "Planmodus",
  "chat.permissionRestart.unconfirmed": "Die Verbindung wurde unterbrochen. Die Berechtigungsänderung konnte nicht bestätigt werden. Stellen Sie die Verbindung wieder her, um die aktuellen Sitzungsberechtigungen zu prüfen.",
  "permission.stateUnavailable": "Berechtigungsstatus nicht verfügbar",
  "permission.currentUnknown": "Aktuelle Berechtigungen unbestätigt",
  "permission.notRunning": "Nicht gestartet",
  "permission.applied": "Angewendet",
  "permission.nextTurn": "Gilt ab der nächsten Nachricht",
  "permission.restart": "Gilt nach dem Neustart dieser Sitzung",
  "permission.nextStart": "Beim nächsten Start",
  "permission.defaultHint": "Standardberechtigung für neu erstellte Sitzungen. Bestehende Sitzungen behalten ihre eigenen Berechtigungseinstellungen.",
  "chat.permissionRestart.title": "Neu starten und Bestätigungen überspringen?",
  "chat.permissionRestart.body": "Claude muss neu gestartet werden, um Bestätigungen zu überspringen. Die aktuelle Antwort wird unterbrochen, der Gesprächsverlauf bleibt erhalten. Nach erfolgreicher Umstellung werden keine Berechtigungen mehr abgefragt.",
  "chat.permissionRestart.confirm": "Neu starten und anwenden",
  "chat.permissionRestart.busy": "Neustart läuft…",
  "chat.permissionRestart.failed": (detail: string) => "Die Berechtigung konnte nicht geändert werden. Der bisherige Modus bleibt erhalten. " + detail,
  "chat.permissionRestart.tasks": "Verarbeiten oder entfernen Sie zuerst die Nachrichten in der Warteschlange und beenden Sie die Hintergrundaufgaben.",
  "chat.permissionRestart.stale": "Der Sitzungsprozess hat sich geändert. Wählen Sie „Ohne Nachfrage“ erneut aus.",
  "chat.permissionRestart.noHistory": "Das Gespräch kann noch nicht fortgesetzt werden. Warten Sie, bis die Initialisierung abgeschlossen ist, und versuchen Sie es erneut.",
  "chat.mode.bypassPermissions": "Ohne Nachfrage",
  "chat.mode.readOnly": "Schreibgeschützt",
  "chat.mode.fullAccess": "Vollzugriff",
  "chat.placeholder": "Nachricht an den Agenten, oder /Befehle, /Skills und @Dateien nutzen",
  "chat.command.clearDescription": "Diese Sitzung archivieren und eine neue Unterhaltung beginnen",
  "chat.command.rewindDescription": "Auswählen, was ab der letzten Benutzernachricht zurückgesetzt wird",
  "chat.command.rewindUnavailable":
    "Zum Zurücksetzen muss eine Benutzernachricht abgeschlossen sein; außerdem dürfen keine Runde, Nachrichten oder Berechtigungsanfragen offen sein.",
  "chat.effortTooltip": "Denkaufwand",
  "chat.effortDefault": "Denken",
  "chat.effort.auto": "Automatisch",
  "chat.effort.low": "Niedrig",
  "chat.effort.medium": "Mittel",
  "chat.effort.high": "Hoch",
  "chat.effort.xhigh": "Sehr hoch",
  "chat.effort.max": "Maximal",
  "chat.effort.ultra": "Extrem",
  "chat.effort.ultracode": "Ultra Code",
  "chat.agentTooltip": "Agent",
  "chat.effort.minimal": "Minimal",
  "chat.filterPlaceholder": "Filtern",
  "chat.placeholderOpencode": "Nachricht an den Agenten; /Befehle und @Dateien möglich, mit ! wird ein Shell-Befehl ausgeführt",
  "chat.command.compactDescription": "Die Unterhaltung zusammenfassen, um Kontext freizugeben",
  "chat.command.undoDescription": "Die letzte Nachricht und die dadurch geänderten Dateien zurücknehmen",
  "chat.command.redoDescription": "Das zuletzt Zurückgenommene wiederherstellen",
  "chat.command.shareDescription": "Einen Link zum Teilen dieser Unterhaltung erstellen",
  "chat.command.unshareDescription": "Diese Unterhaltung nicht mehr teilen",
  "chat.mode.auto": "Automatik",

  // ── Eine Frage des Agenten, als Formular beantwortet ──
  "chat.question.heading": "Der Agent hat eine Frage",
  "chat.question.submit": "Absenden",
  "chat.question.next": "Weiter",
  "chat.question.dismiss": "Verwerfen",
  "chat.question.answerPlaceholder": "Antwort eingeben",
  "chat.question.otherPlaceholder": "Andere Antwort",
  "chat.question.answeredHeading": (n: number) =>
    n === 1 ? "1 Frage beantwortet" : `${n} Fragen beantwortet`,
  "chat.question.blankAnswer": "Leer gelassen",

  // ── Ein Plan wartet auf Freigabe ──
  "chat.plan.heading": "Plan wartet auf Freigabe",
  "chat.plan.implement": "Freigeben und ausführen",
  "chat.plan.reject": "Ablehnen",

  // ── Nachrichten, die während der Arbeit getippt wurden ──
  "chat.placeholderBusy": "Nachricht eingeben; sie wird gesendet, sobald dieser Durchlauf endet",
  "chat.queueTooltip": (combo: string) => `Wird gesendet, sobald dieser Durchlauf endet · ${combo} sendet sofort`,
  "chat.queue.pending": "Ausstehende Nachrichten",
  "chat.queue.view": "Vollständige Nachricht anzeigen",
  "chat.queue.edit": "Bearbeiten",
  "chat.queue.remove": "Entfernen",

  // ── Bilder, die in das Eingabefeld eingefügt oder gezogen wurden ──
  "chat.attach.remove": "Dieses Bild entfernen",
  "chat.attach.tooMany": (max: number) => `Eine Nachricht kann bis zu ${max} Bilder enthalten`,
  "chat.attach.tooLarge": (name: string, mb: number) => `${name} überschreitet ${mb} MB und wurde nicht angehängt`,
  "chat.attach.unreadable": (name: string) => `${name} konnte nicht gelesen werden`,
  // Compacting the conversation… / Context compacted / Context compacted automatically
  "chat.compaction.running": "Unterhaltung wird komprimiert …",
  "chat.compaction.manual": "Kontext komprimiert",
  "chat.compaction.auto": "Kontext automatisch komprimiert",
  "chat.compaction.from": (tokens: string) => `von ${tokens} Tokens`,
  // N steps
  "chat.subagent.steps": (n: number) => (n === 1 ? "1 Schritt" : `${n} Schritte`),
  "chat.subagent.tokens": (tokens: string) => `${tokens} Token`,
  "chat.rewind.edit": "Bearbeiten",
  "chat.rewind.editSend": "Prüfen und erneut senden",
  "chat.rewind.editConfirm": "Löschen und erneut senden",
  "chat.rewind.editWarning": "Die ursprüngliche Nachricht und alle nachfolgenden Nachrichten werden dauerhaft gelöscht. Die bearbeitete Nachricht wird ab dieser Stelle gesendet. Dateiänderungen werden nicht rückgängig gemacht.",
  "chat.rewind.inactive": "Der Konversationsprozess läuft nicht. Diese Aktionen sind nach dem Start verfügbar.",
  "chat.rewind.unsupported": "Der verbundene Agent bietet diese Aktion derzeit nicht an.",
  "chat.rewind.title": "Ab hier zurückspulen",
  "chat.rewind.warning": "Diese Aktion lässt sich nicht rückgängig machen.",
  "chat.rewind.conversation": "Konversation zurückspulen",
  "chat.rewind.files": "Dateien wiederherstellen",
  "chat.rewind.both": "Konversation zurückspulen und Dateien wiederherstellen",
  "chat.rewind.confirm.conversation": "Diese Nachricht und alles danach entfernen?",
  "chat.rewind.confirm.files": "Dateien auf den Stand vor dieser Nachricht zurücksetzen?",
  "chat.rewind.confirm.both": "Diesen Durchlauf entfernen und seine Dateiänderungen zurücksetzen?",
  "chat.rewind.unavailable": "Für diese Nachricht ist kein Datei-Checkpoint vorhanden.",
  "chat.rewind.previewing": "Datei-Checkpoint wird geprüft…",
  "chat.rewind.cancel": "Unverändert lassen",
  "chat.rewind.apply": "Zurückspulen",
  "chat.rewind.applying": "Wird zurückgespult…",
  "chat.rewind.fileSummary": (files: number, insertions: number, deletions: number) =>
    `${files} ${files === 1 ? "Datei wird" : "Dateien werden"} geändert: +${insertions} −${deletions}. Diese Aktion lässt sich nicht rückgängig machen.`,
  // ── Dauerregeln, die eine Berechtigungsfrage anbietet, mit einem Klick übernommen ──
  "chat.suggest.modeSession": (mode: string) => `${mode} für diese Sitzung`,
  "chat.suggest.mode": (mode: string) => `Auf ${mode} wechseln`,
  "chat.suggest.allowSession": (rule: string) => `${rule} für diese Sitzung erlauben`,
  "chat.suggest.allowAlways": (rule: string) => `${rule} immer erlauben`,
  "chat.suggest.dirSession": (dirs: string) => `Zugriff auf ${dirs} für diese Sitzung erlauben`,
  "chat.suggest.dirAlways": (dirs: string) => `Zugriff auf ${dirs} immer erlauben`,
  // ── Codex: dauerhafte Netzwerkregeln, Einwerfen, eigene Befehle sowie die Chips für Tempo und Tonfall ──
  "chat.suggest.networkAlways": (host: string) => `Netzwerkzugriff auf ${host} immer erlauben`,
  "chat.steer": "Ergänzen",
  "chat.stopping": "Aktueller Durchlauf wird angehalten…",
  "chat.stopped": "Aktueller Durchlauf angehalten",
  "chat.steerAccepted": "Ergänzung gesendet",
  "chat.steerTooltip": (combo: string) => `${combo} fügt es dem laufenden Durchlauf hinzu`,
  "chat.command.reviewDescription": "Code prüfen und melden, was Aufmerksamkeit braucht",
  "chat.command.reviewHint": "[branch <Name> | commit <SHA> | Anweisungen]",
  "chat.command.startTimeout": "Der Agent hat seine Sitzung nicht rechtzeitig geöffnet",
  "chat.serviceTierTooltip": "Tempo",
  "chat.serviceTier.default": "Standardtempo",
  "chat.personalityTooltip": "Tonfall",
  "chat.personality.default": "Standard-Tonfall",
  "chat.personality.none": "Neutral",
  "chat.personality.friendly": "Freundlich",
  "chat.personality.pragmatic": "Pragmatisch",
  // ── Langes Gespräch: Folgen von Tool-Aufrufen stehen in einer Zeile, dazu der Weg zurück ans Ende ──
  "chat.toolRun.count": (n: number) => `${n} Tool-Aufrufe`,
  "chat.toolRun.tooltip": "Jeden Aufruf anzeigen",
  "chat.backToEnd": "Zur neuesten Nachricht",
  "chat.turnFold.hide": "Zwischenschritte ausblenden",
  "chat.turnFold.show": (n: number) => (n === 1 ? "1 Zwischenschritt einblenden" : `${n} Zwischenschritte einblenden`),
  "chat.turnFold.hideAll": "Alle Zwischenschritte ausblenden",
  "chat.turnFold.showAll": "Alle Zwischenschritte einblenden",
  "chat.elicitation.heading": (server: string) => `${server} bittet um Eingaben`,
  "chat.elicitation.cancel": "Abbrechen",
  "chat.elicitation.decline": "Ablehnen",
  "chat.elicitation.submit": "Absenden",
  "chat.elicitation.done": "Fertig",
  "chat.elicitation.choose": "Auswählen …",
  "chat.effort.off": "Aus",
  "chat.effort.offHint": "Kein erweitertes Denken",
  "chat.fastMode.label": "Schnell",
  "chat.fastMode.on": "Schnellmodus ist aktiv",
  "chat.fastMode.off": "Schnellmodus ist aus",
  "chat.auth.login": "Anmelden",
  "chat.auth.logout": "Abmelden",
  "chat.auth.confirmLogout": "Abmeldung bestätigen",
  "chat.auth.logoutConfirm": (provider: string) => `Auf diesem Host von ${provider} abmelden? Die gemeinsam verwendeten Zugangsdaten werden gelöscht. Dies betrifft auch andere Sitzungen, die diese Zugangsdaten verwenden. Der Gesprächsverlauf bleibt erhalten.`,
  "chat.auth.signingOut": "Abmeldung läuft…",
  "chat.auth.signedOut": (provider: string) => `Sie wurden von ${provider} abgemeldet. Melden Sie sich an, um diese Unterhaltung fortzusetzen.`,
  "chat.auth.logoutFailed": "Die Abmeldung konnte nicht bestätigt werden. Bitte versuchen Sie es erneut.",
  "chat.auth.wait": "Warten Sie, bis die aktuelle Aufgabe abgeschlossen ist, bevor Sie das Konto wechseln.",
  "chat.auth.title": (provider: string) => `${provider}-Konto`,
  "chat.auth.start": "Erneut anmelden",
  "chat.auth.required": (provider: string) => `Ihre ${provider}-Anmeldung ist nicht mehr gültig. Melden Sie sich erneut an, um fortzufahren.`,
  "chat.auth.starting": "Anmeldung wird vorbereitet…",
  "chat.auth.pending": "Öffnen Sie die Autorisierungsseite und geben Sie diesen Code ein. Diese Ansicht wird nach der Anmeldung automatisch aktualisiert.",
  "chat.auth.success": "Anmeldung erfolgreich. Sie können eine Nachricht senden, um diese Unterhaltung fortzusetzen.",
  "chat.auth.failed": "Die Anmeldung konnte nicht abgeschlossen werden. Versuchen Sie es erneut. Prüfen Sie, ob die Gerätecode-Authentifizierung in ChatGPT aktiviert ist und Ihre Codex CLI diese Funktion unterstützt.",
  "chat.auth.canceled": "Anmeldung abgebrochen. Sie können es jederzeit erneut versuchen.",
  "chat.auth.scope": (provider: string) => `Die Anmeldung aktualisiert das auf diesem Host verwendete ${provider}-Konto. Andere Sitzungen mit denselben Zugangsdaten verwenden ebenfalls dieses Konto.`,
  "chat.auth.canceling": "Anmeldung wird abgebrochen…",
  "chat.auth.submitting": "Autorisierungscode wird geprüft…",
  "chat.auth.claude.pending": "Öffnen Sie die Autorisierungsseite, melden Sie sich an und fügen Sie anschließend den dort angezeigten vollständigen Code ein.",
  "chat.auth.claude.failed": "Die Anmeldung konnte nicht abgeschlossen werden. Versuchen Sie es erneut und prüfen Sie, ob Ihre Claude CLI die Kontoautorisierung unterstützt.",
  "chat.auth.claude.code": "Autorisierungscode",
  "chat.auth.claude.submit": "Code senden",
  "chat.auth.claude.invalidCode": "Fügen Sie den vollständigen Code dieses Autorisierungsversuchs ein, einschließlich des Teils nach #.",
  "chat.auth.claude.externalAuth": "API-Schlüssel und andere konfigurierte Authentifizierungsmethoden bleiben unverändert.",
  "chat.auth.open": "Autorisierungsseite öffnen",
  "chat.resetCredits.label": (n: string) => `Zurücksetzungsguthaben: ${n}`,
  "chat.resetCredits.title": "Guthaben zum Zurücksetzen der Codex-Nutzungslimits",
  "chat.resetCredits.unknown": "Das Zurücksetzungsguthaben ist nicht verfügbar.",
  "chat.resetCredits.confirm": "Ein Guthaben verwenden, um berechtigte Codex-Nutzungslimits zurückzusetzen. Dies kann nicht rückgängig gemacht werden.",
  "chat.resetCredits.reset": "Nutzungslimits zurückgesetzt.",
  "chat.resetCredits.alreadyRedeemed": "Diese Anfrage wurde bereits erfolgreich ausgeführt.",
  "chat.resetCredits.nothingToReset": "Derzeit können keine Nutzungslimits zurückgesetzt werden.",
  "chat.resetCredits.noCredit": "Kein Zurücksetzungsguthaben verfügbar.",
  "chat.resetCredits.error": "Die Anfrage ist fehlgeschlagen oder das aktuelle Guthaben ist nicht verfügbar. Aktualisieren Sie das Guthaben oder wiederholen Sie die ausstehende Zurücksetzung.",
  "chat.resetCredits.busy": "Wird verarbeitet…",
  "chat.resetCredits.retry": "Zurücksetzung wiederholen",
  "chat.resetCredits.use": "Ein Guthaben verwenden",
  "chat.resetCredits.refresh": "Aktualisieren",
  "chat.usage.context": (used: string, max: string, pct: number) =>
    `Kontext: ${used} von ${max} Tokens (${pct} %)`,
  "chat.usage.cost": (usd: string) => `Sitzungskosten: $${usd}`,
  "chat.usage.rateLimited": (resets: string) => `Nutzungslimit erreicht; Zurücksetzung ${resets}`,
  "chat.usage.rateWarning": (pct: number, resets: string) =>
    `Nutzungslimit: ${pct} % verbraucht; Zurücksetzung ${resets}`,
  "chat.autoContinue.fiveHour": (time: string) => `5-Stunden-Nutzungslimit erreicht. Automatische Fortsetzung der Aufgabe: ${time}.`, // 5-hour usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.weekly": (time: string) => `Wöchentliches Nutzungslimit erreicht. Automatische Fortsetzung der Aufgabe: ${time}.`, // Weekly usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.generic": (time: string) => `Nutzungslimit erreicht. Automatische Fortsetzung der Aufgabe: ${time}.`, // Usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.unknownReset": "Nutzungslimit erreicht. Der Zeitpunkt der Zurücksetzung ist unbekannt, daher wird die Aufgabe nicht automatisch fortgesetzt.", // Usage limit reached. The reset time is unknown, so the task will not continue automatically.
  "chat.autoContinue.repeated": "Das Nutzungslimit wurde erneut erreicht. Die Aufgabe wird nicht mehr automatisch fortgesetzt.", // The usage limit was reached again. The task will no longer continue automatically.
  "chat.autoContinue.failed": "Die Aufgabe konnte nicht automatisch fortgesetzt werden. Senden Sie eine Nachricht, um fortzufahren.", // The task could not continue automatically. Send a message to continue.
  "chat.mcp.codexScope": "Dadurch wird Ihre Codex-Benutzerkonfiguration geändert. Dies betrifft auch andere Unterhaltungen, die diese Konfiguration verwenden. Fortfahren?",
  "chat.mcp.tooltip": "MCP-Server",
  "chat.mcp.loading": "Serverliste wird gelesen …",
  "chat.mcp.backendUnsupported": "Das verbundene VelaTerm-Backend unterstützt die MCP-Verwaltung nicht. Aktualisieren Sie dieses Backend, starten Sie es neu und versuchen Sie es erneut.",
  "chat.mcp.none": "Keine MCP-Server konfiguriert",
  "chat.mcp.tools": (n: number) => (n === 1 ? "1 Werkzeug" : `${n} Werkzeuge`),
  "chat.mcp.reconnect": "Neu verbinden",
  "chat.mcp.disable": "Deaktivieren",
  "chat.mcp.enable": "Aktivieren",
  "chat.mcp.status.connected": "Verbunden",
  "chat.mcp.status.disabled": "Deaktiviert",
  "chat.mcp.status.failed": "Fehlgeschlagen",
  "chat.mcp.status.pending": "Verbindung wird hergestellt",
  "chat.mcp.status.disconnected": "Getrennt",
  "chat.mcp.status.other": "Unbekannt",
  "chat.tasks.label": "Aufgaben",
  "chat.tasks.tooltip": "Hintergrundaufgaben",
  "chat.tasks.backgroundAll": "Laufende Arbeit in den Hintergrund verschieben",
  "chat.tasks.none": "Keine Hintergrundaufgaben",
  "chat.tasks.stop": "Stoppen",
  "chat.retry.line": (attempt: number, max: number, seconds: number, message: string) =>
    `Neuer Versuch (${attempt}/${max}) in ${seconds} s: ${message}`,
  "chat.notify.dismiss": "Schließen",
  "settings.completionMode": "Befehlsvorschläge",
  "settings.completionAuto": "Automatisch",
  "settings.completionTab": "Mit Tab",
  "settings.completionOff": "Aus",
  "settings.completionUnavailable": "Einstellungen konnten nicht geladen oder gespeichert werden.",
  "settings.completionHint": "Gilt für neue Zsh-, Bash-4+-, Fish- und PowerShell-Terminals. CMD behält das native Tab-Verhalten bei. Tab fügt den ausgewählten Vorschlag ein; Enter führt den aktuellen Befehl aus, ohne einen Vorschlag zu übernehmen.",


};

export default de;
