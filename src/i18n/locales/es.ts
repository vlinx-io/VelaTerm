//! Spanish dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const es: typeof en = {
  "tree.newPlanExecuteSession": "Nueva sesión de planificación y ejecución…",
  "launch.splitTasks": "Dividir automáticamente en varias tareas",
  "launch.splitTasksHint": "La sesión de planificación propone tareas independientes. Revise las instrucciones, los agentes, los modelos y el esfuerzo de razonamiento antes de iniciar la ejecución.",
  "launch.splitReview": "Revisar tareas de ejecución",
  "launch.splitReviewHint": "Una sola sesión de planificación recibe todos los informes y revisa cada tarea por separado. La ejecución comienza tras su confirmación.",
  "launch.splitConfirmed": "Estas tareas ya se han confirmado.",
  "launch.splitClosed": "Esta propuesta ya no está pendiente de confirmación.",
  "launch.splitRetry": "Reintentar las entregas pendientes",
  "launch.splitSharedDirectory": "Todas las sesiones de ejecución utilizan el directorio de trabajo de la sesión de planificación y comparten su árbol de trabajo si está habilitado.",
  "launch.createIn": "Crear en",
  "launch.workingDirectory": "Ruta del directorio de trabajo",
  "launch.createAndStart": "Crear e iniciar",
  "launch.planExecuteTaskHint": "Describe la tarea, los requisitos y los criterios de aceptación para la planificación.",
  "launch.planExecuteResult": "Primero se inicia la sesión de planificación; cuando el plan está listo, esta crea la sesión de ejecución.",
  "launch.planExecuteWorktreeHint": "Los nuevos worktrees parten del commit actual, sin cambios pendientes de commit. Si no se puede crear un worktree, la sesión correspondiente no se inicia.",
  "launch.workflowDirectorySharedHint": "La sesión de planificación y todas las sesiones de ejecución comparten un nuevo directorio y una rama.",
  "launch.workflowDirectoryEachHint": "La sesión de planificación y cada sesión de ejecución tienen su propio worktree y su propia rama.",
  "launch.planTitle": "Planificación y revisión",
  "launch.execTitle": "Ejecución",
  "launch.planExecuteIntro": "Una sesión independiente planifica el trabajo, revisa el resultado y solicita correcciones.",
  "chat.origin.plan": "Planificación",
  "chat.origin.exec": "Ejecución",

  // Project code intelligence and knowledge entry associations.
  "knowledge.callers": "Símbolos que llaman",
  "knowledge.callees": "Símbolos llamados",
  "knowledge.explore": "Explorar código",
  "knowledge.exploreHint": "Describa una función o un flujo, o indique un archivo o símbolo…",
  "knowledge.impact": "Análisis de impacto",
  "knowledge.path": "Ruta de llamadas",
  "knowledge.target": "Buscar un símbolo de destino…",
  "knowledge.depth": "Profundidad de recorrido",
  "knowledge.noPath": "No se encontró una ruta de llamadas dirigida en el índice.",
  "knowledge.watching": "Sincronización automática activa",
  "knowledge.onDemand": "Sincronizar antes de consultar",
  "knowledge.overview": "Vista general",
  "knowledge.uncertain": "Relación inferida",
  "knowledge.kind": "Tipo de símbolo",
  "knowledge.language": "Lenguaje",
  "knowledge.results": "Resultados",
  "knowledge.resultLarge": "El resultado es demasiado grande para mostrarlo. Acote la consulta o reduzca la profundidad de recorrido.",
  "knowledge.queryFailed": "La consulta de código falló. Reintente o sincronice el índice.",
  "knowledge.liveHelp": "Los cambios se sincronizan mientras el proceso de consultas está activo. Si termina por inactividad, la siguiente consulta incorpora los cambios pendientes.",
  "knowledge.startHelp": "Active la indexación para buscar código, seguir llamadas y analizar el impacto de un cambio. El análisis se ejecuta en el servidor sin un modelo de IA.",
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
  "knowledge.selectSymbol": "Seleccione un símbolo para ver su código fuente, sus relaciones y los artículos de conocimiento vinculados.",
  "knowledge.source": "Código fuente",
  "knowledge.incoming": "Relaciones entrantes",
  "knowledge.outgoing": "Relaciones salientes",
  "knowledge.noEdges": "No hay relaciones indexadas.",
  "knowledge.analysisNote": "Las relaciones proceden de un análisis estático y pueden ser incompletas o inciertas.",
  "knowledge.changed": "El archivo cambió durante la consulta. Sincronice de nuevo antes de usar los números de línea o confirmar la revisión.",
  "knowledge.truncated": "Esta vista tiene un límite. Se omiten algunas relaciones o líneas de código.",
  "knowledge.linkMemory": "Vincular artículo de conocimiento",
  "knowledge.chooseMemory": "Elegir un artículo de conocimiento",
  "knowledge.noLinks": "Aún no hay vínculos con el código. Puede vincular un artículo desde el detalle de un símbolo.",
  "knowledge.inspect": "Revisar código y artículo",
  "knowledge.unlink": "Eliminar vínculo",
  "knowledge.codeReferences": "Referencias al código",
  "knowledge.refresh": "Actualizar",
  "knowledge.current": "Sin cambios",
  "knowledge.review": "Requiere revisión",
  "knowledge.unavailable": "No disponible",
  "knowledge.reviewHelp": "Compare este artículo con el código mostrado. La confirmación registra la versión actual del archivo sin modificar el texto del artículo.",
  "knowledge.confirmReview": "Confirmar revisión",
  "knowledge.agentHint": "Los agentes pueden ejecutar vkb search \"tema\" en este directorio. Las consultas sincronizan los índices activos y devuelven el código y los artículos de conocimiento por separado.",
  "knowledge.busy": "Hay una tarea de indexación en curso. Puede cerrar esta página o desactivar la indexación para detenerla.",
  "knowledge.disabledHelp": "Active la indexación de este directorio para consultar el código. Al desactivarla se conservan el índice y los vínculos con los artículos de conocimiento.",
  "knowledge.conflict": "El código o el artículo han cambiado. Vuelva a cargar ambos antes de guardar este vínculo.",
  "knowledge.symbolMissing": "El símbolo o el código fuente ya no están disponibles. Sincronice y vuelva a buscar.",
  "knowledge.directoryMissing": "Este directorio de trabajo no existe o ha cambiado. Compruebe las rutas del proyecto y de la sesión.",
  "knowledge.partial": "El índice está incompleto. Sincronice de nuevo y compruebe que se pueden leer los archivos de código fuente.",
  "knowledge.interrupted": "La tarea anterior se interrumpió. Sincronice para volver a intentarlo.",
  "knowledge.checksum": "La suma de comprobación de la descarga no coincide. No se ha instalado el entorno de ejecución.",
  "knowledge.downloadFailed": "No se pudo descargar CodeGraph. Compruebe la conexión del servidor con GitHub y vuelva a intentarlo.",
  "knowledge.timeout": "Se agotó el tiempo de indexación. Compruebe el tamaño del repositorio y vuelva a intentarlo.",
  "knowledge.error": "La operación falló. Compruebe el acceso a los directorios y el entorno de ejecución del servidor y vuelva a intentarlo.",

  // Knowledge base: saved knowledge organized by project and session.
  "memory.hierarchy": "Proyectos y sesiones",
  "memory.up": "Subir un nivel",
  "memory.manualGroup": "Creación manual",
  "memory.legacyGroup": "Entradas fusionadas anteriormente",
  "memory.unknownProject": "Proyecto de origen desconocido",
  "nb.addLink": "Insertar enlace",
  "nb.attach": "Adjuntar archivo",
  "nb.browse": "Examinar",
  "nb.chooseNote": "Empieza con una nota",
  "nb.closeHint": "Quitar este cuaderno de la lista. Sus archivos permanecerán en el disco.",
  "nb.closeVault": "Cerrar cuaderno",
  "nb.conflict": "El archivo se ha modificado fuera de este editor. Tu borrador se conserva. Vuelve a cargar el archivo o guarda el borrador como una nota nueva.",
  "nb.copyTo": "Copiar a un cuaderno local",
  "nb.createVault": "Crear base de conocimientos",
  "nb.destination": "Ruta de destino",
  "nb.download": "Descargar",
  "nb.downloadHint": "Descarga este archivo adjunto para abrirlo en otra aplicación.",
  "nb.empty": "Abre una carpeta para empezar a escribir o crea un cuaderno.",
  "nb.emptyImport": "No se han seleccionado archivos que se puedan importar.",
  "nb.emptyNotes": "Las notas se guardan como archivos Markdown.",
  "nb.emptyOutline": "Los encabezados del documento aparecerán aquí.",
  "nb.emptyTrash": "La papelera está vacía.",
  "nb.error": "No se puede acceder al cuaderno. Comprueba la conexión y la carpeta e inténtalo de nuevo.",
  "nb.exists": "El destino ya existe. Elige otro nombre o carpeta.",
  "nb.favorites": "Favoritos",
  "nb.files": "Archivos",
  "nb.folder": "Carpeta",
  "nb.generatedHint": "Conocimiento organizado a partir de tus sesiones, con sus fuentes e historial de revisiones.",
  "nb.homeHint": "Explora la base de conocimientos de sesiones y las bases de conocimientos locales.",
  "nb.homeSearch": "Busca en el conocimiento de las sesiones y en las notas locales…",
  "nb.loadMore": "Cargar más",
  "nb.import": "Importar",
  "nb.importFiles": "Elegir archivos",
  "nb.importFolder": "Elegir carpeta",
  "nb.importHint": "Los archivos se copian a la carpeta seleccionada. No se sobrescriben archivos existentes y se omiten las carpetas de configuración ocultas.",
  "nb.imported": "Importados",
  "nb.imports": "Historial de importaciones",
  "nb.importsEmpty": "Aún no hay importaciones.",
  "nb.importRoot": "Raíz de la base de conocimientos",
  "nb.importBusy": "Ya hay una importación en curso para esta base de conocimientos.",
  "nb.importDelete": "Eliminar registro",
  "nb.importDeleteConfirm": "¿Eliminar este registro de importación? Los archivos ya importados no se eliminan.",
  "nb.importDone": "Importación finalizada",
  "nb.importDuration": (seconds: string) => `${seconds} s`,
  "nb.importFailed": "Error en la importación",
  "nb.importFilePending": "Sin importar",
  "nb.importHideFiles": "Ocultar archivos",
  "nb.importInterruptedHint": "La importación se detuvo antes de terminar.",
  "nb.importProgress": (done: string, total: string) => `${done} / ${total} archivos`,
  "nb.importShowFiles": (count: string) => `Archivos (${count})`,
  "nb.importSkipHidden": "Archivo o carpeta ocultos",
  "nb.importStatusCancelled": "Cancelada",
  "nb.importStatusCompleted": "Completada",
  "nb.importStatusFailed": "Fallida",
  "nb.importStatusInterrupted": "Interrumpida",
  "nb.importStatusRunning": "Importando",
  "nb.incomplete": "No se ha podido completar la operación. Comprueba los archivos e inténtalo de nuevo.",
  "nb.info": "Detalles de la nota",
  "nb.invalid": "El nombre o la ruta no son válidos.",
  "nb.links": "Enlaces salientes",
  "nb.local": "Archivos locales",
  "nb.localVaults": "Bases de conocimientos locales",
  "nb.move": "Renombrar o mover",
  "nb.moveHint": "Introduce una ruta relativa a la carpeta raíz del cuaderno. Los enlaces existentes se actualizan al mover el archivo o la carpeta.",
  "nb.name": "Nombre",
  "nb.newFolder": "Nueva carpeta",
  "nb.newNote": "Nueva nota",
  "nb.noLinks": "Todavía no hay notas enlazadas.",
  "nb.tags": "Etiquetas",
  "nb.notes": "Notas",
  "nb.openVault": "Abrir base de conocimientos",
  "nb.outline": "Esquema",
  "nb.quickOpen": "Apertura rápida",
  "nb.readOnly": "Este archivo no se puede editar como una nota Markdown en UTF-8.",
  "nb.recent": "Notas recientes",
  "nb.restore": "Restaurar",
  "nb.reload": "Volver a cargar del disco",
  "nb.root": "Ruta de la carpeta",
  "nb.rootHint": "Selecciona una carpeta del equipo conectado. Los archivos Markdown y los adjuntos existentes permanecerán en su ubicación actual.",
  "nb.saveCopy": "Guardar como nota nueva",
  "nb.saved": "Guardado en el disco",
  "nb.saving": "Guardando…",
  "nb.search": "Buscar notas…",
  "nb.searchAllVaults": "Todas las bases de conocimientos",
  "nb.searchCount": (count: string) => `${count} resultados`,
  "nb.searchEmpty": "Ninguna nota coincide con esta búsqueda.",
  "nb.searchEmptyAll": "Nada coincide con esta búsqueda.",
  "nb.searchFuzzy": "No hay coincidencias exactas. Se muestran resultados aproximados.",
  "nb.searchLine": (line: string) => `Línea ${line}`,
  "nb.searchMatches": (count: string) => `${count} coincidencias`,
  "nb.searchMore": "Solo se muestran los primeros resultados. Ajusta la búsqueda para ver el resto.",
  "nb.searchRelated": "Notas relacionadas",
  "nb.searchResults": "Resultados de búsqueda",
  "nb.searchScope": "Alcance de la búsqueda",
  "nb.searchThisVault": "Esta base de conocimientos",
  "nb.skipped": "Omitidos",
  "nb.split": "Vista dividida",
  "nb.tooLarge": "El archivo o la selección supera los límites del cuaderno.",
  "nb.trash": "Papelera",
  "nb.trashHint": "Mover este elemento a la papelera del cuaderno. Podrás restaurarlo más adelante.",
  "nb.unsaved": "Cambios sin guardar",
  "nb.vaults": "Bases de conocimientos",
  "nb.view": "Modo de vista",
  "nb.welcome": "Tus cuadernos",
  "nb.welcomeText": "Escribe con libertad, conecta ideas y conserva tus notas en archivos locales normales. Abre una carpeta Markdown existente o importa documentos a un nuevo cuaderno.",
  "memory.globalMemory": "Base de conocimientos de sesiones",
  "memory.collections": "Sesiones archivadas",
  "memory.collectionConversation": "Conversación",
  "memory.collectionEmptyEntries": "Esta conversación aún no tiene artículos de conocimiento.",
  "memory.title": "Base de conocimientos",
  "memory.add": "Organizar en la base de conocimientos de sesiones",
  "memory.intro": "Organiza el conocimiento por proyecto y sesión. Los artículos guardados son independientes de sus fuentes y se pueden editar manualmente.",
  "memory.entries": "Artículos de conocimiento",
  "memory.emptyJobs": "Aún no hay registros de organización.",
  "memory.jobs": "Historial de organización",
  "memory.search": "Buscar en títulos y contenido…",
  "memory.empty": "No hay artículos que coincidan. Genera artículos a partir de una sesión o crea uno manualmente.",
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
  "memory.groupDeleteConfirm": (count: string) => `¿Eliminar los ${count} artículos de conocimiento de este grupo? El proyecto o la sesión se conservan.`,
  "memory.export": "Exportar Markdown",
  "memory.selectAgent": "Agente",
  "memory.model": "Modelo (opcional)",
  "memory.modelHint": "Déjalo vacío para usar el modelo configurado en el agente.",
  "memory.compile": "Organizar y guardar",
  "memory.compileHelp": "El agente seleccionado procesará esta sesión. Si vuelves a generar los artículos, se reemplazarán los generados anteriormente para esta sesión, incluidas las modificaciones manuales. El texto de la sesión se enviará al modelo a través del agente configurado.",
  "memory.unavailable": "Sin instalar o configurar",
  "memory.allTags": "Todas las etiquetas",
  "memory.updated": "Actualización reciente",
  "memory.titleSort": "Título",
  "memory.sourceNote": "Esta instantánea conserva el texto utilizado para generar los artículos, incluso si se elimina la sesión original.",
  "memory.noKnowledge": "No se extrajo conocimiento reutilizable. Se han eliminado las entradas generadas anteriormente para esta sesión.",
  "memory.queued": "Pendiente de inicio",
  "memory.cancelling": "Cancelando",
  "memory.schedulingHint": "Se pueden procesar distintas sesiones en paralelo. Al volver a enviar la solicitud, se cancela cualquier tarea pendiente de esta sesión y se sustituye por la nueva.",
  "memory.waitingHint": "Esta tarea comenzará automáticamente cuando se haya detenido la anterior de esta sesión.",
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
  "memory.notFound": "Este artículo, fuente o proceso ya no existe.",
  "memory.noTranscript": "Esta sesión no tiene una conversación que se pueda leer.",
  "memory.agentUnavailable": "El agente seleccionado no está disponible. Comprueba la ruta de su ejecutable en los ajustes.",
  "memory.invalid": "Algunos campos o enlaces no son válidos. Revisa el título, el contenido y los artículos relacionados.",
  "memory.processFailed": "El agente no pudo terminar. Revisa su inicio de sesión, modelo y configuración CLI, y vuelve a intentarlo.",
  "memory.timeout": "El agente agotó el tiempo de espera. Prueba con un modelo disponible o una conversación más corta.",
  "memory.interrupted": "El proceso se interrumpió. Puedes reintentarlo con la instantánea de origen guardada.",
  "memory.tooLarge": "La fuente, el contexto o la salida supera el tamaño admitido. No se ha recortado ni guardado contenido.",
  "memory.invalidOutput": "El agente devolvió datos estructurados no válidos. No se guardó nada; reinténtalo o elige otro agente.",
  "memory.loadError": "No se pudo cargar la base de conocimientos. Comprueba la conexión y vuelve a intentarlo.",
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
  "common.copyFailed": "No se pudo copiar. Inténtelo de nuevo.",
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
  "titlebar.gameCenter": "Centro de juegos",
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
  "titlebar.feedback": "Comentarios", // Feedback
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
  "settings.agentDefaultsTitle": "Valores predeterminados de nuevas sesiones",
  "settings.referSummaryTitle": "Contexto de referencias de sesión",
  "settings.referSummaryMode": "Modo de contexto",
  "settings.referSummaryFull": "Transcripción completa",
  "settings.referSummaryFirst": "Resumir primero",
  "settings.referSummaryAgent": "Agente de resumen",
  "settings.referSummaryHint":
    "De forma predeterminada, vrefer --ask envía la transcripción completa al agente que responde. «Resumir primero» la comprime con el único agente, modelo y nivel de razonamiento elegidos aquí; la respuesta final también recibe fragmentos relevantes del texto original.",
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
  "settings.agentDefaultView": "Vista predeterminada", // Default view
  "settings.agentDefaultViewHint":
    "Vista en la que se abren las sesiones nuevas de este agente. Las sesiones existentes conservan la vista con la que se crearon.", // Agent default view hint
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
  "settings.dynamicStatusFilter": "Incorporación dinámica al filtro de estado",
  "settings.tabSingle": "Única", // Single
  "settings.tabMulti": "Múltiples", // Multi
  "settings.maxLiveTabs": "Background limit", // Background limit
  "settings.defaultShell": "Shell predeterminada", // Default shell
  "settings.spawnConfirm": "Confirm before spawn", // Confirm before spawn
  "settings.usageAuto": "Usage auto-refresh", // Usage auto-refresh
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.autoContinue": "Continuar tras el restablecimiento", // Continue after limit resets
  "settings.autoContinueHint": "Cuando un límite de uso de 5 horas o semanal detiene a Claude o Codex, la tarea continúa automáticamente después de que se restablezca el límite.", // When a 5-hour or weekly usage limit stops Claude or Codex, the task continues automatically after the limit resets.
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
  "spawn.title": "Iniciar sesión secundaria",
  "spawn.fromSession": "Sesión solicitante",
  "spawn.promptLabel": "Instrucciones de la tarea",
  "spawn.agentLabel": "Tipo de sesión",
  "spawn.worktreeLabel": "Worktree independiente",
  "spawn.modelLabel": "Modelo",
  "spawn.effortLabel": "Esfuerzo de razonamiento",
  "spawn.modelDefault": "Valor predeterminado del agente",
  "spawn.modelLoading": "Cargando modelos…",
  "spawn.modelListUnavailable": "Lista no disponible. Puede introducir un identificador.",
  "spawn.launch": "Iniciar sesión",
  "spawn.remaining": (n: number) => `${n} solicitudes más por revisar`,
  "spawn.notifyTitle": "Sesión secundaria pendiente de confirmación",
  "orch.title": "Iniciar varias sesiones",
  "orch.notifyTitle": "Inicio de sesiones pendiente de confirmación",
  "orch.coordinatorName": "Estado de las sesiones",
  "orch.sharedSettings": "Configuración común",
  "orch.agentLabel": "Agente",
  "orch.modelLabel": "Modelo",
  "orch.effortLabel": "Esfuerzo de razonamiento",
  "orch.nameLabel": "Nombre de la sesión",
  "orch.promptLabel": "Instrucciones de la tarea",
  "orch.worktreeLabel": "Worktree de Git",
  "orch.worktreeNone": "Directorio actual",
  "orch.worktreeShared": "Worktree compartido",
  "orch.worktreeEach": "Un worktree por sesión",
  "orch.follow": "Usar configuración común",
  "orch.overridden": "Configuración individual",
  "orch.remove": "Quitar tarea",
  "orch.launch": (n: number) => `Iniciar ${n} sesiones`,
  "orch.modelPlaceholder": "Valor predeterminado del agente",
  "orch.effortPlaceholder": "Valor predeterminado del agente",
  "launch.terminalHint": "Un terminal normal abre el directorio de trabajo. No ejecuta automáticamente las instrucciones de la tarea.",
  "launch.optionsError": "No se pudieron cargar las opciones. Vuelva a intentarlo antes de iniciar.",
  "launch.singleIntro": "Revise la tarea y la configuración antes de iniciar una sesión secundaria.",
  "launch.taskHint": "Estas instrucciones serán el primer mensaje de la sesión secundaria.",
  "launch.runtime": "Configuración de ejecución",
  "launch.directory": "Directorio de trabajo",
  "launch.directoryCurrentHint": "Las sesiones modifican archivos en el directorio original.",
  "launch.directorySharedHint": "Todas las sesiones usan un mismo directorio nuevo y una misma rama.",
  "launch.directoryEachHint": "Cada sesión dispone de su propio directorio y su propia rama.",
  "launch.worktreeHint": "Los worktrees parten del commit actual, sin cambios pendientes de commit. Si no se pueden crear, se usa el directorio original.",
  "launch.singleResult": "La sesión secundaria aparecerá bajo su sesión principal en la barra lateral.",
  "launch.startError": "No se pudo iniciar. Revise la configuración y vuelva a intentarlo.",
  "launch.starting": "Iniciando…",
  "launch.batchIntro": "Revise la configuración común y seleccione cada tarea para editar sus instrucciones.",
  "launch.sessionCount": (n: number) => `Sesiones: ${n}`,
  "launch.batchName": "Nombre del grupo de tareas",
  "launch.sharedHint": "Se aplica a las sesiones sin configuración individual.",
  "launch.tasks": "Tareas",
  "launch.incomplete": "Faltan datos",
  "launch.undoRemove": "Deshacer eliminación",
  "launch.taskNumber": (n: number) => `Tarea ${n}`,
  "launch.taskSettings": "Configuración de esta sesión",
  "launch.taskAgent": "Agente de esta sesión",
  "launch.sharedDirectoryLocked": "Todas las sesiones de este grupo usan un mismo worktree.",
  "launch.resetSettings": "Restaurar configuración común",
  "launch.monitorHint": "El terminal «Estado de las sesiones» mostrará qué sesiones están trabajando o esperando una entrada. No indica el porcentaje de finalización de las tareas.",
  "launch.taskIncomplete": (n: number) => `Complete el nombre y las instrucciones de la tarea ${n}.`,
  "launch.batchResult": "Cada tarea se inicia en una sesión interactiva independiente.",
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
  "settings.fontListUnavailable": "No se puede obtener la lista de fuentes del sistema. Puede introducir un nombre de fuente manualmente.",
  "settings.fontUnconfirmed": "No se puede confirmar si esta fuente está disponible.",
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
  "tree.collectionInfo": "Información de la colección", // Collection Info
  "tree.projectInfo": "Información del proyecto", // Project Info
  "info.branch": "Rama", // Branch
  "info.path": "Ruta", // Path
  "info.recentCommits": "Commits recientes", // Recent Commits
  "info.noCommits": "Sin commits", // No commits
  "tree.killProcess": "Terminar proceso", // Kill Process
  "tree.killProcessConfirm": (name: string) => `¿Terminar el proceso de «${name}»? Se interrumpirá la tarea actual. Se conservarán el historial de conversación y los archivos guardados.`,
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
  "info.projectId": "ID del proyecto", // Project ID
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
  "agentInstall.pathLabel": "Ruta del ejecutable", // Executable path
  "agentInstall.pathPlaceholder": (bin: string) => `~/.local/bin/${bin}`,
  "agentInstall.pathHint": "¿Ya está instalado fuera del PATH? Introduce la ruta completa del ejecutable.", // Already installed outside PATH?
  "agentInstall.pathSave": "Usar esta ruta", // Use this path
  "agentInstall.pathBrowse": "Examinar…", // Browse…
  "search.placeholder": "Buscar en el terminal", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "WYSIWYG", // WYSIWYG
  "doc.visual": "Visual",
  "doc.source": "Código fuente",
  "doc.compare": "Comparación",
  "doc.editorLoadFailed": "No se pudo cargar el editor Markdown.",
  "doc.imageOnly": "Aquí solo se pueden insertar archivos de imagen.",
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
  "panel.collapseSection": "Contraer sección", // Collapse section
  "panel.expandSection": "Expandir sección", // Collapse section
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
  "login.showPassword": "Mostrar",
  "login.hidePassword": "Ocultar",
  "login.passwordSaveFailed": "Conexión establecida, pero no se pudo guardar la contraseña en este dispositivo. Inténtalo de nuevo.",
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
  "mobile.backConnections": "Volver a las conexiones",
  "mobile.loadSlow": "La carga está tardando más de lo previsto. Puedes volver a intentarlo o regresar a tus conexiones.",
  "mobile.connectionUnavailable": "Conexión no disponible",
  "mobile.pushTitle": "Notificaciones de tareas",
  "mobile.pushHint": "Las notificaciones muestran el nombre de la sesión y una breve vista previa de la respuesta, incluso en segundo plano o con la pantalla bloqueada. Este texto se envía a velaterm.com y al servicio de notificaciones push. No se envían contraseñas de conexión ni claves privadas SSH.",
  "mobile.pushEnable": "Activar notificaciones",
  "mobile.pushDisable": "Desactivar notificaciones",
  "mobile.pushTest": "Enviar notificación de prueba",
  "mobile.pushTestSent": "La notificación de prueba está en cola. Revisa el centro de notificaciones del sistema.",
  "mobile.pushDisabled": "Las notificaciones en segundo plano están desactivadas.",
  "mobile.pushEnabled": "Las notificaciones en segundo plano están activadas.",
  "mobile.pushNotConfigured": "Esta compilación no tiene configurado un servicio de notificaciones push.",
  "mobile.pushDenied": "Permite las notificaciones en los ajustes del sistema.",
  "mobile.pushRegistrationFailed": "No se pudo registrar el dispositivo. Inténtalo de nuevo.",
  "mobile.pushRelayUnavailable": "El servicio de notificaciones no está disponible. Inténtalo de nuevo.",
  "mobile.pushHostUnavailable": "El servidor remoto aún no tiene activadas las notificaciones en segundo plano. Actualízalo y vuelve a conectarte.",
  "mobile.pushDisclosure": "Las notificaciones en segundo plano utilizan Getui y el servicio push del fabricante del dispositivo. Para entregarlas, estos servicios procesan identificadores del dispositivo, información de red, nombres de sesiones y breves vistas previas de las respuestas. No se envían contraseñas de conexión ni claves privadas SSH.",
  "mobile.pushConnectHint": "Tras activar las notificaciones, abre una vez cada conexión para suscribirte.",
  "mobile.pushTarget": "Conexión de prueba",
  "mobile.copyConnection": "Copiar y editar",
  "mobile.copyConnectionHint": "Modifica la configuración a partir de esta conexión. Las credenciales guardadas se transfieren de forma segura. La conexión original no cambia; si la configuración es idéntica, se conserva la conexión existente.",
  "mobile.copyConnectionReused": "Esta configuración ya está guardada. Se ha conservado la conexión existente.",
  "mobile.inputOptions": "Opciones del mensaje",
  "mobile.connections": "Gestionar conexiones",
  "mobile.more": "Más acciones",
  "mobile.toDesktop": "Cambiar a versión de escritorio", // Switch to desktop
  "mobile.empty1": "No hay sesiones.", // No sessions.
  "mobile.noMatch": "No hay sesiones coincidentes", // No matching sessions
  "mobile.empty2":
    "Crea una en la aplicación de escritorio o en el navegador de un ordenador y aparecerá aquí automáticamente.", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ Atrás", // ‹ Back
  "mobile.selCopy": "Copiar", // Copy
  "mobile.selCancel": "Cancelar", // Cancel

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
  "info.collection": "Colección", // Collection
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
  "session.terminalViewHint": "Haga clic aquí para volver a la vista de terminal.",
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
  "chat.moreOptions": "Más",
  "chat.modelTooltip": "Modelo",
  "chat.keepChoice": "Predeterminado",
  "chat.keepChoiceFor": (model) => `Predeterminado para ${model}`,
  "chat.followModelDefault": (agent: string) => `Usar el modelo predeterminado de ${agent}`,
  "chat.followModelDefaultHint": "Utiliza el modelo definido en la configuración del agente.",
  "chat.savedModelDefault": "Predeterminado de la app",
  "chat.catalogWebsite": "Catálogo de modelos del sitio web",
  "chat.catalogCache": "Catálogo de modelos en caché",
  "chat.catalogBundled": "Catálogo de modelos incluido",
  "chat.catalogChecked": (time: string) => `Última comprobación: ${time}`,
  "chat.catalogFailed": "No se pudo actualizar. El catálogo anterior sigue disponible.",
  "chat.catalogRefresh": "Actualizar",
  "chat.modelDefault": "Modelo predeterminado",
  "chat.mode.default": "Preguntar siempre",
  "chat.mode.agentDefault": "Predeterminado del agente",
  "chat.mode.acceptEdits": "Aceptar ediciones",
  "chat.mode.plan": "Modo plan",
  "chat.permissionRestart.unconfirmed": "Se perdió la conexión. No se pudo confirmar el cambio de permisos. Vuelva a conectarse para comprobar los permisos actuales de la sesión.",
  "permission.stateUnavailable": "Estado de permisos no disponible",
  "permission.currentUnknown": "Permisos actuales sin confirmar",
  "permission.notRunning": "Sin ejecutar",
  "permission.applied": "Aplicado",
  "permission.nextTurn": "Se aplica al siguiente mensaje",
  "permission.restart": "Se aplica al reiniciar esta sesión",
  "permission.nextStart": "En el próximo inicio",
  "permission.defaultHint": "Permiso predeterminado para las sesiones nuevas. Las sesiones existentes conservan su propia configuración de permisos.",
  "chat.permissionRestart.title": "¿Reiniciar para omitir las confirmaciones?",
  "chat.permissionRestart.body": "Claude debe reiniciarse para omitir las confirmaciones. Se interrumpirá la respuesta actual y se conservará el historial de la conversación. Cuando el cambio se complete, se omitirán las confirmaciones de permisos.",
  "chat.permissionRestart.confirm": "Reiniciar y aplicar",
  "chat.permissionRestart.busy": "Reiniciando…",
  "chat.permissionRestart.failed": (detail: string) => "No se pudo cambiar el permiso. Se conserva el modo anterior. " + detail,
  "chat.permissionRestart.tasks": "Procese o elimine los mensajes en cola y detenga las tareas en segundo plano antes de reiniciar.",
  "chat.permissionRestart.stale": "El proceso de la sesión ha cambiado. Vuelva a seleccionar «Sin preguntar».",
  "chat.permissionRestart.noHistory": "Todavía no se puede reanudar esta conversación. Espere a que termine la inicialización e inténtelo de nuevo.",
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
  "chat.queue.view": "Ver mensaje completo",
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
  "chat.rewind.edit": "Editar",
  "chat.rewind.editSend": "Revisar y reenviar",
  "chat.rewind.editConfirm": "Eliminar y reenviar",
  "chat.rewind.editWarning": "El mensaje original y todos los mensajes posteriores se eliminarán permanentemente. El mensaje editado se enviará desde este punto. Los cambios en los archivos no se desharán.",
  "chat.rewind.inactive": "El proceso de conversación no está en ejecución. Estas acciones estarán disponibles cuando se inicie.",
  "chat.rewind.unsupported": "El agente conectado no ofrece esta acción actualmente.",
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
  "chat.turnFold.hide": "Ocultar pasos",
  "chat.turnFold.show": (n: number) => (n === 1 ? "Mostrar 1 paso" : `Mostrar ${n} pasos`),
  "chat.turnFold.hideAll": "Ocultar todos los pasos",
  "chat.turnFold.showAll": "Mostrar todos los pasos",
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
  "chat.auth.login": "Iniciar sesión",
  "chat.auth.logout": "Cerrar sesión",
  "chat.auth.confirmLogout": "Confirmar cierre de sesión",
  "chat.auth.logoutConfirm": (provider: string) => `¿Cerrar la sesión de ${provider} en este equipo? Se eliminarán las credenciales compartidas, lo que afectará a las demás sesiones que las utilicen. Se conservará el historial de conversaciones.`,
  "chat.auth.signingOut": "Cerrando sesión…",
  "chat.auth.signedOut": (provider: string) => `Se ha cerrado la sesión de ${provider}. Inicia sesión para continuar esta conversación.`,
  "chat.auth.logoutFailed": "No se pudo confirmar el cierre de sesión. Inténtalo de nuevo.",
  "chat.auth.wait": "Espera a que termine la tarea actual antes de cambiar de cuenta.",
  "chat.auth.title": (provider: string) => `Cuenta de ${provider}`,
  "chat.auth.start": "Volver a iniciar sesión",
  "chat.auth.required": (provider: string) => `Tu sesión de ${provider} ya no es válida. Vuelve a iniciar sesión para continuar.`,
  "chat.auth.starting": "Preparando el inicio de sesión…",
  "chat.auth.pending": "Abre la página de autorización e introduce este código. Esta vista se actualizará cuando finalice el inicio de sesión.",
  "chat.auth.success": "Sesión iniciada. Puedes enviar un mensaje para continuar esta conversación.",
  "chat.auth.failed": "No se pudo completar el inicio de sesión. Inténtalo de nuevo. Comprueba que la autenticación mediante código de dispositivo esté habilitada en ChatGPT y que tu versión de Codex CLI la admita.",
  "chat.auth.canceled": "Inicio de sesión cancelado. Puedes volver a intentarlo en cualquier momento.",
  "chat.auth.scope": (provider: string) => `Al iniciar sesión se actualiza la cuenta de ${provider} utilizada en este equipo. Las demás sesiones que compartan estas credenciales también usarán esa cuenta.`,
  "chat.auth.canceling": "Cancelando el inicio de sesión…",
  "chat.auth.submitting": "Verificando el código de autorización…",
  "chat.auth.claude.pending": "Abre la página de autorización, inicia sesión y pega el código completo que aparece.",
  "chat.auth.claude.failed": "No se pudo completar el inicio de sesión. Inténtalo de nuevo y comprueba que tu versión de Claude CLI admita la autorización de cuentas.",
  "chat.auth.claude.code": "Código de autorización",
  "chat.auth.claude.submit": "Enviar código",
  "chat.auth.claude.invalidCode": "Pega el código completo de este intento de autorización, incluida la parte que aparece después de #.",
  "chat.auth.claude.externalAuth": "Las claves de API y los demás métodos de autenticación configurados no se modificarán.",
  "chat.auth.open": "Abrir página de autorización",
  "chat.resetCredits.label": (n: string) => `Créditos de restablecimiento: ${n}`,
  "chat.resetCredits.title": "Créditos para restablecer los límites de Codex",
  "chat.resetCredits.unknown": "No se puede obtener el número de créditos de restablecimiento.",
  "chat.resetCredits.confirm": "Usar un crédito para restablecer los límites de uso de Codex que cumplan los requisitos. Esta acción no se puede deshacer.",
  "chat.resetCredits.reset": "Límites de uso restablecidos.",
  "chat.resetCredits.alreadyRedeemed": "Esta solicitud ya se completó correctamente.",
  "chat.resetCredits.nothingToReset": "Ningún límite de uso cumple los requisitos para restablecerse.",
  "chat.resetCredits.noCredit": "No hay créditos de restablecimiento disponibles.",
  "chat.resetCredits.error": "La solicitud falló o el saldo actual no está disponible. Actualiza el saldo o reintenta el restablecimiento pendiente.",
  "chat.resetCredits.busy": "Procesando…",
  "chat.resetCredits.retry": "Reintentar restablecimiento",
  "chat.resetCredits.use": "Usar un crédito",
  "chat.resetCredits.refresh": "Actualizar",
  "chat.usage.context": (used: string, max: string, pct: number) =>
    `Contexto: ${used} de ${max} tokens (${pct} %)`,
  "chat.usage.cost": (usd: string) => `Coste de la sesión: $${usd}`,
  "chat.usage.rateLimited": (resets: string) => `Límite de uso alcanzado; se restablece ${resets}`,
  "chat.usage.rateWarning": (pct: number, resets: string) =>
    `Límite de uso: ${pct} % consumido; se restablece ${resets}`,
  "chat.autoContinue.fiveHour": (time: string) => `Límite de uso de 5 horas alcanzado. Reanudación automática de la tarea: ${time}.`, // 5-hour usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.weekly": (time: string) => `Límite de uso semanal alcanzado. Reanudación automática de la tarea: ${time}.`, // Weekly usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.generic": (time: string) => `Límite de uso alcanzado. Reanudación automática de la tarea: ${time}.`, // Usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.unknownReset": "Límite de uso alcanzado. Se desconoce la hora de restablecimiento, por lo que la tarea no continuará automáticamente.", // Usage limit reached. The reset time is unknown, so the task will not continue automatically.
  "chat.autoContinue.repeated": "Se volvió a alcanzar el límite de uso. La tarea ya no continuará automáticamente.", // The usage limit was reached again. The task will no longer continue automatically.
  "chat.autoContinue.failed": "La tarea no pudo continuar automáticamente. Envía un mensaje para continuar.", // The task could not continue automatically. Send a message to continue.
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
  "chat.retry.line": (attempt: number, max: number, seconds: number, message: string) =>
    `Reintentando (${attempt}/${max}) en ${seconds} s: ${message}`,
  "chat.notify.dismiss": "Cerrar",
  "settings.completionMode": "Sugerencias de comandos",
  "settings.completionAuto": "Automáticas",
  "settings.completionTab": "Con Tab",
  "settings.completionOff": "Desactivadas",
  "settings.completionUnavailable": "No se pudo cargar o guardar la configuración.",
  "settings.completionHint": "Se aplica a terminales nuevas de Zsh, Bash 4+, Fish y PowerShell. CMD conserva el comportamiento habitual de Tab. Tab inserta la sugerencia seleccionada; Enter ejecuta el comando actual sin aplicar ninguna sugerencia.",


};

export default es;
