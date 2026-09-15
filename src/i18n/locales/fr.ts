//! French dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const fr: typeof en = {
  "tree.newPlanExecuteSession": "Nouvelle session de planification et d’exécution…",
  "launch.splitTasks": "Diviser automatiquement en plusieurs tâches",
  "launch.splitTasksHint": "La session de planification propose des tâches indépendantes. Vérifiez les instructions, les agents, les modèles et l’effort de raisonnement avant de lancer l’exécution.",
  "launch.splitReview": "Vérifier les tâches à exécuter",
  "launch.splitReviewHint": "Une seule session de planification reçoit tous les rapports et examine chaque tâche séparément. L’exécution commence après votre confirmation.",
  "launch.splitConfirmed": "Ces tâches ont été confirmées.",
  "launch.splitClosed": "Cette proposition n’est plus en attente de confirmation.",
  "launch.splitRetry": "Réessayer les envois en attente",
  "launch.splitSharedDirectory": "Toutes les sessions d’exécution utilisent le répertoire de travail de la session de planification et partagent son arbre de travail si cette option est activée.",
  "launch.createIn": "Créer dans",
  "launch.workingDirectory": "Chemin du dossier de travail",
  "launch.createAndStart": "Créer et démarrer",
  "launch.planExecuteTaskHint": "Décrivez la tâche, les exigences et les critères de validation pour la planification.",
  "launch.planExecuteResult": "La session de planification démarre en premier et crée la session d’exécution une fois le plan prêt.",
  "launch.planExecuteWorktreeHint": "Les nouveaux worktrees partent du commit actuel, sans les modifications non validées. En cas d’échec de création, la session concernée ne démarre pas.",
  "launch.workflowDirectorySharedHint": "La session de planification et toutes les sessions d’exécution partagent un nouveau dossier et une branche.",
  "launch.workflowDirectoryEachHint": "La session de planification et chaque session d’exécution disposent de leur propre worktree et de leur propre branche.",
  "launch.planTitle": "Planification et validation",
  "launch.execTitle": "Exécution",
  "launch.planExecuteIntro": "Une session distincte planifie le travail, vérifie le résultat et demande les corrections nécessaires.",
  "chat.origin.plan": "Planification",
  "chat.origin.exec": "Exécution",

  // Project code intelligence and knowledge entry associations.
  "knowledge.callers": "Appelants",
  "knowledge.callees": "Symboles appelés",
  "knowledge.explore": "Explorer le code",
  "knowledge.exploreHint": "Décrivez une fonctionnalité ou un enchaînement, ou indiquez un fichier ou un symbole…",
  "knowledge.impact": "Analyse des impacts",
  "knowledge.path": "Chemin d’appel",
  "knowledge.target": "Rechercher un symbole cible…",
  "knowledge.depth": "Profondeur de parcours",
  "knowledge.noPath": "Aucun chemin d’appel orienté trouvé dans l’index.",
  "knowledge.watching": "Synchronisation automatique active",
  "knowledge.onDemand": "Synchronisation avant les requêtes",
  "knowledge.overview": "Vue d’ensemble",
  "knowledge.uncertain": "Relation déduite",
  "knowledge.kind": "Type de symbole",
  "knowledge.language": "Langage",
  "knowledge.results": "Résultats",
  "knowledge.resultLarge": "Le résultat est trop volumineux pour être affiché. Précisez la requête ou réduisez la profondeur de parcours.",
  "knowledge.queryFailed": "La requête de code a échoué. Réessayez ou synchronisez l’index.",
  "knowledge.liveHelp": "Les modifications sont synchronisées tant que le processus de requête est actif. Après son arrêt pour inactivité, la requête suivante rattrape les changements.",
  "knowledge.startHelp": "Activez l’indexation pour rechercher du code, suivre les appels et analyser les impacts d’une modification. L’analyse s’exécute sur le serveur sans modèle d’IA.",
  "knowledge.title": "Graphe du code",
  "knowledge.intro": "Explorez les relations du code et associez-les aux décisions de conception enregistrées.",
  "knowledge.setup": "Installez CodeGraph sur ce serveur pour activer l’indexation des projets.",
  "knowledge.downloadNotice": "Télécharge le moteur CodeGraph vérifié depuis GitHub. L’indexation reste sur cette machine ; la télémétrie et la recherche de mises à jour sont désactivées.",
  "knowledge.install": "Télécharger CodeGraph",
  "knowledge.installing": "Téléchargement et installation…",
  "knowledge.directory": "Répertoire de travail",
  "knowledge.enable": "Activer l’indexation",
  "knowledge.disable": "Désactiver l’indexation",
  "knowledge.sync": "Synchroniser",
  "knowledge.ready": "Prêt",
  "knowledge.disabled": "Désactivé",
  "knowledge.indexing": "Indexation…",
  "knowledge.syncing": "Synchronisation…",
  "knowledge.failed": "Échec",
  "knowledge.symbols": "Symboles",
  "knowledge.files": "Fichiers",
  "knowledge.edges": "Relations",
  "knowledge.search": "Rechercher un symbole ou un chemin de fichier…",
  "knowledge.searchButton": "Rechercher",
  "knowledge.noResults": "Aucun symbole correspondant.",
  "knowledge.selectSymbol": "Sélectionnez un symbole pour consulter sa source, ses relations et les articles associés.",
  "knowledge.source": "Source",
  "knowledge.incoming": "Relations entrantes",
  "knowledge.outgoing": "Relations sortantes",
  "knowledge.noEdges": "Aucune relation indexée.",
  "knowledge.analysisNote": "Les relations proviennent d’une analyse statique et peuvent être incomplètes ou incertaines.",
  "knowledge.changed": "Le fichier a changé pendant la requête. Synchronisez à nouveau avant d’utiliser les numéros de ligne ou de confirmer la vérification.",
  "knowledge.truncated": "Cet affichage est limité. Certaines relations ou lignes de code sont omises.",
  "knowledge.linkMemory": "Associer un article",
  "knowledge.chooseMemory": "Choisir un article",
  "knowledge.noLinks": "Aucune association au code. Vous pouvez associer un article depuis le détail d’un symbole.",
  "knowledge.inspect": "Vérifier le code et l’article",
  "knowledge.unlink": "Supprimer l’association",
  "knowledge.codeReferences": "Références au code",
  "knowledge.refresh": "Actualiser",
  "knowledge.current": "Inchangé",
  "knowledge.review": "À vérifier",
  "knowledge.unavailable": "Indisponible",
  "knowledge.reviewHelp": "Comparez cet article au code affiché. La confirmation enregistre la version actuelle du fichier sans modifier le texte de l’article.",
  "knowledge.confirmReview": "Confirmer la vérification",
  "knowledge.agentHint": "Les agents peuvent utiliser vkb search \"sujet\" dans ce répertoire. Les requêtes synchronisent les index activés et renvoient séparément le code et les articles de la base de connaissances.",
  "knowledge.busy": "Une tâche d’indexation est en cours. Vous pouvez fermer cette page ou désactiver l’indexation pour arrêter la tâche.",
  "knowledge.disabledHelp": "Activez l’indexation de ce répertoire pour interroger le code. La désactivation conserve l’index et les associations aux articles.",
  "knowledge.conflict": "Le code ou l’article a changé. Rechargez les deux avant d’enregistrer cette association.",
  "knowledge.symbolMissing": "Le symbole ou la source n’est plus disponible. Synchronisez et relancez la recherche.",
  "knowledge.directoryMissing": "Ce répertoire de travail est introuvable ou a changé. Vérifiez les chemins du projet et de la session.",
  "knowledge.partial": "L’index est incomplet. Synchronisez à nouveau et vérifiez que les fichiers source sont lisibles.",
  "knowledge.interrupted": "La tâche précédente a été interrompue. Synchronisez pour réessayer.",
  "knowledge.checksum": "La somme de contrôle du téléchargement ne correspond pas. Le moteur n’a pas été installé.",
  "knowledge.downloadFailed": "Impossible de télécharger CodeGraph. Vérifiez la connexion du serveur à GitHub et réessayez.",
  "knowledge.timeout": "Le délai d’indexation a été dépassé. Vérifiez la taille du dépôt et réessayez.",
  "knowledge.error": "L’opération a échoué. Vérifiez les droits d’accès aux répertoires et le moteur sur le serveur, puis réessayez.",

  // Knowledge base: saved knowledge organized by project and session.
  "memory.hierarchy": "Projets et sessions",
  "memory.up": "Remonter d’un niveau",
  "memory.manualGroup": "Création manuelle",
  "memory.legacyGroup": "Entrées fusionnées précédemment",
  "memory.unknownProject": "Projet source inconnu",
  "nb.addLink": "Insérer un lien",
  "nb.attach": "Joindre un fichier",
  "nb.browse": "Parcourir",
  "nb.chooseNote": "Commencez par une note",
  "nb.closeHint": "Retirer ce carnet de la liste. Ses fichiers restent sur le disque.",
  "nb.closeVault": "Fermer le carnet",
  "nb.conflict": "Le fichier a été modifié dans une autre application. Votre brouillon est conservé. Rechargez le fichier ou enregistrez le brouillon dans une nouvelle note.",
  "nb.copyTo": "Copier dans un carnet local",
  "nb.createVault": "Créer une base de connaissances",
  "nb.destination": "Chemin de destination",
  "nb.download": "Télécharger",
  "nb.downloadHint": "Téléchargez cette pièce jointe pour l’ouvrir dans une autre application.",
  "nb.empty": "Ouvrez un dossier pour commencer à écrire, ou créez un carnet.",
  "nb.emptyImport": "Aucun fichier pouvant être importé n’a été sélectionné.",
  "nb.emptyNotes": "Les notes sont enregistrées au format Markdown.",
  "nb.emptyOutline": "Les titres du document apparaîtront ici.",
  "nb.emptyTrash": "La corbeille est vide.",
  "nb.error": "Impossible d’accéder au carnet. Vérifiez la connexion et le dossier, puis réessayez.",
  "nb.exists": "La destination existe déjà. Choisissez un autre nom ou dossier.",
  "nb.favorites": "Favoris",
  "nb.files": "Fichiers",
  "nb.folder": "Dossier",
  "nb.generatedHint": "Des connaissances organisées à partir de vos sessions, avec leurs sources et leur historique de révision.",
  "nb.homeHint": "Parcourez la base de connaissances des sessions et les bases de connaissances locales.",
  "nb.homeSearch": "Rechercher dans les connaissances des sessions et les notes locales…",
  "nb.loadMore": "Charger la suite",
  "nb.import": "Importer",
  "nb.importFiles": "Choisir des fichiers",
  "nb.importFolder": "Choisir un dossier",
  "nb.importHint": "Les fichiers sont copiés dans le dossier choisi. Les fichiers existants ne sont jamais écrasés ; les dossiers de configuration masqués sont ignorés.",
  "nb.imported": "Importés",
  "nb.imports": "Historique des importations",
  "nb.importsEmpty": "Aucune importation pour le moment.",
  "nb.importRoot": "Racine de la base de connaissances",
  "nb.importBusy": "Une importation est déjà en cours pour cette base de connaissances.",
  "nb.importDelete": "Supprimer l’entrée",
  "nb.importDeleteConfirm": "Supprimer cette entrée d’importation ? Les fichiers déjà importés ne sont pas supprimés.",
  "nb.importDone": "Importation terminée",
  "nb.importDuration": (seconds: string) => `${seconds} s`,
  "nb.importFailed": "Échec de l’importation",
  "nb.importFilePending": "Non importé",
  "nb.importHideFiles": "Masquer les fichiers",
  "nb.importInterruptedHint": "L’importation s’est arrêtée avant la fin.",
  "nb.importProgress": (done: string, total: string) => `${done} / ${total} fichiers`,
  "nb.importShowFiles": (count: string) => `Fichiers (${count})`,
  "nb.importSkipHidden": "Fichier ou dossier masqué",
  "nb.importStatusCancelled": "Annulée",
  "nb.importStatusCompleted": "Terminée",
  "nb.importStatusFailed": "Échouée",
  "nb.importStatusInterrupted": "Interrompue",
  "nb.importStatusRunning": "Importation en cours",
  "nb.incomplete": "L’opération n’a pas pu aboutir. Vérifiez les fichiers et réessayez.",
  "nb.info": "Détails de la note",
  "nb.invalid": "Le nom ou le chemin n’est pas valide.",
  "nb.links": "Liens sortants",
  "nb.local": "Fichiers locaux",
  "nb.localVaults": "Bases de connaissances locales",
  "nb.move": "Renommer ou déplacer",
  "nb.moveHint": "Indiquez un chemin relatif à la racine du carnet. Les liens existants sont mis à jour lors du déplacement du fichier ou du dossier.",
  "nb.name": "Nom",
  "nb.newFolder": "Nouveau dossier",
  "nb.newNote": "Nouvelle note",
  "nb.noLinks": "Aucune note liée pour le moment.",
  "nb.tags": "Étiquettes",
  "nb.notes": "Notes",
  "nb.openVault": "Ouvrir une base de connaissances",
  "nb.outline": "Plan",
  "nb.quickOpen": "Ouverture rapide",
  "nb.readOnly": "Ce fichier ne peut pas être modifié comme une note Markdown en UTF-8.",
  "nb.recent": "Notes récentes",
  "nb.restore": "Restaurer",
  "nb.reload": "Recharger depuis le disque",
  "nb.root": "Chemin du dossier",
  "nb.rootHint": "Choisissez un dossier sur l’ordinateur connecté. Les fichiers Markdown et les pièces jointes restent à leur emplacement actuel.",
  "nb.saveCopy": "Enregistrer comme nouvelle note",
  "nb.saved": "Enregistré sur le disque",
  "nb.saving": "Enregistrement…",
  "nb.search": "Rechercher des notes…",
  "nb.searchAllVaults": "Toutes les bases de connaissances",
  "nb.searchCount": (count: string) => `${count} résultats`,
  "nb.searchEmpty": "Aucune note ne correspond à cette recherche.",
  "nb.searchEmptyAll": "Aucun résultat ne correspond à cette recherche.",
  "nb.searchFuzzy": "Aucune correspondance exacte. Voici des résultats approximatifs.",
  "nb.searchLine": (line: string) => `Ligne ${line}`,
  "nb.searchMatches": (count: string) => `${count} occurrences`,
  "nb.searchMore": "Seuls les premiers résultats sont listés. Affinez la recherche pour voir les suivants.",
  "nb.searchRelated": "Notes liées",
  "nb.searchResults": "Résultats de recherche",
  "nb.searchScope": "Portée de la recherche",
  "nb.searchThisVault": "Cette base de connaissances",
  "nb.skipped": "Ignorés",
  "nb.split": "Vue partagée",
  "nb.tooLarge": "Le fichier ou la sélection dépasse les limites du carnet.",
  "nb.trash": "Corbeille",
  "nb.trashHint": "Déplacer cet élément dans la corbeille du carnet. Vous pourrez le restaurer ultérieurement.",
  "nb.unsaved": "Modifications non enregistrées",
  "nb.vaults": "Bases de connaissances",
  "nb.view": "Mode d’affichage",
  "nb.welcome": "Vos carnets",
  "nb.welcomeText": "Écrivez librement, reliez vos idées et conservez vos notes dans des fichiers locaux ordinaires. Ouvrez un dossier Markdown existant ou importez vos documents dans un nouveau carnet.",
  "memory.globalMemory": "Base de connaissances des sessions",
  "memory.collections": "Sessions archivées",
  "memory.collectionConversation": "Conversation",
  "memory.collectionEmptyEntries": "Cette conversation n'a pas encore d'articles de connaissance.",
  "memory.title": "Base de connaissances",
  "memory.add": "Organiser dans la base de connaissances des sessions",
  "memory.intro": "Organisez les connaissances par projet et par session. Les articles enregistrés restent indépendants de leurs sources et peuvent être modifiés manuellement.",
  "memory.entries": "Articles de connaissance",
  "memory.emptyJobs": "Aucune synthèse pour le moment.",
  "memory.jobs": "Historique des synthèses",
  "memory.search": "Rechercher dans les titres et le contenu…",
  "memory.empty": "Aucun article correspondant. Générez des articles à partir d’une session ou créez-en un manuellement.",
  "memory.emptyDetail": "Sélectionnez un article pour consulter son contenu, ses liens et ses sources.",
  "memory.new": "Nouvel article",
  "memory.titleField": "Titre",
  "memory.summary": "Résumé",
  "memory.content": "Contenu (Markdown)",
  "memory.tags": "Étiquettes (séparées par des virgules)",
  "memory.related": "Articles associés",
  "memory.backlinks": "Liens vers cet article",
  "memory.sources": "Sources",
  "memory.history": "Historique des révisions",
  "memory.restore": "Restaurer cette révision",
  "memory.restoreConfirm": "Restaurer cette révision en créant une nouvelle version ? La version actuelle restera dans l’historique.",
  "memory.deleteConfirm": "Supprimer cet article et ses révisions ? Les sessions sources seront conservées.",
  "memory.groupDeleteConfirm": (count: string) => `Supprimer les ${count} articles de connaissance de ce groupe ? Le projet ou la session est conservé.`,
  "memory.export": "Exporter en Markdown",
  "memory.selectAgent": "Agent",
  "memory.model": "Modèle (facultatif)",
  "memory.modelHint": "Laissez ce champ vide pour utiliser le modèle configuré dans l’agent.",
  "memory.compile": "Organiser et enregistrer",
  "memory.compileHelp": "L’agent sélectionné traitera cette session. Une nouvelle génération remplacera les entrées déjà générées pour cette session, y compris les modifications manuelles. Le texte de la session sera envoyé au modèle par l’intermédiaire de l’agent configuré.",
  "memory.unavailable": "Non installé ou non configuré",
  "memory.allTags": "Toutes les étiquettes",
  "memory.updated": "Mise à jour récente",
  "memory.titleSort": "Titre",
  "memory.sourceNote": "Cet instantané conserve le texte utilisé pour la synthèse, même si la session d’origine est supprimée.",
  "memory.noKnowledge": "Aucune connaissance réutilisable n’a été extraite. Les entrées précédemment générées pour cette session ont été supprimées.",
  "memory.queued": "En attente de démarrage",
  "memory.cancelling": "Annulation en cours",
  "memory.schedulingHint": "Différentes sessions peuvent être traitées en parallèle. Une nouvelle demande annule toute tâche inachevée pour cette session et la remplace.",
  "memory.waitingHint": "Cette tâche démarrera automatiquement une fois la tâche précédente de cette session arrêtée.",
  "memory.running": "En cours",
  "memory.completed": "Terminé",
  "memory.failed": "Échec",
  "memory.cancelled": "Annulé",
  "memory.extract": "Extraction des thèmes",
  "memory.merge": "Fusion des connaissances",
  "memory.commit": "Enregistrement des articles",
  "memory.done": "Enregistré",
  "memory.closeHint": "Vous pouvez fermer cette fenêtre pendant le traitement et suivre sa progression dans l’historique des synthèses.",
  "memory.conflict": "Cet article a changé pendant l’opération. Rechargez-le avant de réessayer ; vos modifications n’ont pas été enregistrées.",
  "memory.duplicate": "Un article porte déjà ce titre. Ouvrez-le pour y intégrer le contenu.",
  "memory.notFound": "Cet article, cette source ou cette tâche n’existe plus.",
  "memory.noTranscript": "Aucune conversation lisible n’est disponible pour cette session.",
  "memory.agentUnavailable": "L’agent choisi est indisponible. Vérifiez le chemin de son exécutable dans les paramètres.",
  "memory.invalid": "Certains champs ou liens sont invalides. Vérifiez le titre, le contenu et les articles associés.",
  "memory.processFailed": "L’agent n’a pas pu terminer. Vérifiez sa connexion, son modèle et ses paramètres CLI, puis réessayez.",
  "memory.timeout": "Le délai de l’agent est dépassé. Réessayez avec un modèle disponible ou une conversation plus courte.",
  "memory.interrupted": "La synthèse a été interrompue. Réessayez à partir de l’instantané source enregistré.",
  "memory.tooLarge": "La source, le contexte ou la sortie dépasse la taille prise en charge. Aucun contenu n’a été tronqué ni enregistré.",
  "memory.invalidOutput": "L’agent a renvoyé des données structurées invalides. Rien n’a été enregistré ; réessayez ou choisissez un autre agent.",
  "memory.loadError": "Impossible de charger la base de connaissances. Vérifiez la connexion et réessayez.",
  "memory.unsaved": "Abandonner les modifications non enregistrées ?",
  "memory.source": "Instantané source",

  // ── Common ──
  "common.cancel": "Annuler", // Cancel
  "common.confirm": "OK", // OK
  "common.delete": "Supprimer", // Delete
  "common.save": "Enregistrer", // Save
  "common.create": "Créer", // Create
  "common.close": "Fermer", // Close
  "chat.imageViewOriginal": "Afficher l’image originale",
  "chat.imageCopy": "Copier l’image",
  "chat.imageSave": "Enregistrer l’image",
  "chat.imageActionFailed": "Impossible d’effectuer cette opération sur l’image. Veuillez réessayer.",
  "common.copy": "Copier", // Copy
  "common.cut": "Couper", // Cut
  "common.paste": "Coller", // Paste
  "common.selectAll": "Tout sélectionner", // Select All
  "common.copied": "Copié", // Copied
  "common.copyFailed": "La copie a échoué. Veuillez réessayer.",
  "chat.sync.loading": "Synchronisation de la conversation…",
  "chat.sync.failed": "Échec de la synchronisation. Les messages déjà chargés restent disponibles.",
  "chat.sync.history": "Charger les messages précédents",
  "chat.submission.updateRequired": "Mettez le serveur à jour avant d’envoyer des messages depuis ce client.",
  "chat.submission.sending": "Envoi en cours…",
  "chat.submission.sent": "Envoyé",
  "chat.submission.queued": "En attente",
  "chat.submission.failed": "Échec de l’envoi",
  "chat.submission.unknown": "Envoi non confirmé",
  "chat.submission.check": "Vérifier l’état",
  "common.retry": "Réessayer", // Retry
  "common.experimental": "Expérimental",
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
  "titlebar.themeSystem": (resolved) =>
    `Suivre le système (actuellement ${resolved})`, // Follow system (currently {resolved})
  "titlebar.themeDark": "Sombre", // Dark
  "titlebar.themeLight": "Clair", // Light
  "titlebar.gameCenter": "Centre de jeux",
  "titlebar.browser": "Navigateur intégré", // Built-in Browser
  "titlebar.remoteAccess": "Accès distant (navigateur)", // Remote Access (Browser)
  "titlebar.connectRemote": "Se connecter à un serveur distant", // Connect to Remote Server
  "titlebar.mirrored": "Miroir", // Mirrored
  "titlebar.mirroredHint":
    "La duplication est activée : onglets, volets et session active suivent l'hôte. Le commutateur se trouve sur l'hôte.", // Mirroring is on: tabs, splits, and the active session follow the host. The switch is on the host.
  "titlebar.mirroredBy": (n: number) => `Miroir par ${n}`, // Mirrored by {n}
  "titlebar.mirroredByHint": (n: number) =>
    `${n} client${n === 1 ? "" : "s"} distant${n === 1 ? " est connecté" : "s sont connectés"}. Les onglets, les divisions et la session active sont partagés, et chaque côté peut les réorganiser.`, // {n} remote clients are connected. Tabs, splits, and the active session are shared, and either side can rearrange them.
  "titlebar.clientsTitle": "Clients connectés", // Attached clients
  "titlebar.clientUnnamed": "Client sans nom", // Unnamed client
  "titlebar.clientSince": (time: string) => `depuis ${time}`, // since {time}
  "titlebar.feedback": "Votre avis", // Feedback
  "titlebar.share": "Partager", // Share
  // ── Alt-triggered menu bar (Windows/Linux) ──
  "menubar.file": "Fichier", // File
  "menubar.terminal": "Terminal",
  "menubar.help": "Aide", // Help
  "menubar.newTerminal": "Nouveau terminal", // New Terminal
  "menubar.visitWebsite": "Visiter le site web", // Visit Website
  "menubar.sendFeedback": "Envoyer un commentaire", // Send Feedback
  "menubar.clearBadges": "Effacer les pastilles de notification", // Clear Notification Badges
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
  "settings.agentDefaultsTitle": "Valeurs par défaut des nouvelles sessions",
  "settings.referSummaryTitle": "Contexte des références de session",
  "settings.referSummaryMode": "Mode de contexte",
  "settings.referSummaryFull": "Transcription intégrale",
  "settings.referSummaryFirst": "Résumer d’abord",
  "settings.referSummaryAgent": "Agent de synthèse",
  "settings.referSummaryHint":
    "Par défaut, vrefer --ask transmet la transcription intégrale à l’agent qui répond. « Résumer d’abord » utilise l’unique agent, modèle et niveau de réflexion choisis ici pour la compresser ; la réponse finale reçoit aussi des extraits pertinents du texte original.",
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
  "settings.cliHint":
    "Ajoute `vela <chemin-du-projet>` au PATH, comme la commande `code` de VS Code.",
  "settings.agentArgsHint":
    "Arguments de lancement par défaut appliqués aux nouvelles sessions de chaque type d'agent. Les arguments définis par session lors de la création ou de la modification les remplacent. Laisser vide pour aucun.", // Agent default launch args hint
  "settings.agentPathLabel": "Chemin de l'exécutable (facultatif)", // Executable path (optional)
  "settings.agentPathPlaceholder":
    "ex. ~/.local/bin/claude — vide = recherche dans le PATH", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "Si défini, les sessions de ce type se lancent via ce chemin complet au lieu de chercher la commande dans le PATH. Utile quand l'agent est installé mais absent du PATH du shell. Rempli automatiquement après une installation en un clic si l'emplacement est détecté.", // Agent executable path hint
  "settings.agentDefaultView": "Vue par défaut", // Default view
  "settings.agentDefaultViewHint":
    "Vue dans laquelle s'ouvrent les nouvelles sessions de cet agent. Les sessions existantes gardent la vue avec laquelle elles ont été créées.", // Agent default view hint
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
  "settings.usageAuto": "Usage auto-refresh", // Usage auto-refresh
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.autoContinue": "Reprendre après réinitialisation", // Continue after limit resets
  "settings.autoContinueHint": "Lorsqu’une limite d’utilisation de 5 heures ou hebdomadaire interrompt Claude ou Codex, la tâche reprend automatiquement après la réinitialisation de la limite.", // When a 5-hour or weekly usage limit stops Claude or Codex, the task continues automatically after the limit resets.
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
  "spawn.title": "Démarrer une session enfant",
  "spawn.fromSession": "Session à l’origine de la demande",
  "spawn.promptLabel": "Instructions de la tâche",
  "spawn.agentLabel": "Type de session",
  "spawn.worktreeLabel": "Worktree distinct",
  "spawn.modelLabel": "Modèle",
  "spawn.effortLabel": "Effort de raisonnement",
  "spawn.modelDefault": "Valeur par défaut de l’agent",
  "spawn.modelLoading": "Chargement des modèles…",
  "spawn.modelListUnavailable": "Liste indisponible. Vous pouvez saisir un identifiant.",
  "spawn.launch": "Démarrer la session",
  "spawn.remaining": (n: number) => `${n} autres demandes à examiner`,
  "spawn.notifyTitle": "Session enfant en attente de confirmation",
  "orch.title": "Démarrer plusieurs sessions",
  "orch.notifyTitle": "Démarrage groupé en attente de confirmation",
  "orch.coordinatorName": "État des sessions",
  "orch.sharedSettings": "Paramètres communs",
  "orch.agentLabel": "Agent",
  "orch.modelLabel": "Modèle",
  "orch.effortLabel": "Effort de raisonnement",
  "orch.nameLabel": "Nom de la session",
  "orch.promptLabel": "Instructions de la tâche",
  "orch.worktreeLabel": "Worktree Git",
  "orch.worktreeNone": "Dossier actuel",
  "orch.worktreeShared": "Worktree partagé",
  "orch.worktreeEach": "Un worktree par session",
  "orch.follow": "Utiliser les paramètres communs",
  "orch.overridden": "Paramètres individuels",
  "orch.remove": "Retirer la tâche",
  "orch.launch": (n: number) => `Démarrer ${n} sessions`,
  "orch.modelPlaceholder": "Valeur par défaut de l’agent",
  "orch.effortPlaceholder": "Valeur par défaut de l’agent",
  "launch.terminalHint": "Un terminal classique ouvre le dossier de travail. Il n’exécute pas automatiquement les instructions de la tâche.",
  "launch.optionsError": "Impossible de charger les options. Réessayez avant de démarrer.",
  "launch.singleIntro": "Vérifiez la tâche et les paramètres avant de démarrer une session enfant.",
  "launch.taskHint": "Ces instructions constituent le premier message envoyé à la session enfant.",
  "launch.runtime": "Paramètres d’exécution",
  "launch.directory": "Dossier de travail",
  "launch.directoryCurrentHint": "Les sessions modifient les fichiers dans le dossier d’origine.",
  "launch.directorySharedHint": "Toutes les sessions utilisent le même nouveau dossier et la même branche.",
  "launch.directoryEachHint": "Chaque session dispose de son propre dossier et de sa propre branche.",
  "launch.worktreeHint": "Les worktrees partent du commit actuel, sans les modifications non validées. En cas d’échec de création, le dossier d’origine est utilisé.",
  "launch.singleResult": "La session enfant apparaît sous sa session parente dans la barre latérale.",
  "launch.startError": "Le démarrage a échoué. Vérifiez les paramètres et réessayez.",
  "launch.starting": "Démarrage…",
  "launch.batchIntro": "Vérifiez les paramètres communs, puis sélectionnez chaque tâche pour modifier ses instructions.",
  "launch.sessionCount": (n: number) => `Sessions : ${n}`,
  "launch.batchName": "Nom du groupe de tâches",
  "launch.sharedHint": "S’applique aux sessions sans paramètres individuels.",
  "launch.tasks": "Tâches",
  "launch.incomplete": "À compléter",
  "launch.undoRemove": "Annuler le retrait",
  "launch.taskNumber": (n: number) => `Tâche ${n}`,
  "launch.taskSettings": "Paramètres de cette session",
  "launch.taskAgent": "Agent de cette session",
  "launch.sharedDirectoryLocked": "Toutes les sessions de ce groupe utilisent un même worktree.",
  "launch.resetSettings": "Rétablir les paramètres communs",
  "launch.monitorHint": "Un terminal « État des sessions » indique quelles sessions travaillent ou attendent une saisie. Il ne mesure pas le taux d’achèvement des tâches.",
  "launch.taskIncomplete": (n: number) => `Complétez le nom et les instructions de la tâche ${n}.`,
  "launch.batchResult": "Chaque tâche démarre dans une session interactive distincte.",
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
  "git.commitCount": (n: number) =>
    n === 1 ? "Valider 1 fichier" : `Valider ${n} fichiers`,
  "git.commitNoFiles": "Aucun changement de fichier dans ce commit",
  "git.noCommits": "Aucun commit",
  "git.loadMore": "Charger plus",
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
  "tree.moveGroupToWorktree": "Déplacer vers un worktree…",
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
  "settings.termLineHeight": "Interligne du terminal",
  "settings.chatTypography": "Vue de conversation",
  "settings.chatTypographyHint": "Ces paramètres de police sont indépendants du terminal et prennent effet immédiatement.",
  "settings.chatFont": "Police de la conversation",
  "settings.chatFontSize": "Taille de police de la conversation",
  "settings.chatLineHeight": "Interligne de la conversation",
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
  "settings.fontListUnavailable": "Impossible de récupérer la liste des polices système. Vous pouvez saisir un nom de police manuellement.",
  "settings.fontUnconfirmed": "Impossible de confirmer la disponibilité de cette police.",
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
  "settings.notifyUnsupported":
    "Notifications aren't available in this environment.", // TODO translate
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
  "settings.scHint":
    "Cliquez sur un raccourci, puis appuyez sur une nouvelle combinaison (Cmd/Ctrl requis).", // hint
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
  "remote.moreUrls": (n: number) =>
    `${n} autre${n > 1 ? "s" : ""} lien${n > 1 ? "s" : ""}`, // N more urls
  "remote.lessUrls": "Réduire", // Show less
  "remote.stop": "Arrêter le serveur", // Stop Server
  "remote.passwordPlaceholder": "Définir le mot de passe d'accès", // Set access password
  "remote.starting": "Démarrage…", // Starting…
  "remote.start": "Démarrer le serveur", // Start Server
  "remote.portLabel": "Port", // Port
  "remote.portInvalid": "Le port doit être compris entre 1 et 65535", // Port must be between 1 and 65535
  "remote.ipLabel": "IP", // IP address
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
  "connect.sshFingerprintLabel": (kt: string) =>
    `Empreinte de la clé d'hôte SSH (${kt})`,
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
  "connect.showPassword": "Afficher le mot de passe",
  "connect.hidePassword": "Masquer le mot de passe",
  "connect.urlPasswordPlaceholder": "Mot de passe de connexion",
  "connect.mirror": "Refléter l'application de bureau distante", // Mirror the remote desktop app
  "connect.mirrorHint":
    "Les onglets, les divisions et la session active sont les mêmes que dans l'application de bureau de la machine distante ; toute modification d'un côté apparaît des deux côtés. Si l'application de bureau n'est pas lancée, cette connexion ouvre directement sa base de données, ou une base de données séparée s'il n'y en a pas.", // Same tabs, splits, and active session as the desktop app on the remote machine; changes on either side show on both. If the desktop app is not running, this connection opens its database directly, or a separate database when there is none.
  "connect.shareDesktopDb":
    "Utiliser la base de données de l'app de bureau distante",
  "connect.shareDesktopDbHint":
    "Partage une base de données avec l'app de bureau de la machine distante (idéalement même version des deux côtés). Désactivé = base de données isolée.",

  // ── Sidebar ──
  "tree.newSession": "Nouvelle session", // New Session
  "tree.newTerminalSession": "Nouveau terminal", // New Terminal
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
  "tree.collectionInfo": "Infos de la collection", // Collection Info
  "tree.projectInfo": "Infos du projet", // Project Info
  "info.branch": "Branche", // Branch
  "info.path": "Chemin", // Path
  "info.recentCommits": "Commits récents", // Recent Commits
  "info.noCommits": "Aucun commit", // No commits
  "tree.killProcess": "Tuer le processus", // Kill Process
  "tree.killProcessConfirm": (name: string) => `Terminer le processus de « ${name} » ? La tâche en cours sera interrompue. L’historique de conversation et les fichiers enregistrés seront conservés.`,
  "tree.archiveSession": "Archiver la session", // Archive Session
  "tree.archiveGroup": "Archiver le groupe", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "temp", // scratch
  "tree.persistSession": "Convertir en session permanente…", // Make Permanent Session…
  "tree.persistDoc": "Enregistrer sur le disque…", // Save to Disk…
  "tree.closeScratch": "Fermer le brouillon", // Close Scratch
  "tree.importProject": "Importer un projet", // Import Project
  "tree.createProject": "Créer un projet",
  // New Collection / Collection name / research / Create Collection / No folder / Delete Collection
  "tree.newCollection": "Nouvelle collection",
  "tree.deleteCollection": "Supprimer la collection",
  "collection.title": "Nouvelle collection",
  "collection.name": "Nom de la collection",
  "collection.namePlaceholder": "research",
  "collection.submit": "Créer la collection",
  "collection.tag": "Aucun dossier",
  "collection.deleteTitle": "Supprimer la collection",
  "collection.deleteBody": (name) =>
    `Supprimer la collection « ${name} » ? Tous ses groupes et sessions seront également supprimés. Cette action est irréversible.`,
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
  "clone.slowHint":
    "Aucune progression depuis 30 secondes. Vérifiez le réseau ou le proxy de la machine distante ; vous pouvez annuler puis réessayer.",
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
  "tree.viewCount": (n) =>
    `${n} vue${n > 1 ? "s" : ""} arborescente${n > 1 ? "s" : ""}`,
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
  "tree.noProjectsPre":
    "Aucun projet pour l'instant. Cliquez sur l'icône de dossier ou appuyez sur ", // No projects yet. Click the folder button, or press
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
  "tree.engineLabel": "S'ouvre en",
  "tree.engineTui": "Vue terminal",
  "tree.engineChat": "Vue conversation",
  // The agent runs its own terminal interface.
  "tree.engineTuiHint": "L'agent s'exécute dans sa propre interface de terminal.",
  // Messages and tool cards, with buttons for permission questions.
  "tree.engineChatHint": "Présentation sous forme de messages et de cartes d'outils ; les demandes d'autorisation sont traitées dans l'interface.",
  "tree.agentArgsLabel": "Arguments de lancement (optionnel)", // Launch args (optional)
  // Working directory / Leave empty for the default
  "tree.workingDirLabel": "Répertoire de travail",
  "tree.workingDirPlaceholder": "Laisser vide pour le répertoire par défaut",
  "preset.execPathLabel": "Exécutable (facultatif)",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint":
    "Laissez vide pour utiliser la commande configurée de l'agent. Renseignez-le pour que cette session seule utilise un remplaçant compatible.",
  "preset.saveLabel": "Enregistrer comme préréglage",
  "preset.namePlaceholder": "Nommer ce préréglage",
  "preset.iconChoose": "Choisir une icône",
  "preset.iconClear": "Retirer",
  "preset.iconHint":
    "Les images carrées conviennent le mieux ; les autres sont recadrées et réduites en 64x64.",
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
  "info.notYetCaptured":
    "Pas encore généré (capturé après la première exécution)", // Not yet generated (captured after first run)
  "info.sessionId": "ID de session", // Session ID
  "info.projectId": "ID du projet", // Project ID
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
  "importSessions.results": ({ count }: { count: number }) => `Résultats : ${count}`,
  "importSessions.selected": ({ count }: { count: number }) => `Sélection : ${count}`,
  "importSessions.clearSelection": "Désélectionner tout",
  "importSessions.clearSearch": "Effacer la recherche",
  "importSessions.noHistory": "Aucune session antérieure trouvée pour le répertoire de ce projet.",
  "importSessions.title": "Importer des sessions",
  "importSessions.description": "Recherchez les sessions Codex, Claude et OpenCode dont le répertoire de travail correspond à ce projet. Sélectionnez les sessions à ajouter au projet, puis ouvrez-en une pour reprendre la conversation.",
  "importSessions.search": "Rechercher par titre, agent ou ID de session",
  "importSessions.empty": "Aucune session correspondante.",
  "importSessions.imported": "Déjà importée",
  "importSessions.confirm": ({ count }: { count: number }) => `Importer (${count})`,
  "importSessions.success": ({ count }: { count: number }) => `Sessions ajoutées au projet : ${count}.`,
  "resume.title": "Reprendre une session", // Resume Session
  "resume.desc":
    "Choisissez le type d'agent et saisissez le session id propre à l'agent ; l'ouverture reprend la conversation d'origine.", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "Type d'agent", // Agent type
  "resume.sessionIdPlaceholder": "Session id de la conversation", // Conversation session id
  "resume.confirm": "Reprendre et ouvrir", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "Nouvelle session worktree…", // New Worktree Session…
  "worktree.worktreeNameLabel": "Nom du worktree", // Worktree name
  "worktree.worktreeNameHint":
    "Sert de nom de répertoire et de branche du worktree.", // Used as the worktree directory and branch name.
  "worktree.createFailed": "Impossible de créer le worktree", // Couldn't create the worktree
  "worktree.noRepoRoot": "Ce projet n'a pas de chemin de dépôt git utilisable.", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "Aucun", // None
  "worktreeSel.modeNew": "Nouveau", // New
  "worktreeSel.modeExisting": "Existant", // Existing
  "worktreeSel.loading": "Chargement des worktrees…", // Loading worktrees…
  "worktreeSel.empty": "Aucun worktree existant dans ce dépôt.", // No existing worktrees in this repository.
  "worktreeSel.loadFailed":
    "Impossible de lister les worktrees (pas un dépôt git ?).", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint":
    "Les sessions créées dans ce groupe utiliseront ce worktree par défaut.", // Sessions created in this group will use this worktree by default.
  "worktree.moveGroupTitle": "Déplacer le groupe vers un worktree",
  "worktree.moveGroupHint":
    "Les sessions créées désormais dans ce groupe utiliseront ce worktree. Celles qui existent déjà conservent leur répertoire actuel.",

  // ── Archive panel ──
  "archive.title": "Sessions archivées", // Archived Sessions
  "archive.empty1": "Aucune session archivée.", // No archived sessions.
  "archive.empty2":
    "Faites un clic droit sur une session dans la barre latérale et choisissez « Archiver la session » pour la ranger ici.", // Right-click a session in the sidebar…
  "archive.restore": "Restaurer en session normale", // Restore to normal session
  "archive.export": "Exporter le contexte complet en Markdown", // Export full context as Markdown
  "archive.deleteForever": "Supprimer définitivement (avec l'enregistrement)", // Delete permanently (with recording)
  "archive.pickOne":
    "Sélectionnez une session archivée à gauche pour voir sa transcription", // Select an archived session on the left…
  "archive.recordingEnd": "--- Fin de l'enregistrement ---", // --- End of recording ---
  "archive.readRecordingFailed": (err) =>
    `Échec de lecture de l'enregistrement : ${err}`, // Failed to read recording: {err}
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
  "search.hint":
    "Recherchez le contenu des sessions. Les sessions archivées sont exclues par défaut — cochez « Inclure les archives » pour les ajouter.", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "Inclure les archives", // Include archived
  "search.includeArchivedHint":
    "Rechercher aussi dans les sessions archivées (désactivé par défaut)", // Also search archived sessions (off by default)
  "search.searching": "Recherche…", // Searching…
  "search.noResults": "Aucune correspondance", // No matches found
  "search.sessionCount": (n) => (n === 1 ? "1 session" : `${n} sessions`), // n sessions
  "search.matchCount": (n) =>
    n === 1 ? "1 correspondance" : `${n} correspondances`, // n matches
  "search.pickSession":
    "Sélectionnez une session à gauche pour voir ses correspondances", // Select a session on the left to see its matches
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
  "center.noSessionHintPre":
    "Choisissez une session dans la barre latérale, ou appuyez sur ", // Pick a session from the sidebar, or press
  "center.noSessionHintPost": " pour créer un terminal", // to create a terminal
  "center.createTerminal": "Créer un terminal", // Create Terminal
  "tab.unsavedDot": "Modifications non enregistrées", // Unsaved changes
  "tab.newTerminal": "Nouveau terminal", // New terminal
  "tab.newDocument": "Nouveau document", // New document
  "tab.bgTitle": (n) =>
    `Onglets maintenus en arrière-plan : ${n} (processus toujours actifs)`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `Arrière-plan ${n}`, // Background {n}
  "tab.scratchFallback": "(terminal temporaire)", // (scratch terminal)
  "tab.killBgTab":
    "Fermer cet onglet d'arrière-plan (ses processus se termineront)", // Kill this background tab…
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
  "browser.desktopOnly":
    "Les onglets de navigateur s'ouvrent uniquement dans l'application de bureau.", // Browser tabs open in the desktop app only.
  "browser.stop": "Arrêter le chargement", // Stop loading
  "browser.openExternal": "Ouvrir dans le navigateur système", // Open in system browser
  "browser.addressPlaceholder": "Saisir une URL ou des termes de recherche", // Enter URL or search terms
  "browser.quickAccess": "Accès rapide", // Quick access
  "browser.loading": "Chargement…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "Quitter VelaTerm ?", // Quit VelaTerm?
  "quit.body":
    "Toutes les sessions de terminal et d'agent en cours seront arrêtées.", // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "Enregistrer l'espace de travail", // Save workspace
  "quit.saveWorkspaceHint":
    "Rouvrir les mêmes onglets et divisions la prochaine fois. Les terminaux sont restaurés, mais pas redémarrés.", // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "Quitter", // Quit
  "dormant.body":
    "Restauré depuis l'espace de travail enregistré. Aucun processus n'est encore en cours.", // Restored from your saved workspace. No process is running yet.
  "dormant.start": "Démarrer", // Start
  "overlimit.title": (max) => `Limite d'arrière-plan dépassée (${max})`, // Background keep-alive over limit ({max})
  "overlimit.body":
    "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
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
  "term.mirrorBadge": (dims) =>
    `⤢ Miroir${dims} · cliquer pour adapter à cette fenêtre`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) =>
    `⤢ Miroir${dims} · adapter à cette fenêtre`, // ⤢ Mirror{dims} · fit this window
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
  "agentInstall.pathSaved": (label: string) =>
    `Chemin de l'exécutable de ${label} enregistré dans les réglages :`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} est installé`, // {label} is installed
  "agentInstall.doneDesc":
    "Relancez cette session pour commencer à l'utiliser.", // Relaunch this session to start using it.
  "agentInstall.restartNow": "Relancer maintenant", // Relaunch now
  "agentInstall.later": "Plus tard", // Later
  "agentInstall.pathLabel": "Chemin de l'exécutable", // Executable path
  "agentInstall.pathPlaceholder": (bin: string) => `~/.local/bin/${bin}`,
  "agentInstall.pathHint": "Déjà installé en dehors du PATH ? Indiquez le chemin complet de l'exécutable.", // Already installed outside PATH?
  "agentInstall.pathSave": "Utiliser ce chemin", // Use this path
  "agentInstall.pathBrowse": "Parcourir…", // Browse…
  "search.placeholder": "Rechercher dans le terminal", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "WYSIWYG", // WYSIWYG
  "doc.visual": "Visuel",
  "doc.source": "Code source",
  "doc.compare": "Comparaison",
  "doc.editorLoadFailed": "Impossible de charger l’éditeur Markdown.",
  "doc.imageOnly": "Seuls les fichiers image peuvent être insérés ici.",
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
  "doc.overwriteConfirm":
    "Un fichier portant ce nom existe déjà. Cliquez sur « Remplacer » pour le remplacer.", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "Enregistrer", // Save
  "doc.externalChanged":
    "Le fichier a été modifié sur le disque (vous avez des modifications locales non enregistrées).", // The file was modified on disk…
  "doc.reloadDiscard": "Recharger (abandonner mes modifications)", // Reload (discard my changes)
  "doc.externalChangedClean": "Le fichier a été modifié sur le disque.", // The file was modified on disk.
  "doc.reload": "Recharger", // Reload
  "doc.ignore": "Ignorer", // Ignore
  "doc.loadingFile": (title) => `Chargement de ${title}…`, // Loading {title}…
  "doc.closeTitle": "Fermer le document", // Close Document
  "doc.unsavedBody": (title) =>
    `« ${title} » a des modifications non enregistrées.`, // "{title}" has unsaved changes.
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
  "doc.imgDecodeFailed":
    "Impossible d'afficher cette image (format non pris en charge ou fichier corrompu).", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "Ajuster", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "Exporter en PDF", // Export PDF
  "doc.diagramError": "Erreur de diagramme", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "Aucune session sélectionnée", // No session selected
  "panel.collapseSection": "Réduire la section", // Collapse section
  "panel.expandSection": "Développer la section", // Collapse section
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
  "files.dblClickOpen": "Double-cliquer pour ouvrir",
  "files.deleteConfirm": (name) =>
    `Supprimer « ${name} » ? Cette action est irréversible.`, // Delete "{name}"? This can't be undone.

  // ── File transfer (remote access) ──
  "transfer.uploadsTitle": "Envois", // Uploads
  "transfer.download": "Télécharger", // Download
  "transfer.upload": "Envoyer des fichiers…", // Upload Files…
  "transfer.uploadTooltip": "Envoyer des fichiers dans ce dossier", // Upload files to this folder
  "transfer.clear": "Effacer", // Clear
  "transfer.cancelled": "Annulé", // Cancelled
  "transfer.failed": "Échec", // Failed
  "transfer.stalled": "Reconnexion…", // Reconnecting…
  "transfer.foldersUnsupported": "Les dossiers ne peuvent pas être envoyés.", // Folders can't be uploaded.

  // ── Status bar ──
  "statusbar.sessions": (n) => (n === 1 ? "1 session" : `${n} sessions`), // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `Cliquez pour n'afficher que les sessions « ${label} » dans la barre latérale (cliquez à nouveau pour annuler)`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `Arrière-plan ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `Onglets maintenus en arrière-plan (limite ${max} ; au-delà, le plus ancien onglet inactif est fermé automatiquement)`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) =>
    `Onglet d'arrière-plan fermé : ${name} (limite dépassée)`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) => `Accès distant navigateur activé : ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "Droits : Demander", // Perms: Ask
  "statusbar.permSkip": "Droits : Ignorer", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip":
    "Mode d'autorisation de cette session · cliquez pour changer (cette session uniquement)", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "Autorisations de cette session", // This session's permissions
  "statusbar.permOptAsk": "Demander à chaque fois (par défaut)", // Ask each time (default)
  "statusbar.permScopeHint":
    "S'applique uniquement à cette session. Pour les réglages globaux, rendez-vous dans Réglages ▸ Agents.", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg":
    "Autorisation modifiée. La session doit redémarrer pour l'appliquer. Le redémarrage reprend la conversation en cours mais interrompt toute tâche en cours. Redémarrer maintenant ?", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "Redémarrer", // Restart now
  "statusbar.permRestartLater": "Plus tard", // Later
  "statusbar.permScopeTitle": "Appliquer à ?", // Apply to?
  "statusbar.permScopeSession": "Cette session uniquement", // This session only
  "statusbar.permScopeGlobal": "Valeur par défaut globale", // Global default
  "statusbar.permScopeGlobalHint":
    "S'applique maintenant à cette session et devient la valeur par défaut pour les futures sessions de ce type (synchronisé avec les Réglages).", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

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
  "err.uncaughtDesc":
    "Les informations ci-dessous peuvent aider à localiser le problème.", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser":
    "La relecture des enregistrements n'est pas encore prise en charge dans le navigateur", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) => `Échec d'envoi de l'image (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.showPassword": "Afficher",
  "login.hidePassword": "Masquer",
  "login.passwordSaveFailed": "La connexion est établie, mais le mot de passe n’a pas pu être enregistré sur cet appareil. Réessayez.",
  "login.connecting": "Connexion…", // Connecting…
  "login.remoteAccess": "Accès distant", // Remote Access
  "login.desc":
    "Saisissez le mot de passe d'accès pour vous connecter à ce terminal.", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "Mot de passe d'accès", // Access password
  "login.connect": "Se connecter", // Connect
  "login.wrongPassword": "Mot de passe incorrect", // Wrong password
  "login.rateLimited":
    "Trop de tentatives. Veuillez patienter une minute avant de réessayer.", // Too many attempts. Please wait a minute and try again.
  "login.failed": "Échec de connexion, veuillez réessayer", // Login failed, please try again
  "login.pairingRequired":
    "Ce serveur nécessite un lien d'association. Ouvrez le lien généré dans le panneau Accès distant de l'application de bureau.", // This server requires a pairing link
  "login.authFailed":
    "Échec de l'authentification. Vérifiez le mot de passe d'accès, ou ouvrez un nouveau lien d'association s'il a été régénéré.", // Authentication failed, check password or use a new pairing link
  "dir.title": "Choisir le répertoire du projet", // Choose Project Directory
  "dir.pathPlaceholder":
    "Rechercher, ou saisir un chemin puis Entrée (supporte ~)", // Search, or type a path and press Enter (supports ~)
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
  "conn.sshDown":
    "Liaison SSH interrompue — appuyez sur « Reconnecter maintenant » pour réessayer", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "Échec de la requête", // Request failed
  "reqerr.dismiss": "Fermer", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "Journal des erreurs", // Error Log
  "errlog.empty": "Aucune erreur enregistrée.", // No errors recorded.
  "errlog.copyAll": "Tout copier", // Copy all
  "errlog.clear": "Effacer", // Clear
  "errlog.close": "Fermer", // Close

  // ── Mobile ──
  "mobile.backConnections": "Retour aux connexions",
  "mobile.loadSlow": "Le chargement prend plus de temps que prévu. Vous pouvez réessayer ou revenir à vos connexions.",
  "mobile.connectionUnavailable": "Connexion indisponible",
  "mobile.pushTitle": "Notifications de tâches",
  "mobile.pushHint": "Les notifications affichent le nom de la session et un court aperçu de la réponse, y compris en arrière-plan ou lorsque l’écran est verrouillé. Ce texte est transmis à velaterm.com et au service de notifications push. Les mots de passe de connexion et les clés privées SSH ne sont pas envoyés.",
  "mobile.pushEnable": "Activer les notifications",
  "mobile.pushDisable": "Désactiver les notifications",
  "mobile.pushTest": "Envoyer une notification de test",
  "mobile.pushTestSent": "La notification de test est en attente d’envoi. Consultez le centre de notifications du système.",
  "mobile.pushDisabled": "Les notifications en arrière-plan sont désactivées.",
  "mobile.pushEnabled": "Les notifications en arrière-plan sont activées.",
  "mobile.pushNotConfigured": "Aucun service de notifications push n’est configuré dans cette version.",
  "mobile.pushDenied": "Autorisez les notifications dans les paramètres du système.",
  "mobile.pushRegistrationFailed": "L’enregistrement de l’appareil a échoué. Veuillez réessayer.",
  "mobile.pushRelayUnavailable": "Le relais de notifications est indisponible. Veuillez réessayer.",
  "mobile.pushHostUnavailable": "Les notifications en arrière-plan ne sont pas activées sur l’hôte distant. Mettez-le à jour, puis reconnectez-vous.",
  "mobile.pushDisclosure": "Les notifications en arrière-plan utilisent Getui et le service push du fabricant de votre appareil. Pour les acheminer, ces services traitent les identifiants de l’appareil, les informations réseau, les noms des sessions et de courts aperçus des réponses. Les mots de passe de connexion et les clés privées SSH ne sont pas envoyés.",
  "mobile.pushConnectHint": "Après l’activation, ouvrez une fois chaque connexion pour vous abonner aux notifications.",
  "mobile.pushTarget": "Connexion à tester",
  "mobile.copyConnection": "Copier et modifier",
  "mobile.copyConnectionHint": "Modifiez les paramètres à partir de cette connexion. Les identifiants enregistrés sont repris de manière sécurisée. La connexion d’origine reste inchangée ; si les paramètres sont identiques, la connexion existante est conservée.",
  "mobile.copyConnectionReused": "Ces paramètres sont déjà enregistrés. La connexion existante a été conservée.",
  "mobile.inputOptions": "Options du message",
  "mobile.connections": "Gérer les connexions",
  "mobile.more": "Autres actions",
  "mobile.toDesktop": "Passer à la version bureau", // Switch to desktop
  "mobile.empty1": "Aucune session.", // No sessions.
  "mobile.noMatch": "Aucune session correspondante", // No matching sessions
  "mobile.empty2":
    "Créez-en une sur l'application bureau ou un navigateur d'ordinateur, elle apparaîtra ici automatiquement.", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ Retour", // ‹ Back
  "mobile.selCopy": "Copier", // Copy
  "mobile.selCancel": "Annuler", // Cancel

  // ── Mobile connection client (apps/mobile start page) ──
  "mobile.phaseConnecting": "Connecting over SSH…", // TODO translate
  "mobile.phaseConfirming": "Confirm the host fingerprint", // TODO translate
  "mobile.phasePreparing": "Checking or preparing the remote service…", // TODO translate
  "mobile.phaseForwarding": "Opening the SSH tunnel…", // TODO translate
  "mobile.phaseReady": "Connected", // TODO translate
  "mobile.phaseDisconnected": "Disconnected", // TODO translate
  "mobile.phaseError": "Connection failed", // TODO translate
  "mobile.accountAndLogin": "Account and sign-in", // TODO translate
  "mobile.connectionService": "Connection service unavailable", // TODO translate
  "mobile.nativeOnly": "Connecting is only available in the iOS or Android app. The browser is only for previewing the interface.", // TODO translate
  "mobile.managedRemotely": "Projects and sessions are managed by the remote service.", // TODO translate
  "mobile.buildInfo": (version: string, time: string) => `App v${version} · Built ${time}`, // TODO translate
  "mobile.myDevices": "My devices", // TODO translate
  "mobile.account": "Account", // TODO translate
  "mobile.signedInHint": "Signed in. You can view the workspaces, projects, and sessions shared by devices on this account.", // TODO translate
  "mobile.manageAccount": "Manage account", // TODO translate
  "mobile.signOut": "Sign out", // TODO translate
  "mobile.viewMyDevices": "View my devices", // TODO translate
  "mobile.noDevices": "No devices are signed in to this account yet.", // TODO translate
  "mobile.online": "Online", // TODO translate
  "mobile.offline": "Offline", // TODO translate
  "mobile.deviceNotSharing": "The device is not sharing anything yet.", // TODO translate
  "mobile.scopeMachine": "Entire workspace", // TODO translate
  "mobile.scopeProject": "Project", // TODO translate
  "mobile.scopeSession": "Session", // TODO translate
  "mobile.sharingNotReady": "Shared content is not ready yet. Check the sharing settings on that device.", // TODO translate
  "mobile.deviceOffline": "The device is offline. Open VelaTerm on that device and keep it connected to the network.", // TODO translate
  "mobile.viewShared": "View shared content →", // TODO translate
  "mobile.devicesUnavailable": "Could not load the device list. Please try again.", // TODO translate
  "mobile.accountUnavailable": "Could not load the account status. Check your network and try again.", // TODO translate
  "mobile.signInTitle": "Sign in to VelaTerm", // TODO translate
  "mobile.signInHint": "Sign in with your email and password or a third-party account to see your devices and shared content.", // TODO translate
  "mobile.signIn": "Sign in", // TODO translate
  "mobile.checkSignIn": "Check sign-in status", // TODO translate
  "mobile.waitingSignIn": "Waiting for sign-in confirmation…", // TODO translate
  "mobile.workspaceTitle": "Your workspace", // TODO translate
  "mobile.workspaceHint": "Connect to a remote host and pick up where you left off.", // TODO translate
  "mobile.newSsh": "+ SSH connection", // TODO translate
  "mobile.newUrl": "+ URL connection", // TODO translate
  "mobile.remote": "My devices", // TODO translate
  "mobile.scanToConnect": "Scan QR code to connect", // TODO translate
  "mobile.noConnections": "No saved connections yet. Add an SSH or URL connection, or open Remote to see content shared by devices on your account.", // TODO translate
  "mobile.tapToConnect": "Tap to connect →", // TODO translate
  "mobile.webPasswordSaved": "Access password saved", // TODO translate
  "mobile.deleteConnectionTitle": "Delete connection", // TODO translate
  "mobile.deleteConnectionConfirm": (name: string) => `Delete “${name}” and its saved credentials? Remote projects are not deleted.`, // TODO translate
  "mobile.connectionMissing": "Connection not found", // TODO translate
  "mobile.editConnection": "Edit connection", // TODO translate
  "mobile.addSshHost": "Add SSH connection", // TODO translate
  "mobile.addUrlConnection": "Add URL connection", // TODO translate
  "mobile.connectionName": "Connection name", // TODO translate
  "mobile.serviceUrl": "Service address", // TODO translate
  "mobile.scanToFill": "Fill in from QR code", // TODO translate
  "mobile.openingCamera": "Opening the camera…", // TODO translate
  "mobile.scanCancelled": "Scan cancelled", // TODO translate
  "mobile.scanDone": "Service address detected. Check it, then save and connect.", // TODO translate
  "mobile.scanNativeOnly": "QR scanning is only available in the iOS or Android app.", // TODO translate
  "mobile.webPasswordOptional": "Access password (optional)", // TODO translate
  "mobile.keepPassword": "Leave empty to keep the current password", // TODO translate
  "mobile.webPasswordLater": "You can also enter it after connecting", // TODO translate
  "mobile.webPasswordSavedHint": "The access password is saved and used automatically when you reconnect. Leaving the field empty keeps the saved password.", // TODO translate
  "mobile.webPasswordStorageHint": "The password is kept in the phone’s secure storage. You can also choose to remember it when you enter it after connecting.", // TODO translate
  "mobile.sshHost": "SSH host", // TODO translate
  "mobile.sshHostPlaceholder": "Hostname or IP address", // TODO translate
  "mobile.sshPort": "SSH port", // TODO translate
  "mobile.username": "Username", // TODO translate
  "mobile.authMethod": "Authentication", // TODO translate
  "mobile.authPassword": "Password", // TODO translate
  "mobile.authKeyAndroid": "Private key (OpenSSH Ed25519 / RSA)", // TODO translate
  "mobile.authKey": "Private key (OpenSSH Ed25519)", // TODO translate
  "mobile.sshPassword": "SSH password", // TODO translate
  "mobile.privateKey": "Private key", // TODO translate
  "mobile.keepPrivateKey": "Leave empty to keep the saved private key", // TODO translate
  "mobile.pastePrivateKey": "Paste an OpenSSH private key", // TODO translate
  "mobile.passphraseOptional": "Key passphrase (optional)", // TODO translate
  "mobile.keepPassphrase": "Leave empty to keep the current passphrase", // TODO translate
  "mobile.sshSecretSavedHint": "SSH credentials are kept in the phone’s secure storage. Leave the fields empty while editing to keep them.", // TODO translate
  "mobile.remoteService": "Remote service", // TODO translate
  "mobile.serviceAuto": "Find the VelaTerm service automatically", // TODO translate
  "mobile.serviceManual": "Use an existing service port", // TODO translate
  "mobile.remotePort": "Remote loopback HTTP port", // TODO translate
  "mobile.webPasswordAutoHint": "The access password is saved and used automatically when you reconnect.", // TODO translate
  "mobile.prepareService": "Download and start the VelaTerm service when none is available", // TODO translate
  "mobile.prepareServiceHint": "Automatic preparation writes a signature-verified binary, configuration, and logs to ~/.velaterm/ on the remote host and keeps the service running. It needs Python 3 and an OpenSSL with Ed25519 support; reusing an existing service or specifying its port does not.", // TODO translate
  "mobile.saveConnection": "Save connection", // TODO translate
  "mobile.saveAndConnect": "Save and connect", // TODO translate
  "mobile.loginOpening": "Opening the sign-in page in your browser…", // TODO translate
  "mobile.loginFinishInBrowser": "Complete the sign-in in the browser window, then return to the app.", // TODO translate
  "mobile.loginChecking": "Checking sign-in status…", // TODO translate
  "mobile.loginSuccess": "Signed in.", // TODO translate
  "mobile.loginWaiting": "Waiting for sign-in confirmation. Your account and device list update automatically once sign-in completes.", // TODO translate
  "mobile.loginExpired": "The sign-in request has expired. Please sign in again.", // TODO translate
  "mobile.loginRetrying": "The account service is temporarily unreachable. Retrying. You do not need to sign in again.", // TODO translate

  // ── Other shared components ──
  "splitter.dragToResize": "Glisser pour redimensionner", // Drag to resize
  "transport.wsDisconnected": "WebSocket déconnecté", // WebSocket disconnected
  "transport.wsConnectFailed": "Échec de connexion WebSocket", // WebSocket connection failed
  "transport.cmdFailed": "Échec de la commande", // Command failed
  "transport.remoteCmdForbidden": (cmd: string) =>
    `Commande non disponible pour les clients distants : ${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) =>
    `Clé de paramètre non modifiable par les clients distants : ${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) =>
    `Les clients distants ne peuvent pas accéder aux fichiers du répertoire de données de l'application : ${path}`, // Remote clients cannot access files in the app data directory

  // ── Crepe（éditeur WYSIWYG）──
  "crepe.placeholder":
    "Saisissez du texte, ou tapez / pour le menu d'insertion", // Type text, or press / for the insert menu
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
  "info.collection": "Collection", // Collection
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

  // ── Vue conversation (la session de l'agent lue comme un échange) ──
  "session.showConversation": "Vue conversation",
  "session.showTerminal": "Vue terminal",
  "session.switchTitle": "Changer de vue redémarre l'agent",
  "session.switchBody": "Le tour en cours sera interrompu. La conversation est conservée.",
  "session.switchConfirm": "Basculer",
  "session.terminalViewHint": "Cliquez ici pour revenir à la vue terminal.",
  "session.loading": "Lecture de la conversation…",
  "session.unavailable": "Cette conversation n'est pas encore lisible",
  "session.working": "En cours…",
  "session.thinking": "Raisonnement",
  "session.toolRunning": "en cours",
  "session.toolUnknown": "Outil",
  "session.toolFailed": "Échec",
  "session.toolNoDetail": "Rien d'autre n'a été enregistré",
  "session.showMore": (n: number) => `Afficher ${n} caractères de plus`,
  "session.showLess": "Réduire",
  "session.composerHint": "Écrire à l'agent · Entrée pour envoyer, Maj+Entrée pour aller à la ligne",
  "session.send": "Envoyer",

  // ── Moteur conversationnel (une session pilotée par protocole) ──
  "chat.empty": "Saisissez un message ci-dessous pour commencer la conversation.",
  "chat.interrupt": "Arrêter",
  "chat.interruptTooltip": "Arrêter · Esc",
  "chat.allow": "Autoriser",
  "chat.deny": "Refuser",
  "chat.permissionAsk": (tool: string) => `${tool} demande à s'exécuter`,
  "chat.exited": (code: number) => `L'agent s'est arrêté (code ${code})`,
  "chat.modeNextTurn": "Au prochain tour",
  "chat.modePendingHint": (current: string, next: string) =>
    `Autorisations actuelles : ${current}. ${next} s’appliquera au prochain tour ; le tour en cours se poursuivra sans changement.`,
  "chat.modeTooltip": "Mode d'autorisation",
  "chat.collaborationModeTooltip": "Mode de collaboration",
  "chat.collaborationMode.default": "Par défaut",
  "chat.collaborationMode.defaultHint":
    "Avance directement et ne pose une question que lorsqu’une décision est requise",
  "chat.collaborationMode.plan": "Planification",
  "chat.collaborationMode.planHint":
    "Analyse la demande et prépare un plan ; les questions peuvent utiliser des cartes interactives",
  "chat.moreOptions": "Plus",
  "chat.modelTooltip": "Modèle",
  "chat.keepChoice": "Par défaut",
  "chat.keepChoiceFor": (model) => `Par défaut pour ${model}`,
  "chat.followModelDefault": (agent: string) => `Utiliser le modèle par défaut de ${agent}`,
  "chat.followModelDefaultHint": "Utilise le modèle défini dans la configuration de l’agent.",
  "chat.savedModelDefault": "Par défaut dans l’application",
  "chat.catalogWebsite": "Catalogue de modèles du site",
  "chat.catalogCache": "Catalogue de modèles en cache",
  "chat.catalogBundled": "Catalogue de modèles intégré",
  "chat.catalogChecked": (time: string) => `Dernière vérification : ${time}`,
  "chat.catalogFailed": "Échec de la mise à jour. Le catalogue précédent reste disponible.",
  "chat.catalogRefresh": "Actualiser",
  "chat.modelDefault": "Modèle par défaut",
  "chat.mode.default": "Toujours demander",
  "chat.mode.agentDefault": "Par défaut de l'agent",
  "chat.mode.acceptEdits": "Accepter les modifications",
  "chat.mode.plan": "Mode plan",
  "chat.permissionRestart.unconfirmed": "La connexion a été interrompue. Le changement d’autorisation n’a pas pu être confirmé. Reconnectez-vous pour vérifier les autorisations actuelles de la session.",
  "permission.stateUnavailable": "État des autorisations indisponible",
  "permission.currentUnknown": "Autorisations actuelles non confirmées",
  "permission.notRunning": "À l’arrêt",
  "permission.applied": "Appliqué",
  "permission.nextTurn": "S’applique au prochain message",
  "permission.restart": "S’applique après le redémarrage de cette session",
  "permission.nextStart": "Au prochain lancement",
  "permission.defaultHint": "Autorisation par défaut pour les nouvelles sessions. Les sessions existantes conservent leurs propres autorisations.",
  "chat.permissionRestart.title": "Redémarrer pour désactiver les confirmations ?",
  "chat.permissionRestart.body": "Claude doit redémarrer pour désactiver les confirmations. La réponse en cours sera interrompue, mais l’historique de la conversation sera conservé. Une fois le changement effectué, les confirmations d’autorisation seront ignorées.",
  "chat.permissionRestart.confirm": "Redémarrer et appliquer",
  "chat.permissionRestart.busy": "Redémarrage…",
  "chat.permissionRestart.failed": (detail: string) => "Le changement d’autorisation a échoué. Le mode précédent est conservé. " + detail,
  "chat.permissionRestart.tasks": "Traitez ou retirez les messages en attente et arrêtez les tâches en arrière-plan avant de redémarrer.",
  "chat.permissionRestart.stale": "Le processus de la session a changé. Sélectionnez à nouveau « Sans confirmation ».",
  "chat.permissionRestart.noHistory": "Cette conversation ne peut pas encore reprendre. Attendez la fin de l’initialisation, puis réessayez.",
  "chat.mode.bypassPermissions": "Sans confirmation",
  "chat.mode.readOnly": "Lecture seule",
  "chat.mode.fullAccess": "Accès complet",
  "chat.placeholder": "Écrire à l'agent, ou utiliser /commandes, /compétences et @fichiers",
  "chat.command.clearDescription": "Archiver cette session et démarrer une nouvelle conversation",
  "chat.command.rewindDescription": "Choisir quoi rétablir depuis le dernier message utilisateur",
  "chat.command.rewindUnavailable":
    "Le rétablissement nécessite un message utilisateur terminé, sans tour actif, message en attente ni demande d’autorisation.",
  "chat.effortTooltip": "Effort de réflexion",
  "chat.effortDefault": "Réflexion",
  "chat.effort.auto": "Automatique",
  "chat.effort.low": "Faible",
  "chat.effort.medium": "Moyen",
  "chat.effort.high": "Élevé",
  "chat.effort.xhigh": "Très élevé",
  "chat.effort.max": "Maximum",
  "chat.effort.ultra": "Extrême",
  "chat.effort.ultracode": "Ultra Code",
  "chat.agentTooltip": "Agent",
  "chat.effort.minimal": "Minimal",
  "chat.filterPlaceholder": "Filtrer",
  "chat.placeholderOpencode": "Écrivez à l'agent ; /commandes et @fichiers sont disponibles, et un message commençant par ! exécute une commande shell",
  "chat.command.compactDescription": "Résumer la conversation pour libérer du contexte",
  "chat.command.undoDescription": "Annuler le dernier message et les modifications de fichiers qu'il a entraînées",
  "chat.command.redoDescription": "Rétablir ce que la dernière annulation a retiré",
  "chat.command.shareDescription": "Créer un lien de partage pour cette conversation",
  "chat.command.unshareDescription": "Ne plus partager cette conversation",
  "chat.mode.auto": "Mode auto",

  // ── Une question de l'agent, posée sous forme de formulaire ──
  "chat.question.heading": "L'agent a une question",
  "chat.question.submit": "Envoyer",
  "chat.question.next": "Suivant",
  "chat.question.dismiss": "Ignorer",
  "chat.question.answerPlaceholder": "Tapez votre réponse",
  "chat.question.otherPlaceholder": "Autre réponse",
  "chat.question.answeredHeading": (n: number) =>
    n === 1 ? "1 question répondue" : `${n} questions répondues`,
  "chat.question.blankAnswer": "Laissé vide",

  // ── Un plan en attente d'approbation ──
  "chat.plan.heading": "Plan en attente d'approbation",
  "chat.plan.implement": "Approuver et exécuter",
  "chat.plan.reject": "Rejeter",

  // ── Messages écrits pendant que l'agent travaille ──
  "chat.placeholderBusy": "Saisissez un message ; il sera envoyé à la fin de ce tour",
  "chat.queueTooltip": (combo: string) => `Sera envoyé à la fin de ce tour · ${combo} l'envoie immédiatement`,
  "chat.queue.pending": "Messages en attente",
  "chat.queue.view": "Afficher le message complet",
  "chat.queue.edit": "Modifier",
  "chat.queue.remove": "Supprimer",

  // ── Images collées ou déposées dans la zone de saisie ──
  "chat.attach.remove": "Retirer cette image",
  "chat.attach.tooMany": (max: number) => `Un message peut contenir jusqu’à ${max} images`,
  "chat.attach.tooLarge": (name: string, mb: number) => `${name} dépasse ${mb} Mo et n’a pas été joint`,
  "chat.attach.unreadable": (name: string) => `Impossible de lire ${name}`,
  // Compacting the conversation… / Context compacted / Context compacted automatically
  "chat.compaction.running": "Compactage de la conversation…",
  "chat.compaction.manual": "Contexte compacté",
  "chat.compaction.auto": "Contexte compacté automatiquement",
  "chat.compaction.from": (tokens: string) => `depuis ${tokens} tokens`,
  // N steps
  "chat.subagent.steps": (n: number) => (n === 1 ? "1 étape" : `${n} étapes`),
  "chat.subagent.tokens": (tokens: string) => `${tokens} jetons`,
  "chat.rewind.edit": "Modifier",
  "chat.rewind.editSend": "Vérifier et renvoyer",
  "chat.rewind.editConfirm": "Supprimer et renvoyer",
  "chat.rewind.editWarning": "Le message original et tous les messages suivants seront définitivement supprimés. Le message modifié sera envoyé à partir de cet endroit. Les modifications des fichiers ne seront pas annulées.",
  "chat.rewind.inactive": "Le processus de conversation n’est pas démarré. Ces actions seront disponibles après son démarrage.",
  "chat.rewind.unsupported": "L’agent connecté ne propose pas cette action pour le moment.",
  "chat.rewind.title": "Revenir à ce point",
  "chat.rewind.warning": "Cette action est irréversible.",
  "chat.rewind.conversation": "Revenir sur la conversation",
  "chat.rewind.files": "Restaurer les fichiers",
  "chat.rewind.both": "Revenir sur la conversation et restaurer les fichiers",
  "chat.rewind.confirm.conversation": "Supprimer ce message et tout ce qui suit ?",
  "chat.rewind.confirm.files": "Restaurer les fichiers à leur état antérieur à ce message ?",
  "chat.rewind.confirm.both": "Supprimer ce tour et restaurer les fichiers qu’il a modifiés ?",
  "chat.rewind.unavailable": "Aucun point de restauration de fichiers n’existe pour ce message.",
  "chat.rewind.previewing": "Vérification du point de restauration…",
  "chat.rewind.cancel": "Ne rien changer",
  "chat.rewind.apply": "Revenir",
  "chat.rewind.applying": "Retour en cours…",
  "chat.rewind.fileSummary": (files: number, insertions: number, deletions: number) =>
    `${files} ${files === 1 ? "fichier sera modifié" : "fichiers seront modifiés"} : +${insertions} −${deletions}. Cette action est irréversible.`,
  // ── Règles permanentes proposées par une demande d'autorisation, adoptées d'un clic ──
  "chat.suggest.modeSession": (mode: string) => `${mode} pour cette session`,
  "chat.suggest.mode": (mode: string) => `Passer en ${mode}`,
  "chat.suggest.allowSession": (rule: string) => `Autoriser ${rule} pour cette session`,
  "chat.suggest.allowAlways": (rule: string) => `Toujours autoriser ${rule}`,
  "chat.suggest.dirSession": (dirs: string) => `Autoriser l'accès à ${dirs} pour cette session`,
  "chat.suggest.dirAlways": (dirs: string) => `Toujours autoriser l'accès à ${dirs}`,
  // ── Codex : règles réseau permanentes, intervention, commandes propres et puces de vitesse et de ton ──
  "chat.suggest.networkAlways": (host: string) => `Toujours autoriser l'accès réseau à ${host}`,
  "chat.steer": "Ajouter une consigne",
  "chat.stopping": "Arrêt du tour en cours…",
  "chat.stopped": "Tour en cours arrêté",
  "chat.steerAccepted": "Consigne envoyée",
  "chat.steerTooltip": (combo: string) => `${combo} l'ajoute au tour en cours`,
  "chat.command.reviewDescription": "Relire le code et signaler ce qui demande attention",
  "chat.command.reviewHint": "[branch <nom> | commit <sha> | consignes]",
  "chat.command.startTimeout": "L'agent n'a pas ouvert sa session à temps",
  "chat.serviceTierTooltip": "Vitesse",
  "chat.serviceTier.default": "Vitesse standard",
  "chat.personalityTooltip": "Ton",
  "chat.personality.default": "Ton par défaut",
  "chat.personality.none": "Neutre",
  "chat.personality.friendly": "Cordial",
  "chat.personality.pragmatic": "Pragmatique",
  // ── Conversation longue : les séries d'appels d'outils tiennent sur une ligne, avec un retour à la fin ──
  "chat.toolRun.count": (n: number) => `${n} appels d'outils`,
  "chat.toolRun.tooltip": "Afficher chaque appel",
  "chat.backToEnd": "Revenir au message le plus récent",
  "chat.turnFold.hide": "Masquer les étapes",
  "chat.turnFold.show": (n: number) => (n === 1 ? "Afficher 1 étape" : `Afficher ${n} étapes`),
  "chat.turnFold.hideAll": "Masquer toutes les étapes",
  "chat.turnFold.showAll": "Afficher toutes les étapes",
  "chat.elicitation.heading": (server: string) => `${server} demande des informations`,
  "chat.elicitation.cancel": "Annuler",
  "chat.elicitation.decline": "Refuser",
  "chat.elicitation.submit": "Envoyer",
  "chat.elicitation.done": "Terminé",
  "chat.elicitation.choose": "Choisir…",
  "chat.effort.off": "Désactivée",
  "chat.effort.offHint": "Sans réflexion approfondie",
  "chat.fastMode.label": "Rapide",
  "chat.fastMode.on": "Le mode rapide est activé",
  "chat.fastMode.off": "Le mode rapide est désactivé",
  "chat.auth.login": "Se connecter",
  "chat.auth.logout": "Se déconnecter",
  "chat.auth.confirmLogout": "Confirmer la déconnexion",
  "chat.auth.logoutConfirm": (provider: string) => `Se déconnecter de ${provider} sur cet hôte ? Les identifiants partagés seront supprimés, ce qui affectera les autres sessions qui les utilisent. L’historique des conversations sera conservé.`,
  "chat.auth.signingOut": "Déconnexion en cours…",
  "chat.auth.signedOut": (provider: string) => `Déconnexion de ${provider} effectuée. Connectez-vous pour poursuivre cette conversation.`,
  "chat.auth.logoutFailed": "Impossible de confirmer la déconnexion. Veuillez réessayer.",
  "chat.auth.wait": "Attendez la fin de la tâche en cours avant de changer de compte.",
  "chat.auth.title": (provider: string) => `Compte ${provider}`,
  "chat.auth.start": "Se reconnecter",
  "chat.auth.required": (provider: string) => `Votre connexion à ${provider} n’est plus valide. Reconnectez-vous pour continuer.`,
  "chat.auth.starting": "Préparation de la connexion…",
  "chat.auth.pending": "Ouvrez la page d’autorisation et saisissez ce code. Cette vue sera mise à jour une fois la connexion terminée.",
  "chat.auth.success": "Connexion réussie. Vous pouvez envoyer un message pour poursuivre cette conversation.",
  "chat.auth.failed": "La connexion n’a pas pu aboutir. Réessayez. Vérifiez que l’authentification par code d’appareil est activée dans ChatGPT et que votre version de Codex CLI la prend en charge.",
  "chat.auth.canceled": "Connexion annulée. Vous pouvez réessayer à tout moment.",
  "chat.auth.scope": (provider: string) => `La connexion met à jour le compte ${provider} utilisé sur cet hôte. Les autres sessions partageant ces identifiants utiliseront également ce compte.`,
  "chat.auth.canceling": "Annulation de la connexion…",
  "chat.auth.submitting": "Vérification du code d’autorisation…",
  "chat.auth.claude.pending": "Ouvrez la page d’autorisation, connectez-vous, puis collez le code complet qui s’affiche.",
  "chat.auth.claude.failed": "La connexion n’a pas pu aboutir. Réessayez et vérifiez que votre version de Claude CLI prend en charge l’autorisation du compte.",
  "chat.auth.claude.code": "Code d’autorisation",
  "chat.auth.claude.submit": "Envoyer le code",
  "chat.auth.claude.invalidCode": "Collez le code complet de cette tentative d’autorisation, y compris la partie après #.",
  "chat.auth.claude.externalAuth": "Les clés API et les autres méthodes d’authentification configurées restent inchangées.",
  "chat.auth.open": "Ouvrir la page d’autorisation",
  "chat.resetCredits.label": (n: string) => `Crédits de réinitialisation : ${n}`,
  "chat.resetCredits.title": "Crédits de réinitialisation des limites Codex",
  "chat.resetCredits.unknown": "Le nombre de crédits de réinitialisation est indisponible.",
  "chat.resetCredits.confirm": "Utiliser un crédit pour réinitialiser les limites Codex admissibles. Cette action est irréversible.",
  "chat.resetCredits.reset": "Limites réinitialisées.",
  "chat.resetCredits.alreadyRedeemed": "Cette demande a déjà abouti.",
  "chat.resetCredits.nothingToReset": "Aucune limite ne peut être réinitialisée actuellement.",
  "chat.resetCredits.noCredit": "Aucun crédit de réinitialisation disponible.",
  "chat.resetCredits.error": "La demande a échoué ou le solde actuel est indisponible. Actualisez le solde ou réessayez la réinitialisation en attente.",
  "chat.resetCredits.busy": "Traitement…",
  "chat.resetCredits.retry": "Réessayer la réinitialisation",
  "chat.resetCredits.use": "Utiliser un crédit",
  "chat.resetCredits.refresh": "Actualiser",
  "chat.usage.context": (used: string, max: string, pct: number) =>
    `Contexte : ${used} sur ${max} jetons (${pct} %)`,
  "chat.usage.cost": (usd: string) => `Coût de la session : $${usd}`,
  "chat.usage.rateLimited": (resets: string) => `Limite d’utilisation atteinte ; réinitialisation ${resets}`,
  "chat.usage.rateWarning": (pct: number, resets: string) =>
    `Limite d’utilisation : ${pct} % consommés ; réinitialisation ${resets}`,
  "chat.autoContinue.fiveHour": (time: string) => `Limite d’utilisation de 5 heures atteinte. Reprise automatique de la tâche : ${time}.`, // 5-hour usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.weekly": (time: string) => `Limite d’utilisation hebdomadaire atteinte. Reprise automatique de la tâche : ${time}.`, // Weekly usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.generic": (time: string) => `Limite d’utilisation atteinte. Reprise automatique de la tâche : ${time}.`, // Usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.unknownReset": "Limite d’utilisation atteinte. L’heure de réinitialisation est inconnue ; la tâche ne reprendra pas automatiquement.", // Usage limit reached. The reset time is unknown, so the task will not continue automatically.
  "chat.autoContinue.repeated": "La limite d’utilisation a de nouveau été atteinte. La tâche ne reprendra plus automatiquement.", // The usage limit was reached again. The task will no longer continue automatically.
  "chat.autoContinue.failed": "La tâche n’a pas pu reprendre automatiquement. Envoyez un message pour continuer.", // The task could not continue automatically. Send a message to continue.
  "chat.mcp.codexScope": "Cette action modifie votre configuration utilisateur Codex et affecte les autres conversations qui l’utilisent. Continuer ?",
  "chat.mcp.tooltip": "Serveurs MCP",
  "chat.mcp.loading": "Lecture de la liste des serveurs…",
  "chat.mcp.backendUnsupported": "Le serveur VelaTerm connecté ne prend pas en charge la gestion MCP. Mettez-le à jour et redémarrez-le, puis réessayez.",
  "chat.mcp.none": "Aucun serveur MCP configuré",
  "chat.mcp.tools": (n: number) => (n === 1 ? "1 outil" : `${n} outils`),
  "chat.mcp.reconnect": "Reconnecter",
  "chat.mcp.disable": "Désactiver",
  "chat.mcp.enable": "Activer",
  "chat.mcp.status.connected": "Connecté",
  "chat.mcp.status.disabled": "Désactivé",
  "chat.mcp.status.failed": "En échec",
  "chat.mcp.status.pending": "Connexion en cours",
  "chat.mcp.status.disconnected": "Déconnecté",
  "chat.mcp.status.other": "Inconnu",
  "chat.tasks.label": "Tâches",
  "chat.tasks.tooltip": "Tâches en arrière-plan",
  "chat.tasks.backgroundAll": "Passer le travail en cours en arrière-plan",
  "chat.tasks.none": "Aucune tâche en arrière-plan",
  "chat.tasks.stop": "Arrêter",
  "chat.retry.line": (attempt: number, max: number, seconds: number, message: string) =>
    `Nouvelle tentative (${attempt}/${max}) dans ${seconds} s : ${message}`,
  "chat.notify.dismiss": "Fermer",
  "settings.completionMode": "Suggestions de commandes",
  "settings.completionAuto": "Automatiques",
  "settings.completionTab": "Avec Tab",
  "settings.completionOff": "Désactivées",
  "settings.completionUnavailable": "Impossible de charger ou d’enregistrer les paramètres.",
  "settings.completionHint": "S’applique aux nouveaux terminaux Zsh, Bash 4+, Fish et PowerShell. CMD conserve le comportement habituel de Tab. Tab insère la suggestion sélectionnée ; Entrée exécute la commande actuelle sans appliquer de suggestion.",


};

export default fr;
