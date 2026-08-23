//! French dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const fr: typeof en = {
  // ── Common ──
  "common.cancel": "Annuler", // Cancel
  "common.confirm": "OK", // OK
  "common.delete": "Supprimer", // Delete
  "common.save": "Enregistrer", // Save
  "common.create": "Créer", // Create
  "common.close": "Fermer", // Close
  "common.copy": "Copier", // Copy
  "common.cut": "Couper", // Cut
  "common.paste": "Coller", // Paste
  "common.selectAll": "Tout sélectionner", // Select All
  "common.copied": "Copié", // Copied
  "common.retry": "Réessayer", // Retry
  "common.refresh": "Actualiser", // Refresh
  "common.loading": "Chargement…", // Loading…
  "common.prev": "Précédent", // Previous
  "common.next": "Suivant", // Next
  "common.on": "Activé", // On
  "common.off": "Désactivé", // Off
  "common.gotIt": "Compris", // Got it
  "common.rename": "Renommer", // Rename
  "common.edit": "Modifier", // Edit
  "common.open": "Ouvrir", // Open
  "common.session": "Session", // Session

  // ── Session types and status ──
  "kind.terminal": "Terminal", // Terminal
  "kind.browser": "Navigateur", // Browser
  "status.idle": "Inactif", // Idle
  "status.running": "En cours", // Running
  "status.exited": "Terminé", // Exited
  "status.error": "Erreur", // Error
  "status.working": "En traitement", // Working
  "status.asking": "Confirmation requise", // Needs confirmation
  "status.waiting": "Consulté", // Viewed
  "status.unavailable": "État indisponible",
  "indicator.unread": "Non lu · à consulter", // Unread · awaiting review

  // ── Title bar ──
  "titlebar.builtAt": (time) => `Compilé le ${time}`, // Built at {time}
  "titlebar.versionMismatch": (frontend, backend) =>
    `Versions incohérentes : frontend v${frontend} ≠ backend v${backend} — recompilez ou redéployez de façon synchronisée.`, // Version mismatch

  "titlebar.hotReloadedAt": (time) => `Rechargement à chaud à ${time}`, // Hot reloaded at {time}
  "titlebar.themeSystem": (resolved) => `Suivre le système (actuellement ${resolved})`, // Follow system (currently {resolved})
  "titlebar.themeDark": "Sombre", // Dark
  "titlebar.themeLight": "Clair", // Light
  "titlebar.browser": "Navigateur intégré", // Built-in Browser
  "titlebar.remoteAccess": "Accès distant (navigateur)", // Remote Access (Browser)
  "titlebar.connectRemote": "Se connecter à un serveur distant", // Connect to Remote Server
  "titlebar.share": "Partager", // Share
  "share.title": "Partager VelaTerm", // Share VelaTerm
  "share.subtitle":
    "Nous sommes une petite équipe derrière VelaTerm. Si vous l’appréciez, partagez VelaTerm autour de vous. Nous aider à nous faire connaître compte énormément pour notre équipe. Merci pour votre soutien ! ❤️", // We're a small team behind VelaTerm. If you enjoy it, please share VelaTerm with others…
  "share.copyLink": "Copier le lien", // Copy link
  "share.copied": "Copié !", // Copied!
  "share.wechatMoments": "Moments WeChat",
  "share.weibo": "Weibo",
  "share.xiaohongshu": "Xiaohongshu",
  "share.xiaohongshuAction":
    "Copier le texte et le lien, puis ouvrir le Centre des créateurs Xiaohongshu",
  "share.wechatQrTitle": "Partager dans les Moments WeChat",
  "share.wechatQrHint":
    "Scannez le code avec WeChat, ouvrez le lien, puis choisissez de le partager dans les Moments.",
  "share.backToPlatforms": "Retour aux options de partage",
  "titlebar.appearance": "Apparence", // Appearance
  "titlebar.showLeft": "Afficher la barre latérale", // Show sidebar
  "titlebar.hideLeft": "Masquer la barre latérale", // Hide sidebar
  "titlebar.showRight": "Afficher le panneau d'infos", // Show info panel
  "titlebar.hideRight": "Masquer le panneau d'infos", // Hide info panel

  // ── Settings ──
  "settings.title": "Paramètres", // Settings
  "settings.catTerminal": "Terminal", // Terminal
  "settings.catBehavior": "Comportement", // Behavior
  "settings.catAgents": "Agents", // Agents
  "settings.permDefault": "Défaut", // Default
  "settings.permYolo": "YOLO", // YOLO
  "settings.yoloHint": (flag: string) =>
    `Lance avec ${flag}. Ignore toutes les confirmations de permission — à utiliser avec prudence.`, // YOLO flag hint
  "settings.permViaEnvHint":
    "Ignore toutes les confirmations de permission via injection de config (pas de flag CLI). S'applique au lancement de cette session.",
  "settings.catGeneral": "Général", // General
  "settings.cliLabel": "Commande shell",
  "settings.cliInstall": "Installer la commande ‘vela’",
  "settings.cliUninstall": "Désinstaller la commande ‘vela’",
  "settings.cliInstalledAt": (path: string) => `Installée dans ${path}`,
  "settings.cliConflict": (path: string) =>
    `Une autre commande ‘vela’ existe déjà dans ${path}. VelaTerm ne la remplacera pas.`,
  "settings.cliHint": "Ajoute `vela <chemin-du-projet>` au PATH, comme la commande `code` de VS Code.",
  "settings.agentArgsHint":
    "Arguments de lancement par défaut appliqués aux nouvelles sessions de chaque type d'agent. Les arguments définis par session lors de la création ou de la modification les remplacent. Laisser vide pour aucun.", // Agent default launch args hint
  "settings.agentPathLabel": "Chemin de l'exécutable (facultatif)", // Executable path (optional)
  "settings.agentPathPlaceholder": "ex. ~/.local/bin/claude — vide = recherche dans le PATH", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "Si défini, les sessions de ce type se lancent via ce chemin complet au lieu de chercher la commande dans le PATH. Utile quand l'agent est installé mais absent du PATH du shell. Rempli automatiquement après une installation en un clic si l'emplacement est détecté.", // Agent executable path hint
  "settings.appearance": "Apparence", // Appearance
  "settings.accent": "Accent", // Accent
  "settings.accentAuto": "Suivre le thème", // Follow theme
  "settings.density": "Densité", // Density
  "settings.densityCompact": "Compacte", // Compact
  "settings.densityRegular": "Normale", // Regular
  "settings.densityComfy": "Aérée", // Comfy
  "settings.pane": "Volets", // Panes
  "settings.paneFlush": "Sans bord", // Flush
  "settings.paneCard": "Carte", // Card
  "settings.divider": "Séparateur", // Divider
  "settings.dividerSubtle": "Fin", // Subtle
  "settings.dividerVisible": "Visible", // Visible
  "settings.nav": "Barre latérale", // Sidebar
  "settings.navTree": "Arbre", // Tree
  "settings.navCompact": "Compacte", // Compact
  "settings.tabs": "Onglets", // Tabs
  "settings.dynamicStatusFilter": "Ajout dynamique au filtre d’état",
  "settings.tabSingle": "Unique", // Single
  "settings.tabMulti": "Multiples", // Multi
  "settings.maxLiveTabs": "Background limit", // Background limit
  "settings.defaultShell": "Shell par défaut", // Default shell
  "settings.spawnConfirm": "Confirm before spawn", // Confirm before spawn
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.cleanImages": "Nettoyer automatiquement les images collées",
  "settings.cleanImagesHint":
    "Les images collées ou déposées dans le terminal sont d'abord enregistrées comme fichiers temporaires (le chemin est envoyé à l'agent). Si activé, les fichiers temporaires de cette session sont supprimés à la fermeture, et les restes de plus de 24 h sont nettoyés au démarrage. Les images des documents ne sont jamais touchées.",
  "settings.cleanImagesNow": "Nettoyer maintenant",
  "settings.cleanImagesResult": (n: number, size: string) =>
    `${n} image(s) temporaire(s) nettoyée(s) (${size} libéré).`,
  "settings.cleanImagesEmpty": "Aucune image temporaire à nettoyer.",
  "settings.imagePasteMode": "Collage d'image",
  "settings.imagePasteUpload": "Coller le chemin",
  "settings.imagePasteAgent": "Collage natif",
  "settings.imagePasteHint":
    "Choisissez ce qui est inséré lors du collage d'une image (bureau local uniquement). Coller le chemin : l'image est enregistrée temporairement et son chemin est inséré dans Claude ou Codex. Collage natif : Claude ou Codex lit le presse-papiers système et affiche son propre repère d'image.",
  "settings.imagePasteRemoteHint":
    "Les sessions distantes collent toujours le chemin du fichier afin que l'agent puisse lire l'image sur sa machine. Le collage natif est réservé au bureau local.",
  "spawn.title": "Start spawned session?", // Start spawned session?
  "spawn.fromSession": "From", // From
  "spawn.promptLabel": "Prompt", // Prompt
  "spawn.agentLabel": "Agent", // Agent
  "spawn.worktreeLabel": "Separate git worktree", // Separate git worktree
  "spawn.modelLabel": "Modèle", // Model
  "spawn.effortLabel": "Effort", // Effort
  "spawn.launch": "Launch", // Launch
  "spawn.remaining": (n: number) => `${n} more pending`, // ${n} more pending
  "spawn.notifyTitle": "Spawn session awaiting confirmation", // Spawn session awaiting confirmation
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "Voir les modifications…",
  "changes.title": "Modifications",
  "changes.loading": "Chargement…",
  "changes.loadingDiff": "Chargement du diff…",
  "changes.noChanges": "Aucune modification",
  "changes.refresh": "Actualiser",
  "changes.notRepo": "Pas un dépôt git",
  "changes.selectFile": "Sélectionnez un fichier",
  "changes.binary": "Fichier binaire — diff par ligne indisponible",
  "changes.commitTitle": (hash: string) => `Commit ${hash}`,

  "git.staged": "Indexé",
  "git.changes": "Modifications",
  "git.untracked": "Fichiers non suivis",
  "git.committed": "Modifications validées",
  "git.stage": "Indexer",
  "git.unstage": "Désindexer",
  "git.stageAll": "Tout indexer",
  "git.unstageAll": "Tout désindexer",
  "git.discard": "Abandonner",
  "git.deleteFile": "Supprimer",
  "git.viewAll": "Tout afficher",
  "git.detached": "(détaché)",
  "git.aheadBehind": "Commits en avance et en retard sur la branche amont",
  "git.commitPlaceholder": "Message de commit",
  "git.amend": "Modifier le dernier commit",
  "git.amendCommit": "Modifier le commit",
  "git.commitCount": (n: number) => (n === 1 ? "Valider 1 fichier" : `Valider ${n} fichiers`),
  "git.commitNoFiles": "Aucun changement de fichier dans ce commit",
  "git.noCommits": "Aucun commit",
  "git.loadMore": "Charger plus",
  "tree.merge": "Merge…", // TODO translate
  "tree.copyWorktreePath": "Copy worktree path",
  "tree.openWorktreeDir": "Open worktree folder",
  "tree.deleteWorktreeMenu": "Delete worktree…", // TODO translate
  "tree.deleteWorktreeTitle": "Delete worktree", // TODO translate
  "tree.deleteWorktreeBody": "Choose a worktree to remove. This deletes its working directory from disk.", // TODO translate
  "tree.deleteWorktreePlaceholder": "Select a worktree…", // TODO translate
  "tree.deleteWorktreeForce": "Force delete (discard uncommitted changes)", // TODO translate
  "tree.convertToNormalSession": "Convert to normal session", // TODO translate
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
  "settings.renderer": "Moteur de rendu du terminal", // Terminal renderer
  "settings.redrawOnReveal": "Redessiner au changement d'onglet", // Redraw on tab switch
  "settings.catAdvanced": "Avancé", // Advanced
  "settings.outputScheduler": "Sortie prioritaire au premier plan", // Foreground-priority output
  "settings.recordSessions": "Enregistrer les journaux de session", // Record session logs
  "settings.recordSessionsHint":
    "Désactivé par défaut. Une fois activé, la sortie du terminal est enregistrée dans un fichier journal pour la relecture d'archive et la recherche. Les sessions de terminal simples ne sont jamais enregistrées ; les sessions d'agent lisent leur propre transcription.", // Record session logs hint
  "settings.fonts": "Fonts", // TODO translate
  "settings.uiFont": "Interface font", // TODO translate
  "settings.uiFontSize": "Interface size", // TODO translate
  "settings.termFont": "Terminal font", // TODO translate
  "settings.termFontSize": "Terminal size", // TODO translate
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
  "settings.fontUnavailable": "Non installée sur cet appareil",
  "settings.fontAuto": "Auto", // TODO translate
  "settings.fontSmaller": "Smaller", // TODO translate
  "settings.fontLarger": "Larger", // TODO translate
  "settings.fontReset": "Reset", // TODO translate
  "settings.sound": "Son de notification", // Notification sound
  "settings.language": "Langue", // Language
  "settings.langAuto": "Auto (système)", // Auto (system)
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
  "settings.catShortcuts": "Raccourcis", // Shortcuts
  "settings.scOpenProject": "Ouvrir un projet", // Open project
  "settings.scNewTab": "Nouveau terminal", // New terminal
  "settings.scNewBrowserTab": "Nouvel onglet navigateur", // New browser tab
  "settings.scClosePane": "Fermer le volet / l'onglet", // Close pane / tab
  "settings.scSplitRight": "Diviser à droite", // Split right
  "settings.scSplitDown": "Diviser en bas", // Split down
  "settings.scSearch": "Rechercher dans le terminal", // Find in terminal
  "settings.scGlobalSearch": "Rechercher dans toutes les sessions", // Search all sessions
  "settings.scSaveDoc": "Enregistrer le document", // Save document
  "settings.scRecording": "Appuyez sur les touches…", // Press keys…
  "settings.scHint": "Cliquez sur un raccourci, puis appuyez sur une nouvelle combinaison (Cmd/Ctrl requis).", // hint
  "settings.scReset": "Rétablir les valeurs par défaut", // Restore defaults
  "settings.scConflict": (label: string) => `Déjà utilisé par « ${label} »`, // conflict

  // ── Remote access panel ──
  "remote.title": "Accès distant (navigateur)", // Remote Access (Browser)
  "remote.desc":
    "Une fois activé, les appareils du même réseau local peuvent ouvrir l'adresse ci-dessous dans un navigateur, saisir le mot de passe et obtenir la même interface que le bureau.", // Once enabled, devices on the same LAN…
  "remote.needPassword": "Veuillez d'abord définir un mot de passe d'accès", // Please set an access password first
  "remote.running": (port) => `En cours · port ${port}`, // Running · port {port}
  "remote.urlsHint":
    "Ouvrez l'adresse sur le même WiFi / sous-réseau que votre appareil (en cas de plusieurs interfaces réseau, choisissez la bonne ; les adresses VPN/tunnel sont listées en dernier et sont généralement inaccessibles depuis d'autres appareils) :", // Open the address on the same WiFi / subnet…
  "remote.copyUrl": "Cliquer pour copier l'adresse", // Click to copy address
  "remote.moreUrls": (n: number) => `${n} autre${n > 1 ? "s" : ""} lien${n > 1 ? "s" : ""}`, // N more urls
  "remote.lessUrls": "Réduire", // Show less
  "remote.stop": "Arrêter le serveur", // Stop Server
  "remote.passwordPlaceholder": "Définir le mot de passe d'accès", // Set access password
  "remote.starting": "Démarrage…", // Starting…
  "remote.start": "Démarrer le serveur", // Start Server
  "remote.portLabel": "Port", // Port
  "remote.portInvalid": "Le port doit être compris entre 1 et 65535", // Port must be between 1 and 65535
  "remote.ipLabel": "Adresse IP", // IP address
  "remote.ipAuto": "Automatique (première adresse LAN)", // Automatic (first LAN address)
  "remote.ipVpn": "VPN", // VPN
  "remote.qrHint":
    "Scannez avec votre téléphone pour ouvrir le lien d'appairage sur l'adresse sélectionnée.", // Scan with your phone to open the pairing link on the selected address.
  "remote.fingerprintLabel": "Empreinte du certificat (SHA-256)", // Certificate fingerprint (SHA-256)
  "remote.fingerprintHint":
    "Lors de la première connexion, les navigateurs signalent un certificat non approuvé — normal pour un certificat auto-signé. Comparez cette empreinte pour confirmer qu'il s'agit de cette machine.", // On first connect, browsers warn the certificate is untrusted…

  "remote.pairingCreate": "Créer un lien d'appairage", // Create pairing link
  "remote.pairingRegenerate": "Régénérer le lien (déconnecte tout)", // Regenerate link (disconnects all)
  "remote.pairingCreating": "Génération…", // Generating…
  "remote.pairingHint":
    "Ouvrez-le dans un navigateur puis saisissez le mot de passe. Ce lien contient des identifiants d'accès ; ne le partagez qu'avec vos appareils.", // Open in a browser, then enter the password…

  "remote.devicesLabel": "Appareils appairés", // Paired devices
  "remote.lastSeen": "Dernière connexion", // Last seen
  "remote.revoke": "Révoquer", // Revoke
  "remote.deviceBlock": "Bloquer", // Block
  "remote.deviceBlockConfirm": "Confirmer le blocage", // Confirm block
  "remote.deviceBlockHint":
    "Les appareils bloqués sont déconnectés et ne peuvent pas se reconnecter (un nouveau lien d'appairage est nécessaire). Les autres appareils ne sont pas affectés.", // Block hint
  "remote.devicesEmpty": "Aucun appareil appairé", // No paired devices yet
  "remote.autoRestartHint":
    "L'accès distant redémarre automatiquement à la réouverture de l'application. « Arrêter le serveur » le désactive.", // Remote access restarts automatically when the app is reopened. Stop Server turns this off.
  "remote.autostartFailed": "Échec du démarrage automatique :", // Automatic start failed:
  "remote.mirror": "Refléter la disposition sur tous les appareils", // Mirror layout across devices
  "remote.mirrorHint":
    "Les onglets, les divisions et la session active restent identiques sur tous les appareils connectés. Le focus clavier ne bouge pas.", // Tabs, splits, and the active session stay the same on every connected device. Keyboard focus stays put on each one.

  // ── Remote connection panel ──
  "connect.title": "Se connecter à un serveur distant", // Connect to Remote Server
  "connect.pairingPlaceholder": "Coller le lien d'appairage", // Paste pairing link
  "connect.confirmConnect": "Empreinte correcte, connecter", // Fingerprint matches, connect
  "connect.desc":
    "Saisissez l'adresse et le mot de passe d'un VelaTerm distant pour vous y connecter et le contrôler dans une nouvelle fenêtre.", // Enter the address and password…
  "connect.addressPlaceholder": "Adresse IP, ex. 192.168.1.100", // IP address, e.g. 192.168.1.100
  "connect.portPlaceholder": "Port", // Port
  "connect.connecting": "Connexion…", // Connecting…
  "connect.connect": "Se connecter", // Connect
  "connect.stagePreparing": "Préparation du serveur…",
  "connect.stageTransferring": "Transfert du serveur…",
  "connect.stageStarting": "Démarrage du serveur…",
  "connect.sshFingerprintLabel": (kt: string) => `Empreinte de la clé d'hôte SSH (${kt})`,
  "connect.sshHostNew":
    "Première connexion à cet hôte — vérifiez l'empreinte avant de continuer.",
  "connect.sshHostChanged":
    "⚠ La clé de cet hôte a changé — il peut s'agir d'une réinstallation du serveur ou d'une attaque de l'homme du milieu. Ne continuez que si vous êtes sûr.",
  "connect.urlCertChanged":
    "⚠ L'empreinte du certificat de ce serveur a changé depuis votre dernière confirmation — il peut s'agir d'une réinstallation du serveur ou d'une attaque de l'homme du milieu. Ne continuez que si vous êtes sûr.",
  "connect.sshPasswordLabel": "Mot de passe SSH",
  "connect.sshPasswordPlaceholder": "Mot de passe du compte",
  "connect.savedHosts": "Hôtes récents",
  "connect.savedHostsAll": "Tous les hôtes récents",
  "connect.showAllHosts": (n: number) => `Tout afficher (${n})`,
  "connect.forgetHost": "Oublier cet hôte",
  "connect.savedHasPassword": "Mot de passe enregistré",
  "connect.rememberPassword": "Se souvenir du mot de passe",
  "connect.urlPasswordPlaceholder": "Mot de passe de connexion",
  "connect.shareDesktopDb": "Utiliser la base de données de l'app de bureau distante",
  "connect.shareDesktopDbHint":
    "Partage une base de données avec l'app de bureau de la machine distante (idéalement même version des deux côtés). Désactivé = base de données isolée.",

  // ── Sidebar ──
  "tree.newSession": "Nouvelle session", // New Session
  "tree.newTerminalSession": "Nouvelle session terminal", // New Terminal Session
  "tree.newBrowserPage": "Nouvelle page de navigateur", // New Browser Page
  "tree.newAgentSession": (agent) => `Nouvelle session ${agent}`, // New {agent} Session
  "tree.newAgentSessionGroup": "Plus de sessions agent", // More Agent Session
  "tree.newAgentSessionCustom": "Nouveau avec arguments…", // New with launch args…
  "tree.resumeSession": "Reprendre une session…", // Resume Session…
  "tree.newGroup": "Nouveau groupe", // New Group
  "tree.newSubgroup": "Nouveau sous-groupe", // New Subgroup
  "tree.newChildSession": "Nouvelle session enfant", // New Child Session
  "tree.openSelected": "Ouvrir les sessions sélectionnées", // Open Selected Sessions
  "tree.archiveSelected": "Archiver les sessions sélectionnées", // Archive Selected Sessions
  "tree.moveSelected": "Déplacer la sélection vers…", // Move Selected to…
  "tree.deleteSelected": (n) => `Supprimer les ${n} éléments sélectionnés`, // Delete {n} Selected Items
  "tree.removeProject": "Retirer le projet", // Remove Project
  "tree.deleteGroup": "Supprimer le groupe", // Delete Group
  "tree.deleteSession": "Supprimer la session", // Delete Session
  "tree.projectRoot": "Racine du projet (sans groupe)", // Project root (no group)
  "tree.moveToSession": "Déplacer sous une session (en enfant)", // Move under a session (as child)
  "tree.moveTo": "Déplacer vers…", // Move to…
  "tree.openNewTab": "Ouvrir dans un nouvel onglet", // Open in New Tab
  "tree.forkSession": "Forker la session", // Fork Session
  "tree.exportSession": "Exporter la session…", // Export Session…
  "tree.sessionInfo": "Infos de session", // Session Info
  "tree.groupInfo": "Infos du groupe", // Group Info
  "info.branch": "Branche", // Branch
  "info.path": "Chemin", // Path
  "info.recentCommits": "Commits récents", // Recent Commits
  "info.noCommits": "Aucun commit", // No commits
  "tree.killProcess": "Tuer le processus", // Kill Process
  "tree.archiveSession": "Archiver la session", // Archive Session
  "tree.archiveGroup": "Archiver le groupe", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "temp", // scratch
  "tree.persistSession": "Convertir en session permanente…", // Make Permanent Session…
  "tree.persistDoc": "Enregistrer sur le disque…", // Save to Disk…
  "tree.closeScratch": "Fermer le brouillon", // Close Scratch
  "tree.importProject": "Importer un projet", // Import Project
  "tree.createProject": "Créer un projet",
  "tree.cloneProject": "Cloner depuis Git", // Clone from Git
  "createProject.title": "Créer un projet",
  "createProject.name": "Nom du projet",
  "createProject.namePlaceholder": "mon-projet",
  "createProject.into": "Créer dans",
  "createProject.choose": "Choisir…",
  "createProject.noParent": "Choisissez un dossier parent",
  "createProject.invalidName": "Saisissez un seul nom de dossier sans / ni \\.",
  "createProject.creating": "Création…",
  "createProject.submit": "Créer le projet",
  "clone.title": "Cloner un dépôt Git", // Clone Git Repository
  "clone.url": "URL du dépôt", // Repository URL
  "clone.urlPlaceholder": "https://… ou git@…",
  "clone.branch": "Branche (facultatif)", // Branch (optional)
  "clone.branchPlaceholder": "Branche par défaut si vide", // Default branch if empty
  "clone.folder": "Nom du dossier", // Folder name
  "clone.folderPlaceholder": "Auto depuis l’URL", // Auto from URL
  "clone.into": "Cloner dans", // Clone into
  "clone.choose": "Choisir…", // Choose…
  "clone.noParent": "Choisissez un dossier parent", // Choose a parent folder
  "clone.cloning": "Clonage…", // Cloning…
  "clone.cancelling": "Annulation…",
  "clone.stageStarting": "Démarrage de Git…",
  "clone.stageConnecting": "Connexion au dépôt…",
  "clone.stagePreparing": "Préparation des objets…",
  "clone.stageReceiving": "Réception des objets…",
  "clone.stageResolving": "Résolution des deltas…",
  "clone.stageCheckout": "Extraction des fichiers…",
  "clone.stageFinalizing": "Finalisation…",
  "clone.stageImporting": "Importation du projet…",
  "clone.elapsed": (seconds: number) => `${seconds} s écoulées`,
  "clone.slowHint": "Aucune progression depuis 30 secondes. Vérifiez le réseau ou le proxy de la machine distante ; vous pouvez annuler puis réessayer.",
  "clone.submit": "Cloner", // Clone
  "tree.globalSearch": "Rechercher dans toutes les sessions", // Search All Sessions
  "tree.archivedSessions": "Sessions archivées", // Archived Sessions
  "tree.searchPlaceholder": "Rechercher sessions / groupes…", // Search sessions / groups…
  "tree.clearSearch": "Effacer la recherche", // Clear search
  "tree.filterWorking": "En cours", // Working
  "tree.filterAsking": "À traiter", // Pending
  "tree.filterWaiting": "Consulté", // Viewed
  "tree.filterStatus": "Filtrer par état", // Filter by status
  "tree.refreshStatusFilter": "Actualiser le filtre d’état",
  "tree.refreshStatusMatch": "Actualiser l’état",
  "tree.filterStatusSection": "État", // Status
  "tree.filterMarkSection": "Repère", // Mark
  "tree.viewMainName": "Principale",
  "tree.viewUntitled": "Vue sans nom",
  "tree.viewDefaultName": (n) => `Vue ${n}`,
  "tree.viewPrimary": "Vue principale",
  "tree.viewManage": "Gérer la vue",
  "tree.viewSetPrimary": "Définir comme principale",
  "tree.viewRename": "Renommer la vue",
  "tree.viewName": "Nom de la vue",
  "tree.viewDelete": "Supprimer la vue",
  "tree.viewDeletePrimary": "La vue principale ne peut pas être supprimée",
  "tree.viewDeleteTitle": "Supprimer la vue arborescente",
  "tree.viewDeleteConfirm": (name) =>
    `Supprimer « ${name} » ? Ses recherches et filtres enregistrés seront supprimés, sans affecter les projets ni les sessions.`,
  "tree.viewSplitRight": "Scinder la vue de l’arbre vers la droite",
  "tree.viewSplitDown": "Scinder la vue de l’arbre vers le bas",
  "tree.viewAdd": "Copier la vue actuelle dans un nouvel onglet",
  "tree.viewCount": (n) => `${n} vue${n > 1 ? "s" : ""} arborescente${n > 1 ? "s" : ""}`,
  "mark.menu": "Repère", // Mark
  "mark.urgent": "Urgent", // Urgent
  "mark.important": "Important", // Important
  "mark.bug": "Bogue", // Bug
  "mark.done": "Terminé", // Done
  "mark.wip": "En cours", // In progress
  "mark.pinned": "Épinglé", // Pinned
  "mark.idea": "Idée", // Idea
  "mark.caution": "Attention", // Caution
  "tree.clearAllNotifications":
    "Effacer tous les badges de notification (points de session et badge du Dock)", // Clear all notification badges…
  "tree.noProjectsPre": "Aucun projet pour l'instant. Cliquez sur l'icône de dossier ou appuyez sur ", // No projects yet. Click the folder button, or press
  "tree.noProjectsPost": " pour importer un répertoire.", // to import a directory.
  "tree.openProject": "Ouvrir un projet", // Open Project
  "tree.noAttention": "Aucune session ne correspond au filtre d'état", // No sessions match the status filter
  "tree.noMatch": "Aucun résultat", // No matches

  // Dialog fields
  "tree.groupName": "Nom du groupe", // Group name
  "tree.sessionNameAuto": "Nom de session (vide = nommage auto)", // Session name (leave empty to auto-name)
  "tree.editSession": "Modifier la session", // Edit Session
  "tree.sessionName": "Nom de session", // Session name
  "tree.shellLabel": "Shell (vide = défaut système)", // Shell (leave empty for system default)
  "tree.shellMenu": "Shell",
  "tree.downloadFullGitbash": "Télécharger Git Bash complet",
  "gitbash.title": "Git Bash",
  "gitbash.downloading": "Téléchargement de Git Bash complet…",
  "gitbash.extracting": "Extraction de Git Bash complet…",
  "gitbash.done": "Git Bash complet est prêt.",
  "gitbash.failed": "Échec du téléchargement de Git Bash",
  "tree.shellSystemDefault": "Défaut système", // System default
  "form.customOption": "Personnalisé…", // Custom…
  "tree.cwdLabel": "Répertoire de travail (vide = racine du projet)", // Working directory (leave empty for project root)
  "tree.initCmdLabel": "Commande de démarrage (optionnel)", // Startup command (optional)
  "tree.agentArgsLabel": "Arguments de lancement (optionnel)", // Launch args (optional)
  "preset.execPathLabel": "Exécutable (facultatif)",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint": "Laissez vide pour utiliser la commande configurée de l'agent. Renseignez-le pour que cette session seule utilise un remplaçant compatible.",
  "preset.saveLabel": "Enregistrer comme préréglage",
  "preset.namePlaceholder": "Nommer ce préréglage",
  "preset.iconChoose": "Choisir une icône",
  "preset.iconClear": "Retirer",
  "preset.iconHint": "Les images carrées conviennent le mieux ; les autres sont recadrées et réduites en 64x64.",
  "tree.permissionSkipLabel": "Ignorer toutes les confirmations de permission", // Skip all permission confirmations
  "tree.permissionSkipHint":
    "Lance avec le drapeau de contournement de cet agent (p. ex. Claude --dangerously-skip-permissions ; Codex désactive aussi son bac à sable). Appliqué à chaque lancement — à utiliser avec prudence.",
  "tree.permissionUnsupported":
    "OpenCode gère les permissions via son fichier de configuration — aucun drapeau de lancement, donc cette option ne s'applique pas.",
  "tree.permissionUnsupportedPi":
    "Pi exécute les outils sans demandes d'autorisation par conception — cette option ne s'applique pas.",

  // Boîte de dialogue « Nouvelle session agent »
  "newAgent.desc":
    "Vous pouvez nommer la session et ajouter des arguments de lancement personnalisés (transmis à la commande de l'agent, par ex. --model opus). Laissez les deux vides et appuyez sur Entrée pour la démarrer normalement.", // Optionally name the session and add custom launch args…

  // Delete confirmation
  "tree.batchDeleteTitle": "Suppression groupée", // Batch Delete
  "tree.deleteProjectTitle": "Supprimer le projet", // Delete Project
  "tree.deleteGroupTitle": "Supprimer le groupe", // Delete Group
  "tree.deleteSessionTitle": "Supprimer la session", // Delete Session
  "tree.batchDeleteBody": (n) =>
    `Supprimer les ${n} éléments sélectionnés (les projets/groupes suppriment en cascade leurs sous-groupes et sessions). Action irréversible.`, // Delete the {n} selected items…
  "tree.deleteProjectBody": (name) =>
    `Supprimer le projet « ${name} » ? Tous ses sous-groupes et sessions seront aussi supprimés. Action irréversible.`, // Delete project "{name}"?…
  "tree.deleteGroupBody": (name) =>
    `Supprimer le groupe « ${name} » ? Tous ses sous-groupes et sessions seront aussi supprimés. Action irréversible.`, // Delete group "{name}"?…
  "tree.deleteSessionBody": (name) =>
    `Supprimer la session « ${name} » (et toutes ses sessions enfants) ? Action irréversible.`, // Delete session "{name}"…
  "tree.deleteWorktrees": (n) =>
    `Supprimer aussi les worktrees git associés (${n} au total ; la suppression peut échouer si l'arbre de travail a des modifications)`, // Also remove associated git worktrees…

  // Session information dialog
  "info.name": "Nom", // Name
  "info.type": "Type", // Type
  "info.status": "État", // Status
  "info.notYetCaptured": "Pas encore généré (capturé après la première exécution)", // Not yet generated (captured after first run)
  "info.sessionId": "ID de session", // Session ID
  "info.cwd": "Répertoire", // Working dir
  "info.initCmd": "Commande", // Startup cmd
  "info.agentArgs": "Arguments", // Launch args
  "info.launchCmd": "Commande complète", // Full launch command
  "info.permission": "Permission", // Permission
  "info.permissionSkip": "Ignorer toutes les confirmations", // Skip all confirmations
  "info.parentSessionId": "ID parent", // Parent ID
  "info.termTitle": "Titre du terminal", // Terminal title
  "info.createdAt": "Créé le", // Created at

  // Resume-session dialog
  "resume.title": "Reprendre une session", // Resume Session
  "resume.desc":
    "Choisissez le type d'agent et saisissez le session id propre à l'agent ; l'ouverture reprend la conversation d'origine.", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "Type d'agent", // Agent type
  "resume.sessionIdPlaceholder": "Session id de la conversation", // Conversation session id
  "resume.confirm": "Reprendre et ouvrir", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "Nouvelle session worktree…", // New Worktree Session…
  "worktree.worktreeNameLabel": "Nom du worktree", // Worktree name
  "worktree.worktreeNameHint": "Sert de nom de répertoire et de branche du worktree.", // Used as the worktree directory and branch name.
  "worktree.createFailed": "Impossible de créer le worktree", // Couldn't create the worktree
  "worktree.noRepoRoot": "Ce projet n'a pas de chemin de dépôt git utilisable.", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "Aucun", // None
  "worktreeSel.modeNew": "Nouveau", // New
  "worktreeSel.modeExisting": "Existant", // Existing
  "worktreeSel.loading": "Chargement des worktrees…", // Loading worktrees…
  "worktreeSel.empty": "Aucun worktree existant dans ce dépôt.", // No existing worktrees in this repository.
  "worktreeSel.loadFailed": "Impossible de lister les worktrees (pas un dépôt git ?).", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint": "Les sessions créées dans ce groupe utiliseront ce worktree par défaut.", // Sessions created in this group will use this worktree by default.

  // ── Archive panel ──
  "archive.title": "Sessions archivées", // Archived Sessions
  "archive.empty1": "Aucune session archivée.", // No archived sessions.
  "archive.empty2":
    "Faites un clic droit sur une session dans la barre latérale et choisissez « Archiver la session » pour la ranger ici.", // Right-click a session in the sidebar…
  "archive.restore": "Restaurer en session normale", // Restore to normal session
  "archive.export": "Exporter le contexte complet en Markdown", // Export full context as Markdown
  "archive.deleteForever": "Supprimer définitivement (avec l'enregistrement)", // Delete permanently (with recording)
  "archive.pickOne": "Sélectionnez une session archivée à gauche pour voir sa transcription", // Select an archived session on the left…
  "archive.recordingEnd": "--- Fin de l'enregistrement ---", // --- End of recording ---
  "archive.readRecordingFailed": (err) => `Échec de lecture de l'enregistrement : ${err}`, // Failed to read recording: {err}
  "archive.searchRecording": "Rechercher dans l'enregistrement…", // Search in recording…
  "archive.searchTranscript": "Rechercher dans la transcription…", // Search transcript…
  "archive.searchPlaceholder": "Rechercher dans les archives…", // Search archived content…
  "archive.msgCountAll": (n) => (n === 1 ? "1 message" : `${n} messages`), // {n} messages
  "archive.msgCountFiltered": (shown, total) => `${shown} / ${total} messages`, // {shown} / {total} messages
  "archive.you": "Vous", // You
  "archive.toolsUsed": (tools) => `Outils : ${tools}`, // Tools: {tools}
  "archive.noMatch": "Aucun message correspondant", // No matching messages
  "archive.emptyTranscript": "Transcription vide", // Transcript is empty
  "archive.loadingTranscript": "Chargement de la transcription…", // Loading transcript…

  // ── Global session-content search ──
  "search.allPlaceholder": "Rechercher dans tout le contenu des sessions…", // Search across all session content…
  "search.hint": "Recherchez le contenu des sessions. Les sessions archivées sont exclues par défaut — cochez « Inclure les archives » pour les ajouter.", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "Inclure les archives", // Include archived
  "search.includeArchivedHint": "Rechercher aussi dans les sessions archivées (désactivé par défaut)", // Also search archived sessions (off by default)
  "search.searching": "Recherche…", // Searching…
  "search.noResults": "Aucune correspondance", // No matches found
  "search.sessionCount": (n) => (n === 1 ? "1 session" : `${n} sessions`), // n sessions
  "search.matchCount": (n) => (n === 1 ? "1 correspondance" : `${n} correspondances`), // n matches
  "search.pickSession": "Sélectionnez une session à gauche pour voir ses correspondances", // Select a session on the left to see its matches
  "search.openSession": "Ouvrir la session", // Open session
  "search.backToResults": "Retour aux résultats", // Back to results
  "search.archivedBadge": "Archivée", // Archived
  "search.summary": (m, s) =>
    `${m} ${m === 1 ? "correspondance" : "correspondances"} · ${s} ${s === 1 ? "session" : "sessions"}`, // X matches · N sessions
  "search.matchPosition": (n, total) => `${n} sur ${total}`, // N of M
  "search.roleTerminal": "Terminal", // Terminal
  "search.collapseGroup": "Réduire", // Collapse
  "search.expandGroup": "Développer", // Expand
  "search.cappedNote": (l, total) => `${l} sur ${total} localisables`, // L of total locatable

  // ── Center pane ──
  "center.noSession": "Aucune session", // No session
  "center.noSessionHintPre": "Choisissez une session dans la barre latérale, ou appuyez sur ", // Pick a session from the sidebar, or press
  "center.noSessionHintPost": " pour créer un terminal", // to create a terminal
  "center.createTerminal": "Créer un terminal", // Create Terminal
  "tab.unsavedDot": "Modifications non enregistrées", // Unsaved changes
  "tab.newTerminal": "Nouveau terminal", // New terminal
  "tab.newDocument": "Nouveau document", // New document
  "tab.bgTitle": (n) => `Onglets maintenus en arrière-plan : ${n} (processus toujours actifs)`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `Arrière-plan ${n}`, // Background {n}
  "tab.scratchFallback": "(terminal temporaire)", // (scratch terminal)
  "tab.killBgTab": "Fermer cet onglet d'arrière-plan (ses processus se termineront)", // Kill this background tab…
  "tab.newBrowserTab": "Nouvel onglet", // New Tab
  "tab.refreshFile": "Recharger le fichier", // Refresh File
  "tab.closeOthers": "Fermer les autres onglets", // Close Other Tabs
  "tab.closeRight": "Fermer les onglets à droite", // Close Tabs to the Right
  "tab.closeAll": "Fermer tous les onglets", // Close All Tabs
  "tab.sendToBackground": "Passer en arrière-plan", // Send to Background

  // ── Navigateur intégré ──
  "browser.back": "Retour", // Back
  "browser.forward": "Avancer", // Forward
  "browser.reload": "Recharger", // Reload
  "browser.desktopOnly": "Les onglets de navigateur s'ouvrent uniquement dans l'application de bureau.", // Browser tabs open in the desktop app only.
  "browser.stop": "Arrêter le chargement", // Stop loading
  "browser.openExternal": "Ouvrir dans le navigateur système", // Open in system browser
  "browser.addressPlaceholder": "Saisir une URL ou des termes de recherche", // Enter URL or search terms
  "browser.quickAccess": "Accès rapide", // Quick access
  "browser.loading": "Chargement…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "Quitter VelaTerm ?",  // Quit VelaTerm?
  "quit.body": "Toutes les sessions de terminal et d'agent en cours seront arrêtées.",  // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "Enregistrer l'espace de travail",  // Save workspace
  "quit.saveWorkspaceHint": "Rouvrir les mêmes onglets et divisions la prochaine fois. Les terminaux sont restaurés, mais pas redémarrés.",  // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "Quitter",  // Quit
  "dormant.body": "Restauré depuis l'espace de travail enregistré. Aucun processus n'est encore en cours.",  // Restored from your saved workspace. No process is running yet.
  "dormant.start": "Démarrer",  // Start
  "overlimit.title": (max) => `Limite d'arrière-plan dépassée (${max})`, // Background keep-alive over limit ({max})
  "overlimit.body": "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
  "overlimit.kill": "End Selected", // End Selected
  "overlimit.keep": "Keep for Now", // Keep for Now
  "overlimit.earliest": "earliest", // earliest
  "overlimit.statusWorking": "working", // working
  "overlimit.statusAsking": "awaiting reply", // awaiting reply
  "overlimit.statusWaiting": "waiting", // waiting

  // ── Terminal pane ──
  "term.paste": "Coller", // Paste
  "term.pasteUseShortcut": "Coller (appuyez sur ⌘V)", // Paste (press ⌘V)
  "term.selectAll": "Tout sélectionner", // Select All
  "term.autoCopied": (n: number) => `${n} caractères copiés · ⌘V pour coller`,
  "term.clear": "Effacer", // Clear
  "term.searchMenu": "Rechercher…", // Search…  ⌘F
  "term.splitRight": "Diviser à droite", // Split right (⌘D)
  "term.splitDown": "Diviser en bas", // Split down (⌘⇧D)
  "term.closePane": "Fermer le volet", // Close split
  "term.redraw": "Redessiner", // Redraw
  "term.mirrorTooltip":
    "Affichage miroir (taille contrôlée par un autre client). Cliquez pour adapter le PTY à cette fenêtre", // Mirroring (size controlled by another client)…
  "term.mirrorBadge": (dims) => `⤢ Miroir${dims} · cliquer pour adapter à cette fenêtre`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) => `⤢ Miroir${dims} · adapter à cette fenêtre`, // ⤢ Mirror{dims} · fit this window
  "term.imgUploadFailed": (n, lastError) =>
    `Échec d'envoi de ${n} image${n === 1 ? "" : "s"}${lastError ? ` : ${lastError}` : ""}`, // Image upload failed for {n} images…
  "term.imgClipboardUnavailable":
    "Impossible de lire l'image du presse-papiers. Copiez-la de nouveau puis réessayez.",
  "term.starting": (agent) => `Démarrage de ${agent}…`, // Starting {agent}…
  "term.startFailed": (err) => `Échec du démarrage : ${err}`, // Failed to start: {err}

  // ── Carte d'aide à l'installation d'un agent ──
  "agentInstall.title": (label) => `${label} n'est pas installé`, // {label} is not installed
  "agentInstall.desc": (label) =>
    `VelaTerm n'a pas trouvé ${label} dans votre PATH. Installez-le pour lancer cette session.`, // couldn't find {label} on PATH
  "agentInstall.install": "Installer", // Install now
  "agentInstall.retry": "Relancer", // Retry launch
  "agentInstall.dismiss": "Je m'en occupe", // I'll do it myself
  "agentInstall.docs": "Documentation", // Install docs
  "agentInstall.needsNode": "Nécessite Node.js / npm", // Requires Node.js / npm
  "agentInstall.afterInstall": "Après l'installation :", // After install:
  "agentInstall.pathSaved": (label: string) => `Chemin de l'exécutable de ${label} enregistré dans les réglages :`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} est installé`, // {label} is installed
  "agentInstall.doneDesc": "Relancez cette session pour commencer à l'utiliser.", // Relaunch this session to start using it.
  "agentInstall.restartNow": "Relancer maintenant", // Relaunch now
  "agentInstall.later": "Plus tard", // Later
  "search.placeholder": "Rechercher dans le terminal", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "WYSIWYG", // WYSIWYG
  "doc.source": "Source", // Source
  "doc.searchPlaceholder": "Rechercher", // Find
  "doc.searchReplacePlaceholder": "Remplacer", // Replace
  "doc.searchReplace": "Remplacer", // Replace
  "doc.searchReplaceAll": "Tout", // All
  "doc.searchNoMatch": "Aucun résultat", // No results
  "doc.searchCaseSensitive": "Respecter la casse", // Match case
  "doc.searchToggleReplace": "Afficher le remplacement", // Toggle replace
  "doc.fileTree": "Arborescence", // File tree
  "doc.treeUp": "Dossier parent", // Parent folder
  "doc.sidebar": "Panneau latéral", // Sidebar
  "doc.unsaved": "Non enregistré", // Unsaved
  "doc.saveAsTitle": "Enregistrer sous", // Save As
  "doc.saveAsName": "Nom du fichier", // File name
  "doc.outline": "Plan", // Outline
  "doc.outlineEmpty": "Aucun titre", // No headings
  "doc.saving": "Enregistrement…", // Saving…
  "doc.overwriteConfirm": "Un fichier portant ce nom existe déjà. Cliquez sur « Remplacer » pour le remplacer.", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "Enregistrer", // Save
  "doc.externalChanged":
    "Le fichier a été modifié sur le disque (vous avez des modifications locales non enregistrées).", // The file was modified on disk…
  "doc.reloadDiscard": "Recharger (abandonner mes modifications)", // Reload (discard my changes)
  "doc.externalChangedClean": "Le fichier a été modifié sur le disque.", // The file was modified on disk.
  "doc.reload": "Recharger", // Reload
  "doc.ignore": "Ignorer", // Ignore
  "doc.loadingFile": (title) => `Chargement de ${title}…`, // Loading {title}…
  "doc.closeTitle": "Fermer le document", // Close Document
  "doc.unsavedBody": (title) => `« ${title} » a des modifications non enregistrées.`, // "{title}" has unsaved changes.
  "doc.saveAndClose": "Enregistrer et fermer", // Save & Close
  "doc.closeNoSave": "Fermer sans enregistrer", // Close Without Saving
  "doc.conflictTitle": "Conflit d'enregistrement", // Save Conflict
  "doc.conflictBody":
    "Le fichier sur le disque a été modifié de l'extérieur. L'écraser quand même avec le contenu actuel ?", // The file on disk was modified externally…
  "doc.overwrite": "Écraser", // Overwrite
  "doc.saveFailed": (err) => `Échec de l'enregistrement : ${err}`, // Save failed: {err}
  "doc.closeTab": "Fermer l'onglet", // Close Tab
  "doc.truncatedReadonly": (size: string) =>
    `Lecture seule : affichage des 10 premiers Mo sur ${size}. L'enregistrement est désactivé pour ne pas écraser le reste du fichier.`,
  "doc.imgLoading": (title, size) => `Chargement de ${title} (${size})…`, // Loading {title} ({size})…
  "doc.imgBeingWritten":
    "Le fichier est en cours d'écriture ; il sera rechargé automatiquement une fois stabilisé.", // The file is being written; it will reload automatically once it settles.
  "doc.imgDecodeFailed": "Impossible d'afficher cette image (format non pris en charge ou fichier corrompu).", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "Ajuster", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "Exporter en PDF", // Export PDF
  "doc.diagramError": "Erreur de diagramme", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "Aucune session sélectionnée", // No session selected
  "panel.openInEditor": "Ouvrir dans l'éditeur", // Open in Editor
  "panel.openInEditorTooltip":
    "Ouvrir dans l'éditeur de documents du volet central (comme la commande view)", // Open in the document editor…
  "panel.preview": "Aperçu", // Preview
  "panel.cantRead": "(impossible de lire ce fichier)", // (cannot read this file)
  "panel.binary": "(fichier binaire, pas d'aperçu)", // (binary file, no preview)
  "panel.truncated": "\n…(contenu tronqué)", // …(content truncated)
  "panel.showHidden": "Afficher les fichiers cachés", // Show hidden files
  "panel.hideHidden": "Masquer les fichiers cachés", // Hide hidden files

  // ── File-tree actions (Files context menu and header add button) ──
  "files.newFile": "Nouveau fichier", // New File
  "files.newFolder": "Nouveau dossier", // New Folder
  "files.nameLabel": "Nom", // Name
  "files.newTooltip": "Nouveau fichier ou dossier", // New file or folder
  "files.openInTerminal": "Open in Terminal",
  "files.revealInFinder": "Show in File Manager",
  "files.copyPath": "Copy Path",
  "files.copyRelPath": "Copy Relative Path",
  "files.filterPlaceholder": "Filter files…",
  "files.deleteConfirm": (name) => `Supprimer « ${name} » ? Cette action est irréversible.`, // Delete "{name}"? This can't be undone.

  // ── Status bar ──
  "statusbar.sessions": (n) => (n === 1 ? "1 session" : `${n} sessions`), // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `Cliquez pour n'afficher que les sessions « ${label} » dans la barre latérale (cliquez à nouveau pour annuler)`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `Arrière-plan ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `Onglets maintenus en arrière-plan (limite ${max} ; au-delà, le plus ancien onglet inactif est fermé automatiquement)`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) => `Onglet d'arrière-plan fermé : ${name} (limite dépassée)`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) => `Accès distant navigateur activé : ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "Droits : Demander", // Perms: Ask
  "statusbar.permSkip": "Droits : Ignorer", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip": "Mode d'autorisation de cette session · cliquez pour changer (cette session uniquement)", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "Autorisations de cette session", // This session's permissions
  "statusbar.permOptAsk": "Demander à chaque fois (par défaut)", // Ask each time (default)
  "statusbar.permScopeHint": "S'applique uniquement à cette session. Pour les réglages globaux, rendez-vous dans Réglages ▸ Agents.", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg": "Autorisation modifiée. La session doit redémarrer pour l'appliquer. Le redémarrage reprend la conversation en cours mais interrompt toute tâche en cours. Redémarrer maintenant ?", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "Redémarrer", // Restart now
  "statusbar.permRestartLater": "Plus tard", // Later
  "statusbar.permScopeTitle": "Appliquer à ?", // Apply to?
  "statusbar.permScopeSession": "Cette session uniquement", // This session only
  "statusbar.permScopeGlobal": "Valeur par défaut globale", // Global default
  "statusbar.permScopeGlobalHint": "S'applique maintenant à cette session et devient la valeur par défaut pour les futures sessions de ce type (synchronisé avec les Réglages).", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

  // ── Store, notifications, and export ──
  "notify.working": "⏳ En traitement…", // ⏳ Working…
  "notify.asking": "❓ Votre confirmation est requise", // ❓ Needs your confirmation
  "notify.waiting": "✅ Répondu", // ✅ Replied
  "store.subtask": "Sous-tâche", // Subtask
  "store.splitPane": "Volet", // Split
  "export.failedTitle": "Échec de l'export de session", // Failed to export session
  "export.contextSuffix": "contexte", // context

  // ── Error panel ──
  "err.renderTitle": "Erreur de rendu", // Rendering Error
  "err.renderDesc":
    "Une erreur inattendue s'est produite. Les informations ci-dessous peuvent aider à localiser le problème.", // An unexpected error occurred…
  "err.reload": "Recharger", // Reload
  "err.uncaughtTitle": "Erreur non interceptée", // Uncaught Error
  "err.uncaughtDesc": "Les informations ci-dessous peuvent aider à localiser le problème.", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser":
    "La relecture des enregistrements n'est pas encore prise en charge dans le navigateur", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) => `Échec d'envoi de l'image (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.connecting": "Connexion…", // Connecting…
  "login.remoteAccess": "Accès distant", // Remote Access
  "login.desc": "Saisissez le mot de passe d'accès pour vous connecter à ce terminal.", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "Mot de passe d'accès", // Access password
  "login.connect": "Se connecter", // Connect
  "login.wrongPassword": "Mot de passe incorrect", // Wrong password
  "login.rateLimited": "Trop de tentatives. Veuillez patienter une minute avant de réessayer.", // Too many attempts. Please wait a minute and try again.
  "login.failed": "Échec de connexion, veuillez réessayer", // Login failed, please try again
  "login.pairingRequired": "Ce serveur nécessite un lien d'association. Ouvrez le lien généré dans le panneau Accès distant de l'application de bureau.", // This server requires a pairing link
  "login.authFailed": "Échec de l'authentification. Vérifiez le mot de passe d'accès, ou ouvrez un nouveau lien d'association s'il a été régénéré.", // Authentication failed, check password or use a new pairing link
  "dir.title": "Choisir le répertoire du projet", // Choose Project Directory
  "dir.pathPlaceholder": "Rechercher, ou saisir un chemin puis Entrée (supporte ~)", // Search, or type a path and press Enter (supports ~)
  "dir.up": "Dossier parent", // Up one level
  "dir.newFolder": "Nouveau dossier", // New Folder
  "dir.newFolderPlaceholder": "Nom du dossier", // Folder name
  "dir.goInput": "Aller au chemin saisi", // Go to typed path
  "dir.noSubdirs": "(aucun sous-répertoire)", // (no subdirectories)
  "dir.empty": "(dossier vide)", // (empty folder)
  "dir.noMatch": "Aucun élément correspondant", // No matching items
  "dir.target": "Dossier cible", // Target
  "dir.showHidden": "Afficher les éléments masqués", // Show hidden items
  "dir.importing": "Import…", // Importing…
  "dir.choose": "Choisir ce répertoire", // Choose This Directory
  "conn.reconnecting": "Connexion perdue, reconnexion…", // Connection lost, reconnecting…
  "conn.reconnectNow": "Reconnecter maintenant", // Reconnect now
  "conn.retrying": "Reconnexion…", // Reconnecting…
  "conn.sshReconnecting": "Liaison SSH perdue, reconstruction du tunnel…", // SSH link lost, rebuilding the tunnel…
  "conn.sshDown": "Liaison SSH interrompue — appuyez sur « Reconnecter maintenant » pour réessayer", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "Échec de la requête", // Request failed
  "reqerr.dismiss": "Fermer", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "Journal des erreurs", // Error Log
  "errlog.empty": "Aucune erreur enregistrée.", // No errors recorded.
  "errlog.copyAll": "Tout copier", // Copy all
  "errlog.clear": "Effacer", // Clear
  "errlog.close": "Fermer", // Close

  // ── Mobile ──
  "mobile.toDesktop": "Passer à la version bureau", // Switch to desktop
  "mobile.empty1": "Aucune session.", // No sessions.
  "mobile.noMatch": "Aucune session correspondante", // No matching sessions
  "mobile.empty2":
    "Créez-en une sur l'application bureau ou un navigateur d'ordinateur, elle apparaîtra ici automatiquement.", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ Retour", // ‹ Back
  "mobile.selCopy": "Copier", // Copy
  "mobile.selCancel": "Annuler", // Cancel

  // ── Other shared components ──
  "splitter.dragToResize": "Glisser pour redimensionner", // Drag to resize
  "transport.wsDisconnected": "WebSocket déconnecté", // WebSocket disconnected
  "transport.wsConnectFailed": "Échec de connexion WebSocket", // WebSocket connection failed
  "transport.cmdFailed": "Échec de la commande", // Command failed
  "transport.remoteCmdForbidden": (cmd: string) => `Commande non disponible pour les clients distants : ${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) => `Clé de paramètre non modifiable par les clients distants : ${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) => `Les clients distants ne peuvent pas accéder aux fichiers du répertoire de données de l'application : ${path}`, // Remote clients cannot access files in the app data directory

  // ── Crepe（éditeur WYSIWYG）──
  "crepe.placeholder": "Saisissez du texte, ou tapez / pour le menu d'insertion", // Type text, or press / for the insert menu
  "crepe.textGroup": "Texte", // Text
  "crepe.paragraph": "Texte", // Text
  "crepe.h1": "Titre 1", // Heading 1
  "crepe.h2": "Titre 2", // Heading 2
  "crepe.h3": "Titre 3", // Heading 3
  "crepe.h4": "Titre 4", // Heading 4
  "crepe.h5": "Titre 5", // Heading 5
  "crepe.h6": "Titre 6", // Heading 6
  "crepe.quote": "Citation", // Quote
  "crepe.divider": "Séparateur", // Divider
  "crepe.listGroup": "Liste", // List
  "crepe.bulletList": "Liste à puces", // Bullet List
  "crepe.orderedList": "Liste numérotée", // Ordered List
  "crepe.taskList": "Liste de tâches", // Task List
  "crepe.advancedGroup": "Insérer", // Insert
  "crepe.image": "Image", // Image
  "crepe.codeBlock": "Bloc de code", // Code Block
  "crepe.table": "Tableau", // Table
  "crepe.math": "Formule", // Math
  "crepe.linkPlaceholder": "Collez ou saisissez un lien…", // Paste or type a link…
  "crepe.upload": "Téléverser", // Upload
  "crepe.uploadImage": "Téléverser une image", // Upload Image
  "crepe.orPasteImageLink": "ou collez un lien d'image", // or paste an image link
  "crepe.imageCaption": "Légende de l'image", // Image caption
  "crepe.confirm": "Confirmer", // Confirm
  "crepe.searchLanguage": "Rechercher un langage", // Search language
  "crepe.noResult": "Aucun résultat", // No results
  "crepe.edit": "Modifier", // Edit
  "crepe.collapse": "Replier", // Collapse
  // ── Panneau droit / barre inférieure ──
  "info.project": "Projet", // Project
  "panel.sessionInfo": "Infos de session", // Session info
  "panel.gitTitle": "État Git", // Git status
  "panel.gitProbing": "Vérification…", // Checking…
  "panel.gitNotRepo": "Pas un dépôt Git", // Not a Git repository
  "panel.gitBranch": "Branche", // Branch
  "panel.gitStaged": "Indexé", // Staged
  "panel.gitUnstaged": "Modifié", // Changed
  "panel.gitUntracked": "Non suivi", // Untracked
  "bottombar.running": "En cours", // Running
  "bottombar.collapseTasks": "Réduire les tâches", // Collapse tasks
  "bottombar.expandTasks": "Développer les tâches", // Expand tasks
  "bottombar.sound": "🔔 Son", // 🔔 Sound
  "bottombar.muted": "🔕 Muet", // 🔕 Muted
  "bottombar.overview": "Aperçu des sessions", // Sessions overview
  "bottombar.noSessions": "Aucune session", // No sessions
  "doc.pdfFilter": "Fichier PDF", // PDF file
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
  "updater.downloadManuallyHint": "Open the installer download in your browser.", // TODO translate
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

export default fr;
