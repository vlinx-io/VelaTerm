//! German dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const de: typeof en = {
  // ── Common ──
  "common.cancel": "Abbrechen", // Cancel
  "common.confirm": "OK", // OK
  "common.delete": "Löschen", // Delete
  "common.save": "Speichern", // Save
  "common.create": "Erstellen", // Create
  "common.close": "Schließen", // Close
  "common.copy": "Kopieren", // Copy
  "common.cut": "Ausschneiden", // Cut
  "common.paste": "Einfügen", // Paste
  "common.selectAll": "Alles auswählen", // Select All
  "common.copied": "Kopiert", // Copied
  "common.retry": "Erneut versuchen", // Retry
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
  "titlebar.browser": "Integrierter Browser", // Built-in Browser
  "titlebar.remoteAccess": "Fernzugriff (Browser)", // Remote Access (Browser)
  "titlebar.connectRemote": "Mit Remote-Server verbinden", // Connect to Remote Server
  "titlebar.mirrored": "Gespiegelt", // Mirrored
  "titlebar.mirroredHint":
    "Spiegelung ist aktiv: Tabs, Splits und die aktive Sitzung folgen dem Host. Der Schalter liegt auf dem Host.", // Mirroring is on: tabs, splits, and the active session follow the host. The switch is on the host.
  "titlebar.share": "Teilen", // Share
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
  "settings.cliHint": "Fügt `vela <Projektpfad>` wie den VS-Code-Befehl `code` zum PATH hinzu.",
  "settings.agentArgsHint":
    "Standard-Startargumente für neue Sitzungen jedes Agententyps. Beim Erstellen oder Bearbeiten pro Sitzung gesetzte Argumente haben Vorrang. Leer lassen für keine.", // Agent default launch args hint
  "settings.agentPathLabel": "Pfad zur ausführbaren Datei (optional)", // Executable path (optional)
  "settings.agentPathPlaceholder": "z. B. ~/.local/bin/claude — leer = über PATH suchen", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "Wenn gesetzt, starten Sitzungen dieses Typs über diesen vollständigen Pfad, statt den Befehl im PATH zu suchen. Nützlich, wenn der Agent installiert ist, aber nicht im PATH der Shell liegt. Wird nach erfolgreicher Ein-Klick-Installation automatisch ausgefüllt, wenn der Ort erkannt wird.", // Agent executable path hint
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
  "settings.usageRefresh": "Usage refresh", // Usage refresh
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
  "spawn.title": "Start spawned session?", // Start spawned session?
  "spawn.fromSession": "From", // From
  "spawn.promptLabel": "Prompt", // Prompt
  "spawn.agentLabel": "Agent", // Agent
  "spawn.worktreeLabel": "Separate git worktree", // Separate git worktree
  "spawn.modelLabel": "Modell", // Model
  "spawn.effortLabel": "Aufwand", // Effort
  "spawn.modelDefault": "Standard", // Default
  "spawn.modelLoading": "Modelle werden geladen…", // Listing models…
  "spawn.modelListUnavailable": "Keine Modellliste verfügbar — Kennung oben eintippen", // No model list available — type an identifier above
  "spawn.launch": "Launch", // Launch
  "spawn.remaining": (n: number) => `${n} more pending`, // ${n} more pending
  "spawn.notifyTitle": "Spawn session awaiting confirmation", // Spawn session awaiting confirmation
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
  "git.commitCount": (n: number) => (n === 1 ? "1 Datei committen" : `${n} Dateien committen`),
  "git.commitNoFiles": "Keine Dateiänderungen in diesem Commit",
  "git.noCommits": "Noch keine Commits",
  "git.loadMore": "Mehr laden",
  "tree.merge": "Merge…", // TODO translate
  "tree.copyWorktreePath": "Copy worktree path",
  "tree.openWorktreeDir": "Open worktree folder",
  "tree.deleteWorktreeMenu": "Delete worktree…", // TODO translate
  "tree.deleteWorktreeTitle": "Delete worktree", // TODO translate
  "tree.deleteWorktreeBody": "Choose a worktree to remove. This deletes its working directory from disk.", // TODO translate
  "tree.deleteWorktreePlaceholder": "Select a worktree…", // TODO translate
  "tree.deleteWorktreeForce": "Force delete (discard uncommitted changes)", // TODO translate
  "tree.convertToNormalSession": "Convert to normal session", // TODO translate
  "tree.moveGroupToWorktree": "Zu Worktree verschieben…",
  "tree.convertToNormalGroup": "Convert to normal group", // TODO translate
  "merge.title": "Merge branches", // TODO translate
  "merge.desc": "Pick a source and a target branch; the source merges into the target.", // TODO translate
  "merge.notRepo": "This session's directory is not a git repository.", // TODO translate
  "merge.loadingBranches": "Loading branches…", // TODO translate
  "merge.loadingDiff": "Loading diff…", // TODO translate
  "merge.sourceLabel": "Source branch", // TODO translate
  "merge.targetLabel": "Target branch", // TODO translate
  "merge.selectBranch": "Select a branch…", // TODO translate
  "merge.swap": "Swap direction", // TODO translate
  "merge.pickHint": "Pick both branches to preview the changes this merge brings in.", // TODO translate
  "merge.changes": (target: string) => `Changes brought into "${target}"`, // TODO translate
  "merge.noChanges": "No file changes.", // TODO translate
  "merge.sameBranch": "Source and target are the same branch.", // TODO translate
  "merge.branchGone": "A selected branch no longer exists. Pick again.", // TODO translate
  "merge.upToDate": "The target branch already contains the source branch. Nothing to merge.", // TODO translate
  "merge.targetNotCheckedOut": (target: string) =>
    `Target branch "${target}" isn't checked out in any worktree, so a local merge can't run. Check it out first.`, // TODO translate
  "merge.targetDirty":
    "The target branch's working tree has uncommitted changes; the merge may be blocked.", // TODO translate
  "merge.sourceDirtyNote":
    "The source branch's working tree has uncommitted changes; they will be committed first.", // TODO translate
  "merge.commitMsgLabel": "Commit message", // TODO translate
  "merge.commitMsgPlaceholder": "Describe this change (used as the commit message)", // TODO translate
  "merge.apply": "Merge", // TODO translate
  "merge.commitAndApply": "Commit & merge", // TODO translate
  "merge.working": "Merging…", // TODO translate
  "merge.doneMsg": (source: string, target: string) => `Merged "${source}" into "${target}".`, // TODO translate
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
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
  "settings.fontUnavailable": "Auf diesem Gerät nicht installiert",
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
  "settings.notifyUnsupported": "Notifications aren't available in this environment.", // TODO translate
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
  "settings.scHint": "Klicke ein Kürzel an und drücke eine neue Kombination (Cmd/Strg erforderlich).", // hint
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
  "connect.sshFingerprintLabel": (kt: string) => `SSH-Hostschlüssel-Fingerabdruck (${kt})`,
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
  "connect.mirror": "Layout auf allen Geräten spiegeln", // Mirror layout across devices
  "connect.mirrorHint":
    "Tabs, Splits und die aktive Sitzung bleiben auf allen mit diesem Remote-Dienst verbundenen Geräten gleich. Aus = jedes Gerät behält sein eigenes Layout.", // Tabs, splits, and the active session stay the same on every device connected to this remote service. Off = each device keeps its own layout.
  "connect.shareDesktopDb": "Datenbank der Remote-Desktop-App mitnutzen",
  "connect.shareDesktopDbHint":
    "Teilt sich eine Datenbank mit der Desktop-App des Remote-Rechners (am besten bei gleicher Version). Aus = eigene Datenbank.",

  // ── Sidebar ──
  "tree.newSession": "Neue Sitzung", // New Session
  "tree.newTerminalSession": "Neue Terminal-Sitzung", // New Terminal Session
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
  "info.branch": "Branch", // Branch
  "info.path": "Pfad", // Path
  "info.recentCommits": "Letzte Commits", // Recent Commits
  "info.noCommits": "Keine Commits", // No commits
  "tree.killProcess": "Prozess beenden", // Kill Process
  "tree.archiveSession": "Sitzung archivieren", // Archive Session
  "tree.archiveGroup": "Gruppe archivieren", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "temp", // scratch
  "tree.persistSession": "In dauerhafte Sitzung umwandeln…", // Make Permanent Session…
  "tree.persistDoc": "Auf Datenträger speichern…", // Save to Disk…
  "tree.closeScratch": "Entwurf schließen", // Close Scratch
  "tree.importProject": "Projekt importieren", // Import Project
  "tree.createProject": "Projekt erstellen",
  "tree.cloneProject": "Von Git klonen", // Clone from Git
  "createProject.title": "Projekt erstellen",
  "createProject.name": "Projektname",
  "createProject.namePlaceholder": "mein-projekt",
  "createProject.into": "Erstellen in",
  "createProject.choose": "Auswählen…",
  "createProject.noParent": "Übergeordneten Ordner auswählen",
  "createProject.invalidName": "Geben Sie einen einzelnen Ordnernamen ohne / oder \\ ein.",
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
  "clone.slowHint": "Seit 30 Sekunden kein Fortschritt. Prüfen Sie Netzwerk oder Proxy des Remote-Rechners; Sie können abbrechen und erneut versuchen.",
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
  "tree.noProjectsPre": "Noch keine Projekte. Klicken Sie auf das Ordnersymbol oder drücken Sie ", // No projects yet. Click the folder button, or press
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
  "tree.agentArgsLabel": "Startargumente (optional)", // Launch args (optional)
  "preset.execPathLabel": "Programmdatei (optional)",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint": "Leer lassen, um den konfigurierten Befehl des Agenten zu verwenden. Mit Angabe läuft nur diese Sitzung mit einem kompatiblen Ersatz.",
  "preset.saveLabel": "Als Vorlage speichern",
  "preset.namePlaceholder": "Vorlage benennen",
  "preset.iconChoose": "Symbol wählen",
  "preset.iconClear": "Entfernen",
  "preset.iconHint": "Quadratische Bilder eignen sich am besten; andere werden zugeschnitten und auf 64x64 skaliert.",
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
  "resume.title": "Sitzung fortsetzen", // Resume Session
  "resume.desc":
    "Agententyp wählen und die eigene Session-ID des Agenten eingeben; beim Öffnen wird das ursprüngliche Gespräch fortgesetzt.", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "Agententyp", // Agent type
  "resume.sessionIdPlaceholder": "Session-ID des Gesprächs", // Conversation session id
  "resume.confirm": "Fortsetzen & öffnen", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "Neue Worktree-Sitzung…", // New Worktree Session…
  "worktree.worktreeNameLabel": "Worktree-Name", // Worktree name
  "worktree.worktreeNameHint": "Wird als Worktree-Verzeichnis und Branch-Name verwendet.", // Used as the worktree directory and branch name.
  "worktree.createFailed": "Worktree konnte nicht erstellt werden", // Couldn't create the worktree
  "worktree.noRepoRoot": "Dieses Projekt hat keinen nutzbaren Git-Repository-Pfad.", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "Keiner", // None
  "worktreeSel.modeNew": "Neu", // New
  "worktreeSel.modeExisting": "Vorhanden", // Existing
  "worktreeSel.loading": "Worktrees werden geladen…", // Loading worktrees…
  "worktreeSel.empty": "Keine vorhandenen Worktrees in diesem Repository.", // No existing worktrees in this repository.
  "worktreeSel.loadFailed": "Worktrees konnten nicht aufgelistet werden (kein Git-Repository?).", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint": "In dieser Gruppe erstellte Sitzungen verwenden standardmäßig dieses Worktree.", // Sessions created in this group will use this worktree by default.
  "worktree.moveGroupTitle": "Gruppe zu Worktree verschieben",
  "worktree.moveGroupHint": "Ab jetzt in dieser Gruppe erstellte Sitzungen verwenden dieses Worktree. Bereits vorhandene behalten ihr aktuelles Verzeichnis.",

  // ── Archive panel ──
  "archive.title": "Archivierte Sitzungen", // Archived Sessions
  "archive.empty1": "Keine archivierten Sitzungen.", // No archived sessions.
  "archive.empty2":
    "Rechtsklick auf eine Sitzung in der Seitenleiste und „Sitzung archivieren“ wählen, um sie hier abzulegen.", // Right-click a session in the sidebar…
  "archive.restore": "Als normale Sitzung wiederherstellen", // Restore to normal session
  "archive.export": "Vollständigen Kontext als Markdown exportieren", // Export full context as Markdown
  "archive.deleteForever": "Endgültig löschen (samt Aufzeichnung)", // Delete permanently (with recording)
  "archive.pickOne": "Links eine archivierte Sitzung wählen, um das Transkript zu sehen", // Select an archived session on the left…
  "archive.recordingEnd": "--- Ende der Aufzeichnung ---", // --- End of recording ---
  "archive.readRecordingFailed": (err) => `Aufzeichnung konnte nicht gelesen werden: ${err}`, // Failed to read recording: {err}
  "archive.searchRecording": "In Aufzeichnung suchen…", // Search in recording…
  "archive.searchTranscript": "Transkript durchsuchen…", // Search transcript…
  "archive.searchPlaceholder": "Archivierte Inhalte durchsuchen…", // Search archived content…
  "archive.msgCountAll": (n) => (n === 1 ? "1 Nachricht" : `${n} Nachrichten`), // {n} messages
  "archive.msgCountFiltered": (shown, total) => `${shown} / ${total} Nachrichten`, // {shown} / {total} messages
  "archive.you": "Du", // You
  "archive.toolsUsed": (tools) => `Werkzeuge: ${tools}`, // Tools: {tools}
  "archive.noMatch": "Keine passenden Nachrichten", // No matching messages
  "archive.emptyTranscript": "Transkript ist leer", // Transcript is empty
  "archive.loadingTranscript": "Transkript wird geladen…", // Loading transcript…

  // ── Global session-content search ──
  "search.allPlaceholder": "Gesamten Sitzungsinhalt durchsuchen…", // Search across all session content…
  "search.hint": "Durchsuchen Sie Sitzungsinhalte. Archivierte Sitzungen sind standardmäßig ausgeschlossen — „Archivierte einbeziehen“ aktivieren, um sie hinzuzufügen.", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "Archivierte einbeziehen", // Include archived
  "search.includeArchivedHint": "Auch archivierte Sitzungen durchsuchen (standardmäßig aus)", // Also search archived sessions (off by default)
  "search.searching": "Suche…", // Searching…
  "search.noResults": "Keine Treffer gefunden", // No matches found
  "search.sessionCount": (n) => (n === 1 ? "1 Sitzung" : `${n} Sitzungen`), // n sessions
  "search.matchCount": (n) => (n === 1 ? "1 Treffer" : `${n} Treffer`), // n matches
  "search.pickSession": "Wählen Sie links eine Sitzung, um ihre Treffer zu sehen", // Select a session on the left to see its matches
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
  "browser.desktopOnly": "Browser-Tabs lassen sich nur in der Desktop-App öffnen.", // Browser tabs open in the desktop app only.
  "browser.stop": "Laden abbrechen", // Stop loading
  "browser.openExternal": "Im System-Browser öffnen", // Open in system browser
  "browser.addressPlaceholder": "URL oder Suchbegriffe eingeben", // Enter URL or search terms
  "browser.quickAccess": "Schnellzugriff", // Quick access
  "browser.loading": "Lädt…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "VelaTerm beenden?",  // Quit VelaTerm?
  "quit.body": "Alle laufenden Terminal- und Agent-Sitzungen werden beendet.",  // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "Arbeitsbereich speichern",  // Save workspace
  "quit.saveWorkspaceHint": "Beim nächsten Start dieselben Tabs und Teilungen öffnen. Terminals werden wiederhergestellt, aber nicht neu gestartet.",  // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "Beenden",  // Quit
  "dormant.body": "Aus dem gespeicherten Arbeitsbereich wiederhergestellt. Es läuft noch kein Prozess.",  // Restored from your saved workspace. No process is running yet.
  "dormant.start": "Starten",  // Start
  "overlimit.title": (max) => `Hintergrund-Limit überschritten (${max})`, // Background keep-alive over limit ({max})
  "overlimit.body": "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
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
  "agentInstall.pathSaved": (label: string) => `Pfad zur ausführbaren Datei von ${label} in den Einstellungen gespeichert:`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} ist installiert`, // {label} is installed
  "agentInstall.doneDesc": "Starten Sie diese Sitzung neu, um loszulegen.", // Relaunch this session to start using it.
  "agentInstall.restartNow": "Jetzt neu starten", // Relaunch now
  "agentInstall.later": "Später", // Later
  "search.placeholder": "Im Terminal suchen", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "WYSIWYG", // WYSIWYG
  "doc.source": "Quelltext", // Source
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
  "doc.overwriteConfirm": "Eine Datei mit diesem Namen existiert bereits. Zum Ersetzen auf „Überschreiben“ klicken.", // A file with this name already exists. Click "Overwrite" to replace it.
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
  "doc.imgDecodeFailed": "Dieses Bild kann nicht angezeigt werden (nicht unterstütztes oder beschädigtes Format).", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "Einpassen", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "Als PDF exportieren", // Export PDF
  "doc.diagramError": "Diagrammfehler", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "Keine Sitzung ausgewählt", // No session selected
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
  "files.deleteConfirm": (name) => `„${name}" löschen? Dies kann nicht rückgängig gemacht werden.`, // Delete "{name}"? This can't be undone.

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
  "statusbar.bgEvicted": (name) => `Hintergrund-Tab beendet: ${name} (Limit überschritten)`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) => `Browser-Fernzugriff aktiviert: ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "Rechte: Fragen", // Perms: Ask
  "statusbar.permSkip": "Rechte: Überspringen", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip": "Berechtigungsmodus dieser Sitzung · zum Ändern klicken (nur diese Sitzung)", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "Berechtigungen dieser Sitzung", // This session's permissions
  "statusbar.permOptAsk": "Jedes Mal fragen (Standard)", // Ask each time (default)
  "statusbar.permScopeHint": "Gilt nur für diese Sitzung. Für globale Standardwerte gehen Sie zu Einstellungen ▸ Agenten.", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg": "Berechtigung geändert. Die Sitzung muss neu gestartet werden, damit dies wirkt. Der Neustart setzt das aktuelle Gespräch fort, unterbricht aber laufende Aufgaben. Jetzt neu starten?", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "Jetzt neu starten", // Restart now
  "statusbar.permRestartLater": "Später", // Later
  "statusbar.permScopeTitle": "Anwenden auf?", // Apply to?
  "statusbar.permScopeSession": "Nur diese Sitzung", // This session only
  "statusbar.permScopeGlobal": "Globaler Standard", // Global default
  "statusbar.permScopeGlobalHint": "Gilt jetzt für diese Sitzung und wird zum Standard für künftige neue Sitzungen dieser Art (mit Einstellungen synchronisiert).", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

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
  "transport.imgUploadHttp": (status) => `Bild-Upload fehlgeschlagen (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.connecting": "Verbinde…", // Connecting…
  "login.remoteAccess": "Fernzugriff", // Remote Access
  "login.desc": "Geben Sie das Zugangspasswort ein, um sich mit diesem Terminal zu verbinden.", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "Zugangspasswort", // Access password
  "login.connect": "Verbinden", // Connect
  "login.wrongPassword": "Falsches Passwort", // Wrong password
  "login.rateLimited": "Zu viele Versuche. Bitte eine Minute warten und erneut versuchen.", // Too many attempts. Please wait a minute and try again.
  "login.failed": "Anmeldung fehlgeschlagen, bitte erneut versuchen", // Login failed, please try again
  "login.pairingRequired": "Dieser Server erfordert einen Kopplungslink. Öffnen Sie den im Fernzugriff-Bereich der Desktop-App erzeugten Link.", // This server requires a pairing link
  "login.authFailed": "Authentifizierung fehlgeschlagen. Prüfen Sie das Zugangspasswort, oder öffnen Sie einen neuen Kopplungslink, falls der Link neu erstellt wurde.", // Authentication failed, check password or use a new pairing link
  "dir.title": "Projektverzeichnis wählen", // Choose Project Directory
  "dir.pathPlaceholder": "Suchen oder Pfad eingeben und Enter drücken (unterstützt ~)", // Search, or type a path and press Enter (supports ~)
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
  "conn.sshReconnecting": "SSH-Verbindung unterbrochen, Tunnel wird neu aufgebaut…", // SSH link lost, rebuilding the tunnel…
  "conn.sshDown": "SSH-Verbindung getrennt — zum erneuten Versuch „Jetzt neu verbinden“ drücken", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "Anfrage fehlgeschlagen", // Request failed
  "reqerr.dismiss": "Schließen", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "Fehlerprotokoll", // Error Log
  "errlog.empty": "Keine Fehler aufgezeichnet.", // No errors recorded.
  "errlog.copyAll": "Alle kopieren", // Copy all
  "errlog.clear": "Löschen", // Clear
  "errlog.close": "Schließen", // Close

  // ── Mobile ──
  "mobile.toDesktop": "Zur Desktop-Version wechseln", // Switch to desktop
  "mobile.empty1": "Keine Sitzungen.", // No sessions.
  "mobile.noMatch": "Keine passenden Sitzungen", // No matching sessions
  "mobile.empty2":
    "Erstellen Sie eine in der Desktop-App oder im Computer-Browser, sie erscheint hier automatisch.", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ Zurück", // ‹ Back
  "mobile.selCopy": "Kopieren", // Copy
  "mobile.selCancel": "Abbrechen", // Cancel

  // ── Other shared components ──
  "splitter.dragToResize": "Zum Anpassen ziehen", // Drag to resize
  "transport.wsDisconnected": "WebSocket getrennt", // WebSocket disconnected
  "transport.wsConnectFailed": "WebSocket-Verbindung fehlgeschlagen", // WebSocket connection failed
  "transport.cmdFailed": "Befehl fehlgeschlagen", // Command failed
  "transport.remoteCmdForbidden": (cmd: string) => `Befehl für Remote-Clients nicht verfügbar: ${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) => `Einstellungsschlüssel für Remote-Clients nicht schreibbar: ${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) => `Remote-Clients können nicht auf Dateien im App-Datenverzeichnis zugreifen: ${path}`, // Remote clients cannot access files in the app data directory

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
  "updater.hideHint": "Keep downloading in the background. Progress stays in the status bar.", // TODO translate
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
};

export default de;
