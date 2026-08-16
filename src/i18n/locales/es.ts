//! Spanish dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const es: typeof en = {
  // ── Common ──
  "common.cancel": "Cancelar", // Cancel
  "common.confirm": "Aceptar", // OK
  "common.delete": "Eliminar", // Delete
  "common.save": "Guardar", // Save
  "common.create": "Crear", // Create
  "common.close": "Cerrar", // Close
  "common.copy": "Copiar", // Copy
  "common.cut": "Cortar", // Cut
  "common.paste": "Pegar", // Paste
  "common.selectAll": "Seleccionar todo", // Select All
  "common.copied": "Copiado", // Copied
  "common.retry": "Reintentar", // Retry
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
  "titlebar.themeSystem": (resolved) => `Seguir al sistema (actualmente ${resolved})`, // Follow system (currently {resolved})
  "titlebar.themeDark": "Oscuro", // Dark
  "titlebar.themeLight": "Claro", // Light
  "titlebar.browser": "Navegador integrado (⌘⇧B)", // Built-in Browser (⌘⇧B)
  "titlebar.remoteAccess": "Acceso remoto (navegador)", // Remote Access (Browser)
  "titlebar.connectRemote": "Conectar a servidor remoto", // Connect to Remote Server
  "titlebar.share": "Compartir", // Share
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
  "settings.cliHint": "Añade `vela <ruta-del-proyecto>` al PATH, como el comando `code` de VS Code.",
  "settings.agentArgsHint":
    "Argumentos de inicio predeterminados aplicados a las nuevas sesiones de cada tipo de agente. Los argumentos por sesión definidos al crear o editar tienen prioridad. Dejar vacío para ninguno.", // Agent default launch args hint
  "settings.agentPathLabel": "Ruta del ejecutable (opcional)", // Executable path (optional)
  "settings.agentPathPlaceholder": "p. ej. ~/.local/bin/claude — vacío = buscar en PATH", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "Si se define, las sesiones de este tipo se inician con esta ruta completa en lugar de buscar el comando en el PATH. Útil cuando el agente está instalado pero no en el PATH del shell. Se rellena automáticamente tras una instalación con un clic si se detecta la ubicación.", // Agent executable path hint
  "settings.catOrchestration": "Orquestación",
  "settings.orchProfilesTitle": "Perfiles de worker",
  "settings.orchProfile": "Perfil",
  "settings.orchDescription": "Descripción",
  "settings.orchDescriptionPlaceholder": "Describe cuándo se debe usar este perfil.",
  "settings.orchNewProfile": "Nombre del nuevo perfil",
  "settings.orchAddNew": "Añadir nuevo",
  "settings.orchDelete": "Eliminar",
  "settings.orchNoProfiles": "Aún no hay perfiles. Cree uno para reutilizarlo en cada spawn.",
  "settings.orchProfilesHint":
    "Los perfiles indican a un agente líder qué configuración de worker corresponde a cada tarea. Describe cuándo usar cada perfil y elige su agente, modelo, esfuerzo y worktree.",
  "settings.orchModel": "Modelo",
  "settings.orchEffort": "Esfuerzo",
  "settings.orchWorktree": "Worktree propio",
  "settings.orchPermissionMode": "Modo de permisos",
  "settings.orchPermissionDefault": "Predeterminado",
  "settings.orchPermissionSkip": "Omitir confirmaciones",
  "settings.orchPermissionInherit": "Heredar del padre",
  "settings.orchPermissionSkipWarning":
    "Este trabajador se ejecuta sin confirmación dentro de su worktree.",
  "settings.orchPermissionSkipNoWorktreeWarning":
    "Este worker se ejecuta sin confirmación directamente en el directorio padre. Activa un worktree propio para contenerlo.",
  "settings.orchAgentUnsetHint":
    "Aún no hay agente guardado; los workers usan el agente de la sesión padre. Elige uno para hacerlo explícito.",
  "settings.orchLimitsTitle": "Límites",
  "settings.orchMaxDescendants": "Máx. de descendientes",
  "settings.orchMaxParallel": "Máx. en paralelo",
  "settings.orchMaxDepth": "Profundidad máx.",
  "settings.orchConfirmAbove": "Confirmar por encima de",
  "settings.orchTimeout": "Tiempo de espera predeterminado (segundos)",
  "settings.orchAutoApprove": "Auto-approve /orch spawns", // Auto-approve /orch spawns
  "settings.orchAutoApproveHint": "Launch /orch child sessions without the confirmation card. The confirmation threshold still requires review.", // Launch /orch child sessions without the confirmation card. The confirmation threshold still requires review.
  "settings.orchAutoApproveRetire": "Aprobar el retiro automáticamente",
  "settings.orchAutoApproveRetireHint":
    "Archiva un worker estable sin la tarjeta de confirmación, incluida una limpieza de worktree verificada. Un retiro que reanuda una limpieza no verificada siempre pregunta.",
  "settings.orchConfirmAboveHint":
    "La tarjeta de confirmación aparece cuando un spawn elevaría el número de sesiones descendientes activas por encima de este valor, aunque la confirmación de spawn esté desactivada. Las sesiones descendientes activas se están iniciando, están trabajando o esperan una solicitud de permiso.",
  "settings.orchLimitsHint":
    "Máx. de descendientes cuenta cada sesión descendiente conservada, incluidas las sesiones terminadas, que mantienen su plaza hasta que las archives o elimines. Máx. en paralelo cuenta las sesiones descendientes que se están iniciando, están trabajando o esperan una solicitud de permiso.",
  "settings.orchCopyPatterns": "Patrones de copia al worktree",
  "settings.orchCopyPatternsHint":
    "Un glob por línea. Los archivos sin seguimiento o ignorados que coincidan se copian desde la raíz del repositorio a cada nuevo worktree. Las salidas de compilación como node_modules nunca se copian, por lo que los workers siguen compilando desde cero.",
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
  "spawn.modelLabel": "Modelo", // Model
  "spawn.effortLabel": "Esfuerzo", // Effort
  "spawn.optionDefault": "predeterminado", // default
  "spawn.optionOther": "Otro...", // Other...
  "spawn.worktreeLabel": "Separate git worktree", // Separate git worktree
  "spawn.launch": "Launch", // Launch
  "spawn.launchWarningsTitle": "Revisar valores de lanzamiento",
  "spawn.launchWarning": (field: string, value: string, kind: string) =>
    `El valor de ${field} "${value}" está fuera de los valores curados de VelaTerm para ${kind}. Verifica que la CLI instalada lo acepte.`,
  "spawn.remaining": (n: number) => `${n} more pending`, // ${n} more pending
  "spawn.notifyTitle": "Spawn session awaiting confirmation", // Spawn session awaiting confirmation
  "retire.title": "¿Retirar esta sesión?",
  "retire.session": "Sesión",
  "retire.actionArchive": "Archivar la sesión. No se borra ningún worktree.",
  "retire.actionCleanup":
    "Borrar los worktrees siguientes y luego archivar la sesión.",
  "retire.actionDiscard":
    "Descartar los cambios sin confirmar en el worktree de este worker.",
  "retire.discardWarning":
    "Se eliminan todos los cambios sin confirmar de este worktree. El trabajo confirmado se conserva.",
  "retire.descendants": (n: number) =>
    `Incluye ${n} sesión${n === 1 ? "" : "es"} descendiente${n === 1 ? "" : "s"}.`,
  "retire.irreversible": "Esto no se puede deshacer",
  "retire.worktreeCount": (n: number) =>
    `Se borrarán ${n} worktree${n === 1 ? "" : "s"} y su${n === 1 ? "" : "s"} rama${n === 1 ? "" : "s"}.`,
  "retire.pathLabel": "Ruta",
  "retire.branchLabel": "Rama",
  "retire.branchUnknown": "desconocida",
  "retire.resumed": "El directorio ya no existe; solo queda la rama.",
  "retire.keep": "Conservar",
  "retire.approve": "Retirar",
  "retire.remaining": (n: number) => `${n} pendientes más`,
  "retire.notifyTitle": "Retiro pendiente de confirmación", // Retire awaiting confirmation
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
  "settings.renderer": "Renderizador del terminal", // Terminal renderer
  "settings.rendererHint":
    "DOM (predeterminado) funciona en todas partes, pero muestra huecos en los bordes de TUI con alturas de línea mayores que 1. Canvas dibuja esos bordes sin huecos. WebGL es el más rápido, pero puede perder su contexto de GPU.",
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
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
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
  "settings.notifyUnsupported": "Notifications aren't available in this environment.", // TODO translate
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
  "settings.scHint": "Haz clic en un atajo y pulsa una nueva combinación (se requiere Cmd/Ctrl).", // hint
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
  "remote.moreUrls": (n: number) => (n > 1 ? `${n} enlaces más` : `${n} enlace más`), // N more urls
  "remote.lessUrls": "Mostrar menos", // Show less
  "remote.stop": "Detener servidor", // Stop Server
  "remote.passwordPlaceholder": "Establecer contraseña de acceso", // Set access password
  "remote.starting": "Iniciando…", // Starting…
  "remote.start": "Iniciar servidor", // Start Server
  "remote.portLabel": "Puerto", // Port
  "remote.portInvalid": "El puerto debe estar entre 1 y 65535", // Port must be between 1 and 65535
  "remote.ipLabel": "Dirección IP", // IP address
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
  "connect.sshFingerprintLabel": (kt: string) => `Huella de la clave del host SSH (${kt})`,
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
  "connect.urlPasswordPlaceholder": "Contraseña de acceso",
  "connect.shareDesktopDb": "Usar la base de datos de la app de escritorio remota",
  "connect.shareDesktopDbHint":
    "Comparte una base de datos con la app de escritorio del equipo remoto (mejor si ambas tienen la misma versión). Desactivado = base de datos independiente.",

  // ── Sidebar ──
  "tree.newSession": "Nueva sesión", // New Session
  "tree.newTerminalSession": "Nueva sesión de terminal", // New Terminal Session
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
  "tree.archiveBlockedTitle": "No se puede archivar la sesión", // Cannot Archive Session
  "tree.archiveBlockedBody": "Se rechazó el archivado. Una sesión archivada sale del alcance de limpieza, por lo que su worktree y su rama quedarían abandonados.", // Archiving was refused. An archived session leaves the worker cleanup scope, so its worktree and branch would be stranded.
  "tree.archiveConfirmTitle": "¿Archivar y borrar worktrees?", // Archive and delete worktrees?
  "tree.archiveConfirmBody": (n: number) =>
    `Archivar borra ${n} worktree(s) de worker y sus ramas. Esto no se puede deshacer. Solo se puede archivar el trabajo verificado como aterrizado en la rama padre; cualquier otro caso bloquea el archivado.`,
  "tree.archiveConfirmAction": "Archivar", // Archive
  "archive.blockedRow": (name: string, reason: string) =>
    `"${name}" aún conserva un worktree: ${reason}`,
  "archive.blockedSuffix": "Aterriza o retira el worker primero.",
  "archive.reason.uncommittedChanges": "el worktree tiene cambios sin confirmar",
  "archive.reason.noVerifiedLanding": "el worktree no tiene un aterrizaje verificado",
  "archive.reason.workerChangedAfterLanding": "el worker cambió después de su aterrizaje verificado",
  "archive.reason.landingNotOnTarget": "el commit de aterrizaje verificado no está en la rama de destino",
  "archive.reason.repoRootUnavailable": "la raíz del repositorio del worktree no está disponible",
  "archive.reason.missingNeverLanded": "falta el directorio del worktree y el worker nunca aterrizó",
  "archive.reason.missingUnverifiedLanding": "falta el directorio del worktree y su aterrizaje nunca se verificó",
  "archive.reason.missingDifferentParent": "falta el directorio del worktree y su aterrizaje pertenece a otro padre",
  "archive.reason.missingRepoRootUnavailable": "falta el directorio del worktree y la raíz del repositorio no está disponible",
  "archive.reason.missingLandingNotOnTarget": "falta el directorio del worktree y su commit de aterrizaje no está en la rama de destino",
  "archive.reason.branchMovedAfterLanding": "la rama del worker se movió después de su aterrizaje verificado",
  // Temporary (draft) sessions
  "tree.scratchTag": "temp", // scratch
  "tree.persistSession": "Convertir en sesión permanente…", // Make Permanent Session…
  "tree.persistDoc": "Guardar en disco…", // Save to Disk…
  "tree.closeScratch": "Cerrar borrador", // Close Scratch
  "tree.importProject": "Importar proyecto", // Import Project
  "tree.createProject": "Crear proyecto",
  "tree.cloneProject": "Clonar desde Git", // Clone from Git
  "createProject.title": "Crear proyecto",
  "createProject.name": "Nombre del proyecto",
  "createProject.namePlaceholder": "mi-proyecto",
  "createProject.into": "Crear en",
  "createProject.choose": "Elegir…",
  "createProject.noParent": "Elige una carpeta principal",
  "createProject.invalidName": "Introduce un único nombre de carpeta sin / ni \\.",
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
  "clone.slowHint": "No hay progreso desde hace 30 segundos. Comprueba la red o el proxy del equipo remoto; puedes cancelar y volver a intentarlo.",
  "clone.submit": "Clonar", // Clone
  "tree.globalSearch": "Buscar en todas las sesiones (⌘⇧F)", // Search All Sessions (⌘⇧F)
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
  "tree.noProjectsPre": "Aún no hay proyectos. Haz clic en el icono de carpeta o pulsa ", // No projects yet. Click the folder button, or press
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
  "tree.agentArgsLabel": "Argumentos de inicio (opcional)", // Launch args (optional)
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
  "info.notYetCaptured": "Aún no generado (se captura tras la primera ejecución)", // Not yet generated (captured after first run)
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
  "resume.title": "Reanudar sesión", // Resume Session
  "resume.desc":
    "Elige el tipo de agente e introduce el session id propio del agente; al abrir se retoma la conversación original.", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "Tipo de agente", // Agent type
  "resume.sessionIdPlaceholder": "Session id de la conversación", // Conversation session id
  "resume.confirm": "Reanudar y abrir", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "Nueva sesión de worktree…", // New Worktree Session…
  "worktree.worktreeNameLabel": "Nombre del worktree", // Worktree name
  "worktree.worktreeNameHint": "Se usa como nombre del directorio y la rama del worktree.", // Used as the worktree directory and branch name.
  "worktree.createFailed": "No se pudo crear el worktree", // Couldn't create the worktree
  "worktree.noRepoRoot": "Este proyecto no tiene una ruta de repositorio git utilizable.", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "Ninguno", // None
  "worktreeSel.modeNew": "Nuevo", // New
  "worktreeSel.modeExisting": "Existente", // Existing
  "worktreeSel.loading": "Cargando worktrees…", // Loading worktrees…
  "worktreeSel.empty": "No hay worktrees existentes en este repositorio.", // No existing worktrees in this repository.
  "worktreeSel.loadFailed": "No se pudieron listar los worktrees (¿no es un repositorio git?).", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint": "Las sesiones creadas en este grupo usarán este worktree de forma predeterminada.", // Sessions created in this group will use this worktree by default.

  // ── Archive panel ──
  "archive.title": "Sesiones archivadas", // Archived Sessions
  "archive.empty1": "No hay sesiones archivadas.", // No archived sessions.
  "archive.empty2":
    "Haz clic derecho en una sesión de la barra lateral y elige «Archivar sesión» para guardarla aquí.", // Right-click a session in the sidebar…
  "archive.restore": "Restaurar como sesión normal", // Restore to normal session
  "archive.retiredNote": "Un worker retirado no puede reanudarse de forma exacta, porque el retiro ya eliminó su worktree y su rama.", // A retired worker cannot resume exactly, because retire already deleted its worktree and branch.
  "archive.export": "Exportar contexto completo como Markdown", // Export full context as Markdown
  "archive.deleteForever": "Eliminar permanentemente (incluida la grabación)", // Delete permanently (with recording)
  "archive.pickOne": "Selecciona una sesión archivada a la izquierda para ver su transcripción", // Select an archived session on the left…
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
  "search.hint": "Busca en el contenido de las sesiones. Las archivadas se excluyen por defecto; marca «Incluir archivadas» para añadirlas.", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "Incluir archivadas", // Include archived
  "search.includeArchivedHint": "Buscar también en sesiones archivadas (desactivado por defecto)", // Also search archived sessions (off by default)
  "search.searching": "Buscando…", // Searching…
  "search.noResults": "No se encontraron coincidencias", // No matches found
  "search.sessionCount": (n) => (n === 1 ? "1 sesión" : `${n} sesiones`), // n sessions
  "search.matchCount": (n) => (n === 1 ? "1 coincidencia" : `${n} coincidencias`), // n matches
  "search.pickSession": "Selecciona una sesión a la izquierda para ver sus coincidencias", // Select a session on the left to see its matches
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
  "tab.bgTitle": (n) => `Pestañas en segundo plano: ${n} (procesos aún en ejecución)`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `Fondo ${n}`, // Background {n}
  "tab.scratchFallback": "(terminal temporal)", // (scratch terminal)
  "tab.killBgTab": "Terminar esta pestaña en segundo plano (sus procesos finalizarán)", // Kill this background tab…
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
  "browser.stop": "Detener la carga", // Stop loading
  "browser.openExternal": "Abrir en el navegador del sistema", // Open in system browser
  "browser.addressPlaceholder": "Introduce una URL o términos de búsqueda", // Enter URL or search terms
  "browser.quickAccess": "Acceso rápido", // Quick access
  "browser.loading": "Cargando…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "¿Salir de VelaTerm?",  // Quit VelaTerm?
  "quit.body": "Se detendrán todas las sesiones de terminal y de agente en ejecución.",  // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "Guardar espacio de trabajo",  // Save workspace
  "quit.saveWorkspaceHint": "Abrir las mismas pestañas y divisiones la próxima vez. Las terminales se restauran, pero no se reinician.",  // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "Salir",  // Quit
  "dormant.body": "Restaurado desde el espacio de trabajo guardado. Todavía no hay ningún proceso en ejecución.",  // Restored from your saved workspace. No process is running yet.
  "dormant.start": "Iniciar",  // Start
  "overlimit.title": (max) => `Límite de segundo plano superado (${max})`, // Background keep-alive over limit ({max})
  "overlimit.body": "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
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
  "term.mirrorBadge": (dims) => `⤢ Espejo${dims} · clic para ajustar a esta ventana`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) => `⤢ Espejo${dims} · ajustar a esta ventana`, // ⤢ Mirror{dims} · fit this window
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
  "agentInstall.pathSaved": (label: string) => `Ruta del ejecutable de ${label} guardada en Ajustes:`, // executable path saved to Settings
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
  "doc.overwriteConfirm": "Ya existe un archivo con ese nombre. Pulsa «Sobrescribir» para reemplazarlo.", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "Guardar (⌘S)", // Save (⌘S)
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
  "doc.imgDecodeFailed": "No se puede mostrar esta imagen (formato no compatible o archivo dañado).", // Cannot display this image (unsupported or corrupted format).
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
  "files.deleteConfirm": (name) => `¿Eliminar "${name}"? Esto no se puede deshacer.`, // Delete "{name}"? This can't be undone.

  // ── Status bar ──
  "statusbar.sessions": (n) => (n === 1 ? "1 sesión" : `${n} sesiones`), // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `Clic para mostrar solo sesiones "${label}" en la barra lateral (clic de nuevo para quitar)`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `Fondo ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `Pestañas en segundo plano (límite ${max}; al superarlo se termina automáticamente la pestaña inactiva más antigua)`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) => `Pestaña en segundo plano terminada: ${name} (límite superado)`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) => `Acceso remoto por navegador activado: ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "Permisos: Preguntar", // Perms: Ask
  "statusbar.permSkip": "Permisos: Omitir", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip": "Modo de permisos de esta sesión · haz clic para cambiar (solo esta sesión)", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "Permisos de esta sesión", // This session's permissions
  "statusbar.permOptAsk": "Preguntar cada vez (predeterminado)", // Ask each time (default)
  "statusbar.permScopeHint": "Se aplica solo a esta sesión. Para los ajustes globales, ve a Ajustes ▸ Agentes.", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg": "Permiso cambiado. La sesión debe reiniciarse para aplicarlo. El reinicio reanuda la conversación actual, pero interrumpe cualquier tarea en curso. ¿Reiniciar ahora?", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "Reiniciar ahora", // Restart now
  "statusbar.permRestartLater": "Más tarde", // Later
  "statusbar.permScopeTitle": "¿Aplicar a?", // Apply to?
  "statusbar.permScopeSession": "Solo esta sesión", // This session only
  "statusbar.permScopeGlobal": "Predeterminado global", // Global default
  "statusbar.permScopeGlobalHint": "Se aplica ahora a esta sesión y pasa a ser el valor predeterminado para futuras sesiones de este tipo (sincronizado con Ajustes).", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

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
  "err.uncaughtDesc": "La información de abajo puede ayudar a localizar el problema.", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser":
    "La reproducción de grabaciones aún no es compatible en el navegador", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) => `Error al subir la imagen (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.connecting": "Conectando…", // Connecting…
  "login.remoteAccess": "Acceso remoto", // Remote Access
  "login.desc": "Introduce la contraseña de acceso para conectarte a este terminal.", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "Contraseña de acceso", // Access password
  "login.connect": "Conectar", // Connect
  "login.wrongPassword": "Contraseña incorrecta", // Wrong password
  "login.rateLimited": "Demasiados intentos. Espera un minuto y vuelve a intentarlo.", // Too many attempts. Please wait a minute and try again.
  "login.failed": "Error de inicio de sesión, inténtalo de nuevo", // Login failed, please try again
  "login.pairingRequired": "Este servidor requiere un enlace de emparejamiento. Abre el enlace generado en el panel de Acceso remoto de la app de escritorio.", // This server requires a pairing link
  "login.authFailed": "Contraseña incorrecta o el enlace de emparejamiento ha caducado. Vuelve a conectarte con un nuevo enlace de emparejamiento.", // Wrong password or pairing link expired
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
  "conn.sshDown": "El enlace SSH está caído — pulsa «Reconectar ahora» para reintentar", // SSH link is down — press Reconnect now to try again
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
  "transport.remoteCmdForbidden": (cmd: string) => `Comando no disponible para clientes remotos: ${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) => `Clave de configuración no modificable por clientes remotos: ${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) => `Los clientes remotos no pueden acceder a archivos del directorio de datos de la aplicación: ${path}`, // Remote clients cannot access files in the app data directory

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

export default es;
