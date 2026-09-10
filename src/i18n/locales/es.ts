//! Spanish dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const es: typeof en = {
  // Project code intelligence and memory associations.
  "knowledge.title": "Grafo de código",
  "knowledge.intro": "Explore las relaciones del código y vincúlelas con las decisiones de diseño guardadas.",
  "knowledge.setup": "Instale CodeGraph en este servidor para activar la indexación de proyectos.",
  "knowledge.downloadNotice": "Descarga el entorno de ejecución de CodeGraph verificado desde GitHub. La indexación se realiza en este equipo; la telemetría y la búsqueda de actualizaciones están desactivadas.",
  "knowledge.install": "Descargar CodeGraph",
  "knowledge.installing": "Descargando e instalando…",
  "knowledge.directory": "Directorio de trabajo",
  "knowledge.enable": "Activar indexación",
  "knowledge.disable": "Desactivar indexación",
  "knowledge.sync": "Sincronizar",
  "knowledge.ready": "Listo",
  "knowledge.disabled": "Desactivado",
  "knowledge.indexing": "Indexando…",
  "knowledge.syncing": "Sincronizando…",
  "knowledge.failed": "Error",
  "knowledge.symbols": "Símbolos",
  "knowledge.files": "Archivos",
  "knowledge.edges": "Relaciones",
  "knowledge.search": "Buscar símbolos o rutas de archivo…",
  "knowledge.searchButton": "Buscar",
  "knowledge.noResults": "No hay símbolos coincidentes.",
  "knowledge.selectSymbol": "Seleccione un símbolo para ver su código fuente, sus relaciones y las memorias vinculadas.",
  "knowledge.source": "Código fuente",
  "knowledge.incoming": "Relaciones entrantes",
  "knowledge.outgoing": "Relaciones salientes",
  "knowledge.noEdges": "No hay relaciones indexadas.",
  "knowledge.analysisNote": "Las relaciones proceden de un análisis estático y pueden ser incompletas o inciertas.",
  "knowledge.changed": "El archivo cambió durante la consulta. Sincronice de nuevo antes de usar los números de línea o confirmar la revisión.",
  "knowledge.truncated": "Esta vista tiene un límite. Se omiten algunas relaciones o líneas de código.",
  "knowledge.linkMemory": "Vincular memoria",
  "knowledge.chooseMemory": "Elegir una entrada de memoria",
  "knowledge.noLinks": "Aún no hay vínculos con el código. Puede vincular una memoria desde el detalle de un símbolo.",
  "knowledge.inspect": "Revisar código y memoria",
  "knowledge.unlink": "Eliminar vínculo",
  "knowledge.codeReferences": "Referencias al código",
  "knowledge.refresh": "Actualizar",
  "knowledge.current": "Sin cambios",
  "knowledge.review": "Requiere revisión",
  "knowledge.unavailable": "No disponible",
  "knowledge.reviewHelp": "Compare esta memoria con el código mostrado. La confirmación registra la versión actual del archivo sin modificar el texto de la memoria.",
  "knowledge.confirmReview": "Confirmar revisión",
  "knowledge.agentHint": "Los agentes pueden ejecutar vknowledge search \"tema\" en este directorio. Las consultas sincronizan los índices activos y devuelven el código y las memorias por separado.",
  "knowledge.busy": "Hay una tarea de indexación en curso. Puede cerrar esta página o desactivar la indexación para detenerla.",
  "knowledge.disabledHelp": "Active la indexación de este directorio para consultar el código. Al desactivarla se conservan el índice y los vínculos con las memorias.",
  "knowledge.conflict": "El código o la memoria han cambiado. Vuelva a cargar ambos antes de guardar este vínculo.",
  "knowledge.symbolMissing": "El símbolo o el código fuente ya no están disponibles. Sincronice y vuelva a buscar.",
  "knowledge.directoryMissing": "Este directorio de trabajo no existe o ha cambiado. Compruebe las rutas del proyecto y de la sesión.",
  "knowledge.partial": "El índice está incompleto. Sincronice de nuevo y compruebe que se pueden leer los archivos de código fuente.",
  "knowledge.interrupted": "La tarea anterior se interrumpió. Sincronice para volver a intentarlo.",
  "knowledge.checksum": "La suma de comprobación de la descarga no coincide. No se ha instalado el entorno de ejecución.",
  "knowledge.downloadFailed": "No se pudo descargar CodeGraph. Compruebe la conexión del servidor con GitHub y vuelva a intentarlo.",
  "knowledge.timeout": "Se agotó el tiempo de indexación. Compruebe el tamaño del repositorio y vuelva a intentarlo.",
  "knowledge.error": "La operación falló. Compruebe el acceso a los directorios y el entorno de ejecución del servidor y vuelva a intentarlo.",

  // Global Memory: a thematic LLM Wiki shared across sessions.
  "memory.title": "Memoria global",
  "memory.add": "Añadir a la memoria global",
  "memory.intro": "Un wiki en constante actualización para compartir conocimientos, decisiones y aprendizajes entre sesiones.",
  "memory.entries": "Artículos de memoria",
  "memory.emptyJobs": "Aún no hay registros de organización.",
  "memory.jobs": "Historial de organización",
  "memory.search": "Buscar en títulos y contenido…",
  "memory.empty": "No hay artículos coincidentes. Añade una conversación para empezar tu wiki.",
  "memory.emptyDetail": "Selecciona un artículo para consultar su contenido, relaciones y fuentes.",
  "memory.new": "Nuevo artículo",
  "memory.titleField": "Título",
  "memory.summary": "Resumen",
  "memory.content": "Contenido (Markdown)",
  "memory.tags": "Etiquetas (separadas por comas)",
  "memory.related": "Artículos relacionados",
  "memory.backlinks": "Enlaces a este artículo",
  "memory.sources": "Fuentes",
  "memory.history": "Historial de revisiones",
  "memory.restore": "Restaurar esta revisión",
  "memory.restoreConfirm": "¿Restaurar esta revisión como una versión nueva? La versión actual se conservará en el historial.",
  "memory.deleteConfirm": "¿Eliminar este artículo y su historial? Las sesiones de origen se conservarán.",
  "memory.export": "Exportar Markdown",
  "memory.selectAgent": "Agente",
  "memory.model": "Modelo (opcional)",
  "memory.modelHint": "Déjalo vacío para usar el modelo configurado en el agente.",
  "memory.compile": "Organizar y guardar",
  "memory.compileHelp": "El agente seleccionado organiza la conversación por temas y la integra en los artículos existentes. El texto de la conversación y los artículos pertinentes se envían al modelo mediante tu agente configurado.",
  "memory.unavailable": "Sin instalar o configurar",
  "memory.allTags": "Todas las etiquetas",
  "memory.updated": "Actualización reciente",
  "memory.titleSort": "Título",
  "memory.sourceNote": "Esta instantánea conserva el texto utilizado para organizar la memoria, incluso si se elimina la sesión original.",
  "memory.noKnowledge": "No se encontraron conocimientos reutilizables; no se modificó ningún artículo.",
  "memory.running": "En curso",
  "memory.completed": "Completado",
  "memory.failed": "Error",
  "memory.cancelled": "Cancelado",
  "memory.extract": "Extrayendo temas",
  "memory.merge": "Integrando conocimientos",
  "memory.commit": "Guardando artículos",
  "memory.done": "Guardado",
  "memory.closeHint": "Puedes cerrar esta ventana durante el proceso y consultar el progreso en el historial de organización.",
  "memory.conflict": "Este artículo cambió durante la operación. Vuelve a cargarlo antes de intentarlo de nuevo; tus cambios no se han guardado.",
  "memory.duplicate": "Ya existe un artículo con este título. Ábrelo para integrar el contenido.",
  "memory.busy": "Ya hay otro proceso en curso. Espera a que termine o cancélalo en el historial.",
  "memory.notFound": "Este artículo, fuente o proceso ya no existe.",
  "memory.noTranscript": "Esta sesión no tiene una conversación que se pueda leer.",
  "memory.agentUnavailable": "El agente seleccionado no está disponible. Comprueba la ruta de su ejecutable en los ajustes.",
  "memory.invalid": "Algunos campos o enlaces no son válidos. Revisa el título, el contenido y los artículos relacionados.",
  "memory.processFailed": "El agente no pudo terminar. Revisa su inicio de sesión, modelo y configuración CLI, y vuelve a intentarlo.",
  "memory.timeout": "El agente agotó el tiempo de espera. Prueba con un modelo disponible o una conversación más corta.",
  "memory.interrupted": "El proceso se interrumpió. Puedes reintentarlo con la instantánea de origen guardada.",
  "memory.tooLarge": "La fuente, el contexto o la salida supera el tamaño admitido. No se ha recortado ni guardado contenido.",
  "memory.invalidOutput": "El agente devolvió datos estructurados no válidos. No se guardó nada; reinténtalo o elige otro agente.",
  "memory.loadError": "No se pudo cargar la memoria. Comprueba la conexión y vuelve a intentarlo.",
  "memory.unsaved": "¿Descartar los cambios sin guardar?",
  "memory.source": "Instantánea de origen",

  // ── Common ──
  "common.cancel": "Cancelar", // Cancel
  "common.confirm": "Aceptar", // OK
  "common.delete": "Eliminar", // Delete
  "common.save": "Guardar", // Save
  "common.create": "Crear", // Create
  "common.close": "Cerrar", // Close
  "chat.imageViewOriginal": "Ver imagen original",
  "chat.imageCopy": "Copiar imagen",
  "chat.imageSave": "Guardar imagen",
  "chat.imageActionFailed": "No se pudo completar la operación con la imagen. Inténtelo de nuevo.",
  "common.copy": "Copiar", // Copy
  "common.cut": "Cortar", // Cut
  "common.paste": "Pegar", // Paste
  "common.selectAll": "Seleccionar todo", // Select All
  "common.copied": "Copiado", // Copied
  "chat.sync.loading": "Sincronizando conversación…",
  "chat.sync.failed": "No se pudo sincronizar. Los mensajes cargados siguen disponibles.",
  "chat.sync.history": "Cargar mensajes anteriores",
  "chat.submission.updateRequired": "Actualiza el servidor antes de enviar mensajes desde este cliente.",
  "chat.submission.sending": "Enviando…",
  "chat.submission.sent": "Enviado",
  "chat.submission.queued": "En cola",
  "chat.submission.failed": "Error al enviar",
  "chat.submission.unknown": "Entrega sin confirmar",
  "chat.submission.check": "Comprobar estado",
  "common.retry": "Reintentar", // Retry
  "common.experimental": "Experimental",
  "common.refresh": "Actualizar", // Refresh
  "common.loading": "Cargando…", // Loading…
  "common.prev": "Anterior", // Previous
  "common.next": "Siguiente", // Next
  "common.on": "Sí", // On
  "common.off": "No", // Off
  "common.gotIt": "Entendido", // Got it
  "common.rename": "Renombrar", // Rename
  "common.edit": "Editar", // Edit
  "common.open": "Abrir", // Open
  "common.session": "Sesión", // Session

  // ── Session types and status ──
  "kind.terminal": "Terminal", // Terminal
  "kind.browser": "Navegador", // Browser
  "status.idle": "Inactivo", // Idle
  "status.running": "En ejecución", // Running
  "status.exited": "Finalizado", // Exited
  "status.error": "Error", // Error
  "status.working": "Procesando", // Working
  "status.asking": "Requiere confirmación", // Needs confirmation
  "status.waiting": "Visto", // Viewed
  "status.unavailable": "Estado no disponible",
  "indicator.unread": "No leído · por revisar", // Unread · awaiting review

  // ── Title bar ──
  "titlebar.builtAt": (time) => `Compilado el ${time}`, // Built at {time}
  "titlebar.versionMismatch": (frontend, backend) =>
    `Versiones no coinciden: frontend v${frontend} ≠ backend v${backend}: recompila o redespliega sincronizado.`, // Version mismatch

  "titlebar.hotReloadedAt": (time) => `Recarga en caliente a las ${time}`, // Hot reloaded at {time}
  "titlebar.themeSystem": (resolved) =>
    `Seguir al sistema (actualmente ${resolved})`, // Follow system (currently {resolved})
  "titlebar.themeDark": "Oscuro", // Dark
  "titlebar.themeLight": "Claro", // Light
  "titlebar.browser": "Navegador integrado", // Built-in Browser
  "titlebar.remoteAccess": "Acceso remoto (navegador)", // Remote Access (Browser)
  "titlebar.connectRemote": "Conectar a servidor remoto", // Connect to Remote Server
  "titlebar.mirrored": "Reflejado", // Mirrored
  "titlebar.mirroredHint":
    "La duplicación está activada: las pestañas, las divisiones y la sesión activa siguen al anfitrión. El interruptor está en el anfitrión.", // Mirroring is on: tabs, splits, and the active session follow the host. The switch is on the host.
  "titlebar.mirroredBy": (n: number) => `Reflejado por ${n}`, // Mirrored by {n}
  "titlebar.mirroredByHint": (n: number) =>
    `${n} cliente${n === 1 ? "" : "s"} remoto${n === 1 ? " está conectado" : "s están conectados"}. Las pestañas, las divisiones y la sesión activa se comparten, y cualquiera de los dos lados puede reorganizarlas.`, // {n} remote clients are connected. Tabs, splits, and the active session are shared, and either side can rearrange them.
  "titlebar.clientsTitle": "Clientes conectados", // Attached clients
  "titlebar.clientUnnamed": "Cliente sin nombre", // Unnamed client
  "titlebar.clientSince": (time: string) => `desde ${time}`, // since {time}
  "titlebar.share": "Compartir", // Share
  // ── Alt-triggered menu bar (Windows/Linux) ──
  "menubar.file": "Archivo", // File
  "menubar.terminal": "Terminal",
  "menubar.help": "Ayuda", // Help
  "menubar.newTerminal": "Nuevo terminal", // New Terminal
  "menubar.visitWebsite": "Visitar el sitio web", // Visit Website
  "menubar.sendFeedback": "Enviar comentarios", // Send Feedback
  "menubar.clearBadges": "Borrar indicadores de notificación", // Clear Notification Badges
  "share.title": "Compartir VelaTerm", // Share VelaTerm
  "share.subtitle":
    "Somos el pequeño equipo que está detrás de VelaTerm. Si te gusta, comparte VelaTerm con otras personas. Ayudarnos a que más gente nos conozca significa muchísimo para nuestro equipo. ¡Gracias por tu apoyo! ❤️", // We're a small team behind VelaTerm. If you enjoy it, please share VelaTerm with others…
  "share.copyLink": "Copiar enlace", // Copy link
  "share.copied": "¡Copiado!", // Copied!
  "share.wechatMoments": "Momentos de WeChat",
  "share.weibo": "Weibo",
  "share.xiaohongshu": "Xiaohongshu",
  "share.xiaohongshuAction":
    "Copiar el texto y el enlace, y abrir el Centro de creadores de Xiaohongshu",
  "share.wechatQrTitle": "Compartir en Momentos de WeChat",
  "share.wechatQrHint":
    "Escanea el código con WeChat, abre el enlace y elige compartirlo en Momentos.",
  "share.backToPlatforms": "Volver a las opciones para compartir",
  "titlebar.appearance": "Apariencia", // Appearance
  "titlebar.showLeft": "Mostrar barra lateral", // Show sidebar
  "titlebar.hideLeft": "Ocultar barra lateral", // Hide sidebar
  "titlebar.showRight": "Mostrar panel de info", // Show info panel
  "titlebar.hideRight": "Ocultar panel de info", // Hide info panel

  // ── Settings ──
  "settings.title": "Ajustes", // Settings
  "settings.catTerminal": "Terminal", // Terminal
  "settings.catBehavior": "Comportamiento", // Behavior
  "settings.catAgents": "Agentes", // Agents
  "settings.permDefault": "Predeterminado", // Default
  "settings.permYolo": "YOLO", // YOLO
  "settings.yoloHint": (flag: string) =>
    `Inicia con ${flag}. Omite todas las confirmaciones de permiso — usar con cuidado.`, // YOLO flag hint
  "settings.permViaEnvHint":
    "Omite todas las confirmaciones de permiso mediante inyección de configuración (sin flag CLI). Se aplica al iniciar la sesión.",
  "settings.catGeneral": "General", // General
  "settings.cliLabel": "Comando de shell",
  "settings.cliInstall": "Instalar el comando ‘vela’",
  "settings.cliUninstall": "Desinstalar el comando ‘vela’",
  "settings.cliInstalledAt": (path: string) => `Instalado en ${path}`,
  "settings.cliConflict": (path: string) =>
    `Ya existe otro comando ‘vela’ en ${path}. VelaTerm no lo sobrescribirá.`,
  "settings.cliHint":
    "Añade `vela <ruta-del-proyecto>` al PATH, como el comando `code` de VS Code.",
  "settings.agentArgsHint":
    "Argumentos de inicio predeterminados aplicados a las nuevas sesiones de cada tipo de agente. Los argumentos por sesión definidos al crear o editar tienen prioridad. Dejar vacío para ninguno.", // Agent default launch args hint
  "settings.agentPathLabel": "Ruta del ejecutable (opcional)", // Executable path (optional)
  "settings.agentPathPlaceholder":
    "p. ej. ~/.local/bin/claude — vacío = buscar en PATH", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "Si se define, las sesiones de este tipo se inician con esta ruta completa en lugar de buscar el comando en el PATH. Útil cuando el agente está instalado pero no en el PATH del shell. Se rellena automáticamente tras una instalación con un clic si se detecta la ubicación.", // Agent executable path hint
  "settings.appearance": "Apariencia", // Appearance
  "settings.accent": "Acento", // Accent
  "settings.accentAuto": "Seguir al tema", // Follow theme
  "settings.density": "Densidad", // Density
  "settings.densityCompact": "Compacta", // Compact
  "settings.densityRegular": "Normal", // Regular
  "settings.densityComfy": "Amplia", // Comfy
  "settings.pane": "Paneles", // Panes
  "settings.paneFlush": "Sin borde", // Flush
  "settings.paneCard": "Tarjeta", // Card
  "settings.divider": "Divisor", // Divider
  "settings.dividerSubtle": "Sutil", // Subtle
  "settings.dividerVisible": "Visible", // Visible
  "settings.nav": "Barra lateral", // Sidebar
  "settings.navTree": "Árbol", // Tree
  "settings.navCompact": "Compacta", // Compact
  "settings.tabs": "Pestañas", // Tabs
  "settings.defaultSessionEngine": "Vista predeterminada de las sesiones nuevas",
  "settings.defaultSessionEngineHint": "Las sesiones existentes conservan la vista con la que se crearon.",
  "settings.dynamicStatusFilter": "Incorporación dinámica al filtro de estado",
  "settings.tabSingle": "Única", // Single
  "settings.tabMulti": "Múltiples", // Multi
  "settings.maxLiveTabs": "Background limit", // Background limit
  "settings.defaultShell": "Shell predeterminada", // Default shell
  "settings.spawnConfirm": "Confirm before spawn", // Confirm before spawn
  "settings.usageAuto": "Usage auto-refresh", // Usage auto-refresh
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.cleanImages": "Limpiar imágenes pegadas automáticamente",
  "settings.cleanImagesHint":
    "Las imágenes pegadas o arrastradas a la terminal se guardan primero como archivos temporales (la ruta se envía al agente). Si está activado, los archivos temporales de esta sesión se eliminan al salir, y los restos de más de 24 h se limpian al iniciar. Las imágenes de los documentos no se tocan.",
  "settings.cleanImagesNow": "Limpiar ahora",
  "settings.cleanImagesResult": (n: number, size: string) =>
    `${n} imágenes temporales limpiadas (${size} liberados).`,
  "settings.cleanImagesEmpty": "No hay imágenes temporales que limpiar.",
  "settings.imagePasteMode": "Pegar imagen",
  "settings.imagePasteUpload": "Pegar ruta de archivo",
  "settings.imagePasteAgent": "Pegado nativo",
  "settings.imagePasteHint":
    "Elige qué se inserta al pegar una imagen (solo escritorio local). Pegar ruta de archivo: guarda la imagen temporalmente e inserta su ruta en Claude o Codex. Pegado nativo: deja que Claude o Codex lea el portapapeles del sistema y muestre su propio marcador de imagen.",
  "settings.imagePasteRemoteHint":
    "Las sesiones remotas siempre pegan la ruta del archivo para que el agente pueda leer la imagen en su equipo. El pegado nativo solo está disponible en el escritorio local.",
  "spawn.title": "Start spawned session?", // Start spawned session?
  "spawn.fromSession": "From", // From
  "spawn.promptLabel": "Prompt", // Prompt
  "spawn.agentLabel": "Agent", // Agent
  "spawn.worktreeLabel": "Separate git worktree", // Separate git worktree
  "spawn.modelLabel": "Modelo", // Model
  "spawn.effortLabel": "Esfuerzo", // Effort
  "spawn.modelDefault": "Predeterminado", // Default
  "spawn.modelLoading": "Cargando modelos…", // Listing models…
  "spawn.modelListUnavailable":
    "Sin lista de modelos — escribe un identificador arriba", // No model list available — type an identifier above
  "spawn.launch": "Launch", // Launch
  "spawn.remaining": (n: number) => `${n} more pending`, // ${n} more pending
  "spawn.notifyTitle": "Spawn session awaiting confirmation", // Spawn session awaiting confirmation
  "orch.title": "¿Iniciar estos agentes?",
  "orch.notifyTitle": "Orquestación pendiente de confirmación",
  "orch.coordinatorName": "Progreso",
  "orch.sharedSettings": "Ajustes comunes",
  "orch.agentLabel": "Agente",
  "orch.modelLabel": "Modelo",
  "orch.effortLabel": "Esfuerzo",
  "orch.nameLabel": "Nombre",
  "orch.promptLabel": "Tarea",
  "orch.worktreeLabel": "Worktree",
  "orch.worktreeNone": "Usar el directorio actual",
  "orch.worktreeShared": "Un worktree compartido",
  "orch.worktreeEach": "Un worktree para cada uno",
  "orch.follow": "Seguir los ajustes comunes",
  "orch.overridden": "cambiado",
  "orch.remove": "Quitar",
  "orch.launch": (n: number) => `Iniciar ${n}`,
  "orch.modelPlaceholder": "predeterminado del agente",
  "orch.effortPlaceholder": "predeterminado del agente",
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "Ver cambios…",
  "changes.title": "Cambios",
  "changes.loading": "Cargando…",
  "changes.loadingDiff": "Cargando diff…",
  "changes.noChanges": "Sin cambios",
  "changes.refresh": "Actualizar",
  "changes.notRepo": "No es un repositorio git",
  "changes.selectFile": "Selecciona un archivo",
  "changes.binary": "Archivo binario: diff por línea no disponible",
  "changes.commitTitle": (hash: string) => `Commit ${hash}`,

  "git.staged": "Preparado",
  "git.changes": "Cambios",
  "git.untracked": "Archivos sin seguimiento",
  "git.committed": "Cambios confirmados",
  "git.stage": "Preparar",
  "git.unstage": "Quitar de preparado",
  "git.stageAll": "Preparar todo",
  "git.unstageAll": "Quitar todo",
  "git.discard": "Descartar",
  "git.deleteFile": "Eliminar",
  "git.viewAll": "Ver todo",
  "git.detached": "(desacoplado)",
  "git.aheadBehind": "Commits por delante y por detrás de la rama upstream",
  "git.commitPlaceholder": "Mensaje del commit",
  "git.amend": "Enmendar el último commit",
  "git.amendCommit": "Enmendar commit",
  "git.commitCount": (n: number) =>
    n === 1 ? "Confirmar 1 archivo" : `Confirmar ${n} archivos`,
  "git.commitNoFiles": "Este commit no cambia archivos",
  "git.noCommits": "Aún no hay commits",
  "git.loadMore": "Cargar más",
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
  "tree.moveGroupToWorktree": "Mover a un worktree…",
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
  "settings.renderer": "Renderizador del terminal", // Terminal renderer
  "settings.redrawOnReveal": "Redibujar al cambiar de pestaña", // Redraw on tab switch
  "settings.catAdvanced": "Avanzado", // Advanced
  "settings.outputScheduler": "Salida con prioridad en primer plano", // Foreground-priority output
  "settings.recordSessions": "Registrar registros de sesión", // Record session logs
  "settings.recordSessionsHint":
    "Desactivado por defecto. Si se activa, la salida del terminal se guarda en un archivo de registro para reproducción de archivo y búsqueda. Las sesiones de terminal normales nunca se graban; las sesiones de agente leen su propia transcripción.", // Record session logs hint
  "settings.fonts": "Fonts", // TODO translate
  "settings.uiFont": "Interface font", // TODO translate
  "settings.uiFontSize": "Interface size", // TODO translate
  "settings.termFont": "Terminal font", // TODO translate
  "settings.termFontSize": "Terminal size", // TODO translate
  "settings.termLineHeight": "Interlineado del terminal",
  "settings.chatTypography": "Vista de conversación",
  "settings.chatTypographyHint": "Estos ajustes de fuente son independientes del terminal y se aplican de inmediato.",
  "settings.chatFont": "Fuente de la conversación",
  "settings.chatFontSize": "Tamaño de fuente de la conversación",
  "settings.chatLineHeight": "Interlineado de la conversación",
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
  "settings.fontUnavailable": "No instalada en este dispositivo",
  "settings.fontAuto": "Auto", // TODO translate
  "settings.fontSmaller": "Smaller", // TODO translate
  "settings.fontLarger": "Larger", // TODO translate
  "settings.fontReset": "Reset", // TODO translate
  "settings.sound": "Sonido de notificación", // Notification sound
  "settings.language": "Idioma", // Language
  "settings.langAuto": "Auto (sistema)", // Auto (system)
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
  "settings.catShortcuts": "Atajos", // Shortcuts
  "settings.scOpenProject": "Abrir proyecto", // Open project
  "settings.scNewTab": "Nueva terminal", // New terminal
  "settings.scNewBrowserTab": "Nueva pestaña de navegador", // New browser tab
  "settings.scClosePane": "Cerrar panel / pestaña", // Close pane / tab
  "settings.scSplitRight": "Dividir a la derecha", // Split right
  "settings.scSplitDown": "Dividir abajo", // Split down
  "settings.scSearch": "Buscar en la terminal", // Find in terminal
  "settings.scGlobalSearch": "Buscar en todas las sesiones", // Search all sessions
  "settings.scSaveDoc": "Guardar documento", // Save document
  "settings.scRecording": "Pulsa las teclas…", // Press keys…
  "settings.scHint":
    "Haz clic en un atajo y pulsa una nueva combinación (se requiere Cmd/Ctrl).", // hint
  "settings.scReset": "Restaurar valores predeterminados", // Restore defaults
  "settings.scConflict": (label: string) => `Ya lo usa "${label}"`, // conflict

  // ── Remote access panel ──
  "remote.title": "Acceso remoto (navegador)", // Remote Access (Browser)
  "remote.desc":
    "Una vez activado, los dispositivos de la misma LAN pueden abrir la dirección de abajo en un navegador, introducir la contraseña y obtener la misma interfaz que el escritorio.", // Once enabled, devices on the same LAN…
  "remote.needPassword": "Primero establece una contraseña de acceso", // Please set an access password first
  "remote.running": (port) => `En ejecución · puerto ${port}`, // Running · port {port}
  "remote.urlsHint":
    "Abre la dirección que esté en el mismo WiFi / subred que tu dispositivo (con varias interfaces de red, elige la correcta; las direcciones VPN/túnel aparecen al final y normalmente no son accesibles desde otros dispositivos):", // Open the address on the same WiFi / subnet…
  "remote.copyUrl": "Clic para copiar la dirección", // Click to copy address
  "remote.moreUrls": (n: number) =>
    n > 1 ? `${n} enlaces más` : `${n} enlace más`, // N more urls
  "remote.lessUrls": "Mostrar menos", // Show less
  "remote.stop": "Detener servidor", // Stop Server
  "remote.passwordPlaceholder": "Establecer contraseña de acceso", // Set access password
  "remote.starting": "Iniciando…", // Starting…
  "remote.start": "Iniciar servidor", // Start Server
  "remote.portLabel": "Puerto", // Port
  "remote.portInvalid": "El puerto debe estar entre 1 y 65535", // Port must be between 1 and 65535
  "remote.ipLabel": "IP", // IP address
  "remote.ipAuto": "Automática (primera dirección LAN)", // Automatic (first LAN address)
  "remote.ipVpn": "VPN", // VPN
  "remote.qrHint":
    "Escanea con tu teléfono para abrir el enlace de emparejamiento en la dirección seleccionada.", // Scan with your phone to open the pairing link on the selected address.
  "remote.fingerprintLabel": "Huella del certificado (SHA-256)", // Certificate fingerprint (SHA-256)
  "remote.fingerprintHint":
    "En la primera conexión, los navegadores advierten de que el certificado no es de confianza: es normal en un certificado autofirmado. Compara esta huella para confirmar que es este equipo.", // On first connect, browsers warn the certificate is untrusted…

  "remote.pairingCreate": "Crear enlace de emparejamiento", // Create pairing link
  "remote.pairingRegenerate": "Regenerar enlace (desconecta todos)", // Regenerate link (disconnects all)
  "remote.pairingCreating": "Generando…", // Generating…
  "remote.pairingHint":
    "Ábrelo en un navegador e introduce la contraseña. Este enlace contiene credenciales de acceso; compártelo solo con tus dispositivos.", // Open in a browser, then enter the password…

  "remote.devicesLabel": "Dispositivos emparejados", // Paired devices
  "remote.lastSeen": "Última conexión", // Last seen
  "remote.revoke": "Revocar", // Revoke
  "remote.deviceBlock": "Bloquear", // Block
  "remote.deviceBlockConfirm": "Confirmar bloqueo", // Confirm block
  "remote.deviceBlockHint":
    "Los dispositivos bloqueados se desconectan y no pueden volver a conectarse (necesitan un nuevo enlace de emparejamiento). Los demás dispositivos no se ven afectados.", // Block hint
  "remote.devicesEmpty": "No hay dispositivos emparejados", // No paired devices yet
  "remote.autoRestartHint":
    "El acceso remoto se reinicia automáticamente al volver a abrir la aplicación. «Detener servidor» lo desactiva.", // Remote access restarts automatically when the app is reopened. Stop Server turns this off.
  "remote.autostartFailed": "Error en el inicio automático:", // Automatic start failed:
  "remote.mirror": "Reflejar el diseño en todos los dispositivos", // Mirror layout across devices
  "remote.mirrorHint":
    "Las pestañas, las divisiones y la sesión activa se mantienen iguales en todos los dispositivos conectados. El foco del teclado no cambia en ninguno.", // Tabs, splits, and the active session stay the same on every connected device. Keyboard focus stays put on each one.

  // ── Remote connection panel ──
  "connect.title": "Conectar a servidor remoto", // Connect to Remote Server
  "connect.pairingPlaceholder": "Pega el enlace de emparejamiento", // Paste pairing link
  "connect.confirmConnect": "Huella correcta, conectar", // Fingerprint matches, connect
  "connect.desc":
    "Introduce la dirección y la contraseña de un VelaTerm remoto para conectarte y controlarlo en una ventana nueva.", // Enter the address and password…
  "connect.addressPlaceholder": "Dirección IP, p. ej. 192.168.1.100", // IP address, e.g. 192.168.1.100
  "connect.portPlaceholder": "Puerto", // Port
  "connect.connecting": "Conectando…", // Connecting…
  "connect.connect": "Conectar", // Connect
  "connect.stagePreparing": "Preparando servidor…",
  "connect.stageTransferring": "Transfiriendo servidor…",
  "connect.stageStarting": "Iniciando servidor…",
  "connect.sshFingerprintLabel": (kt: string) =>
    `Huella de la clave del host SSH (${kt})`,
  "connect.sshHostNew":
    "Primera conexión a este host: verifica la huella antes de continuar.",
  "connect.sshHostChanged":
    "⚠ La clave de este host ha cambiado: podría ser una reinstalación del servidor o un ataque de intermediario. Continúa solo si estás seguro.",
  "connect.urlCertChanged":
    "⚠ La huella del certificado de este servidor ha cambiado desde tu última confirmación: podría ser una reinstalación del servidor o un ataque de intermediario. Continúa solo si estás seguro.",
  "connect.sshPasswordLabel": "Contraseña SSH",
  "connect.sshPasswordPlaceholder": "Contraseña de la cuenta",
  "connect.savedHosts": "Hosts recientes",
  "connect.savedHostsAll": "Todos los hosts recientes",
  "connect.showAllHosts": (n: number) => `Ver todos (${n})`,
  "connect.forgetHost": "Olvidar este host",
  "connect.savedHasPassword": "Contraseña guardada",
  "connect.rememberPassword": "Recordar contraseña",
  "connect.showPassword": "Mostrar contraseña",
  "connect.hidePassword": "Ocultar contraseña",
  "connect.urlPasswordPlaceholder": "Contraseña de acceso",
  "connect.mirror": "Reflejar la aplicación de escritorio remota", // Mirror the remote desktop app
  "connect.mirrorHint":
    "Las pestañas, las divisiones y la sesión activa coinciden con la aplicación de escritorio de la máquina remota; los cambios de cualquier lado se ven en ambos. Si la aplicación de escritorio no está en ejecución, esta conexión abre directamente su base de datos, o una base de datos separada si no existe.", // Same tabs, splits, and active session as the desktop app on the remote machine; changes on either side show on both. If the desktop app is not running, this connection opens its database directly, or a separate database when there is none.
  "connect.shareDesktopDb":
    "Usar la base de datos de la app de escritorio remota",
  "connect.shareDesktopDbHint":
    "Comparte una base de datos con la app de escritorio del equipo remoto (mejor si ambas tienen la misma versión). Desactivado = base de datos independiente.",

  // ── Sidebar ──
  "tree.newSession": "Nueva sesión", // New Session
  "tree.newTerminalSession": "Nuevo terminal", // New Terminal
  "tree.newBrowserPage": "Nueva página de navegador", // New Browser Page
  "tree.newAgentSession": (agent) => `Nueva sesión de ${agent}`, // New {agent} Session
  "tree.newAgentSessionGroup": "Más sesiones de agente", // More Agent Session
  "tree.newAgentSessionCustom": "Nuevo con argumentos…", // New with launch args…
  "tree.resumeSession": "Reanudar sesión…", // Resume Session…
  "tree.newGroup": "Nuevo grupo", // New Group
  "tree.newSubgroup": "Nuevo subgrupo", // New Subgroup
  "tree.newChildSession": "Nueva sesión hija", // New Child Session
  "tree.openSelected": "Abrir sesiones seleccionadas", // Open Selected Sessions
  "tree.archiveSelected": "Archivar sesiones seleccionadas", // Archive Selected Sessions
  "tree.moveSelected": "Mover selección a…", // Move Selected to…
  "tree.deleteSelected": (n) => `Eliminar ${n} elementos seleccionados`, // Delete {n} Selected Items
  "tree.removeProject": "Quitar proyecto", // Remove Project
  "tree.deleteGroup": "Eliminar grupo", // Delete Group
  "tree.deleteSession": "Eliminar sesión", // Delete Session
  "tree.projectRoot": "Raíz del proyecto (sin grupo)", // Project root (no group)
  "tree.moveToSession": "Mover bajo una sesión (como hija)", // Move under a session (as child)
  "tree.moveTo": "Mover a…", // Move to…
  "tree.openNewTab": "Abrir en pestaña nueva", // Open in New Tab
  "tree.forkSession": "Bifurcar sesión", // Fork Session
  "tree.exportSession": "Exportar sesión…", // Export Session…
  "tree.sessionInfo": "Información de sesión", // Session Info
  "tree.groupInfo": "Información del grupo", // Group Info
  "info.branch": "Rama", // Branch
  "info.path": "Ruta", // Path
  "info.recentCommits": "Commits recientes", // Recent Commits
  "info.noCommits": "Sin commits", // No commits
  "tree.killProcess": "Terminar proceso", // Kill Process
  "tree.archiveSession": "Archivar sesión", // Archive Session
  "tree.archiveGroup": "Archivar grupo", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "temp", // scratch
  "tree.persistSession": "Convertir en sesión permanente…", // Make Permanent Session…
  "tree.persistDoc": "Guardar en disco…", // Save to Disk…
  "tree.closeScratch": "Cerrar borrador", // Close Scratch
  "tree.importProject": "Importar proyecto", // Import Project
  "tree.createProject": "Crear proyecto",
  // New Collection / Collection name / research / Create Collection / No folder / Delete Collection
  "tree.newCollection": "Nueva colección",
  "tree.deleteCollection": "Eliminar colección",
  "collection.title": "Nueva colección",
  "collection.name": "Nombre de la colección",
  "collection.namePlaceholder": "research",
  "collection.submit": "Crear colección",
  "collection.tag": "Sin carpeta",
  "collection.deleteTitle": "Eliminar colección",
  "collection.deleteBody": (name) =>
    `¿Eliminar la colección "${name}"? También se eliminarán sus grupos y sesiones. Esto no se puede deshacer.`,
  "tree.cloneProject": "Clonar desde Git", // Clone from Git
  "createProject.title": "Crear proyecto",
  "createProject.name": "Nombre del proyecto",
  "createProject.namePlaceholder": "mi-proyecto",
  "createProject.into": "Crear en",
  "createProject.choose": "Elegir…",
  "createProject.noParent": "Elige una carpeta principal",
  "createProject.invalidName":
    "Introduce un único nombre de carpeta sin / ni \\.",
  "createProject.creating": "Creando…",
  "createProject.submit": "Crear proyecto",
  "clone.title": "Clonar repositorio Git", // Clone Git Repository
  "clone.url": "URL del repositorio", // Repository URL
  "clone.urlPlaceholder": "https://… o git@…",
  "clone.branch": "Rama (opcional)", // Branch (optional)
  "clone.branchPlaceholder": "Rama por defecto si se deja vacío", // Default branch if empty
  "clone.folder": "Nombre de carpeta", // Folder name
  "clone.folderPlaceholder": "Automático desde la URL", // Auto from URL
  "clone.into": "Clonar en", // Clone into
  "clone.choose": "Elegir…", // Choose…
  "clone.noParent": "Elige una carpeta principal", // Choose a parent folder
  "clone.cloning": "Clonando…", // Cloning…
  "clone.cancelling": "Cancelando…",
  "clone.stageStarting": "Iniciando Git…",
  "clone.stageConnecting": "Conectando con el repositorio…",
  "clone.stagePreparing": "Preparando objetos…",
  "clone.stageReceiving": "Recibiendo objetos…",
  "clone.stageResolving": "Resolviendo deltas…",
  "clone.stageCheckout": "Extrayendo archivos…",
  "clone.stageFinalizing": "Finalizando…",
  "clone.stageImporting": "Importando proyecto…",
  "clone.elapsed": (seconds: number) => `${seconds} s transcurridos`,
  "clone.slowHint":
    "No hay progreso desde hace 30 segundos. Comprueba la red o el proxy del equipo remoto; puedes cancelar y volver a intentarlo.",
  "clone.submit": "Clonar", // Clone
  "tree.globalSearch": "Buscar en todas las sesiones", // Search All Sessions
  "tree.archivedSessions": "Sesiones archivadas", // Archived Sessions
  "tree.searchPlaceholder": "Buscar sesiones / grupos…", // Search sessions / groups…
  "tree.clearSearch": "Borrar búsqueda", // Clear search
  "tree.filterWorking": "En curso", // Working
  "tree.filterAsking": "Pendiente", // Pending
  "tree.filterWaiting": "Visto", // Viewed
  "tree.filterStatus": "Filtrar por estado", // Filter by status
  "tree.refreshStatusFilter": "Actualizar filtro de estado",
  "tree.refreshStatusMatch": "Actualizar estado",
  "tree.filterStatusSection": "Estado", // Status
  "tree.filterMarkSection": "Marca", // Mark
  "tree.viewMainName": "Principal",
  "tree.viewUntitled": "Vista sin nombre",
  "tree.viewDefaultName": (n) => `Vista ${n}`,
  "tree.viewPrimary": "Vista principal",
  "tree.viewManage": "Gestionar vista",
  "tree.viewSetPrimary": "Establecer como principal",
  "tree.viewRename": "Cambiar nombre de vista",
  "tree.viewName": "Nombre de la vista",
  "tree.viewDelete": "Eliminar vista",
  "tree.viewDeletePrimary": "La vista principal no se puede eliminar",
  "tree.viewDeleteTitle": "Eliminar vista de árbol",
  "tree.viewDeleteConfirm": (name) =>
    `¿Eliminar “${name}”? Se quitarán su búsqueda y filtros guardados; los proyectos y las sesiones no se verán afectados.`,
  "tree.viewSplitRight": "Dividir la vista de árbol a la derecha",
  "tree.viewSplitDown": "Dividir la vista de árbol hacia abajo",
  "tree.viewAdd": "Copiar la vista de árbol actual a una pestaña nueva",
  "tree.viewCount": (n) => `${n} vista${n === 1 ? "" : "s"} de árbol`,
  "mark.menu": "Marca", // Mark
  "mark.urgent": "Urgente", // Urgent
  "mark.important": "Importante", // Important
  "mark.bug": "Error", // Bug
  "mark.done": "Hecho", // Done
  "mark.wip": "En curso", // In progress
  "mark.pinned": "Fijado", // Pinned
  "mark.idea": "Idea", // Idea
  "mark.caution": "Precaución", // Caution
  "tree.clearAllNotifications":
    "Borrar todas las insignias de notificación (puntos de sesión e insignia del Dock)", // Clear all notification badges…
  "tree.noProjectsPre":
    "Aún no hay proyectos. Haz clic en el icono de carpeta o pulsa ", // No projects yet. Click the folder button, or press
  "tree.noProjectsPost": " para importar un directorio.", // to import a directory.
  "tree.openProject": "Abrir proyecto", // Open Project
  "tree.noAttention": "Ninguna sesión coincide con el filtro de estado", // No sessions match the status filter
  "tree.noMatch": "Sin coincidencias", // No matches

  // Dialog fields
  "tree.groupName": "Nombre del grupo", // Group name
  "tree.sessionNameAuto": "Nombre de sesión (vacío = automático)", // Session name (leave empty to auto-name)
  "tree.editSession": "Editar sesión", // Edit Session
  "tree.sessionName": "Nombre de sesión", // Session name
  "tree.shellLabel": "Shell (vacío = predeterminado del sistema)", // Shell (leave empty for system default)
  "tree.shellMenu": "Shell",
  "tree.downloadFullGitbash": "Descargar Git Bash completo",
  "gitbash.title": "Git Bash",
  "gitbash.downloading": "Descargando Git Bash completo…",
  "gitbash.extracting": "Extrayendo Git Bash completo…",
  "gitbash.done": "Git Bash completo está listo.",
  "gitbash.failed": "Error al descargar Git Bash",
  "tree.shellSystemDefault": "Predeterminado del sistema", // System default
  "form.customOption": "Personalizado…", // Custom…
  "tree.cwdLabel": "Directorio de trabajo (vacío = raíz del proyecto)", // Working directory (leave empty for project root)
  "tree.initCmdLabel": "Comando de inicio (opcional)", // Startup command (optional)
  "tree.engineLabel": "Se abre en",
  "tree.engineTui": "Vista de terminal",
  "tree.engineChat": "Vista de conversación",
  // The agent runs its own terminal interface.
  "tree.engineTuiHint": "El agente usa su propia interfaz de terminal.",
  // Messages and tool cards, with buttons for permission questions.
  "tree.engineChatHint": "Presentación con mensajes y tarjetas de herramientas; las solicitudes de permiso se responden en la interfaz.",
  "tree.agentArgsLabel": "Argumentos de inicio (opcional)", // Launch args (optional)
  // Working directory / Leave empty for the default
  "tree.workingDirLabel": "Directorio de trabajo",
  "tree.workingDirPlaceholder": "Déjalo vacío para usar el predeterminado",
  "preset.execPathLabel": "Ejecutable (opcional)",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint":
    "Déjalo vacío para usar el comando configurado del agente. Indícalo para que solo esta sesión use un reemplazo compatible.",
  "preset.saveLabel": "Guardar como preajuste",
  "preset.namePlaceholder": "Nombra este preajuste",
  "preset.iconChoose": "Elegir icono",
  "preset.iconClear": "Quitar",
  "preset.iconHint":
    "Las imágenes cuadradas funcionan mejor; el resto se recorta y escala a 64x64.",
  "tree.permissionSkipLabel": "Omitir todas las confirmaciones de permiso", // Skip all permission confirmations
  "tree.permissionSkipHint":
    "Inicia con el indicador de omisión de este agente (p. ej. Claude --dangerously-skip-permissions; Codex también desactiva su sandbox). Se aplica en cada inicio; úsalo con cuidado.",
  "tree.permissionUnsupported":
    "OpenCode controla los permisos mediante su archivo de configuración: no hay indicador de inicio, así que esto no aplica.",
  "tree.permissionUnsupportedPi":
    "Pi ejecuta las herramientas sin solicitudes de permiso por diseño: esta opción no aplica.",

  // Diálogo «Nueva sesión de agente»
  "newAgent.desc":
    "Opcionalmente, asigna un nombre a la sesión y agrega argumentos de inicio personalizados (pasados al comando del agente, p. ej. --model opus). Deja ambos vacíos y pulsa Enter para iniciarla como de costumbre.", // Optionally name the session and add custom launch args…

  // Delete confirmation
  "tree.batchDeleteTitle": "Eliminación en lote", // Batch Delete
  "tree.deleteProjectTitle": "Eliminar proyecto", // Delete Project
  "tree.deleteGroupTitle": "Eliminar grupo", // Delete Group
  "tree.deleteSessionTitle": "Eliminar sesión", // Delete Session
  "tree.batchDeleteBody": (n) =>
    `Eliminar los ${n} elementos seleccionados (los proyectos/grupos eliminan en cascada sus subgrupos y sesiones). Esta acción no se puede deshacer.`, // Delete the {n} selected items…
  "tree.deleteProjectBody": (name) =>
    `¿Eliminar el proyecto «${name}»? También se eliminarán todos sus subgrupos y sesiones. Esta acción no se puede deshacer.`, // Delete project "{name}"?…
  "tree.deleteGroupBody": (name) =>
    `¿Eliminar el grupo «${name}»? También se eliminarán todos sus subgrupos y sesiones. Esta acción no se puede deshacer.`, // Delete group "{name}"?…
  "tree.deleteSessionBody": (name) =>
    `¿Eliminar la sesión «${name}» (y todas sus sesiones hijas)? Esta acción no se puede deshacer.`, // Delete session "{name}"…
  "tree.deleteWorktrees": (n) =>
    `Eliminar también los worktrees de git asociados (${n} en total; puede fallar si el árbol de trabajo tiene cambios)`, // Also remove associated git worktrees…

  // Session information dialog
  "info.name": "Nombre", // Name
  "info.type": "Tipo", // Type
  "info.status": "Estado", // Status
  "info.notYetCaptured":
    "Aún no generado (se captura tras la primera ejecución)", // Not yet generated (captured after first run)
  "info.sessionId": "ID de sesión", // Session ID
  "info.cwd": "Directorio", // Working dir
  "info.initCmd": "Comando", // Startup cmd
  "info.agentArgs": "Argumentos", // Launch args
  "info.launchCmd": "Comando completo", // Full launch command
  "info.permission": "Permiso", // Permission
  "info.permissionSkip": "Omitir todas las confirmaciones", // Skip all confirmations
  "info.parentSessionId": "ID del padre", // Parent ID
  "info.termTitle": "Título del terminal", // Terminal title
  "info.createdAt": "Creado el", // Created at

  // Resume-session dialog
  "importSessions.results": ({ count }: { count: number }) => `Resultados: ${count}`,
  "importSessions.selected": ({ count }: { count: number }) => `Seleccionadas: ${count}`,
  "importSessions.clearSelection": "Borrar selección",
  "importSessions.clearSearch": "Borrar búsqueda",
  "importSessions.noHistory": "No se encontraron sesiones anteriores para este directorio de proyecto.",
  "importSessions.title": "Importar sesiones",
  "importSessions.description": "Busca sesiones existentes de Codex, Claude y OpenCode cuyo directorio de trabajo coincida con este proyecto. Selecciona las sesiones que quieras añadir al proyecto y abre una para continuar la conversación.",
  "importSessions.search": "Buscar por título, agente o ID de sesión",
  "importSessions.empty": "No se encontraron sesiones coincidentes.",
  "importSessions.imported": "Ya importada",
  "importSessions.confirm": ({ count }: { count: number }) => `Importar (${count})`,
  "importSessions.success": ({ count }: { count: number }) => `Sesiones añadidas al proyecto: ${count}.`,
  "resume.title": "Reanudar sesión", // Resume Session
  "resume.desc":
    "Elige el tipo de agente e introduce el session id propio del agente; al abrir se retoma la conversación original.", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "Tipo de agente", // Agent type
  "resume.sessionIdPlaceholder": "Session id de la conversación", // Conversation session id
  "resume.confirm": "Reanudar y abrir", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "Nueva sesión de worktree…", // New Worktree Session…
  "worktree.worktreeNameLabel": "Nombre del worktree", // Worktree name
  "worktree.worktreeNameHint":
    "Se usa como nombre del directorio y la rama del worktree.", // Used as the worktree directory and branch name.
  "worktree.createFailed": "No se pudo crear el worktree", // Couldn't create the worktree
  "worktree.noRepoRoot":
    "Este proyecto no tiene una ruta de repositorio git utilizable.", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "Ninguno", // None
  "worktreeSel.modeNew": "Nuevo", // New
  "worktreeSel.modeExisting": "Existente", // Existing
  "worktreeSel.loading": "Cargando worktrees…", // Loading worktrees…
  "worktreeSel.empty": "No hay worktrees existentes en este repositorio.", // No existing worktrees in this repository.
  "worktreeSel.loadFailed":
    "No se pudieron listar los worktrees (¿no es un repositorio git?).", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint":
    "Las sesiones creadas en este grupo usarán este worktree de forma predeterminada.", // Sessions created in this group will use this worktree by default.
  "worktree.moveGroupTitle": "Mover el grupo a un worktree",
  "worktree.moveGroupHint":
    "Las sesiones que crees en este grupo a partir de ahora usarán este worktree. Las que ya existen conservan su directorio actual.",

  // ── Archive panel ──
  "archive.title": "Sesiones archivadas", // Archived Sessions
  "archive.empty1": "No hay sesiones archivadas.", // No archived sessions.
  "archive.empty2":
    "Haz clic derecho en una sesión de la barra lateral y elige «Archivar sesión» para guardarla aquí.", // Right-click a session in the sidebar…
  "archive.restore": "Restaurar como sesión normal", // Restore to normal session
  "archive.export": "Exportar contexto completo como Markdown", // Export full context as Markdown
  "archive.deleteForever": "Eliminar permanentemente (incluida la grabación)", // Delete permanently (with recording)
  "archive.pickOne":
    "Selecciona una sesión archivada a la izquierda para ver su transcripción", // Select an archived session on the left…
  "archive.recordingEnd": "--- Fin de la grabación ---", // --- End of recording ---
  "archive.readRecordingFailed": (err) => `Error al leer la grabación: ${err}`, // Failed to read recording: {err}
  "archive.searchRecording": "Buscar en la grabación…", // Search in recording…
  "archive.searchTranscript": "Buscar en la transcripción…", // Search transcript…
  "archive.searchPlaceholder": "Buscar en archivadas…", // Search archived content…
  "archive.msgCountAll": (n) => (n === 1 ? "1 mensaje" : `${n} mensajes`), // {n} messages
  "archive.msgCountFiltered": (shown, total) => `${shown} / ${total} mensajes`, // {shown} / {total} messages
  "archive.you": "Tú", // You
  "archive.toolsUsed": (tools) => `Herramientas: ${tools}`, // Tools: {tools}
  "archive.noMatch": "No hay mensajes coincidentes", // No matching messages
  "archive.emptyTranscript": "La transcripción está vacía", // Transcript is empty
  "archive.loadingTranscript": "Cargando transcripción…", // Loading transcript…

  // ── Global session-content search ──
  "search.allPlaceholder": "Buscar en todo el contenido de las sesiones…", // Search across all session content…
  "search.hint":
    "Busca en el contenido de las sesiones. Las archivadas se excluyen por defecto; marca «Incluir archivadas» para añadirlas.", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "Incluir archivadas", // Include archived
  "search.includeArchivedHint":
    "Buscar también en sesiones archivadas (desactivado por defecto)", // Also search archived sessions (off by default)
  "search.searching": "Buscando…", // Searching…
  "search.noResults": "No se encontraron coincidencias", // No matches found
  "search.sessionCount": (n) => (n === 1 ? "1 sesión" : `${n} sesiones`), // n sessions
  "search.matchCount": (n) =>
    n === 1 ? "1 coincidencia" : `${n} coincidencias`, // n matches
  "search.pickSession":
    "Selecciona una sesión a la izquierda para ver sus coincidencias", // Select a session on the left to see its matches
  "search.openSession": "Abrir sesión", // Open session
  "search.backToResults": "Volver a los resultados", // Back to results
  "search.archivedBadge": "Archivada", // Archived
  "search.summary": (m, s) =>
    `${m} ${m === 1 ? "coincidencia" : "coincidencias"} · ${s} ${s === 1 ? "sesión" : "sesiones"}`, // X matches · N sessions
  "search.matchPosition": (n, total) => `${n} de ${total}`, // N of M
  "search.roleTerminal": "Terminal", // Terminal
  "search.collapseGroup": "Contraer", // Collapse
  "search.expandGroup": "Expandir", // Expand
  "search.cappedNote": (l, total) => `${l} de ${total} localizables`, // L of total locatable

  // ── Center pane ──
  "center.noSession": "Sin sesión", // No session
  "center.noSessionHintPre": "Elige una sesión en la barra lateral, o pulsa ", // Pick a session from the sidebar, or press
  "center.noSessionHintPost": " para crear un terminal", // to create a terminal
  "center.createTerminal": "Crear terminal", // Create Terminal
  "tab.unsavedDot": "Cambios sin guardar", // Unsaved changes
  "tab.newTerminal": "Nuevo terminal", // New terminal
  "tab.newDocument": "Nuevo documento", // New document
  "tab.bgTitle": (n) =>
    `Pestañas en segundo plano: ${n} (procesos aún en ejecución)`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `Fondo ${n}`, // Background {n}
  "tab.scratchFallback": "(terminal temporal)", // (scratch terminal)
  "tab.killBgTab":
    "Terminar esta pestaña en segundo plano (sus procesos finalizarán)", // Kill this background tab…
  "tab.newBrowserTab": "Nueva pestaña", // New Tab
  "tab.refreshFile": "Recargar archivo", // Refresh File
  "tab.closeOthers": "Cerrar otras pestañas", // Close Other Tabs
  "tab.closeRight": "Cerrar pestañas a la derecha", // Close Tabs to the Right
  "tab.closeAll": "Cerrar todas las pestañas", // Close All Tabs
  "tab.sendToBackground": "Enviar al segundo plano", // Send to Background

  // ── Navegador integrado ──
  "browser.back": "Atrás", // Back
  "browser.forward": "Adelante", // Forward
  "browser.reload": "Recargar", // Reload
  "browser.desktopOnly":
    "Las pestañas del navegador solo se abren en la aplicación de escritorio.", // Browser tabs open in the desktop app only.
  "browser.stop": "Detener la carga", // Stop loading
  "browser.openExternal": "Abrir en el navegador del sistema", // Open in system browser
  "browser.addressPlaceholder": "Introduce una URL o términos de búsqueda", // Enter URL or search terms
  "browser.quickAccess": "Acceso rápido", // Quick access
  "browser.loading": "Cargando…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "¿Salir de VelaTerm?", // Quit VelaTerm?
  "quit.body":
    "Se detendrán todas las sesiones de terminal y de agente en ejecución.", // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "Guardar espacio de trabajo", // Save workspace
  "quit.saveWorkspaceHint":
    "Abrir las mismas pestañas y divisiones la próxima vez. Las terminales se restauran, pero no se reinician.", // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "Salir", // Quit
  "dormant.body":
    "Restaurado desde el espacio de trabajo guardado. Todavía no hay ningún proceso en ejecución.", // Restored from your saved workspace. No process is running yet.
  "dormant.start": "Iniciar", // Start
  "overlimit.title": (max) => `Límite de segundo plano superado (${max})`, // Background keep-alive over limit ({max})
  "overlimit.body":
    "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
  "overlimit.kill": "End Selected", // End Selected
  "overlimit.keep": "Keep for Now", // Keep for Now
  "overlimit.earliest": "earliest", // earliest
  "overlimit.statusWorking": "working", // working
  "overlimit.statusAsking": "awaiting reply", // awaiting reply
  "overlimit.statusWaiting": "waiting", // waiting

  // ── Terminal pane ──
  "term.paste": "Pegar", // Paste
  "term.pasteUseShortcut": "Pegar (pulsa ⌘V)", // Paste (press ⌘V)
  "term.selectAll": "Seleccionar todo", // Select All
  "term.autoCopied": (n: number) => `${n} caracteres copiados · ⌘V para pegar`,
  "term.clear": "Limpiar", // Clear
  "term.searchMenu": "Buscar…", // Search…  ⌘F
  "term.splitRight": "Dividir a la derecha", // Split right (⌘D)
  "term.splitDown": "Dividir abajo", // Split down (⌘⇧D)
  "term.closePane": "Cerrar división", // Close split
  "term.redraw": "Redibujar", // Redraw
  "term.mirrorTooltip":
    "Mostrando en espejo (el tamaño lo controla otro cliente). Haz clic para ajustar el PTY a esta ventana", // Mirroring (size controlled by another client)…
  "term.mirrorBadge": (dims) =>
    `⤢ Espejo${dims} · clic para ajustar a esta ventana`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) =>
    `⤢ Espejo${dims} · ajustar a esta ventana`, // ⤢ Mirror{dims} · fit this window
  "term.imgUploadFailed": (n, lastError) =>
    `Error al subir ${n} imagen${n === 1 ? "" : "es"}${lastError ? `: ${lastError}` : ""}`, // Image upload failed for {n} images…
  "term.imgClipboardUnavailable":
    "No se pudo leer la imagen del portapapeles. Vuelve a copiarla e inténtalo de nuevo.",
  "term.starting": (agent) => `Iniciando ${agent}…`, // Starting {agent}…
  "term.startFailed": (err) => `Error al iniciar: ${err}`, // Failed to start: {err}

  // ── Tarjeta de ayuda para instalar un agente ──
  "agentInstall.title": (label) => `${label} no está instalado`, // {label} is not installed
  "agentInstall.desc": (label) =>
    `VelaTerm no encontró ${label} en tu PATH. Instálalo para iniciar esta sesión.`, // couldn't find {label} on PATH
  "agentInstall.install": "Instalar ahora", // Install now
  "agentInstall.retry": "Reintentar", // Retry launch
  "agentInstall.dismiss": "Lo haré yo mismo", // I'll do it myself
  "agentInstall.docs": "Documentación", // Install docs
  "agentInstall.needsNode": "Requiere Node.js / npm", // Requires Node.js / npm
  "agentInstall.afterInstall": "Después de instalar:", // After install:
  "agentInstall.pathSaved": (label: string) =>
    `Ruta del ejecutable de ${label} guardada en Ajustes:`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} está instalado`, // {label} is installed
  "agentInstall.doneDesc": "Reinicia esta sesión para empezar a usarlo.", // Relaunch this session to start using it.
  "agentInstall.restartNow": "Reiniciar ahora", // Relaunch now
  "agentInstall.later": "Más tarde", // Later
  "search.placeholder": "Buscar en el terminal", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "WYSIWYG", // WYSIWYG
  "doc.source": "Código", // Source
  "doc.searchPlaceholder": "Buscar", // Find
  "doc.searchReplacePlaceholder": "Reemplazar", // Replace
  "doc.searchReplace": "Reemplazar", // Replace
  "doc.searchReplaceAll": "Todo", // All
  "doc.searchNoMatch": "Sin resultados", // No results
  "doc.searchCaseSensitive": "Coincidir mayúsculas", // Match case
  "doc.searchToggleReplace": "Alternar reemplazo", // Toggle replace
  "doc.fileTree": "Árbol de archivos", // File tree
  "doc.treeUp": "Carpeta superior", // Parent folder
  "doc.sidebar": "Barra lateral", // Sidebar
  "doc.unsaved": "Sin guardar", // Unsaved
  "doc.saveAsTitle": "Guardar como", // Save As
  "doc.saveAsName": "Nombre de archivo", // File name
  "doc.outline": "Esquema", // Outline
  "doc.outlineEmpty": "Sin encabezados", // No headings
  "doc.saving": "Guardando…", // Saving…
  "doc.overwriteConfirm":
    "Ya existe un archivo con ese nombre. Pulsa «Sobrescribir» para reemplazarlo.", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "Guardar", // Save
  "doc.externalChanged":
    "El archivo fue modificado en el disco (tienes cambios locales sin guardar).", // The file was modified on disk…
  "doc.reloadDiscard": "Recargar (descartar mis cambios)", // Reload (discard my changes)
  "doc.externalChangedClean": "El archivo fue modificado en el disco.", // The file was modified on disk.
  "doc.reload": "Recargar", // Reload
  "doc.ignore": "Ignorar", // Ignore
  "doc.loadingFile": (title) => `Cargando ${title}…`, // Loading {title}…
  "doc.closeTitle": "Cerrar documento", // Close Document
  "doc.unsavedBody": (title) => `«${title}» tiene cambios sin guardar.`, // "{title}" has unsaved changes.
  "doc.saveAndClose": "Guardar y cerrar", // Save & Close
  "doc.closeNoSave": "Cerrar sin guardar", // Close Without Saving
  "doc.conflictTitle": "Conflicto al guardar", // Save Conflict
  "doc.conflictBody":
    "El archivo en el disco fue modificado externamente. ¿Sobrescribirlo igualmente con el contenido actual?", // The file on disk was modified externally…
  "doc.overwrite": "Sobrescribir", // Overwrite
  "doc.saveFailed": (err) => `Error al guardar: ${err}`, // Save failed: {err}
  "doc.closeTab": "Cerrar pestaña", // Close Tab
  "doc.truncatedReadonly": (size: string) =>
    `Solo lectura: mostrando los primeros 10 MB de ${size}. Se desactivó guardar para no sobrescribir el resto del archivo.`,
  "doc.imgLoading": (title, size) => `Cargando ${title} (${size})…`, // Loading {title} ({size})…
  "doc.imgBeingWritten":
    "El archivo se está escribiendo; se recargará automáticamente cuando se estabilice.", // The file is being written; it will reload automatically once it settles.
  "doc.imgDecodeFailed":
    "No se puede mostrar esta imagen (formato no compatible o archivo dañado).", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "Ajustar", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "Exportar PDF", // Export PDF
  "doc.diagramError": "Error de diagrama", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "Ninguna sesión seleccionada", // No session selected
  "panel.openInEditor": "Abrir en el editor", // Open in Editor
  "panel.openInEditorTooltip":
    "Abrir en el editor de documentos del panel central (igual que el comando view)", // Open in the document editor…
  "panel.preview": "Vista previa", // Preview
  "panel.cantRead": "(no se puede leer este archivo)", // (cannot read this file)
  "panel.binary": "(archivo binario, sin vista previa)", // (binary file, no preview)
  "panel.truncated": "\n…(contenido truncado)", // …(content truncated)
  "panel.showHidden": "Mostrar archivos ocultos", // Show hidden files
  "panel.hideHidden": "Ocultar archivos ocultos", // Hide hidden files

  // ── File-tree actions (Files context menu and header add button) ──
  "files.newFile": "Nuevo archivo", // New File
  "files.newFolder": "Nueva carpeta", // New Folder
  "files.nameLabel": "Nombre", // Name
  "files.newTooltip": "Nuevo archivo o carpeta", // New file or folder
  "files.openInTerminal": "Open in Terminal",
  "files.revealInFinder": "Show in File Manager",
  "files.copyPath": "Copy Path",
  "files.copyRelPath": "Copy Relative Path",
  "files.filterPlaceholder": "Filter files…",
  "files.dblClickOpen": "Doble clic para abrir",
  "files.deleteConfirm": (name) =>
    `¿Eliminar "${name}"? Esto no se puede deshacer.`, // Delete "{name}"? This can't be undone.

  // ── File transfer (remote access) ──
  "transfer.uploadsTitle": "Subidas", // Uploads
  "transfer.download": "Descargar", // Download
  "transfer.upload": "Subir archivos…", // Upload Files…
  "transfer.uploadTooltip": "Subir archivos a esta carpeta", // Upload files to this folder
  "transfer.clear": "Limpiar", // Clear
  "transfer.cancelled": "Cancelado", // Cancelled
  "transfer.failed": "Falló", // Failed
  "transfer.stalled": "Reconectando…", // Reconnecting…
  "transfer.foldersUnsupported": "No se pueden subir carpetas.", // Folders can't be uploaded.

  // ── Status bar ──
  "statusbar.sessions": (n) => (n === 1 ? "1 sesión" : `${n} sesiones`), // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `Clic para mostrar solo sesiones "${label}" en la barra lateral (clic de nuevo para quitar)`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `Fondo ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `Pestañas en segundo plano (límite ${max}; al superarlo se termina automáticamente la pestaña inactiva más antigua)`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) =>
    `Pestaña en segundo plano terminada: ${name} (límite superado)`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) =>
    `Acceso remoto por navegador activado: ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "Permisos: Preguntar", // Perms: Ask
  "statusbar.permSkip": "Permisos: Omitir", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip":
    "Modo de permisos de esta sesión · haz clic para cambiar (solo esta sesión)", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "Permisos de esta sesión", // This session's permissions
  "statusbar.permOptAsk": "Preguntar cada vez (predeterminado)", // Ask each time (default)
  "statusbar.permScopeHint":
    "Se aplica solo a esta sesión. Para los ajustes globales, ve a Ajustes ▸ Agentes.", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg":
    "Permiso cambiado. La sesión debe reiniciarse para aplicarlo. El reinicio reanuda la conversación actual, pero interrumpe cualquier tarea en curso. ¿Reiniciar ahora?", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "Reiniciar ahora", // Restart now
  "statusbar.permRestartLater": "Más tarde", // Later
  "statusbar.permScopeTitle": "¿Aplicar a?", // Apply to?
  "statusbar.permScopeSession": "Solo esta sesión", // This session only
  "statusbar.permScopeGlobal": "Predeterminado global", // Global default
  "statusbar.permScopeGlobalHint":
    "Se aplica ahora a esta sesión y pasa a ser el valor predeterminado para futuras sesiones de este tipo (sincronizado con Ajustes).", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

  // ── Store, notifications, and export ──
  "notify.working": "⏳ Procesando…", // ⏳ Working…
  "notify.asking": "❓ Necesita tu confirmación", // ❓ Needs your confirmation
  "notify.waiting": "✅ Respondido", // ✅ Replied
  "store.subtask": "Subtarea", // Subtask
  "store.splitPane": "División", // Split
  "export.failedTitle": "Error al exportar la sesión", // Failed to export session
  "export.contextSuffix": "contexto", // context

  // ── Error panel ──
  "err.renderTitle": "Error de renderizado", // Rendering Error
  "err.renderDesc":
    "Ocurrió un error inesperado. La información de abajo puede ayudar a localizar el problema.", // An unexpected error occurred…
  "err.reload": "Recargar", // Reload
  "err.uncaughtTitle": "Error no capturado", // Uncaught Error
  "err.uncaughtDesc":
    "La información de abajo puede ayudar a localizar el problema.", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser":
    "La reproducción de grabaciones aún no es compatible en el navegador", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) => `Error al subir la imagen (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.connecting": "Conectando…", // Connecting…
  "login.remoteAccess": "Acceso remoto", // Remote Access
  "login.desc":
    "Introduce la contraseña de acceso para conectarte a este terminal.", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "Contraseña de acceso", // Access password
  "login.connect": "Conectar", // Connect
  "login.wrongPassword": "Contraseña incorrecta", // Wrong password
  "login.rateLimited":
    "Demasiados intentos. Espera un minuto y vuelve a intentarlo.", // Too many attempts. Please wait a minute and try again.
  "login.failed": "Error de inicio de sesión, inténtalo de nuevo", // Login failed, please try again
  "login.pairingRequired":
    "Este servidor requiere un enlace de emparejamiento. Abre el enlace generado en el panel de Acceso remoto de la app de escritorio.", // This server requires a pairing link
  "login.authFailed":
    "Error de autenticación. Comprueba la contraseña de acceso o abre un nuevo enlace de emparejamiento si se ha regenerado.", // Authentication failed, check password or use a new pairing link
  "dir.title": "Elegir directorio del proyecto", // Choose Project Directory
  "dir.pathPlaceholder": "Busca, o escribe una ruta y pulsa Enter (admite ~)", // Search, or type a path and press Enter (supports ~)
  "dir.up": "Subir un nivel", // Up one level
  "dir.newFolder": "Nueva carpeta", // New Folder
  "dir.newFolderPlaceholder": "Nombre de la carpeta", // Folder name
  "dir.goInput": "Ir a la ruta escrita", // Go to typed path
  "dir.noSubdirs": "(sin subdirectorios)", // (no subdirectories)
  "dir.empty": "(carpeta vacía)", // (empty folder)
  "dir.noMatch": "Sin elementos coincidentes", // No matching items
  "dir.target": "Carpeta destino", // Target
  "dir.showHidden": "Mostrar elementos ocultos", // Show hidden items
  "dir.importing": "Importando…", // Importing…
  "dir.choose": "Elegir este directorio", // Choose This Directory
  "conn.reconnecting": "Conexión perdida, reconectando…", // Connection lost, reconnecting…
  "conn.reconnectNow": "Reconectar ahora", // Reconnect now
  "conn.retrying": "Reconectando…", // Reconnecting…
  "conn.sshReconnecting": "Enlace SSH perdido, reconstruyendo el túnel…", // SSH link lost, rebuilding the tunnel…
  "conn.sshDown":
    "El enlace SSH está caído — pulsa «Reconectar ahora» para reintentar", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "Error en la solicitud", // Request failed
  "reqerr.dismiss": "Cerrar", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "Registro de errores", // Error Log
  "errlog.empty": "No hay errores registrados.", // No errors recorded.
  "errlog.copyAll": "Copiar todo", // Copy all
  "errlog.clear": "Borrar", // Clear
  "errlog.close": "Cerrar", // Close

  // ── Mobile ──
  "mobile.toDesktop": "Cambiar a versión de escritorio", // Switch to desktop
  "mobile.empty1": "No hay sesiones.", // No sessions.
  "mobile.noMatch": "No hay sesiones coincidentes", // No matching sessions
  "mobile.empty2":
    "Crea una en la aplicación de escritorio o en el navegador de un ordenador y aparecerá aquí automáticamente.", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ Atrás", // ‹ Back
  "mobile.selCopy": "Copiar", // Copy
  "mobile.selCancel": "Cancelar", // Cancel

  // ── Other shared components ──
  "splitter.dragToResize": "Arrastra para redimensionar", // Drag to resize
  "transport.wsDisconnected": "WebSocket desconectado", // WebSocket disconnected
  "transport.wsConnectFailed": "Fallo de conexión WebSocket", // WebSocket connection failed
  "transport.cmdFailed": "El comando falló", // Command failed
  "transport.remoteCmdForbidden": (cmd: string) =>
    `Comando no disponible para clientes remotos: ${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) =>
    `Clave de configuración no modificable por clientes remotos: ${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) =>
    `Los clientes remotos no pueden acceder a archivos del directorio de datos de la aplicación: ${path}`, // Remote clients cannot access files in the app data directory

  // ── Crepe（editor WYSIWYG）──
  "crepe.placeholder": "Escribe texto, o pulsa / para el menú de inserción", // Type text, or press / for the insert menu
  "crepe.textGroup": "Texto", // Text
  "crepe.paragraph": "Texto", // Text
  "crepe.h1": "Título 1", // Heading 1
  "crepe.h2": "Título 2", // Heading 2
  "crepe.h3": "Título 3", // Heading 3
  "crepe.h4": "Título 4", // Heading 4
  "crepe.h5": "Título 5", // Heading 5
  "crepe.h6": "Título 6", // Heading 6
  "crepe.quote": "Cita", // Quote
  "crepe.divider": "Divisor", // Divider
  "crepe.listGroup": "Lista", // List
  "crepe.bulletList": "Lista con viñetas", // Bullet List
  "crepe.orderedList": "Lista numerada", // Ordered List
  "crepe.taskList": "Lista de tareas", // Task List
  "crepe.advancedGroup": "Insertar", // Insert
  "crepe.image": "Imagen", // Image
  "crepe.codeBlock": "Bloque de código", // Code Block
  "crepe.table": "Tabla", // Table
  "crepe.math": "Fórmula", // Math
  "crepe.linkPlaceholder": "Pega o escribe un enlace…", // Paste or type a link…
  "crepe.upload": "Subir", // Upload
  "crepe.uploadImage": "Subir imagen", // Upload Image
  "crepe.orPasteImageLink": "o pega un enlace de imagen", // or paste an image link
  "crepe.imageCaption": "Pie de imagen", // Image caption
  "crepe.confirm": "Confirmar", // Confirm
  "crepe.searchLanguage": "Buscar lenguaje", // Search language
  "crepe.noResult": "Sin resultados", // No results
  "crepe.edit": "Editar", // Edit
  "crepe.collapse": "Plegar", // Collapse
  // ── Panel derecho / barra inferior ──
  "info.project": "Proyecto", // Project
  "panel.sessionInfo": "Información de sesión", // Session info
  "panel.gitTitle": "Estado de Git", // Git status
  "panel.gitProbing": "Comprobando…", // Checking…
  "panel.gitNotRepo": "No es un repositorio Git", // Not a Git repository
  "panel.gitBranch": "Rama", // Branch
  "panel.gitStaged": "Preparado", // Staged
  "panel.gitUnstaged": "Modificado", // Changed
  "panel.gitUntracked": "Sin seguimiento", // Untracked
  "bottombar.running": "En ejecución", // Running
  "bottombar.collapseTasks": "Contraer tareas", // Collapse tasks
  "bottombar.expandTasks": "Expandir tareas", // Expand tasks
  "bottombar.sound": "🔔 Sonido", // 🔔 Sound
  "bottombar.muted": "🔕 Silencio", // 🔕 Muted
  "bottombar.overview": "Resumen de sesiones", // Sessions overview
  "bottombar.noSessions": "Sin sesiones", // No sessions
  "doc.pdfFilter": "Archivo PDF", // PDF file
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

  // ── Vista de conversación (la sesión del agente leída como diálogo) ──
  "session.showConversation": "Vista de conversación",
  "session.showTerminal": "Vista de terminal",
  "session.switchTitle": "Cambiar de vista reinicia el agente",
  "session.switchBody": "El turno en curso se interrumpirá. La conversación se conserva.",
  "session.switchConfirm": "Cambiar",
  "session.loading": "Leyendo la conversación…",
  "session.unavailable": "Esta conversación aún no se puede leer",
  "session.working": "Trabajando…",
  "session.thinking": "Razonamiento",
  "session.toolRunning": "en curso",
  "session.toolUnknown": "Herramienta",
  "session.toolFailed": "Falló",
  "session.toolNoDetail": "No se registró nada más",
  "session.showMore": (n: number) => `Mostrar ${n} caracteres más`,
  "session.showLess": "Mostrar menos",
  "session.composerHint": "Escribe al agente · Intro envía, Mayús+Intro salta de línea",
  "session.send": "Enviar",

  // ── Motor de conversación (una sesión gobernada por protocolo) ──
  "chat.empty": "Escriba en el campo inferior para iniciar la conversación.",
  "chat.interrupt": "Detener",
  "chat.interruptTooltip": "Detener · Esc",
  "chat.allow": "Permitir",
  "chat.deny": "Denegar",
  "chat.permissionAsk": (tool: string) => `${tool} pide ejecutarse`,
  "chat.exited": (code: number) => `El agente se detuvo (código ${code})`,
  "chat.modeNextTurn": "En el próximo turno",
  "chat.modePendingHint": (current: string, next: string) =>
    `Permisos actuales: ${current}. ${next} se aplicará en el próximo turno; el turno actual continuará sin cambios.`,
  "chat.modeTooltip": "Modo de permisos",
  "chat.collaborationModeTooltip": "Modo de colaboración",
  "chat.collaborationMode.default": "Predeterminado",
  "chat.collaborationMode.defaultHint":
    "Avanza directamente y solo pregunta cuando hace falta una decisión",
  "chat.collaborationMode.plan": "Planificación",
  "chat.collaborationMode.planHint":
    "Analiza la tarea y prepara un plan; las preguntas pueden mostrarse en tarjetas interactivas",
  "chat.modelTooltip": "Modelo",
  "chat.keepChoice": "Predeterminado",
  "chat.keepChoiceFor": (model) => `Predeterminado para ${model}`,
  "chat.modelDefault": "Modelo predeterminado",
  "chat.mode.default": "Preguntar siempre",
  "chat.mode.acceptEdits": "Aceptar ediciones",
  "chat.mode.plan": "Modo plan",
  "chat.mode.bypassPermissions": "Sin preguntar",
  "chat.mode.readOnly": "Solo lectura",
  "chat.mode.fullAccess": "Acceso total",
  "chat.placeholder": "Escribe al agente, o usa /comandos, /habilidades y @archivos",
  "chat.command.clearDescription": "Archivar esta sesión e iniciar una conversación nueva",
  "chat.command.rewindDescription": "Elegir qué revertir desde el último mensaje del usuario",
  "chat.command.rewindUnavailable":
    "Para revertir debe haber un mensaje del usuario completado y no puede haber turnos activos, mensajes en cola ni solicitudes de permiso.",
  "chat.effortTooltip": "Esfuerzo de razonamiento",
  "chat.effortDefault": "Razonamiento",
  "chat.effort.auto": "Automático",
  "chat.effort.low": "Bajo",
  "chat.effort.medium": "Medio",
  "chat.effort.high": "Alto",
  "chat.effort.xhigh": "Muy alto",
  "chat.effort.max": "Máximo",
  "chat.effort.ultra": "Extremo",
  "chat.effort.ultracode": "Ultra Code",
  "chat.agentTooltip": "Agente",
  "chat.effort.minimal": "Mínimo",
  "chat.filterPlaceholder": "Filtrar",
  "chat.placeholderOpencode": "Escribe al agente; puedes usar /comandos y @archivos, o empezar con ! para ejecutar un comando de shell",
  "chat.command.compactDescription": "Resumir la conversación para liberar contexto",
  "chat.command.undoDescription": "Revertir el último mensaje y los cambios de archivos que provocó",
  "chat.command.redoDescription": "Restaurar lo que revirtió el último deshacer",
  "chat.command.shareDescription": "Crear un enlace para compartir esta conversación",
  "chat.command.unshareDescription": "Dejar de compartir esta conversación",
  "chat.mode.auto": "Modo automático",

  // ── Una pregunta del agente, respondida como formulario ──
  "chat.question.heading": "El agente tiene una pregunta",
  "chat.question.submit": "Enviar",
  "chat.question.next": "Siguiente",
  "chat.question.dismiss": "Descartar",
  "chat.question.answerPlaceholder": "Escribe tu respuesta",
  "chat.question.otherPlaceholder": "Otra respuesta",
  "chat.question.answeredHeading": (n: number) =>
    n === 1 ? "1 pregunta respondida" : `${n} preguntas respondidas`,
  "chat.question.blankAnswer": "Sin responder",

  // ── Un plan a la espera de aprobación ──
  "chat.plan.heading": "Plan a la espera de aprobación",
  "chat.plan.implement": "Aprobar y ejecutar",
  "chat.plan.reject": "Rechazar",

  // ── Mensajes escritos mientras el agente trabaja ──
  "chat.placeholderBusy": "Escribe un mensaje; se enviará cuando termine este turno",
  "chat.queueTooltip": (combo: string) => `Se enviará cuando termine este turno · ${combo} lo envía ahora`,
  "chat.queue.pending": "Mensajes pendientes",
  "chat.queue.edit": "Editar",
  "chat.queue.remove": "Eliminar",

  // ── Imágenes pegadas o soltadas en el redactor ──
  "chat.attach.remove": "Quitar esta imagen",
  "chat.attach.tooMany": (max: number) => `Un mensaje puede incluir hasta ${max} imágenes`,
  "chat.attach.tooLarge": (name: string, mb: number) => `${name} supera los ${mb} MB y no se adjuntó`,
  "chat.attach.unreadable": (name: string) => `No se pudo leer ${name}`,
  // Compacting the conversation… / Context compacted / Context compacted automatically
  "chat.compaction.running": "Compactando la conversación…",
  "chat.compaction.manual": "Contexto compactado",
  "chat.compaction.auto": "Contexto compactado automáticamente",
  "chat.compaction.from": (tokens: string) => `desde ${tokens} tokens`,
  // N steps
  "chat.subagent.steps": (n: number) => (n === 1 ? "1 paso" : `${n} pasos`),
  "chat.subagent.tokens": (tokens: string) => `${tokens} tokens`,
  "chat.rewind.title": "Volver a este punto",
  "chat.rewind.warning": "Esta acción no se puede deshacer.",
  "chat.rewind.conversation": "Retroceder la conversación",
  "chat.rewind.files": "Restaurar archivos",
  "chat.rewind.both": "Retroceder la conversación y restaurar archivos",
  "chat.rewind.confirm.conversation": "¿Eliminar este mensaje y todo lo posterior?",
  "chat.rewind.confirm.files": "¿Restaurar los archivos al estado anterior a este mensaje?",
  "chat.rewind.confirm.both": "¿Eliminar este turno y restaurar los archivos que modificó?",
  "chat.rewind.unavailable": "No hay un punto de restauración de archivos para este mensaje.",
  "chat.rewind.previewing": "Comprobando el punto de restauración…",
  "chat.rewind.cancel": "Dejar como está",
  "chat.rewind.apply": "Retroceder",
  "chat.rewind.applying": "Retrocediendo…",
  "chat.rewind.fileSummary": (files: number, insertions: number, deletions: number) =>
    `Se ${files === 1 ? "modificará" : "modificarán"} ${files} ${files === 1 ? "archivo" : "archivos"}: +${insertions} −${deletions}. Esta acción no se puede deshacer.`,
  // ── Reglas permanentes que ofrece una solicitud de permiso, adoptadas con un clic ──
  "chat.suggest.modeSession": (mode: string) => `${mode} en esta sesión`,
  "chat.suggest.mode": (mode: string) => `Cambiar a ${mode}`,
  "chat.suggest.allowSession": (rule: string) => `Permitir ${rule} en esta sesión`,
  "chat.suggest.allowAlways": (rule: string) => `Permitir ${rule} siempre`,
  "chat.suggest.dirSession": (dirs: string) => `Permitir el acceso a ${dirs} en esta sesión`,
  "chat.suggest.dirAlways": (dirs: string) => `Permitir el acceso a ${dirs} siempre`,
  // ── Codex: reglas de red permanentes, intervenir, comandos propios y los chips de velocidad y tono ──
  "chat.suggest.networkAlways": (host: string) => `Permitir siempre el acceso de red a ${host}`,
  "chat.steer": "Añadir indicación",
  "chat.stopping": "Deteniendo el turno actual…",
  "chat.stopped": "Turno actual detenido",
  "chat.steerAccepted": "Indicación enviada",
  "chat.steerTooltip": (combo: string) => `${combo} lo añade al turno en curso`,
  "chat.command.reviewDescription": "Revisar el código e informar de lo que requiere atención",
  "chat.command.reviewHint": "[branch <nombre> | commit <sha> | instrucciones]",
  "chat.command.startTimeout": "El agente no abrió su sesión a tiempo",
  "chat.serviceTierTooltip": "Velocidad",
  "chat.serviceTier.default": "Velocidad estándar",
  "chat.personalityTooltip": "Tono",
  "chat.personality.default": "Tono predeterminado",
  "chat.personality.none": "Neutral",
  "chat.personality.friendly": "Cordial",
  "chat.personality.pragmatic": "Pragmático",
  // ── Conversación larga: las tandas de llamadas a herramientas caben en una línea, con vuelta al final ──
  "chat.toolRun.count": (n: number) => `${n} llamadas a herramientas`,
  "chat.toolRun.tooltip": "Ver cada llamada",
  "chat.backToEnd": "Volver al mensaje más reciente",
  "chat.elicitation.heading": (server: string) => `${server} solicita datos`,
  "chat.elicitation.cancel": "Cancelar",
  "chat.elicitation.decline": "Rechazar",
  "chat.elicitation.submit": "Enviar",
  "chat.elicitation.done": "Listo",
  "chat.elicitation.choose": "Elegir…",
  "chat.effort.off": "Desactivado",
  "chat.effort.offHint": "Sin razonamiento extendido",
  "chat.fastMode.label": "Rápido",
  "chat.fastMode.on": "El modo rápido está activado",
  "chat.fastMode.off": "El modo rápido está desactivado",
  "chat.usage.context": (used: string, max: string, pct: number) =>
    `Contexto: ${used} de ${max} tokens (${pct} %)`,
  "chat.usage.cost": (usd: string) => `Coste de la sesión: $${usd}`,
  "chat.usage.rateLimited": (resets: string) => `Límite de uso alcanzado; se restablece ${resets}`,
  "chat.usage.rateWarning": (pct: number, resets: string) =>
    `Límite de uso: ${pct} % consumido; se restablece ${resets}`,
  "chat.mcp.codexScope": "Esta acción modifica tu configuración de usuario de Codex y afecta a otras conversaciones que la utilicen. ¿Continuar?",
  "chat.mcp.tooltip": "Servidores MCP",
  "chat.mcp.loading": "Leyendo la lista de servidores…",
  "chat.mcp.backendUnsupported": "El servidor VelaTerm conectado no admite la gestión de MCP. Actualícelo y reinícielo; después, vuelva a intentarlo.",
  "chat.mcp.none": "No hay servidores MCP configurados",
  "chat.mcp.tools": (n: number) => (n === 1 ? "1 herramienta" : `${n} herramientas`),
  "chat.mcp.reconnect": "Reconectar",
  "chat.mcp.disable": "Desactivar",
  "chat.mcp.enable": "Activar",
  "chat.mcp.status.connected": "Conectado",
  "chat.mcp.status.disabled": "Desactivado",
  "chat.mcp.status.failed": "Con error",
  "chat.mcp.status.pending": "Conectando",
  "chat.mcp.status.disconnected": "Desconectado",
  "chat.mcp.status.other": "Desconocido",
  "chat.tasks.label": "Tareas",
  "chat.tasks.tooltip": "Tareas en segundo plano",
  "chat.tasks.backgroundAll": "Pasar el trabajo en curso a segundo plano",
  "chat.tasks.none": "No hay tareas en segundo plano",
  "chat.tasks.stop": "Detener",
  "chat.tasks.open": "Open task", // TODO translate
  "chat.tasks.tabTooltip": "Background task", // TODO translate
  "chat.tasks.status.running": "Running", // TODO translate
  "chat.tasks.status.completed": "Completed", // TODO translate
  "chat.tasks.status.failed": "Failed", // TODO translate
  "chat.tasks.status.canceled": "Stopped", // TODO translate
  "chat.tasks.status.ended": "Ended", // TODO translate
  "chat.tasks.stale": "No longer reported by the agent", // TODO translate
  "chat.tasks.elapsed": "Elapsed", // TODO translate
  "chat.tasks.tokens": "Tokens", // TODO translate
  "chat.tasks.toolUses": "Tool calls", // TODO translate
  "chat.tasks.currentAgent": "Current agent", // TODO translate
  "chat.tasks.started": "Started", // TODO translate
  "chat.tasks.finished": "Finished", // TODO translate
  "chat.tasks.summary": "Summary", // TODO translate
  "chat.tasks.outputFile": "Output file", // TODO translate
  "chat.tasks.phases": "Phases", // TODO translate
  "chat.tasks.noProgress": "This task reports no per-agent progress.", // TODO translate
  "chat.tasks.attempt": (n: number) => `attempt ${n}`, // TODO translate
  "chat.tasks.prompt": "Prompt", // TODO translate
  "chat.tasks.result": "Result", // TODO translate
  "chat.tasks.agentState.start": "running", // TODO translate
  "chat.tasks.agentState.done": "done", // TODO translate
  "chat.retry.line": (attempt: number, max: number, seconds: number, message: string) =>
    `Reintentando (${attempt}/${max}) en ${seconds} s: ${message}`,
  "chat.notify.dismiss": "Cerrar",
};

export default es;
