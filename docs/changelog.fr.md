## v0.1.102 — 2026-08-23

### Agents IA

- **Préréglages d'agent : faites tourner côte à côte plusieurs CLI compatibles.** Chaque type d'agent était lié en dur à un seul exécutable ; un fork, une version nightly ou une seconde CLI parlant le même protocole n'avaient donc aucun moyen d'entrer — on modifiait les arguments de lancement d'un type existant et on perdait l'original. Un préréglage désigne désormais son propre exécutable, sa propre icône et ses propres arguments de lancement, et apparaît dans le menu de nouvelle session à côté des types intégrés. Les sessions retiennent le préréglage qui les a créées : forker l'une d'elles conserve donc le même exécutable, et un préréglage créé sur le bureau apparaît aussi sur les navigateurs appairés et les clients distants, icône comprise, parce que l'icône voyage sous forme de données et non de chemin sur une machine donnée. Les sessions existantes ne bougent pas : une base de données issue d'une version antérieure démarre exactement comme avant.

- **La carte de sous-tâche choisit le modèle et le niveau d'effort, et une seule réponse tranche partout.** Quand un agent demande à lancer une sous-tâche, la carte de confirmation propose désormais le modèle et — lorsque l'agent le prend en charge — le niveau d'effort de raisonnement, préremplis depuis les arguments de lancement de la session parente, si bien que le cas courant tient en un clic. Basculer la carte sur un autre agent recalcule les deux valeurs : un nom de modèle venu d'une CLI ne peut donc plus se retrouver sur la ligne de commande d'une autre. La carte s'affiche sur tous les clients connectés, et y répondre depuis l'un la fait disparaître sur les autres ; la première réponse réserve aussi la tâche sur le serveur, si bien que confirmer sur un téléphone et sur un ordinateur dans la même seconde crée un seul worktree et une seule session enfant au lieu de deux.

### Espace de travail

- **Mode miroir : une disposition partagée sur tous les clients.** Le flux du terminal a toujours été partagé — un seul PTY, un seul flux d'octets — mais la disposition autour de lui ne vivait que dans le stockage du navigateur de chaque client : un navigateur ouvert depuis le réseau local affichait ses propres onglets et ses propres divisions, et réorganiser un écran ne changeait rien à l'autre. Avec le mode miroir activé, les onglets, les divisions, la session active, la sélection dans la barre latérale et les panneaux repliés sont publiés vers tous les clients et suivis par tous. C'est l'hôte qui commande l'interrupteur, depuis le panneau d'accès à distance. Réorganiser d'un côté prend effet de l'autre ; une session qui quitte la disposition de cette fenêtre est détachée et non arrêtée, si bien que suivre un pair ne met jamais fin au processus de quelqu'un ; et appliquer la disposition d'un pair ne vole pas le clavier à celui qui est en train de taper en local. Les téléphones restent à l'écart : leur navigation à deux niveaux est une interface d'une autre forme, et y recopier un arbre de divisions de bureau n'aiderait personne.

- **L'onglet Git de la barre latérale droite est devenu un client Git utilisable.** Il ne faisait qu'énumérer les fichiers modifiés. Il permet désormais d'indexer et de désindexer des fichiers isolés ou des groupes entiers, d'abandonner des modifications, d'écrire un commit (y compris en amend) et de consulter l'historique avec, pour chaque commit, ses fichiers et ses diffs — le tout réparti en sections indexé, modifié, non suivi et validé, qui se replient. Les chemins sont traités depuis la racine du dépôt : une session ouverte dans un sous-répertoire agit donc bien sur les fichiers qu'elle annonce, et un HEAD détaché est signalé comme tel au lieu d'afficher une branche nommée HEAD.

### Interface

- **⌘Q pose désormais la même question que la fermeture de la fenêtre.** L'entrée Quitter du menu de l'application était celle du système, qui met fin au processus sur-le-champ : appuyer sur ⌘Q sautait la confirmation d'enregistrement de l'espace de travail qu'affiche le bouton de fermeture, si bien qu'une même intention se comportait différemment selon la manière de l'exprimer. Les deux chemins passent maintenant par une seule confirmation. Si la fenêtre qui l'affiche s'est rechargée ou a planté entre-temps, un nouvel appui sur ⌘Q repose la question et se rabat sur une boîte de dialogue native, plutôt que de laisser l'application impossible à quitter.

- **Les indications de raccourci affichent les touches qui fonctionnent vraiment.** Les valeurs par défaut diffèrent d'une plateforme à l'autre, et un navigateur se réserve les combinaisons ⌘/Ctrl + lettre — ⌘D met en favori, ⌘T ouvre un onglet —, si bien que sous macOS les raccourcis ⌘ de l'application n'atteignaient jamais la page lorsque VelaTerm était ouvert comme une simple URL. Les onglets de navigateur ordinaires utilisent désormais les combinaisons Ctrl+Alt sur tous les systèmes d'exploitation, tandis que les applications de bureau et les fenêtres de connexion distante conservent ⌘. Les infobulles et l'indication de l'onglet vide affichent la combinaison réellement en vigueur, y compris celle que vous avez redéfinie vous-même, au lieu d'une combinaison ⌘ figée dans le code ; et le terminal bloque exactement les combinaisons que l'application s'est réservées, si bien que redéfinir une action déplace la touche avec elle.

- **Les préréglages de police couvrent les Nerd Fonts et le CJK, et une police personnalisée absente du système le dit.** La liste des préréglages accueille désormais les familles Nerd Font et CJK courantes, et une police saisie à la main est réaffichée puis vérifiée : si le système ne la possède pas, la page des réglages le signale au lieu de retomber en silence sur une police par défaut qui ne ressemble en rien à ce que vous aviez demandé.

- **Les champs de texte sous macOS ne mettent plus de majuscules et ne corrigent plus ce que vous tapez.** La mise en majuscule automatique, la correction automatique et la vérification orthographique du système s'appliquaient à chaque champ de saisie de l'application, y compris les noms de session et les champs de commande, où « npm » devenait « Npm ». Elles sont désormais désactivées partout.

### Windows

- **La saisie en chinois, en japonais ou en coréen réaffiche le texte de composition et la fenêtre de candidats.** Les deux étaient invisibles : on tapait à l'aveugle et on ne voyait le résultat qu'après avoir appuyé sur Entrée. Deux de nos propres règles CSS en étaient responsables : le conteneur du calque de composition s'effondrait à une largeur nulle, et le décalage `right` de ce calque s'y résolvait à rien du tout. La fenêtre de candidats suivait, puisque le système d'exploitation la positionne d'après le rectangle du calque. Le calque est de nouveau dessiné, aux couleurs de l'application, et le champ de saisie invisible qui le porte relâche sa géométrie dès la fin de la composition : cliquer et glisser sur cette zone atteint donc le terminal, et non plus un élément vide qui la recouvrait sans cesse.

- **La barre de titre native suit le réglage clair/sombre.** L'application conserve la barre de titre du système, et Windows la peint en clair tant qu'on ne lui dit pas le contraire : une interface sombre portait donc un bandeau blanc au-dessus d'elle. Elle s'accorde désormais à l'application, y compris pour les fenêtres ouvertes plus tard comme les fenêtres SSH et de connexion distante. Choisir « suivre le système » rend la main au système d'exploitation au lieu de figer une valeur.

- **Le carré parasite au démarrage à froid a disparu.** Le plugin d'instance unique crée une fenêtre de messages cachée sans jamais lui donner la transparence que son propre style annonçait ; Windows agrandissait donc parfois cette fenêtre de taille nulle jusqu'à sa taille minimale et peignait un petit carré pendant le démarrage. Elle est maintenant correctement transparente ; le comportement d'instance unique est inchangé.

### Performances

- **L'accès à distance charge bien moins de choses au premier affichage.** Les ressources statiques sont désormais compressées à la demande et servies avec des validateurs de cache : une seconde visite revalide au lieu de tout retélécharger, et les paquets de langue ainsi que les moteurs de rendu optionnels du terminal ne se chargent que lorsque quelque chose en a besoin, au lieu de faire partie du premier envoi. Au total, le transfert initial tombe à environ un cinquième de ce qu'il était.

### Corrections

- **Une session enfant démarre désormais comme la session qui l'a demandée.** Les sessions enfants n'héritaient ni du mode d'autorisation ni des arguments de lancement de leur parent : l'enfant d'une session lancée en ignorant les confirmations les redemandait, et un modèle fixé sur le parent était perdu. Les deux sont désormais hérités, avec repli sur les valeurs par défaut globales du type d'agent — les mêmes que celles qu'applique le menu « Nouvelle session d'agent ».

- **Reconnecter une fenêtre distante ne signale plus un faux « échec d'authentification ».** À la reconnexion, le nouveau WebSocket et celui qu'il remplaçait se faisaient la course ; la fermeture du perdant était signalée comme un échec d'authentification, et la bannière accusait un appairage parfaitement valide d'avoir été refusé.

- **Un groupe ne peut plus être glissé dans son propre sous-arbre.** Déposer un groupe sur l'un de ses propres descendants détachait toute cette branche de l'arborescence, et les sessions qu'elle contenait disparaissaient de la barre latérale jusqu'à réparation manuelle de la base de données. Le déplacement est désormais refusé.

- **La sortie des agents garde ses couleurs quand VelaTerm est lancé depuis un autre outil.** Un terminal hérite de l'environnement de ce qui l'a démarré : lancer VelaTerm depuis un IDE ou un harnais d'agent qui exporte `NO_COLOR`, `CI` ou `FORCE_COLOR=0` rendait donc monochrome toute TUI d'agent à l'intérieur de VelaTerm, alors même que le terminal annonce la couleur complète. Ces valeurs héritées sont écartées au démarrage d'une session ; les mêmes variables exportées depuis votre propre profil de shell continuent de s'appliquer, puisque ce profil s'exécute à l'intérieur de la session.

## v0.1.101 — 2026-08-15

### Accès à distance

- **Choisissez l’adresse utilisée par le lien de partage — les adresses Tailscale apparaissent désormais.** La liste d’adresses n’acceptait que les plages IPv4 privées classiques ; les maillages VPN comme Tailscale, qui attribuent des adresses de la plage NAT d’opérateur (100.64.0.0/10), étaient donc silencieusement écartés du panneau d’accès à distance et du lien d’appairage, alors que le serveur y était déjà joignable. Ces adresses sont désormais listées ; les tunnels VPN sont classés en dernier afin de ne jamais devenir la valeur par défaut. Un nouveau sélecteur d’IP dans le panneau — visible avant le démarrage comme pendant l’exécution — affiche chaque candidate avec le nom de son interface et signale les tunnels VPN ; en choisir une place son URL en tête et régénère le lien d’appairage avec exactement cet hôte, si bien que le lien copié fonctionne sur un appareil qui n’atteint cette machine que par le VPN, sans modifier l’URL à la main. Un code QR sous le lien d’appairage se scanne directement avec le téléphone. Le choix est mémorisé ; si l’interface choisie disparaît, le panneau revient à « Automatique » sans l’oublier. Le serveur lui-même est inchangé et continue d’écouter sur toutes les interfaces. Choisir une adresse apparue seulement après le démarrage du serveur — un VPN connecté plus tard, par exemple — met désormais aussi à jour immédiatement l’URL copiée et le code QR, au lieu du seul lien d’appairage jusqu’au prochain redémarrage ; les tunnels VPN restent derrière les adresses LAN sur toutes les plateformes, une adresse choisie serveur arrêté détermine le tout premier lien d’appairage après le démarrage, et des régénérations de lien qui se chevauchent ne peuvent plus écraser un lien plus récent par un plus ancien.

- **Le partage survit désormais à un redémarrage.** Le jeton d'appairage était régénéré à chaque démarrage du serveur : fermer puis rouvrir VelaTerm invalidait silencieusement tous les liens partagés, et chaque téléphone devait être appairé à nouveau. Le jeton, les appareils appairés et la liste des appareils bloqués sont désormais enregistrés dans un fichier du répertoire de données lisible uniquement par son propriétaire : un appareil déjà appairé se reconnecte avec son URL enregistrée après un redémarrage — le mot de passe d'accès reste un second facteur obligatoire — et un appareil révoqué reste révoqué. VelaTerm se souvient aussi que le partage était actif : quittez l'application pendant que le serveur tourne, et le prochain lancement le relance sur le même port, dans l'application de bureau comme sur un serveur sans interface avec `--serve` ; arrêtez-le vous-même, et rien ne démarre automatiquement. Si le démarrage automatique échoue, par exemple parce que le port est occupé, l'application démarre normalement et le panneau d'accès à distance en affiche la raison. Le champ du port retient désormais le port réellement utilisé au lieu de revenir à la valeur par défaut, et « Régénérer le lien » reste le coupe-circuit explicite : il émet immédiatement un nouveau jeton, invalide tous les anciens liens et écrase l'état enregistré. Le mot de passe d'accès lui-même n'est jamais écrit sur le disque — seul un hachage exigeant en mémoire (Argon2id) est conservé.

### Sécurité

- **Un appareil appairé ne peut plus administrer le partage lui-même.** N'importe quel navigateur appairé pouvait appeler les mêmes commandes d'administration que l'application de bureau — créer un nouveau lien d'appairage (ce qui vide aussi la liste de blocage des appareils), lister et révoquer d'autres appareils, ou arrêter et reconfigurer le serveur — et le magasin de réglages livrait à chaque client la table complète des réglages, y compris le hachage à mémoire dure du mot de passe d'accès et les réglages de démarrage automatique lus au prochain lancement. Les commandes d'administration sont désormais réservées à l'application de bureau et au shell Electron ; l'API des réglages filtre les clés d'accès à distance et le jeton Gitea de chaque lecture venant d'un appareil appairé et refuse les écritures sur ces clés. Un appareil appairé conserve ce pour quoi l'appairage existe — ses sessions de terminal avec accès complet au shell — mais il ne peut plus lire le vérificateur du mot de passe, inviter ou évincer d'autres appareils, ni détourner le port que le prochain démarrage utilisera. Les commandes qui lisent, écrivent ou suppriment des secrets enregistrés — le jeton Gitea et les mots de passe d'hôte mémorisés — sont désormais refusées elles aussi pour un appareil appairé, et les commandes qui prennent un chemin — lecture, aperçu, écriture, création, renommage et suppression, de même que l'affichage du diff git d'un fichier ou le choix du dossier où cloner un dépôt — résolvent d'abord les liens symboliques et rejettent les chemins situés dans le répertoire de données de VelaTerm, où résident l'état d'appairage et les clés ; tout autre chemin continue de fonctionner, si bien que la navigation et l'édition de fichiers à distance restent intactes. Un test énumère chaque commande distante qui accepte un chemin, si bien qu'une nouvelle commande ne peut pas contourner ce contrôle sans être remarquée. Quand l'une de ces protections rejette une requête, le navigateur affiche désormais un message correctement traduit au lieu d'une erreur brute en anglais.

- **Une révocation ou un lien régénéré survit désormais aussi à la configuration à double instance.** Sur un serveur sans interface (`--serve`) avec démarrage automatique activé, deux instances du serveur détenaient chacune leur propre copie de l'état d'appairage enregistré et le réécrivaient en entier : une révocation ou un nouveau lien passé par l'une pouvait être défait en silence par l'autre. Toutes les instances d'un même processus partagent désormais un seul état d'appairage par répertoire de données : révocation et rotation prennent effet partout immédiatement, et exactement un écrivain persiste le fichier, qui reste la source de vérité entre les vrais redémarrages.

- **Les connexions échouées répétées sont freinées.** La vérification du mot de passe d'accès utilise Argon2id, volontairement coûteux — et quiconque atteint le port peut essayer. Après cinq échecs depuis une même adresse, les tentatives suivantes sont rejetées pendant une minute avant tout travail de hachage, et le hachage lui-même s'exécute désormais hors de la boucle d'événements du serveur, avec un plafond strict de vérifications simultanées : un déluge de mauvais mots de passe ne peut plus saturer le serveur de hachage à mémoire dure ni le ralentir pour les appareils déjà connectés. Le frein vit en mémoire et se réinitialise avec le serveur ; le jeton d'appairage et le mot de passe restent la véritable barrière. La limite est désormais partagée par toutes les instances du serveur utilisant le même répertoire de données — la configuration à double instance avec `--serve` ne double plus le budget de tentatives — et chaque tentative est réservée avant le début de la vérification du mot de passe, de sorte que des requêtes parallèles venant d'une même adresse ne puissent pas passer sous la limite. Un navigateur freiné voit désormais un message dédié de limitation sur l'écran de connexion au lieu de s'entendre dire que le mot de passe était faux ; le freinage n'est en outre plus mémorisé comme un mot de passe erroné : la pause passée, la tentative suivante aboutit de nouveau sans recharger la page. Une tentative abandonnée en cours de route — l'onglet fermé pendant que le mot de passe était encore vérifié — libère désormais immédiatement sa réservation au lieu de compter contre l'adresse pour le reste de la minute, et une connexion réussie ne libère que sa propre réservation au lieu d'effacer tout l'historique de l'adresse : derrière une adresse réseau partagée, une connexion correcte ne remet plus à zéro le budget de tentatives d'un attaquant, et les échecs enregistrés n'expirent qu'avec leur minute.

- **Les secrets sur disque et dans les journaux sont traités avec plus de soin.** Le fichier de l'état d'appairage et la clé de chiffrement de bout en bout sont désormais créés lisibles par le seul propriétaire dès l'origine, au lieu d'être restreints après la première écriture, et la base de données de sessions — qui contient le hachage du mot de passe — est elle aussi restreinte au propriétaire. Un serveur sans interface (`--serve`) n'imprime plus le secret longue durée du lien d'appairage dans les journaux : si la sortie n'est pas un terminal, le lien est retenu et une indication s'affiche à la place ; `--print-pairing` le réactive explicitement. Le registre des appareils est plafonné à 32 entrées aux noms de longueur limitée, afin qu'un client appairé ne puisse pas faire grossir le fichier enregistré sans limite, et si l'enregistrement d'une révocation ou d'un nouveau lien échoue, l'erreur remonte désormais à l'appelant au lieu de finir dans une ligne de journal. Le démarrage automatique ne remplace plus un serveur déjà lancé à la main, et une erreur de démarrage automatique périmée disparaît dès que vous arrêtez le serveur vous-même.

### Corrections

- **L'appairage peut désormais être géré depuis le shell Electron.** La création d'un lien d'appairage, la liste des appareils appairés et la révocation d'un appareil n'existaient que comme commandes de bureau (Tauri) ; le répartiteur WebSocket utilisé par le shell Electron et les clients navigateur répondait « Unknown command », laissant le panneau d'accès distant inopérant à cet endroit. Les trois commandes passent désormais par les mêmes fonctions centrales sur les deux transports, si bien qu'elles ne peuvent plus diverger, et des tests de régression couvrent les nouvelles routes de répartition — y compris la création d'un véritable lien d'appairage auprès d'un serveur local en cours d'exécution.

## v0.1.100 — 2026-08-10

### Agents IA

- **Kiro CLI devient un type de session de premier plan.** Les sessions Kiro disposent de leur propre nœud dans l'arborescence, d'une pastille d'état Travail/Attente faisant autorité et pilotée par les lifecycle hooks de Kiro lui-même, de notifications à la fin d'un tour, de la reprise automatique de la même conversation à la réouverture du nœud, d'arguments de lancement et d'une option pour ignorer les confirmations, ainsi que du lancement via vspawn — tout ce dont les autres agents disposaient déjà. VelaTerm clone votre agent Kiro par défaut vers son propre agent `vlx-term`, ajoute à la copie des lifecycle hooks en observation seule, puis lance celle-ci : votre propre fichier d'agent n'est jamais modifié, et votre invite, vos outils et vos serveurs MCP suivent sans changement. Kiro ne dispose d'aucun hook de demande d'autorisation, si bien que la pastille reste sur Travail pendant qu'il attend votre accord.

### Corrections

- **Les programmes lancés depuis le terminal n'héritent plus de l'environnement propre à l'AppImage (Linux).** Le lanceur AppImage fait pointer `PYTHONHOME`, `PYTHONPATH`, `PERLLIB`, `QT_PLUGIN_PATH` et les chemins de plugins GStreamer vers le répertoire de montage temporaire du bundle, et place les répertoires du bundle avant tout le reste dans `PATH` et `LD_LIBRARY_PATH`. Un terminal transmet tout son environnement au shell qu'il démarre : le `python3` du système cherchait donc sa bibliothèque standard à l'intérieur du bundle et refusait purement et simplement de s'exécuter, tandis que d'autres programmes liés dynamiquement chargeaient la copie d'une bibliothèque fournie par le bundle plutôt que celle du système. VelaTerm retire désormais ces chemins du bundle avant de démarrer un shell ou un outil externe, et laisse intactes les valeurs que vous avez définies vous-même. `APPDIR` et `APPIMAGE` restent visibles, si bien que les programmes qui vérifient s'ils s'exécutent depuis une AppImage obtiennent toujours leur réponse. Seules les versions AppImage étaient concernées ; le paquet deb, macOS et Windows se comportent comme avant.

## v0.1.99 — 2026-08-09

### Terminal

- **Shift+Entrée insère un saut de ligne au lieu d'envoyer.** Les terminaux ne disposent d'aucun encodage pour Entrée avec une touche de modification : les CLI d'agents comme Claude Code et Codex ne recevaient qu'un retour chariot ordinaire et envoyaient l'invite alors qu'on était encore en train de l'écrire. VelaTerm émet désormais ESC+CR, la séquence même que ces outils attendent d'une correspondance de touches iTerm2, ce qui rend les saisies multilignes utilisables — y compris sur macOS, où le gestionnaire de touches personnalisé n'était tout simplement pas installé. La composition dans une méthode de saisie reste inchangée : Entrée valide toujours le candidat.

### Projets et organisation

- **Actualiser l'état d'une seule session.** Dans un volet filtré par état, les sessions disposent d'une action « Actualiser l'état » qui réévalue uniquement cette session selon les conditions propres au volet, l'ajoutant ou la retirant tandis que toutes les autres restent en place. L'action appartient au volet depuis lequel le menu a été ouvert, si bien que les divisions imbriquées n'empruntent jamais le filtre d'un autre volet. Le résultat est conservé par volet et restauré après un redémarrage.
- **Retirer un marqueur ne demande qu'un clic.** Choisir l'emoji déjà appliqué le supprime, ce qui rend inutiles l'entrée dédiée au retrait et son séparateur. La pastille emoji du bouton de filtre disparaît elle aussi : la mise en évidence indique déjà qu'un filtre par marqueur est actif, et le menu précise lequel.

### Corrections

- **L'intégration au bureau de l'AppImage Linux s'installe sur n'importe quelle machine.** L'icône fournie était un lien symbolique vers un chemin absolu de la machine de compilation ; des outils comme Gear Lever et AppImageLauncher ne parvenaient donc pas à l'extraire, alors même que l'application fonctionnait normalement. Le lien est désormais relatif. L'exigence glibc annoncée a également été corrigée à 2.35 après mesure des bibliothèques fournies et non du seul exécutable, ce qui fait d'Ubuntu 22.04 la plus ancienne distribution prise en charge par l'application de bureau.

## v0.1.98 — 2026-08-02

### Agents IA

- **Grok Build devient un agent de premier plan dans VelaTerm.** Installez, lancez et reprenez Grok 4.5 avec des identifiants de session stables, les lifecycle hooks officiels, des états de travail et d’autorisation précis, des transcriptions fusionnées, le détail de l’utilisation et une icône officielle adaptée au thème, de façon cohérente sur ordinateur, navigateur et mobile.

### Projets et organisation

- **Divisez la barre latérale des projets en vues de travail indépendantes.** Chaque volet de l’arborescence peut à nouveau être divisé vers le bas et retrouve après redémarrage sa recherche, ses filtres d’état et d’emoji, son état de repli et son ratio de redimensionnement. Tous les volets restent des projections de la même arborescence gérée par le backend : les modifications se synchronisent sans dupliquer les données métier.
- **Marquez et filtrez les nœuds sans perdre leur contexte.** Les projets, groupes et sessions peuvent porter des marqueurs emoji. Un conteneur marqué conserve tout son sous-arbre, l’appartenance aux états reste stable pendant le travail, l’ajout dynamique et l’actualisation manuelle sont disponibles, et les conditions d’état et d’emoji sont réunies par union.
- **Créez un projet vide sur place.** Choisissez le répertoire parent, validez le nom, puis créez et importez le dossier dans un même parcours. En cas d’échec partiel, seule l’importation est relancée, sans créer de répertoire en double.

### Interface

- **Partagez VelaTerm là où se trouve votre communauté.** La boîte de dialogue prend désormais en charge WeChat Moments, Weibo, Xiaohongshu, X, Reddit, Hacker News, LinkedIn, Facebook, Telegram et WhatsApp, avec un parcours par code QR pour WeChat et une invitation au partage dans la fenêtre de mise à jour.
- **Des interactions plus soignées jusque dans les détails.** Les onglets de terminal temporaires peuvent être renommés avant de devenir des sessions enregistrées. Les champs ordinaires désactivent la mise en majuscule automatique des claviers mobiles sans modifier la saisie dans le terminal.

## v0.1.97 — 2026-07-25

### Agents IA

- **Les sessions ne restent plus bloquées sur « en cours ».** Codex signalait l’activité des outils et la fin d’un tour depuis des processus éphémères distincts, dont les rappels pouvaient arriver dans le désordre et laisser un tour terminé affiché comme encore en cours. Les rapports intermédiaires arrivant après la fin de leur propre tour sont désormais ignorés, et un nouveau hook de fin de session couvre les sessions qui se terminent sans événement de complétion.
- **Les tours interrompus se stabilisent en quelques secondes.** Appuyer sur Esc, ou une erreur de flux, met fin à un tour Claude ou Codex sans le moindre rappel de complétion. Six secondes de silence du terminal corrigent maintenant discrètement une telle session en attente, sans déclencher de notification « a répondu ».

### Interface

- **Raccourcis de division fiables sous macOS.** Diviser à droite (Cmd+D) et diviser vers le bas (Cmd+Shift+D) sont désormais enregistrés comme commandes du menu Terminal natif, si bien que macOS n’intercepte plus la combinaison avant VelaTerm.
- **Un seul enregistrement par frappe.** Cmd+S était traité à la fois par le raccourci global et par l’éditeur actif, ce qui pouvait écrire deux fois le même fichier en une seule frappe.

## v0.1.96 — 2026-07-23

### Agents IA

- **L’état de Codex repose sur les lifecycle hooks, pas sur des suppositions tirées du terminal.** Les sessions Codex récentes utilisent désormais uniquement les lifecycle hooks officiels comme source d’activité. Une poignée de main `SessionStart` vérifie la liaison, l’absence de rappel affiche « État indisponible », et le texte ou l’activité du terminal ne peut plus écraser les états de travail, de confirmation ou de fin.
- **Une utilisation Codex plus fraîche après chaque tour.** Le panneau Info affiche immédiatement le snapshot rollout local, le rapproche des limites en direct, actualise à nouveau après l’écriture du snapshot token final par Codex et ignore les réponses tardives d’une ancienne session.

### Interface

- **Ciblage fiable dans l’arborescence des projets sous macOS.** Les lignes virtuelles ne dépendent plus des transform du compositeur, ce qui empêche d’anciennes coordonnées de hit-test WKWebView d’envoyer le survol, le clic ou le glisser vers une autre ligne après un défilement ou une mise à jour de l’arborescence.

## v0.1.95 — 2026-07-21

### Agents IA

- **Kimi Code et Zoo Code rejoignent l’arborescence des sessions.** VelaTerm peut désormais lancer, reprendre, installer et configurer ces deux agents. Kimi utilise ses lifecycle hooks officiels pour signaler précisément les états de travail, d’autorisation et d’attente ; Zoo Code conserve un identifiant de tâche stable et utilise la détection du terminal en l’absence de hooks externes.
- **Actualisation en direct de l’utilisation Codex.** Le panneau Info interroge le Codex app server pour obtenir les limites actuelles, avec repli compatible sur l’instantané rollout local.

### Projets et terminaux

- **Ouvrez un projet avec `vela <path>`.** Les versions empaquetées peuvent installer une commande shell à la manière de VS Code. Un second appel transmet le projet à la fenêtre VelaTerm existante au lieu d’ouvrir une instance en double.
- **Clonage Git visible et annulable.** Clone Project affiche les étapes Git, le pourcentage et le temps écoulé, avertit en cas de blocage et peut arrêter tout l’arbre de processus Git sans laisser de cible incomplète. Les identifiants et query tokens sont masqués dans les erreurs et journaux d’audit.
- **Terminaux WSL sous Windows.** Toutes les distributions WSL installées sont proposées avec PowerShell, cmd et Git Bash pour les terminaux ordinaires. Les agents restent dans le shell hôte Windows afin de préserver la fiabilité des hooks et des chemins exécutables.

### Interface et fiabilité

- **Contrôle plus clair des sessions en arrière-plan.** Les menus affichent l’état en direct de chaque session et la boîte de dépassement de limite peut fermer plusieurs onglets sélectionnés à la fois.
- **Cycle de vie plus sûr et notes multilingues.** Une confirmation précède l’arrêt des sessions actives ; l’identité lifecycle exacte de Codex prime sur les scans rollout ambigus ; les notes de mise à jour couvrent toutes les langues intégrées.

## v0.1.94 — 2026-07-12

### Localisation

- **Interface en vietnamien.** Tiếng Việt est désormais disponible dans le sélecteur de langue et est sélectionné automatiquement lorsque le système utilise une locale vietnamienne.

### Navigateur

- **Démarrage plus rapide du navigateur intégré.** Chaque onglet comporte maintenant des raccourcis en un clic vers ChatGPT, Claude, Gemini et Google. Les menus contextuels des projets et des groupes permettent aussi de créer directement une page de navigateur permanente à l'endroit correspondant dans l'arborescence des sessions.

### Images et documents

- **Collage fiable des chemins d'image sous macOS.** Lorsque WebKit n'expose pas une image copiée sous forme de fichier, VelaTerm la lit désormais depuis le presse-papiers natif et l'envoie tout de même sous forme de chemin de fichier, sans basculer silencieusement vers l'espace réservé aux images propre à l'agent. Les fenêtres distantes affichent toujours le réglage de collage d'image, expliquent pourquoi le mode chemin de fichier est requis et désactivent l'option native indisponible.
- **Collage d'images dans les documents source.** L'éditeur source accepte désormais les images du presse-papiers. Les documents Markdown enregistrés les stockent à côté du document dans `assets/` et insèrent une syntaxe d'image Markdown portable ; les brouillons non enregistrés intègrent les données de l'image afin qu'elles ne soient pas perdues lors du nettoyage des fichiers temporaires.

### Interface

- **Des menus contextuels visibles et ciblant le bon élément.** Les menus ouverts près du bord droit sont correctement mesurés et repositionnés. Un clic droit sur un nœud de l'arborescence ne met désormais en évidence que la cible du menu sans modifier la sélection existante, et les menus de groupe proposent un terminal limité à ce groupe.
- **Édition et libellés d'état plus nets.** Le texte source n'affiche plus de ligatures en forme de flèche pour des séquences telles que les commentaires HTML, les pourcentages d'utilisation sont explicitement libellés comme utilisés, et le menu contextuel natif sans rapport du WebView hôte n'apparaît plus derrière les menus de VelaTerm.

### Corrections

- **Codex reste dans l'historique normal du terminal.** Les sessions Codex lancées par VelaTerm utilisent désormais le mode terminal en ligne. Appuyer sur Esc pour interrompre ou revenir en arrière ne change donc plus les tampons d'écran du terminal et ne fait plus sauter la vue de défilement en haut. Votre propre configuration Codex reste intacte.
