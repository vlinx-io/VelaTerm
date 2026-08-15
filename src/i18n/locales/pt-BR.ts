//! Brazilian Portuguese dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const ptBR: typeof en = {
  // ── Common ──
  "common.cancel": "Cancelar", // Cancel
  "common.confirm": "OK", // OK
  "common.delete": "Excluir", // Delete
  "common.save": "Salvar", // Save
  "common.create": "Criar", // Create
  "common.close": "Fechar", // Close
  "common.copy": "Copiar", // Copy
  "common.cut": "Recortar", // Cut
  "common.paste": "Colar", // Paste
  "common.selectAll": "Selecionar tudo", // Select All
  "common.copied": "Copiado", // Copied
  "common.retry": "Tentar novamente", // Retry
  "common.refresh": "Atualizar", // Refresh
  "common.loading": "Carregando…", // Loading…
  "common.prev": "Anterior", // Previous
  "common.next": "Próximo", // Next
  "common.on": "Ligado", // On
  "common.off": "Desligado", // Off
  "common.gotIt": "Entendi", // Got it
  "common.rename": "Renomear", // Rename
  "common.edit": "Editar", // Edit
  "common.open": "Abrir", // Open
  "common.session": "Sessão", // Session

  // ── Session types and status ──
  "kind.terminal": "Terminal", // Terminal
  "kind.browser": "Navegador", // Browser
  "status.idle": "Ocioso", // Idle
  "status.running": "Em execução", // Running
  "status.exited": "Encerrado", // Exited
  "status.error": "Erro", // Error
  "status.working": "Processando", // Working
  "status.asking": "Requer confirmação", // Needs confirmation
  "status.waiting": "Visto", // Viewed
  "status.unavailable": "Status indisponível",
  "indicator.unread": "Não lido · a revisar", // Unread · awaiting review

  // ── Title bar ──
  "titlebar.builtAt": (time) => `Compilado em ${time}`, // Built at {time}
  "titlebar.versionMismatch": (frontend, backend) =>
    `Versões divergentes: frontend v${frontend} ≠ backend v${backend} — recompile ou reimplante em sincronia.`, // Version mismatch

  "titlebar.hotReloadedAt": (time) => `Hot reload às ${time}`, // Hot reloaded at {time}
  "titlebar.themeSystem": (resolved) => `Seguir o sistema (atualmente ${resolved})`, // Follow system (currently {resolved})
  "titlebar.themeDark": "Escuro", // Dark
  "titlebar.themeLight": "Claro", // Light
  "titlebar.browser": "Navegador integrado (⌘⇧B)", // Built-in Browser (⌘⇧B)
  "titlebar.remoteAccess": "Acesso remoto (navegador)", // Remote Access (Browser)
  "titlebar.connectRemote": "Conectar a servidor remoto", // Connect to Remote Server
  "titlebar.share": "Compartilhar", // Share
  "share.title": "Compartilhar o VelaTerm", // Share VelaTerm
  "share.subtitle":
    "Somos uma pequena equipe por trás do VelaTerm. Se você gosta dele, compartilhe o VelaTerm com outras pessoas. Ajudar mais gente a nos conhecer significa muito para nossa equipe. Obrigado pelo apoio! ❤️", // We're a small team behind VelaTerm. If you enjoy it, please share VelaTerm with others…
  "share.copyLink": "Copiar link", // Copy link
  "share.copied": "Copiado!", // Copied!
  "share.wechatMoments": "Momentos do WeChat",
  "share.weibo": "Weibo",
  "share.xiaohongshu": "Xiaohongshu",
  "share.xiaohongshuAction":
    "Copiar o texto e o link e abrir a Central de Criadores do Xiaohongshu",
  "share.wechatQrTitle": "Compartilhar nos Momentos do WeChat",
  "share.wechatQrHint":
    "Escaneie o código com o WeChat, abra o link e escolha compartilhá-lo nos Momentos.",
  "share.backToPlatforms": "Voltar às opções de compartilhamento",
  "titlebar.appearance": "Aparência", // Appearance
  "titlebar.showLeft": "Mostrar barra lateral", // Show sidebar
  "titlebar.hideLeft": "Ocultar barra lateral", // Hide sidebar
  "titlebar.showRight": "Mostrar painel de info", // Show info panel
  "titlebar.hideRight": "Ocultar painel de info", // Hide info panel

  // ── Settings ──
  "settings.title": "Configurações", // Settings
  "settings.catTerminal": "Terminal", // Terminal
  "settings.catBehavior": "Comportamento", // Behavior
  "settings.catAgents": "Agentes", // Agents
  "settings.permDefault": "Padrão", // Default
  "settings.permYolo": "YOLO", // YOLO
  "settings.yoloHint": (flag: string) =>
    `Inicia com ${flag}. Pula todas as confirmações de permissão — use com cuidado.`, // YOLO flag hint
  "settings.permViaEnvHint":
    "Pula todas as confirmações de permissão via injeção de configuração (sem flag CLI). Aplica-se a esta sessão no início.",
  "settings.catGeneral": "Geral", // General
  "settings.cliLabel": "Comando do shell",
  "settings.cliInstall": "Instalar o comando ‘vela’",
  "settings.cliUninstall": "Desinstalar o comando ‘vela’",
  "settings.cliInstalledAt": (path: string) => `Instalado em ${path}`,
  "settings.cliConflict": (path: string) =>
    `Já existe outro comando ‘vela’ em ${path}. O VelaTerm não irá sobrescrevê-lo.`,
  "settings.cliHint": "Adiciona `vela <caminho-do-projeto>` ao PATH, como o comando `code` do VS Code.",
  "settings.agentArgsHint":
    "Argumentos de inicialização padrão aplicados a novas sessões de cada tipo de agente. Argumentos por sessão definidos ao criar ou editar têm prioridade. Deixe em branco para nenhum.", // Agent default launch args hint
  "settings.agentPathLabel": "Caminho do executável (opcional)", // Executable path (optional)
  "settings.agentPathPlaceholder": "ex.: ~/.local/bin/claude — vazio = buscar no PATH", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "Quando definido, sessões deste tipo iniciam por este caminho completo em vez de procurar o comando no PATH. Útil quando o agente está instalado mas fora do PATH do shell. Preenchido automaticamente após uma instalação em um clique quando o local é detectado.", // Agent executable path hint
  "settings.catOrchestration": "Orquestração",
  "settings.orchProfilesTitle": "Perfis de worker",
  "settings.orchProfile": "Perfil",
  "settings.orchDescription": "Descrição",
  "settings.orchDescriptionPlaceholder": "Descreva quando este perfil deve ser usado.",
  "settings.orchNewProfile": "Nome do novo perfil",
  "settings.orchAdd": "Adicionar",
  "settings.orchAddNew": "Adicionar novo",
  "settings.orchDelete": "Excluir",
  "settings.orchNoProfiles": "Ainda não há perfis. Crie um para reutilizá-lo em cada spawn.",
  "settings.orchProfilesHint":
    "Os perfis indicam a um agente líder qual configuração de worker serve para cada tarefa. Descreva quando usar cada perfil e escolha seu agente, modelo, esforço e worktree.",
  "settings.orchModel": "Modelo",
  "settings.orchEffort": "Esforço",
  "settings.orchWorktree": "Worktree próprio",
  "settings.orchPermissionMode": "Modo de permissão",
  "settings.orchPermissionDefault": "Padrão",
  "settings.orchPermissionSkip": "Ignorar confirmações",
  "settings.orchPermissionInherit": "Herdar do pai",
  "settings.orchPermissionSkipWarning":
    "Este worker é executado sem confirmação dentro de seu worktree.",
  "settings.orchLimitsTitle": "Limites",
  "settings.orchMaxDescendants": "Máx. de descendentes",
  "settings.orchMaxParallel": "Máx. em paralelo",
  "settings.orchMaxDepth": "Profundidade máx.",
  "settings.orchConfirmAbove": "Confirmar acima de",
  "settings.orchTimeout": "Tempo limite padrão (segundos)",
  "settings.orchAutoApprove": "Auto-approve /orch spawns", // Auto-approve /orch spawns
  "settings.orchAutoApproveHint": "Launch /orch child sessions without the confirmation card. The confirmation threshold still requires review.", // Launch /orch child sessions without the confirmation card. The confirmation threshold still requires review.
  "settings.orchConfirmAboveHint":
    "O cartão de confirmação aparece quando um spawn elevaria o número de sessões descendentes ativas acima deste valor, mesmo com a confirmação de spawn desativada. Sessões descendentes ativas estão iniciando, trabalhando ou aguardando uma solicitação de permissão.",
  "settings.orchLimitsHint":
    "Máx. de descendentes conta cada sessão descendente retida, incluindo sessões concluídas, que ocupam sua vaga até serem arquivadas ou removidas. Máx. em paralelo conta as sessões descendentes que estão iniciando, trabalhando ou aguardando uma solicitação de permissão.",
  "settings.orchCopyPatterns": "Padrões de cópia para o worktree",
  "settings.orchCopyPatternsHint":
    "Um glob por linha. Arquivos não rastreados ou ignorados que correspondam são copiados da raiz do repositório para cada novo worktree. Saídas de build como node_modules nunca são copiadas, então os workers ainda compilam do zero.",
  "settings.appearance": "Aparência", // Appearance
  "settings.accent": "Destaque", // Accent
  "settings.accentAuto": "Seguir o tema", // Follow theme
  "settings.density": "Densidade", // Density
  "settings.densityCompact": "Compacta", // Compact
  "settings.densityRegular": "Normal", // Regular
  "settings.densityComfy": "Espaçosa", // Comfy
  "settings.pane": "Painéis", // Panes
  "settings.paneFlush": "Sem borda", // Flush
  "settings.paneCard": "Cartão", // Card
  "settings.divider": "Divisor", // Divider
  "settings.dividerSubtle": "Sutil", // Subtle
  "settings.dividerVisible": "Visível", // Visible
  "settings.nav": "Barra lateral", // Sidebar
  "settings.navTree": "Árvore", // Tree
  "settings.navCompact": "Compacta", // Compact
  "settings.tabs": "Abas", // Tabs
  "settings.dynamicStatusFilter": "Inclusão dinâmica no filtro de status",
  "settings.tabSingle": "Única", // Single
  "settings.tabMulti": "Múltiplas", // Multi
  "settings.maxLiveTabs": "Background limit", // Background limit
  "settings.defaultShell": "Shell padrão", // Default shell
  "settings.spawnConfirm": "Confirm before spawn", // Confirm before spawn
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.cleanImages": "Limpar imagens coladas automaticamente",
  "settings.cleanImagesHint":
    "Imagens coladas ou arrastadas para o terminal são salvas primeiro como arquivos temporários (o caminho é enviado ao agente). Quando ativado, os arquivos temporários desta sessão são removidos ao sair, e sobras com mais de 24 h são limpas na inicialização. Imagens dentro de documentos não são afetadas.",
  "settings.cleanImagesNow": "Limpar agora",
  "settings.cleanImagesResult": (n: number, size: string) =>
    `${n} imagens temporárias limpas (${size} liberados).`,
  "settings.cleanImagesEmpty": "Nenhuma imagem temporária para limpar.",
  "settings.imagePasteMode": "Colar imagem",
  "settings.imagePasteUpload": "Colar caminho do arquivo",
  "settings.imagePasteAgent": "Colagem nativa",
  "settings.imagePasteHint":
    "Escolha o que será inserido ao colar uma imagem (apenas desktop local). Colar caminho do arquivo: salva a imagem temporariamente e insere o caminho no Claude ou Codex. Colagem nativa: deixa o Claude ou Codex ler a área de transferência do sistema e mostrar seu próprio marcador de imagem.",
  "settings.imagePasteRemoteHint":
    "Sessões remotas sempre colam o caminho do arquivo para que o agente leia a imagem em sua própria máquina. A colagem nativa está disponível apenas no desktop local.",
  "spawn.title": "Start spawned session?", // Start spawned session?
  "spawn.fromSession": "From", // From
  "spawn.promptLabel": "Prompt", // Prompt
  "spawn.agentLabel": "Agent", // Agent
  "spawn.modelLabel": "Modelo", // Model
  "spawn.effortLabel": "Esforço", // Effort
  "spawn.optionDefault": "padrão", // default
  "spawn.optionOther": "Outro...", // Other...
  "spawn.worktreeLabel": "Separate git worktree", // Separate git worktree
  "spawn.launch": "Launch", // Launch
  "spawn.remaining": (n: number) => `${n} more pending`, // ${n} more pending
  "spawn.notifyTitle": "Spawn session awaiting confirmation", // Spawn session awaiting confirmation
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "Ver alterações…",
  "changes.title": "Alterações",
  "changes.loading": "Carregando…",
  "changes.loadingDiff": "Carregando diff…",
  "changes.noChanges": "Sem alterações",
  "changes.selectFile": "Selecione um arquivo",
  "changes.binary": "Arquivo binário — diff por linha indisponível",
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
  "settings.renderer": "Renderizador do terminal", // Terminal renderer
  "settings.redrawOnReveal": "Redesenhar ao trocar de aba", // Redraw on tab switch
  "settings.catAdvanced": "Avançado", // Advanced
  "settings.outputScheduler": "Saída com prioridade em primeiro plano", // Foreground-priority output
  "settings.recordSessions": "Gravar registros de sessão", // Record session logs
  "settings.recordSessionsHint":
    "Desativado por padrão. Quando ativado, a saída do terminal é salva em um arquivo de log para reprodução de arquivo e busca. Sessões de terminal comuns nunca são gravadas; sessões de agente leem sua própria transcrição.", // Record session logs hint
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
  "settings.sound": "Som de notificação", // Notification sound
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
  "settings.catShortcuts": "Atalhos", // Shortcuts
  "settings.scOpenProject": "Abrir projeto", // Open project
  "settings.scNewTab": "Novo terminal", // New terminal
  "settings.scNewBrowserTab": "Nova aba do navegador", // New browser tab
  "settings.scClosePane": "Fechar painel / aba", // Close pane / tab
  "settings.scSplitRight": "Dividir à direita", // Split right
  "settings.scSplitDown": "Dividir abaixo", // Split down
  "settings.scSearch": "Buscar no terminal", // Find in terminal
  "settings.scGlobalSearch": "Buscar em todas as sessões", // Search all sessions
  "settings.scSaveDoc": "Salvar documento", // Save document
  "settings.scRecording": "Pressione as teclas…", // Press keys…
  "settings.scHint": "Clique em um atalho e pressione uma nova combinação (Cmd/Ctrl obrigatório).", // hint
  "settings.scReset": "Restaurar padrões", // Restore defaults
  "settings.scConflict": (label: string) => `Já usado por "${label}"`, // conflict

  // ── Remote access panel ──
  "remote.title": "Acesso remoto (navegador)", // Remote Access (Browser)
  "remote.desc":
    "Depois de ativado, dispositivos na mesma rede local podem abrir o endereço abaixo no navegador, digitar a senha e obter a mesma interface do desktop.", // Once enabled, devices on the same LAN…
  "remote.needPassword": "Defina primeiro uma senha de acesso", // Please set an access password first
  "remote.running": (port) => `Em execução · porta ${port}`, // Running · port {port}
  "remote.urlsHint":
    "Abra o endereço que esteja no mesmo WiFi / sub-rede do seu dispositivo (com várias interfaces de rede, escolha a correta; endereços VPN/túnel ficam por último e geralmente não são acessíveis de outros dispositivos):", // Open the address on the same WiFi / subnet…
  "remote.copyUrl": "Clique para copiar o endereço", // Click to copy address
  "remote.moreUrls": (n: number) => `mais ${n} link${n > 1 ? "s" : ""}`, // N more urls
  "remote.lessUrls": "Recolher", // Show less
  "remote.stop": "Parar servidor", // Stop Server
  "remote.passwordPlaceholder": "Definir senha de acesso", // Set access password
  "remote.starting": "Iniciando…", // Starting…
  "remote.start": "Iniciar servidor", // Start Server
  "remote.portLabel": "Porta", // Port
  "remote.portInvalid": "A porta deve estar entre 1 e 65535", // Port must be between 1 and 65535
  "remote.fingerprintLabel": "Impressão digital do certificado (SHA-256)", // Certificate fingerprint (SHA-256)
  "remote.fingerprintHint":
    "Na primeira conexão, os navegadores avisam que o certificado não é confiável — normal em certificado autoassinado. Compare esta impressão digital para confirmar que é esta máquina.", // On first connect, browsers warn the certificate is untrusted…

  "remote.pairingCreate": "Criar link de emparelhamento", // Create pairing link
  "remote.pairingRegenerate": "Regenerar link (desconecta todos)", // Regenerate link (disconnects all)
  "remote.pairingCreating": "Gerando…", // Generating…
  "remote.pairingHint":
    "Abra no navegador e digite a senha. Este link contém credenciais de acesso; compartilhe apenas com seus dispositivos.", // Open in a browser, then enter the password…

  "remote.devicesLabel": "Dispositivos emparelhados", // Paired devices
  "remote.lastSeen": "Última conexão", // Last seen
  "remote.revoke": "Revogar", // Revoke
  "remote.deviceBlock": "Bloquear", // Block
  "remote.deviceBlockConfirm": "Confirmar bloqueio", // Confirm block
  "remote.deviceBlockHint":
    "Dispositivos bloqueados são desconectados e não podem reconectar (precisam de um novo link de emparelhamento). Os outros dispositivos não são afetados.", // Block hint
  "remote.devicesEmpty": "Nenhum dispositivo emparelhado", // No paired devices yet

  // ── Remote connection panel ──
  "connect.title": "Conectar a servidor remoto", // Connect to Remote Server
  "connect.pairingPlaceholder": "Cole o link de emparelhamento", // Paste pairing link
  "connect.confirmConnect": "Impressão correta, conectar", // Fingerprint matches, connect
  "connect.desc":
    "Digite o endereço e a senha de um VelaTerm remoto para conectar e controlá-lo em uma nova janela.", // Enter the address and password…
  "connect.addressPlaceholder": "Endereço IP, ex.: 192.168.1.100", // IP address, e.g. 192.168.1.100
  "connect.portPlaceholder": "Porta", // Port
  "connect.connecting": "Conectando…", // Connecting…
  "connect.connect": "Conectar", // Connect
  "connect.stagePreparing": "Preparando servidor…",
  "connect.stageTransferring": "Transferindo servidor…",
  "connect.stageStarting": "Iniciando servidor…",
  "connect.sshFingerprintLabel": (kt: string) => `Impressão digital da chave do host SSH (${kt})`,
  "connect.sshHostNew":
    "Primeira conexão com este host — verifique a impressão digital antes de continuar.",
  "connect.sshHostChanged":
    "⚠ A chave deste host mudou — pode ser uma reinstalação do servidor ou um ataque man-in-the-middle. Continue apenas se tiver certeza.",
  "connect.urlCertChanged":
    "⚠ A impressão digital do certificado deste servidor mudou desde a última confirmação — pode ser uma reinstalação do servidor ou um ataque man-in-the-middle. Continue apenas se tiver certeza.",
  "connect.sshPasswordLabel": "Senha SSH",
  "connect.sshPasswordPlaceholder": "Senha da conta",
  "connect.savedHosts": "Hosts recentes",
  "connect.savedHostsAll": "Todos os hosts recentes",
  "connect.showAllHosts": (n: number) => `Ver todos (${n})`,
  "connect.forgetHost": "Esquecer este host",
  "connect.savedHasPassword": "Senha salva",
  "connect.rememberPassword": "Lembrar senha",
  "connect.urlPasswordPlaceholder": "Senha de login",
  "connect.shareDesktopDb": "Usar o banco de dados do app de desktop remoto",
  "connect.shareDesktopDbHint":
    "Compartilha um banco de dados com o app de desktop da máquina remota (melhor quando ambos têm a mesma versão). Desativado = banco de dados isolado.",

  // ── Sidebar ──
  "tree.newSession": "Nova sessão", // New Session
  "tree.newTerminalSession": "Nova sessão de terminal", // New Terminal Session
  "tree.newBrowserPage": "Nova página de navegador", // New Browser Page
  "tree.newAgentSession": (agent) => `Nova sessão ${agent}`, // New {agent} Session
  "tree.newAgentSessionGroup": "Mais sessões de agente", // More Agent Session
  "tree.newAgentSessionCustom": "Novo com argumentos…", // New with launch args…
  "tree.resumeSession": "Retomar sessão…", // Resume Session…
  "tree.newGroup": "Novo grupo", // New Group
  "tree.newSubgroup": "Novo subgrupo", // New Subgroup
  "tree.newChildSession": "Nova sessão filha", // New Child Session
  "tree.openSelected": "Abrir sessões selecionadas", // Open Selected Sessions
  "tree.archiveSelected": "Arquivar sessões selecionadas", // Archive Selected Sessions
  "tree.moveSelected": "Mover selecionados para…", // Move Selected to…
  "tree.deleteSelected": (n) => `Excluir ${n} itens selecionados`, // Delete {n} Selected Items
  "tree.removeProject": "Remover projeto", // Remove Project
  "tree.deleteGroup": "Excluir grupo", // Delete Group
  "tree.deleteSession": "Excluir sessão", // Delete Session
  "tree.projectRoot": "Raiz do projeto (sem grupo)", // Project root (no group)
  "tree.moveToSession": "Mover para baixo de uma sessão (como filha)", // Move under a session (as child)
  "tree.moveTo": "Mover para…", // Move to…
  "tree.openNewTab": "Abrir em nova aba", // Open in New Tab
  "tree.forkSession": "Bifurcar sessão", // Fork Session
  "tree.exportSession": "Exportar sessão…", // Export Session…
  "tree.sessionInfo": "Informações da sessão", // Session Info
  "tree.groupInfo": "Informações do grupo", // Group Info
  "info.branch": "Ramo", // Branch
  "info.path": "Caminho", // Path
  "info.recentCommits": "Commits recentes", // Recent Commits
  "info.noCommits": "Sem commits", // No commits
  "tree.killProcess": "Encerrar processo", // Kill Process
  "tree.archiveSession": "Arquivar sessão", // Archive Session
  "tree.archiveGroup": "Arquivar grupo", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "temp", // scratch
  "tree.persistSession": "Tornar sessão permanente…", // Make Permanent Session…
  "tree.persistDoc": "Salvar no disco…", // Save to Disk…
  "tree.closeScratch": "Fechar rascunho", // Close Scratch
  "tree.importProject": "Importar projeto", // Import Project
  "tree.createProject": "Criar projeto",
  "tree.cloneProject": "Clonar do Git", // Clone from Git
  "createProject.title": "Criar projeto",
  "createProject.name": "Nome do projeto",
  "createProject.namePlaceholder": "meu-projeto",
  "createProject.into": "Criar em",
  "createProject.choose": "Escolher…",
  "createProject.noParent": "Escolha uma pasta principal",
  "createProject.invalidName": "Digite um único nome de pasta sem / ou \\.",
  "createProject.creating": "Criando…",
  "createProject.submit": "Criar projeto",
  "clone.title": "Clonar repositório Git", // Clone Git Repository
  "clone.url": "URL do repositório", // Repository URL
  "clone.urlPlaceholder": "https://… ou git@…",
  "clone.branch": "Branch (opcional)", // Branch (optional)
  "clone.branchPlaceholder": "Branch padrão se vazio", // Default branch if empty
  "clone.folder": "Nome da pasta", // Folder name
  "clone.folderPlaceholder": "Automático pela URL", // Auto from URL
  "clone.into": "Clonar em", // Clone into
  "clone.choose": "Escolher…", // Choose…
  "clone.noParent": "Escolha uma pasta principal", // Choose a parent folder
  "clone.cloning": "Clonando…", // Cloning…
  "clone.cancelling": "Cancelando…",
  "clone.stageStarting": "Iniciando o Git…",
  "clone.stageConnecting": "Conectando ao repositório…",
  "clone.stagePreparing": "Preparando objetos…",
  "clone.stageReceiving": "Recebendo objetos…",
  "clone.stageResolving": "Resolvendo deltas…",
  "clone.stageCheckout": "Extraindo arquivos…",
  "clone.stageFinalizing": "Finalizando…",
  "clone.stageImporting": "Importando projeto…",
  "clone.elapsed": (seconds: number) => `${seconds} s decorridos`,
  "clone.slowHint": "Sem progresso há 30 segundos. Verifique a rede ou o proxy da máquina remota; você pode cancelar e tentar novamente.",
  "clone.submit": "Clonar", // Clone
  "tree.globalSearch": "Pesquisar em todas as sessões (⌘⇧F)", // Search All Sessions (⌘⇧F)
  "tree.archivedSessions": "Sessões arquivadas", // Archived Sessions
  "tree.searchPlaceholder": "Buscar sessões / grupos…", // Search sessions / groups…
  "tree.clearSearch": "Limpar busca", // Clear search
  "tree.filterWorking": "Em andamento", // Working
  "tree.filterAsking": "Pendente", // Pending
  "tree.filterWaiting": "Visto", // Viewed
  "tree.filterStatus": "Filtrar por status", // Filter by status
  "tree.refreshStatusFilter": "Atualizar filtro de status",
  "tree.refreshStatusMatch": "Atualizar status",
  "tree.filterStatusSection": "Status", // Status
  "tree.filterMarkSection": "Marca", // Mark
  "tree.viewMainName": "Principal",
  "tree.viewUntitled": "Visualização sem nome",
  "tree.viewDefaultName": (n) => `Visualização ${n}`,
  "tree.viewPrimary": "Visualização principal",
  "tree.viewManage": "Gerenciar visualização",
  "tree.viewSetPrimary": "Definir como principal",
  "tree.viewRename": "Renomear visualização",
  "tree.viewName": "Nome da visualização",
  "tree.viewDelete": "Excluir visualização",
  "tree.viewDeletePrimary": "A visualização principal não pode ser excluída",
  "tree.viewDeleteTitle": "Excluir visualização em árvore",
  "tree.viewDeleteConfirm": (name) =>
    `Excluir “${name}”? A busca e os filtros salvos serão removidos; projetos e sessões não serão afetados.`,
  "tree.viewSplitRight": "Dividir a visualização em árvore à direita",
  "tree.viewSplitDown": "Dividir a visualização em árvore abaixo",
  "tree.viewAdd": "Copiar a visualização atual para uma nova aba",
  "tree.viewCount": (n) => `${n} visualizaç${n === 1 ? "ão" : "ões"} em árvore`,
  "mark.menu": "Marca", // Mark
  "mark.urgent": "Urgente", // Urgent
  "mark.important": "Importante", // Important
  "mark.bug": "Bug", // Bug
  "mark.done": "Concluído", // Done
  "mark.wip": "Em andamento", // In progress
  "mark.pinned": "Fixado", // Pinned
  "mark.idea": "Ideia", // Idea
  "mark.caution": "Atenção", // Caution
  "tree.clearAllNotifications":
    "Limpar todos os indicadores de notificação (pontos de sessão e selo do Dock)", // Clear all notification badges…
  "tree.noProjectsPre": "Nenhum projeto ainda. Clique no ícone de pasta ou pressione ", // No projects yet. Click the folder button, or press
  "tree.noProjectsPost": " para importar um diretório.", // to import a directory.
  "tree.openProject": "Abrir projeto", // Open Project
  "tree.noAttention": "Nenhuma sessão corresponde ao filtro de status", // No sessions match the status filter
  "tree.noMatch": "Sem resultados", // No matches

  // Dialog fields
  "tree.groupName": "Nome do grupo", // Group name
  "tree.sessionNameAuto": "Nome da sessão (vazio = automático)", // Session name (leave empty to auto-name)
  "tree.editSession": "Editar sessão", // Edit Session
  "tree.sessionName": "Nome da sessão", // Session name
  "tree.shellLabel": "Shell (vazio = padrão do sistema)", // Shell (leave empty for system default)
  "tree.shellMenu": "Shell",
  "tree.downloadFullGitbash": "Baixar Git Bash completo",
  "gitbash.title": "Git Bash",
  "gitbash.downloading": "Baixando Git Bash completo…",
  "gitbash.extracting": "Extraindo Git Bash completo…",
  "gitbash.done": "Git Bash completo está pronto.",
  "gitbash.failed": "Falha ao baixar o Git Bash",
  "tree.shellSystemDefault": "Padrão do sistema", // System default
  "form.customOption": "Personalizado…", // Custom…
  "tree.cwdLabel": "Diretório de trabalho (vazio = raiz do projeto)", // Working directory (leave empty for project root)
  "tree.initCmdLabel": "Comando de inicialização (opcional)", // Startup command (optional)
  "tree.agentArgsLabel": "Argumentos de inicialização (opcional)", // Launch args (optional)
  "tree.permissionSkipLabel": "Pular todas as confirmações de permissão", // Skip all permission confirmations
  "tree.permissionSkipHint":
    "Inicia com a flag de bypass deste agente (ex.: Claude --dangerously-skip-permissions; o Codex também desativa o sandbox). Vale a cada inicialização — use com cuidado.",
  "tree.permissionUnsupported":
    "O OpenCode controla permissões pelo arquivo de configuração — sem flag de inicialização, então isto não se aplica.",
  "tree.permissionUnsupportedPi":
    "O Pi executa ferramentas sem solicitações de permissão por design — isto não se aplica.",

  // Diálogo "Nova sessão de agente"
  "newAgent.desc":
    "Opcionalmente, nomeie a sessão e adicione argumentos de inicialização personalizados (passados ao comando do agente, ex.: --model opus). Deixe ambos vazios e pressione Enter para iniciá-la normalmente.", // Optionally name the session and add custom launch args…

  // Delete confirmation
  "tree.batchDeleteTitle": "Exclusão em lote", // Batch Delete
  "tree.deleteProjectTitle": "Excluir projeto", // Delete Project
  "tree.deleteGroupTitle": "Excluir grupo", // Delete Group
  "tree.deleteSessionTitle": "Excluir sessão", // Delete Session
  "tree.batchDeleteBody": (n) =>
    `Excluir os ${n} itens selecionados (projetos/grupos excluem em cascata seus subgrupos e sessões). Esta ação não pode ser desfeita.`, // Delete the {n} selected items…
  "tree.deleteProjectBody": (name) =>
    `Excluir o projeto "${name}"? Todos os seus subgrupos e sessões também serão excluídos. Esta ação não pode ser desfeita.`, // Delete project "{name}"?…
  "tree.deleteGroupBody": (name) =>
    `Excluir o grupo "${name}"? Todos os seus subgrupos e sessões também serão excluídos. Esta ação não pode ser desfeita.`, // Delete group "{name}"?…
  "tree.deleteSessionBody": (name) =>
    `Excluir a sessão "${name}" (e todas as suas sessões filhas)? Esta ação não pode ser desfeita.`, // Delete session "{name}"…
  "tree.deleteWorktrees": (n) =>
    `Também remover os worktrees git associados (${n} no total; a remoção pode falhar se a árvore de trabalho tiver alterações)`, // Also remove associated git worktrees…

  // Session information dialog
  "info.name": "Nome", // Name
  "info.type": "Tipo", // Type
  "info.status": "Estado", // Status
  "info.notYetCaptured": "Ainda não gerado (capturado após a primeira execução)", // Not yet generated (captured after first run)
  "info.sessionId": "ID da sessão", // Session ID
  "info.cwd": "Diretório", // Working dir
  "info.initCmd": "Comando", // Startup cmd
  "info.agentArgs": "Argumentos", // Launch args
  "info.launchCmd": "Comando completo", // Full launch command
  "info.permission": "Permissão", // Permission
  "info.permissionSkip": "Pular todas as confirmações", // Skip all confirmations
  "info.parentSessionId": "ID do pai", // Parent ID
  "info.termTitle": "Título do terminal", // Terminal title
  "info.createdAt": "Criado em", // Created at

  // Resume-session dialog
  "resume.title": "Retomar sessão", // Resume Session
  "resume.desc":
    "Escolha o tipo de agente e informe o session id próprio do agente; ao abrir, a conversa original é retomada.", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "Tipo de agente", // Agent type
  "resume.sessionIdPlaceholder": "Session id da conversa", // Conversation session id
  "resume.confirm": "Retomar e abrir", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "Nova sessão de worktree…", // New Worktree Session…
  "worktree.worktreeNameLabel": "Nome do worktree", // Worktree name
  "worktree.worktreeNameHint": "Usado como nome do diretório e do branch do worktree.", // Used as the worktree directory and branch name.
  "worktree.createFailed": "Não foi possível criar o worktree", // Couldn't create the worktree
  "worktree.noRepoRoot": "Este projeto não tem um caminho de repositório git utilizável.", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "Nenhum", // None
  "worktreeSel.modeNew": "Novo", // New
  "worktreeSel.modeExisting": "Existente", // Existing
  "worktreeSel.loading": "Carregando worktrees…", // Loading worktrees…
  "worktreeSel.empty": "Nenhum worktree existente neste repositório.", // No existing worktrees in this repository.
  "worktreeSel.loadFailed": "Não foi possível listar os worktrees (não é um repositório git?).", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint": "As sessões criadas neste grupo usarão este worktree por padrão.", // Sessions created in this group will use this worktree by default.

  // ── Archive panel ──
  "archive.title": "Sessões arquivadas", // Archived Sessions
  "archive.empty1": "Nenhuma sessão arquivada.", // No archived sessions.
  "archive.empty2":
    "Clique com o botão direito em uma sessão na barra lateral e escolha \"Arquivar sessão\" para guardá-la aqui.", // Right-click a session in the sidebar…
  "archive.restore": "Restaurar como sessão normal", // Restore to normal session
  "archive.export": "Exportar contexto completo como Markdown", // Export full context as Markdown
  "archive.deleteForever": "Excluir permanentemente (com a gravação)", // Delete permanently (with recording)
  "archive.pickOne": "Selecione uma sessão arquivada à esquerda para ver a transcrição", // Select an archived session on the left…
  "archive.recordingEnd": "--- Fim da gravação ---", // --- End of recording ---
  "archive.readRecordingFailed": (err) => `Falha ao ler a gravação: ${err}`, // Failed to read recording: {err}
  "archive.searchRecording": "Buscar na gravação…", // Search in recording…
  "archive.searchTranscript": "Buscar na transcrição…", // Search transcript…
  "archive.searchPlaceholder": "Buscar conteúdo arquivado…", // Search archived content…
  "archive.msgCountAll": (n) => (n === 1 ? "1 mensagem" : `${n} mensagens`), // {n} messages
  "archive.msgCountFiltered": (shown, total) => `${shown} / ${total} mensagens`, // {shown} / {total} messages
  "archive.you": "Você", // You
  "archive.toolsUsed": (tools) => `Ferramentas: ${tools}`, // Tools: {tools}
  "archive.noMatch": "Nenhuma mensagem correspondente", // No matching messages
  "archive.emptyTranscript": "A transcrição está vazia", // Transcript is empty
  "archive.loadingTranscript": "Carregando transcrição…", // Loading transcript…

  // ── Global session-content search ──
  "search.allPlaceholder": "Pesquisar em todo o conteúdo das sessões…", // Search across all session content…
  "search.hint": "Pesquise o conteúdo das sessões. As arquivadas são excluídas por padrão — marque \"Incluir arquivadas\" para incluí-las.", // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "Incluir arquivadas", // Include archived
  "search.includeArchivedHint": "Pesquisar também em sessões arquivadas (desativado por padrão)", // Also search archived sessions (off by default)
  "search.searching": "Pesquisando…", // Searching…
  "search.noResults": "Nenhuma correspondência encontrada", // No matches found
  "search.sessionCount": (n) => (n === 1 ? "1 sessão" : `${n} sessões`), // n sessions
  "search.matchCount": (n) => (n === 1 ? "1 correspondência" : `${n} correspondências`), // n matches
  "search.pickSession": "Selecione uma sessão à esquerda para ver as correspondências", // Select a session on the left to see its matches
  "search.openSession": "Abrir sessão", // Open session
  "search.backToResults": "Voltar aos resultados", // Back to results
  "search.archivedBadge": "Arquivada", // Archived
  "search.summary": (m, s) =>
    `${m} ${m === 1 ? "correspondência" : "correspondências"} · ${s} ${s === 1 ? "sessão" : "sessões"}`, // X matches · N sessions
  "search.matchPosition": (n, total) => `${n} de ${total}`, // N of M
  "search.roleTerminal": "Terminal", // Terminal
  "search.collapseGroup": "Recolher", // Collapse
  "search.expandGroup": "Expandir", // Expand
  "search.cappedNote": (l, total) => `${l} de ${total} localizáveis`, // L of total locatable

  // ── Center pane ──
  "center.noSession": "Sem sessão", // No session
  "center.noSessionHintPre": "Escolha uma sessão na barra lateral ou pressione ", // Pick a session from the sidebar, or press
  "center.noSessionHintPost": " para criar um terminal", // to create a terminal
  "center.createTerminal": "Criar terminal", // Create Terminal
  "tab.unsavedDot": "Alterações não salvas", // Unsaved changes
  "tab.newTerminal": "Novo terminal", // New terminal
  "tab.newDocument": "Novo documento", // New document
  "tab.bgTitle": (n) => `Abas em segundo plano: ${n} (processos ainda em execução)`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `Fundo ${n}`, // Background {n}
  "tab.scratchFallback": "(terminal temporário)", // (scratch terminal)
  "tab.killBgTab": "Encerrar esta aba em segundo plano (seus processos serão encerrados)", // Kill this background tab…
  "tab.newBrowserTab": "Nova aba", // New Tab
  "tab.refreshFile": "Recarregar arquivo", // Refresh File
  "tab.closeOthers": "Fechar outras abas", // Close Other Tabs
  "tab.closeRight": "Fechar abas à direita", // Close Tabs to the Right
  "tab.closeAll": "Fechar todas as abas", // Close All Tabs
  "tab.sendToBackground": "Enviar para segundo plano", // Send to Background

  // ── Navegador integrado ──
  "browser.back": "Voltar", // Back
  "browser.forward": "Avançar", // Forward
  "browser.reload": "Recarregar", // Reload
  "browser.stop": "Parar o carregamento", // Stop loading
  "browser.openExternal": "Abrir no navegador do sistema", // Open in system browser
  "browser.addressPlaceholder": "Digite uma URL ou termos de busca", // Enter URL or search terms
  "browser.quickAccess": "Acesso rápido", // Quick access
  "browser.loading": "Carregando…", // Loading…
  "overlimit.title": (max) => `Limite de segundo plano excedido (${max})`, // Background keep-alive over limit ({max})
  "overlimit.body": "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
  "overlimit.kill": "End Selected", // End Selected
  "overlimit.keep": "Keep for Now", // Keep for Now
  "overlimit.earliest": "earliest", // earliest
  "overlimit.statusWorking": "working", // working
  "overlimit.statusAsking": "awaiting reply", // awaiting reply
  "overlimit.statusWaiting": "waiting", // waiting

  // ── Terminal pane ──
  "term.paste": "Colar", // Paste
  "term.pasteUseShortcut": "Colar (pressione ⌘V)", // Paste (press ⌘V)
  "term.selectAll": "Selecionar tudo", // Select All
  "term.autoCopied": (n: number) => `${n} caracteres copiados · ⌘V para colar`,
  "term.clear": "Limpar", // Clear
  "term.searchMenu": "Buscar…", // Search…  ⌘F
  "term.splitRight": "Dividir à direita", // Split right (⌘D)
  "term.splitDown": "Dividir abaixo", // Split down (⌘⇧D)
  "term.closePane": "Fechar divisão", // Close split
  "term.redraw": "Redesenhar", // Redraw
  "term.mirrorTooltip":
    "Exibindo em espelho (tamanho controlado por outro cliente). Clique para ajustar o PTY a esta janela", // Mirroring (size controlled by another client)…
  "term.mirrorBadge": (dims) => `⤢ Espelho${dims} · clique para ajustar a esta janela`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) => `⤢ Espelho${dims} · ajustar a esta janela`, // ⤢ Mirror{dims} · fit this window
  "term.imgUploadFailed": (n, lastError) =>
    `Falha no envio de ${n} imagem${n === 1 ? "" : "ns"}${lastError ? `: ${lastError}` : ""}`, // Image upload failed for {n} images…
  "term.imgClipboardUnavailable":
    "Não foi possível ler a imagem da área de transferência. Copie-a novamente e tente outra vez.",
  "term.starting": (agent) => `Iniciando ${agent}…`, // Starting {agent}…
  "term.startFailed": (err) => `Falha ao iniciar: ${err}`, // Failed to start: {err}

  // ── Cartão de ajuda para instalar um agente ──
  "agentInstall.title": (label) => `${label} não está instalado`, // {label} is not installed
  "agentInstall.desc": (label) =>
    `O VelaTerm não encontrou ${label} no seu PATH. Instale-o para iniciar esta sessão.`, // couldn't find {label} on PATH
  "agentInstall.install": "Instalar agora", // Install now
  "agentInstall.retry": "Tentar novamente", // Retry launch
  "agentInstall.dismiss": "Faço eu mesmo", // I'll do it myself
  "agentInstall.docs": "Documentação", // Install docs
  "agentInstall.needsNode": "Requer Node.js / npm", // Requires Node.js / npm
  "agentInstall.afterInstall": "Após instalar:", // After install:
  "agentInstall.pathSaved": (label: string) => `Caminho do executável de ${label} salvo nas Configurações:`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} está instalado`, // {label} is installed
  "agentInstall.doneDesc": "Reinicie esta sessão para começar a usá-lo.", // Relaunch this session to start using it.
  "agentInstall.restartNow": "Reiniciar agora", // Relaunch now
  "agentInstall.later": "Mais tarde", // Later
  "search.placeholder": "Buscar no terminal", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "WYSIWYG", // WYSIWYG
  "doc.source": "Código", // Source
  "doc.searchPlaceholder": "Localizar", // Find
  "doc.searchReplacePlaceholder": "Substituir", // Replace
  "doc.searchReplace": "Substituir", // Replace
  "doc.searchReplaceAll": "Tudo", // All
  "doc.searchNoMatch": "Sem resultados", // No results
  "doc.searchCaseSensitive": "Diferenciar maiúsculas", // Match case
  "doc.searchToggleReplace": "Alternar substituição", // Toggle replace
  "doc.fileTree": "Árvore de arquivos", // File tree
  "doc.treeUp": "Pasta pai", // Parent folder
  "doc.sidebar": "Barra lateral", // Sidebar
  "doc.unsaved": "Não salvo", // Unsaved
  "doc.saveAsTitle": "Salvar como", // Save As
  "doc.saveAsName": "Nome do arquivo", // File name
  "doc.outline": "Estrutura", // Outline
  "doc.outlineEmpty": "Sem títulos", // No headings
  "doc.saving": "Salvando…", // Saving…
  "doc.overwriteConfirm": "Já existe um arquivo com esse nome. Clique em “Substituir” para substituí-lo.", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "Salvar (⌘S)", // Save (⌘S)
  "doc.externalChanged":
    "O arquivo foi modificado no disco (você tem alterações locais não salvas).", // The file was modified on disk…
  "doc.reloadDiscard": "Recarregar (descartar minhas alterações)", // Reload (discard my changes)
  "doc.externalChangedClean": "O arquivo foi modificado no disco.", // The file was modified on disk.
  "doc.reload": "Recarregar", // Reload
  "doc.ignore": "Ignorar", // Ignore
  "doc.loadingFile": (title) => `Carregando ${title}…`, // Loading {title}…
  "doc.closeTitle": "Fechar documento", // Close Document
  "doc.unsavedBody": (title) => `"${title}" tem alterações não salvas.`, // "{title}" has unsaved changes.
  "doc.saveAndClose": "Salvar e fechar", // Save & Close
  "doc.closeNoSave": "Fechar sem salvar", // Close Without Saving
  "doc.conflictTitle": "Conflito ao salvar", // Save Conflict
  "doc.conflictBody":
    "O arquivo no disco foi modificado externamente. Sobrescrever mesmo assim com o conteúdo atual?", // The file on disk was modified externally…
  "doc.overwrite": "Sobrescrever", // Overwrite
  "doc.saveFailed": (err) => `Falha ao salvar: ${err}`, // Save failed: {err}
  "doc.closeTab": "Fechar aba", // Close Tab
  "doc.truncatedReadonly": (size: string) =>
    `Somente leitura: mostrando os primeiros 10 MB de ${size}. Salvar está desativado para não sobrescrever o resto do arquivo.`,
  "doc.imgLoading": (title, size) => `Carregando ${title} (${size})…`, // Loading {title} ({size})…
  "doc.imgBeingWritten":
    "O arquivo está sendo gravado; será recarregado automaticamente quando estabilizar.", // The file is being written; it will reload automatically once it settles.
  "doc.imgDecodeFailed": "Não é possível exibir esta imagem (formato não suportado ou arquivo corrompido).", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "Ajustar", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "Exportar PDF", // Export PDF
  "doc.diagramError": "Erro de diagrama", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "Nenhuma sessão selecionada", // No session selected
  "panel.openInEditor": "Abrir no editor", // Open in Editor
  "panel.openInEditorTooltip":
    "Abrir no editor de documentos do painel central (igual ao comando view)", // Open in the document editor…
  "panel.preview": "Visualizar", // Preview
  "panel.cantRead": "(não foi possível ler este arquivo)", // (cannot read this file)
  "panel.binary": "(arquivo binário, sem pré-visualização)", // (binary file, no preview)
  "panel.truncated": "\n…(conteúdo truncado)", // …(content truncated)
  "panel.showHidden": "Mostrar arquivos ocultos", // Show hidden files
  "panel.hideHidden": "Ocultar arquivos ocultos", // Hide hidden files

  // ── File-tree actions (Files context menu and header add button) ──
  "files.newFile": "Novo arquivo", // New File
  "files.newFolder": "Nova pasta", // New Folder
  "files.nameLabel": "Nome", // Name
  "files.newTooltip": "Novo arquivo ou pasta", // New file or folder
  "files.openInTerminal": "Open in Terminal",
  "files.revealInFinder": "Show in File Manager",
  "files.copyPath": "Copy Path",
  "files.copyRelPath": "Copy Relative Path",
  "files.filterPlaceholder": "Filter files…",
  "files.deleteConfirm": (name) => `Excluir "${name}"? Isso não pode ser desfeito.`, // Delete "{name}"? This can't be undone.

  // ── Status bar ──
  "statusbar.sessions": (n) => (n === 1 ? "1 sessão" : `${n} sessões`), // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `Clique para mostrar apenas sessões "${label}" na barra lateral (clique novamente para limpar)`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `Fundo ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `Abas em segundo plano (limite ${max}; ao exceder, a aba inativa mais antiga é encerrada automaticamente)`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) => `Aba em segundo plano encerrada: ${name} (limite excedido)`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) => `Acesso remoto pelo navegador ativado: ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "Permissões: Perguntar", // Perms: Ask
  "statusbar.permSkip": "Permissões: Ignorar", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip": "Modo de permissão desta sessão · clique para alterar (somente esta sessão)", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "Permissões desta sessão", // This session's permissions
  "statusbar.permOptAsk": "Perguntar sempre (padrão)", // Ask each time (default)
  "statusbar.permScopeHint": "Aplica-se apenas a esta sessão. Para configurações globais, acesse Configurações ▸ Agentes.", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg": "Permissão alterada. A sessão precisa ser reiniciada para aplicar. Reiniciar retoma a conversa atual, mas interrompe qualquer tarefa em andamento. Reiniciar agora?", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "Reiniciar agora", // Restart now
  "statusbar.permRestartLater": "Mais tarde", // Later
  "statusbar.permScopeTitle": "Aplicar a?", // Apply to?
  "statusbar.permScopeSession": "Somente esta sessão", // This session only
  "statusbar.permScopeGlobal": "Padrão global", // Global default
  "statusbar.permScopeGlobalHint": "Aplica-se agora a esta sessão e passa a ser o padrão para futuras sessões deste tipo (sincronizado com as Configurações).", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

  // ── Store, notifications, and export ──
  "notify.working": "⏳ Processando…", // ⏳ Working…
  "notify.asking": "❓ Precisa da sua confirmação", // ❓ Needs your confirmation
  "notify.waiting": "✅ Respondido", // ✅ Replied
  "store.subtask": "Subtarefa", // Subtask
  "store.splitPane": "Divisão", // Split
  "export.failedTitle": "Falha ao exportar a sessão", // Failed to export session
  "export.contextSuffix": "contexto", // context

  // ── Error panel ──
  "err.renderTitle": "Erro de renderização", // Rendering Error
  "err.renderDesc":
    "Ocorreu um erro inesperado. As informações abaixo podem ajudar a localizar o problema.", // An unexpected error occurred…
  "err.reload": "Recarregar", // Reload
  "err.uncaughtTitle": "Erro não capturado", // Uncaught Error
  "err.uncaughtDesc": "As informações abaixo podem ajudar a localizar o problema.", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser":
    "A reprodução de gravações ainda não é suportada no navegador", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) => `Falha no envio da imagem (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.connecting": "Conectando…", // Connecting…
  "login.remoteAccess": "Acesso remoto", // Remote Access
  "login.desc": "Digite a senha de acesso para se conectar a este terminal.", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "Senha de acesso", // Access password
  "login.connect": "Conectar", // Connect
  "login.wrongPassword": "Senha incorreta", // Wrong password
  "login.failed": "Falha no login, tente novamente", // Login failed, please try again
  "login.pairingRequired": "Este servidor requer um link de emparelhamento. Abra o link gerado no painel de Acesso remoto do app de desktop.", // This server requires a pairing link
  "login.authFailed": "Senha incorreta ou o link de emparelhamento expirou. Reconecte-se com um novo link de emparelhamento.", // Wrong password or pairing link expired
  "dir.title": "Escolher diretório do projeto", // Choose Project Directory
  "dir.pathPlaceholder": "Pesquise, ou digite um caminho e pressione Enter (aceita ~)", // Search, or type a path and press Enter (supports ~)
  "dir.up": "Um nível acima", // Up one level
  "dir.newFolder": "Nova pasta", // New Folder
  "dir.newFolderPlaceholder": "Nome da pasta", // Folder name
  "dir.goInput": "Ir ao caminho digitado", // Go to typed path
  "dir.noSubdirs": "(sem subdiretórios)", // (no subdirectories)
  "dir.empty": "(pasta vazia)", // (empty folder)
  "dir.noMatch": "Nenhum item correspondente", // No matching items
  "dir.target": "Pasta de destino", // Target
  "dir.showHidden": "Mostrar itens ocultos", // Show hidden items
  "dir.importing": "Importando…", // Importing…
  "dir.choose": "Escolher este diretório", // Choose This Directory
  "conn.reconnecting": "Conexão perdida, reconectando…", // Connection lost, reconnecting…
  "conn.reconnectNow": "Reconectar agora", // Reconnect now
  "conn.retrying": "Reconectando…", // Reconnecting…
  "conn.sshReconnecting": "Conexão SSH perdida, reconstruindo o túnel…", // SSH link lost, rebuilding the tunnel…
  "conn.sshDown": "A conexão SSH caiu — clique em \"Reconectar agora\" para tentar novamente", // SSH link is down — press Reconnect now to try again
  "reqerr.title": "Falha na solicitação", // Request failed
  "reqerr.dismiss": "Fechar", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "Registro de erros", // Error Log
  "errlog.empty": "Nenhum erro registrado.", // No errors recorded.
  "errlog.copyAll": "Copiar tudo", // Copy all
  "errlog.clear": "Limpar", // Clear
  "errlog.close": "Fechar", // Close

  // ── Mobile ──
  "mobile.toDesktop": "Mudar para a versão desktop", // Switch to desktop
  "mobile.empty1": "Nenhuma sessão.", // No sessions.
  "mobile.noMatch": "Nenhuma sessão correspondente", // No matching sessions
  "mobile.empty2":
    "Crie uma no aplicativo desktop ou no navegador de um computador e ela aparecerá aqui automaticamente.", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ Voltar", // ‹ Back
  "mobile.selCopy": "Copiar", // Copy
  "mobile.selCancel": "Cancelar", // Cancel

  // ── Other shared components ──
  "splitter.dragToResize": "Arraste para redimensionar", // Drag to resize
  "transport.wsDisconnected": "WebSocket desconectado", // WebSocket disconnected
  "transport.wsConnectFailed": "Falha na conexão WebSocket", // WebSocket connection failed
  "transport.cmdFailed": "O comando falhou", // Command failed

  // ── Crepe（editor WYSIWYG）──
  "crepe.placeholder": "Digite texto ou pressione / para o menu de inserção", // Type text, or press / for the insert menu
  "crepe.textGroup": "Texto", // Text
  "crepe.paragraph": "Texto", // Text
  "crepe.h1": "Título 1", // Heading 1
  "crepe.h2": "Título 2", // Heading 2
  "crepe.h3": "Título 3", // Heading 3
  "crepe.h4": "Título 4", // Heading 4
  "crepe.h5": "Título 5", // Heading 5
  "crepe.h6": "Título 6", // Heading 6
  "crepe.quote": "Citação", // Quote
  "crepe.divider": "Divisor", // Divider
  "crepe.listGroup": "Lista", // List
  "crepe.bulletList": "Lista com marcadores", // Bullet List
  "crepe.orderedList": "Lista numerada", // Ordered List
  "crepe.taskList": "Lista de tarefas", // Task List
  "crepe.advancedGroup": "Inserir", // Insert
  "crepe.image": "Imagem", // Image
  "crepe.codeBlock": "Bloco de código", // Code Block
  "crepe.table": "Tabela", // Table
  "crepe.math": "Fórmula", // Math
  "crepe.linkPlaceholder": "Cole ou digite um link…", // Paste or type a link…
  "crepe.upload": "Enviar", // Upload
  "crepe.uploadImage": "Enviar imagem", // Upload Image
  "crepe.orPasteImageLink": "ou cole um link de imagem", // or paste an image link
  "crepe.imageCaption": "Legenda da imagem", // Image caption
  "crepe.confirm": "Confirmar", // Confirm
  "crepe.searchLanguage": "Buscar linguagem", // Search language
  "crepe.noResult": "Sem resultados", // No results
  "crepe.edit": "Editar", // Edit
  "crepe.collapse": "Recolher", // Collapse
  // ── Painel direito / barra inferior ──
  "info.project": "Projeto", // Project
  "panel.sessionInfo": "Informações da sessão", // Session info
  "panel.gitTitle": "Status do Git", // Git status
  "panel.gitProbing": "Verificando…", // Checking…
  "panel.gitNotRepo": "Não é um repositório Git", // Not a Git repository
  "panel.gitBranch": "Branch", // Branch
  "panel.gitStaged": "Preparado", // Staged
  "panel.gitUnstaged": "Modificado", // Changed
  "panel.gitUntracked": "Não rastreado", // Untracked
  "bottombar.running": "Em execução", // Running
  "bottombar.collapseTasks": "Recolher tarefas", // Collapse tasks
  "bottombar.expandTasks": "Expandir tarefas", // Expand tasks
  "bottombar.sound": "🔔 Som", // 🔔 Sound
  "bottombar.muted": "🔕 Mudo", // 🔕 Muted
  "bottombar.overview": "Visão geral das sessões", // Sessions overview
  "bottombar.noSessions": "Nenhuma sessão", // No sessions
  "doc.pdfFilter": "Arquivo PDF", // PDF file
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

export default ptBR;
