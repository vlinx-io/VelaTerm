//! Brazilian Portuguese dictionary. Each entry includes its English source in a trailing review comment; en.ts enforces the complete key set.

import type en from "./en";

const ptBR: typeof en = {
  "tree.newPlanExecuteSession": "Nova sessão de planejamento e execução…",
  "launch.splitTasks": "Dividir automaticamente em várias tarefas",
  "launch.splitTasksHint": "A sessão de planejamento propõe tarefas independentes. Revise as instruções, os agentes, os modelos e o esforço de raciocínio antes de iniciar a execução.",
  "launch.splitReview": "Revisar tarefas de execução",
  "launch.splitReviewHint": "Uma única sessão de planejamento recebe todos os relatórios e revisa cada tarefa separadamente. A execução começa após sua confirmação.",
  "launch.splitConfirmed": "Estas tarefas já foram confirmadas.",
  "launch.splitClosed": "Esta proposta não está mais aguardando confirmação.",
  "launch.splitRetry": "Tentar reenviar as mensagens pendentes",
  "launch.splitSharedDirectory": "Todas as sessões de execução usam o diretório de trabalho da sessão de planejamento e compartilham sua árvore de trabalho quando ela está habilitada.",
  "launch.createIn": "Criar em",
  "launch.workingDirectory": "Caminho do diretório de trabalho",
  "launch.createAndStart": "Criar e iniciar",
  "launch.planExecuteTaskHint": "Descreva a tarefa, os requisitos e os critérios de aceitação para o planejamento.",
  "launch.planExecuteResult": "A sessão de planejamento inicia primeiro e cria a sessão de execução quando o plano está pronto.",
  "launch.planExecuteWorktreeHint": "Os novos worktrees partem do commit atual, sem alterações não commitadas. Se a criação falhar, a sessão correspondente não será iniciada.",
  "launch.workflowDirectorySharedHint": "A sessão de planejamento e todas as sessões de execução compartilham um novo diretório e uma branch.",
  "launch.workflowDirectoryEachHint": "A sessão de planejamento e cada sessão de execução têm seu próprio worktree e sua própria branch.",
  "launch.planTitle": "Planejamento e revisão",
  "launch.execTitle": "Execução",
  "launch.planExecuteIntro": "Uma sessão separada planeja o trabalho, verifica o resultado e solicita correções.",
  "chat.origin.plan": "Planejamento",
  "chat.origin.exec": "Execução",

  // Project code intelligence and knowledge entry associations.
  "knowledge.callers": "Símbolos que chamam",
  "knowledge.callees": "Símbolos chamados",
  "knowledge.explore": "Explorar código",
  "knowledge.exploreHint": "Descreva uma funcionalidade ou um fluxo, ou informe um arquivo ou símbolo…",
  "knowledge.impact": "Análise de impacto",
  "knowledge.path": "Caminho de chamadas",
  "knowledge.target": "Buscar um símbolo de destino…",
  "knowledge.depth": "Profundidade da busca",
  "knowledge.noPath": "Nenhum caminho de chamadas direcionado foi encontrado no índice.",
  "knowledge.watching": "Sincronização automática ativa",
  "knowledge.onDemand": "Sincronizar antes das consultas",
  "knowledge.overview": "Visão geral",
  "knowledge.uncertain": "Relação inferida",
  "knowledge.kind": "Tipo de símbolo",
  "knowledge.language": "Linguagem",
  "knowledge.results": "Resultados",
  "knowledge.resultLarge": "O resultado é grande demais para exibição. Restrinja a consulta ou reduza a profundidade da busca.",
  "knowledge.queryFailed": "A consulta de código falhou. Tente novamente ou sincronize o índice.",
  "knowledge.liveHelp": "As alterações são sincronizadas enquanto o processo de consultas está ativo. Após encerrar por inatividade, a próxima consulta incorpora as alterações pendentes.",
  "knowledge.startHelp": "Ative a indexação para buscar código, seguir chamadas e analisar o impacto de uma alteração. A análise é executada no servidor sem um modelo de IA.",
  "knowledge.title": "Grafo de código",
  "knowledge.intro": "Explore as relações do código e vincule-as às decisões de projeto salvas.",
  "knowledge.setup": "Instale o CodeGraph neste servidor para ativar a indexação dos projetos.",
  "knowledge.downloadNotice": "Baixa o ambiente de execução verificado do CodeGraph pelo GitHub. A indexação ocorre nesta máquina; a telemetria e a verificação de atualizações ficam desativadas.",
  "knowledge.install": "Baixar CodeGraph",
  "knowledge.installing": "Baixando e instalando…",
  "knowledge.directory": "Diretório de trabalho",
  "knowledge.enable": "Ativar indexação",
  "knowledge.disable": "Desativar indexação",
  "knowledge.sync": "Sincronizar",
  "knowledge.ready": "Pronto",
  "knowledge.disabled": "Desativado",
  "knowledge.indexing": "Indexando…",
  "knowledge.syncing": "Sincronizando…",
  "knowledge.failed": "Falha",
  "knowledge.symbols": "Símbolos",
  "knowledge.files": "Arquivos",
  "knowledge.edges": "Relações",
  "knowledge.search": "Buscar símbolos ou caminhos de arquivos…",
  "knowledge.searchButton": "Buscar",
  "knowledge.noResults": "Nenhum símbolo correspondente.",
  "knowledge.selectSymbol": "Selecione um símbolo para ver o código-fonte, as relações e os artigos vinculados.",
  "knowledge.source": "Código-fonte",
  "knowledge.incoming": "Relações de entrada",
  "knowledge.outgoing": "Relações de saída",
  "knowledge.noEdges": "Nenhuma relação indexada.",
  "knowledge.analysisNote": "As relações vêm de análise estática e podem ser incompletas ou incertas.",
  "knowledge.changed": "O arquivo mudou durante a consulta. Sincronize novamente antes de usar os números de linha ou confirmar a revisão.",
  "knowledge.truncated": "Esta visualização é limitada. Algumas relações ou linhas de código foram omitidas.",
  "knowledge.linkMemory": "Vincular artigo de conhecimento",
  "knowledge.chooseMemory": "Escolher um artigo de conhecimento",
  "knowledge.noLinks": "Ainda não há vínculos com o código. Você pode vincular um artigo nos detalhes de um símbolo.",
  "knowledge.inspect": "Revisar código e artigo",
  "knowledge.unlink": "Remover vínculo",
  "knowledge.codeReferences": "Referências ao código",
  "knowledge.refresh": "Atualizar",
  "knowledge.current": "Sem alterações",
  "knowledge.review": "Requer revisão",
  "knowledge.unavailable": "Indisponível",
  "knowledge.reviewHelp": "Compare este artigo com o código exibido. A confirmação registra a versão atual do arquivo sem alterar o texto do artigo.",
  "knowledge.confirmReview": "Confirmar revisão",
  "knowledge.agentHint": "Os agentes podem executar vkb search \"tema\" neste diretório. As consultas sincronizam os índices ativos e retornam o código e os artigos de conhecimento separadamente.",
  "knowledge.busy": "Uma tarefa de indexação está em andamento. Você pode fechar esta página ou desativar a indexação para interrompê-la.",
  "knowledge.disabledHelp": "Ative a indexação deste diretório para consultar o código. A desativação preserva o índice e os vínculos com os artigos.",
  "knowledge.conflict": "O código ou o artigo mudou. Recarregue ambos antes de salvar este vínculo.",
  "knowledge.symbolMissing": "O símbolo ou o código-fonte não está mais disponível. Sincronize e faça a busca novamente.",
  "knowledge.directoryMissing": "Este diretório de trabalho não existe ou mudou. Verifique os caminhos do projeto e da sessão.",
  "knowledge.partial": "O índice está incompleto. Sincronize novamente e verifique se os arquivos de código-fonte podem ser lidos.",
  "knowledge.interrupted": "A tarefa anterior foi interrompida. Sincronize para tentar novamente.",
  "knowledge.checksum": "A soma de verificação do download não corresponde. O ambiente de execução não foi instalado.",
  "knowledge.downloadFailed": "Não foi possível baixar o CodeGraph. Verifique a conexão do servidor com o GitHub e tente novamente.",
  "knowledge.timeout": "A indexação excedeu o tempo limite. Verifique o tamanho do repositório e tente novamente.",
  "knowledge.error": "A operação falhou. Verifique o acesso aos diretórios e o ambiente de execução do servidor e tente novamente.",

  // Knowledge base: saved knowledge organized by project and session.
  "memory.hierarchy": "Projetos e sessões",
  "memory.up": "Subir um nível",
  "memory.manualGroup": "Criação manual",
  "memory.legacyGroup": "Entradas mescladas anteriormente",
  "memory.unknownProject": "Projeto de origem desconhecido",
  "nb.addLink": "Inserir link",
  "nb.attach": "Anexar arquivo",
  "nb.browse": "Procurar",
  "nb.chooseNote": "Comece com uma nota",
  "nb.closeHint": "Remover este caderno da lista. Os arquivos permanecerão no disco.",
  "nb.closeVault": "Fechar caderno",
  "nb.conflict": "O arquivo foi alterado fora deste editor. Seu rascunho foi preservado. Recarregue o arquivo ou salve o rascunho como uma nova nota.",
  "nb.copyTo": "Copiar para um caderno local",
  "nb.createVault": "Criar base de conhecimento",
  "nb.destination": "Caminho de destino",
  "nb.download": "Baixar",
  "nb.downloadHint": "Baixe este anexo para abri-lo em outro aplicativo.",
  "nb.empty": "Abra uma pasta para começar a escrever ou crie um caderno.",
  "nb.emptyImport": "Nenhum arquivo que possa ser importado foi selecionado.",
  "nb.emptyNotes": "As notas são salvas como arquivos Markdown.",
  "nb.emptyOutline": "Os títulos do documento aparecerão aqui.",
  "nb.emptyTrash": "A lixeira está vazia.",
  "nb.error": "Não foi possível acessar o caderno. Verifique a conexão e a pasta e tente novamente.",
  "nb.exists": "O destino já existe. Escolha outro nome ou pasta.",
  "nb.favorites": "Favoritos",
  "nb.files": "Arquivos",
  "nb.folder": "Pasta",
  "nb.generatedHint": "Conhecimento organizado a partir das suas sessões, com fontes e histórico de revisões.",
  "nb.homeHint": "Explore a base de conhecimento de sessões e as bases de conhecimento locais.",
  "nb.homeSearch": "Pesquise no conhecimento das sessões e nas notas locais…",
  "nb.loadMore": "Carregar mais",
  "nb.import": "Importar",
  "nb.importFiles": "Escolher arquivos",
  "nb.importFolder": "Escolher pasta",
  "nb.importHint": "Os arquivos são copiados para a pasta selecionada. Arquivos existentes não são sobrescritos; pastas de configuração ocultas são ignoradas.",
  "nb.imported": "Importados",
  "nb.imports": "Histórico de importações",
  "nb.importsEmpty": "Ainda não há importações.",
  "nb.importRoot": "Raiz da base de conhecimento",
  "nb.importBusy": "Já existe uma importação em andamento para esta base de conhecimento.",
  "nb.importDelete": "Excluir registro",
  "nb.importDeleteConfirm": "Excluir este registro de importação? Os arquivos já importados não são removidos.",
  "nb.importDone": "Importação concluída",
  "nb.importDuration": (seconds: string) => `${seconds} s`,
  "nb.importFailed": "Falha na importação",
  "nb.importFilePending": "Não importado",
  "nb.importHideFiles": "Ocultar arquivos",
  "nb.importInterruptedHint": "A importação foi interrompida antes de terminar.",
  "nb.importProgress": (done: string, total: string) => `${done} / ${total} arquivos`,
  "nb.importShowFiles": (count: string) => `Arquivos (${count})`,
  "nb.importSkipHidden": "Arquivo ou pasta ocultos",
  "nb.importStatusCancelled": "Cancelada",
  "nb.importStatusCompleted": "Concluída",
  "nb.importStatusFailed": "Falhou",
  "nb.importStatusInterrupted": "Interrompida",
  "nb.importStatusRunning": "Importando",
  "nb.incomplete": "Não foi possível concluir a operação. Verifique os arquivos e tente novamente.",
  "nb.info": "Detalhes da nota",
  "nb.invalid": "O nome ou caminho é inválido.",
  "nb.links": "Links de saída",
  "nb.local": "Arquivos locais",
  "nb.localVaults": "Bases de conhecimento locais",
  "nb.move": "Renomear ou mover",
  "nb.moveHint": "Informe um caminho relativo à raiz do caderno. Os links existentes são atualizados ao mover o arquivo ou a pasta.",
  "nb.name": "Nome",
  "nb.newFolder": "Nova pasta",
  "nb.newNote": "Nova nota",
  "nb.noLinks": "Ainda não há notas vinculadas.",
  "nb.tags": "Marcadores",
  "nb.notes": "Notas",
  "nb.openVault": "Abrir base de conhecimento",
  "nb.outline": "Estrutura",
  "nb.quickOpen": "Abertura rápida",
  "nb.readOnly": "Este arquivo não pode ser editado como uma nota Markdown em UTF-8.",
  "nb.recent": "Notas recentes",
  "nb.restore": "Restaurar",
  "nb.reload": "Recarregar do disco",
  "nb.root": "Caminho da pasta",
  "nb.rootHint": "Selecione uma pasta no computador conectado. Os arquivos Markdown e anexos existentes permanecem no local original.",
  "nb.saveCopy": "Salvar como nova nota",
  "nb.saved": "Salvo no disco",
  "nb.saving": "Salvando…",
  "nb.search": "Pesquisar notas…",
  "nb.searchAllVaults": "Todas as bases de conhecimento",
  "nb.searchCount": (count: string) => `${count} resultados`,
  "nb.searchEmpty": "Nenhuma nota corresponde a esta busca.",
  "nb.searchEmptyAll": "Nada corresponde a esta busca.",
  "nb.searchFuzzy": "Nenhuma correspondência exata. Exibindo resultados aproximados.",
  "nb.searchLine": (line: string) => `Linha ${line}`,
  "nb.searchMatches": (count: string) => `${count} ocorrências`,
  "nb.searchMore": "Apenas os primeiros resultados são listados. Refine a busca para ver os demais.",
  "nb.searchRelated": "Notas relacionadas",
  "nb.searchResults": "Resultados da pesquisa",
  "nb.searchScope": "Escopo da busca",
  "nb.searchThisVault": "Esta base de conhecimento",
  "nb.skipped": "Ignorados",
  "nb.split": "Visualização dividida",
  "nb.tooLarge": "O arquivo ou a seleção excede os limites do caderno.",
  "nb.trash": "Lixeira",
  "nb.trashHint": "Mover este item para a lixeira do caderno. Ele poderá ser restaurado depois.",
  "nb.unsaved": "Alterações não salvas",
  "nb.vaults": "Bases de conhecimento",
  "nb.view": "Modo de visualização",
  "nb.welcome": "Seus cadernos",
  "nb.welcomeText": "Escreva livremente, conecte ideias e mantenha suas notas em arquivos locais comuns. Abra uma pasta Markdown existente ou importe documentos para um novo caderno.",
  "memory.globalMemory": "Base de conhecimento de sessões",
  "memory.collections": "Sessões arquivadas",
  "memory.collectionConversation": "Conversa",
  "memory.collectionEmptyEntries": "Esta conversa ainda não tem artigos de conhecimento.",
  "memory.title": "Base de conhecimento",
  "memory.add": "Organizar na base de conhecimento de sessões",
  "memory.intro": "Organize o conhecimento por projeto e sessão. Os artigos salvos permanecem independentes das fontes e podem ser editados manualmente.",
  "memory.entries": "Artigos de conhecimento",
  "memory.emptyJobs": "Ainda não há registros de organização.",
  "memory.jobs": "Histórico de organização",
  "memory.search": "Pesquisar títulos e conteúdo…",
  "memory.empty": "Nenhum artigo correspondente. Gere artigos a partir de uma sessão ou crie um manualmente.",
  "memory.emptyDetail": "Selecione um artigo para consultar o conteúdo, as relações e as fontes.",
  "memory.new": "Novo artigo",
  "memory.titleField": "Título",
  "memory.summary": "Resumo",
  "memory.content": "Conteúdo (Markdown)",
  "memory.tags": "Etiquetas (separadas por vírgulas)",
  "memory.related": "Artigos relacionados",
  "memory.backlinks": "Links para este artigo",
  "memory.sources": "Fontes",
  "memory.history": "Histórico de revisões",
  "memory.restore": "Restaurar esta revisão",
  "memory.restoreConfirm": "Restaurar esta revisão como uma nova versão? A versão atual será mantida no histórico.",
  "memory.deleteConfirm": "Excluir este artigo e seu histórico? As sessões de origem serão mantidas.",
  "memory.groupDeleteConfirm": (count: string) => `Excluir todos os ${count} artigos de conhecimento deste grupo? O projeto ou a sessão em si serão mantidos.`,
  "memory.export": "Exportar Markdown",
  "memory.selectAgent": "Agente",
  "memory.model": "Modelo (opcional)",
  "memory.modelHint": "Deixe em branco para usar o modelo configurado no agente.",
  "memory.compile": "Organizar e salvar",
  "memory.compileHelp": "O agente selecionado processará esta sessão. Ao gerar novamente, as entradas geradas anteriormente para esta sessão serão substituídas, incluindo as edições manuais. O texto da sessão será enviado ao modelo pelo agente configurado.",
  "memory.unavailable": "Não instalado ou configurado",
  "memory.allTags": "Todas as etiquetas",
  "memory.updated": "Atualização recente",
  "memory.titleSort": "Título",
  "memory.sourceNote": "Este instantâneo preserva o texto usado na organização, mesmo que a sessão original seja excluída.",
  "memory.noKnowledge": "Nenhum conhecimento reutilizável foi extraído. As entradas geradas anteriormente para esta sessão foram removidas.",
  "memory.queued": "Aguardando início",
  "memory.cancelling": "Cancelando",
  "memory.schedulingHint": "Sessões diferentes podem ser processadas em paralelo. Ao enviar novamente, qualquer tarefa ainda não concluída desta sessão é cancelada e substituída pela nova.",
  "memory.waitingHint": "Esta tarefa começará automaticamente quando a anterior desta sessão tiver parado.",
  "memory.running": "Em andamento",
  "memory.completed": "Concluído",
  "memory.failed": "Falha",
  "memory.cancelled": "Cancelado",
  "memory.extract": "Extraindo temas",
  "memory.merge": "Integrando conhecimentos",
  "memory.commit": "Salvando artigos",
  "memory.done": "Salvo",
  "memory.closeHint": "Você pode fechar esta janela durante o processamento e acompanhar o progresso no histórico de organização.",
  "memory.conflict": "Este artigo mudou durante a operação. Recarregue-o antes de tentar novamente; suas alterações não foram salvas.",
  "memory.duplicate": "Já existe um artigo com este título. Abra-o para integrar o conteúdo.",
  "memory.notFound": "Este artigo, fonte ou tarefa não existe mais.",
  "memory.noTranscript": "Esta sessão não tem uma conversa disponível para leitura.",
  "memory.agentUnavailable": "O agente selecionado está indisponível. Confira o caminho do executável nas configurações.",
  "memory.invalid": "Alguns campos ou links são inválidos. Confira o título, o conteúdo e os artigos relacionados.",
  "memory.processFailed": "O agente não conseguiu concluir. Confira o login, o modelo e as configurações da CLI e tente novamente.",
  "memory.timeout": "O tempo limite do agente foi excedido. Tente um modelo disponível ou uma conversa mais curta.",
  "memory.interrupted": "O processamento foi interrompido. Tente novamente com o instantâneo de origem salvo.",
  "memory.tooLarge": "A fonte, o contexto ou a saída excede o tamanho compatível. Nenhum conteúdo foi truncado ou salvo.",
  "memory.invalidOutput": "O agente retornou dados estruturados inválidos. Nada foi salvo; tente novamente ou escolha outro agente.",
  "memory.loadError": "Não foi possível carregar a base de conhecimento. Confira a conexão e tente novamente.",
  "memory.unsaved": "Descartar alterações não salvas?",
  "memory.source": "Instantâneo de origem",

  // ── Common ──
  "common.cancel": "Cancelar", // Cancel
  "common.confirm": "OK", // OK
  "common.delete": "Excluir", // Delete
  "common.save": "Salvar", // Save
  "common.create": "Criar", // Create
  "common.close": "Fechar", // Close
  "chat.imageViewOriginal": "Ver imagem original",
  "chat.imageCopy": "Copiar imagem",
  "chat.imageSave": "Salvar imagem",
  "chat.imageActionFailed": "Não foi possível concluir a operação com a imagem. Tente novamente.",
  "common.copy": "Copiar", // Copy
  "common.cut": "Recortar", // Cut
  "common.paste": "Colar", // Paste
  "common.selectAll": "Selecionar tudo", // Select All
  "common.copied": "Copiado", // Copied
  "common.copyFailed": "Não foi possível copiar. Tente novamente.",
  "chat.sync.loading": "Sincronizando conversa…",
  "chat.sync.failed": "Não foi possível sincronizar. As mensagens carregadas continuam disponíveis.",
  "chat.sync.history": "Carregar mensagens anteriores",
  "chat.submission.updateRequired": "Atualize o servidor antes de enviar mensagens por este cliente.",
  "chat.submission.sending": "Enviando…",
  "chat.submission.sent": "Enviado",
  "chat.submission.queued": "Na fila",
  "chat.submission.failed": "Falha no envio",
  "chat.submission.unknown": "Entrega não confirmada",
  "chat.submission.check": "Verificar status",
  "common.retry": "Tentar novamente", // Retry
  "common.experimental": "Experimental",
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
  "titlebar.themeSystem": (resolved) =>
    `Seguir o sistema (atualmente ${resolved})`, // Follow system (currently {resolved})
  "titlebar.themeDark": "Escuro", // Dark
  "titlebar.themeLight": "Claro", // Light
  "titlebar.gameCenter": "Central de jogos",
  "titlebar.browser": "Navegador integrado", // Built-in Browser
  "titlebar.remoteAccess": "Acesso remoto (navegador)", // Remote Access (Browser)
  "titlebar.connectRemote": "Conectar a servidor remoto", // Connect to Remote Server
  "titlebar.mirrored": "Espelhado", // Mirrored
  "titlebar.mirroredHint":
    "O espelhamento está ativado: abas, divisões e a sessão ativa acompanham o host. O interruptor fica no host.", // Mirroring is on: tabs, splits, and the active session follow the host. The switch is on the host.
  "titlebar.mirroredBy": (n: number) => `Espelhado por ${n}`, // Mirrored by {n}
  "titlebar.mirroredByHint": (n: number) =>
    `${n} cliente${n === 1 ? "" : "s"} remoto${n === 1 ? " está conectado" : "s estão conectados"}. Abas, divisões e a sessão ativa são compartilhadas, e qualquer lado pode reorganizá-las.`, // {n} remote clients are connected. Tabs, splits, and the active session are shared, and either side can rearrange them.
  "titlebar.clientsTitle": "Clientes conectados", // Attached clients
  "titlebar.clientUnnamed": "Cliente sem nome", // Unnamed client
  "titlebar.clientSince": (time: string) => `desde ${time}`, // since {time}
  "titlebar.feedback": "Feedback", // Feedback
  "titlebar.share": "Compartilhar", // Share
  // ── Alt-triggered menu bar (Windows/Linux) ──
  "menubar.file": "Arquivo", // File
  "menubar.terminal": "Terminal",
  "menubar.help": "Ajuda", // Help
  "menubar.newTerminal": "Novo terminal", // New Terminal
  "menubar.visitWebsite": "Visitar o site", // Visit Website
  "menubar.sendFeedback": "Enviar feedback", // Send Feedback
  "menubar.clearBadges": "Limpar marcadores de notificação", // Clear Notification Badges
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
  "settings.agentDefaultsTitle": "Padrões de novas sessões",
  "settings.referSummaryTitle": "Contexto de referências de sessão",
  "settings.referSummaryMode": "Modo de contexto",
  "settings.referSummaryFull": "Transcrição completa",
  "settings.referSummaryFirst": "Resumir primeiro",
  "settings.referSummaryAgent": "Agente de resumo",
  "settings.referSummaryHint":
    "Por padrão, o vrefer --ask envia a transcrição completa ao agente que responde. “Resumir primeiro” a compacta com o único agente, modelo e nível de raciocínio escolhidos aqui; a resposta final também recebe trechos relevantes do texto original.",
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
  "settings.cliHint":
    "Adiciona `vela <caminho-do-projeto>` ao PATH, como o comando `code` do VS Code.",
  "settings.agentArgsHint":
    "Argumentos de inicialização padrão aplicados a novas sessões de cada tipo de agente. Argumentos por sessão definidos ao criar ou editar têm prioridade. Deixe em branco para nenhum.", // Agent default launch args hint
  "settings.agentPathLabel": "Caminho do executável (opcional)", // Executable path (optional)
  "settings.agentPathPlaceholder":
    "ex.: ~/.local/bin/claude — vazio = buscar no PATH", // e.g. path — empty = find on PATH
  "settings.agentPathHint":
    "Quando definido, sessões deste tipo iniciam por este caminho completo em vez de procurar o comando no PATH. Útil quando o agente está instalado mas fora do PATH do shell. Preenchido automaticamente após uma instalação em um clique quando o local é detectado.", // Agent executable path hint
  "settings.agentDefaultView": "Visão padrão", // Default view
  "settings.agentDefaultViewHint":
    "Visão em que as novas sessões deste agente são abertas. As sessões existentes mantêm a visão com que foram criadas.", // Agent default view hint
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
  "settings.usageAuto": "Usage auto-refresh", // Usage auto-refresh
  "settings.usageRefresh": "Usage refresh", // Usage refresh
  "settings.autoContinue": "Continuar após a redefinição", // Continue after limit resets
  "settings.autoContinueHint": "Quando um limite de uso de 5 horas ou semanal interrompe o Claude ou o Codex, a tarefa continua automaticamente após a redefinição do limite.", // When a 5-hour or weekly usage limit stops Claude or Codex, the task continues automatically after the limit resets.
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
  "spawn.title": "Iniciar sessão filha",
  "spawn.fromSession": "Sessão solicitante",
  "spawn.promptLabel": "Instruções da tarefa",
  "spawn.agentLabel": "Tipo de sessão",
  "spawn.worktreeLabel": "Worktree separado",
  "spawn.modelLabel": "Modelo",
  "spawn.effortLabel": "Esforço de raciocínio",
  "spawn.modelDefault": "Padrão do agente",
  "spawn.modelLoading": "Carregando modelos…",
  "spawn.modelListUnavailable": "Lista indisponível. Você pode inserir um identificador.",
  "spawn.launch": "Iniciar sessão",
  "spawn.remaining": (n: number) => `Mais ${n} solicitações para revisar`,
  "spawn.notifyTitle": "Sessão filha aguardando confirmação",
  "orch.title": "Iniciar várias sessões",
  "orch.notifyTitle": "Início de sessões aguardando confirmação",
  "orch.coordinatorName": "Estado das sessões",
  "orch.sharedSettings": "Configurações comuns",
  "orch.agentLabel": "Agente",
  "orch.modelLabel": "Modelo",
  "orch.effortLabel": "Esforço de raciocínio",
  "orch.nameLabel": "Nome da sessão",
  "orch.promptLabel": "Instruções da tarefa",
  "orch.worktreeLabel": "Worktree do Git",
  "orch.worktreeNone": "Diretório atual",
  "orch.worktreeShared": "Worktree compartilhado",
  "orch.worktreeEach": "Um worktree por sessão",
  "orch.follow": "Usar configurações comuns",
  "orch.overridden": "Configurações individuais",
  "orch.remove": "Remover tarefa",
  "orch.launch": (n: number) => `Iniciar ${n} sessões`,
  "orch.modelPlaceholder": "Padrão do agente",
  "orch.effortPlaceholder": "Padrão do agente",
  "launch.terminalHint": "Um terminal comum abre o diretório de trabalho. As instruções da tarefa não são executadas automaticamente.",
  "launch.optionsError": "Não foi possível carregar as opções. Tente novamente antes de iniciar.",
  "launch.singleIntro": "Revise a tarefa e as configurações antes de iniciar uma sessão filha.",
  "launch.taskHint": "Estas instruções serão a primeira mensagem enviada à sessão filha.",
  "launch.runtime": "Configurações de execução",
  "launch.directory": "Diretório de trabalho",
  "launch.directoryCurrentHint": "As sessões editam arquivos no diretório original.",
  "launch.directorySharedHint": "Todas as sessões usam o mesmo novo diretório e a mesma branch.",
  "launch.directoryEachHint": "Cada sessão tem seu próprio diretório e sua própria branch.",
  "launch.worktreeHint": "Os worktrees partem do commit atual, sem alterações não commitadas. Se a criação falhar, será usado o diretório original.",
  "launch.singleResult": "A sessão filha aparece abaixo da sessão de origem na barra lateral.",
  "launch.startError": "Não foi possível iniciar. Verifique as configurações e tente novamente.",
  "launch.starting": "Iniciando…",
  "launch.batchIntro": "Revise as configurações comuns e selecione cada tarefa para editar suas instruções.",
  "launch.sessionCount": (n: number) => `Sessões: ${n}`,
  "launch.batchName": "Nome do grupo de tarefas",
  "launch.sharedHint": "Aplica-se às sessões sem configurações individuais.",
  "launch.tasks": "Tarefas",
  "launch.incomplete": "Faltam informações",
  "launch.undoRemove": "Desfazer remoção",
  "launch.taskNumber": (n: number) => `Tarefa ${n}`,
  "launch.taskSettings": "Configurações desta sessão",
  "launch.taskAgent": "Agente desta sessão",
  "launch.sharedDirectoryLocked": "Todas as sessões deste grupo usam um único worktree compartilhado.",
  "launch.resetSettings": "Restaurar configurações comuns",
  "launch.monitorHint": "O terminal “Estado das sessões” mostra quais sessões estão trabalhando ou aguardando entrada. Ele não indica a porcentagem de conclusão das tarefas.",
  "launch.taskIncomplete": (n: number) => `Preencha o nome e as instruções da tarefa ${n}.`,
  "launch.batchResult": "Cada tarefa inicia uma sessão interativa independente.",
  "tree.worktreeMenu": "Worktree",
  "tree.gitMenu": "Git",
  "tree.viewChanges": "Ver alterações…",
  "changes.title": "Alterações",
  "changes.loading": "Carregando…",
  "changes.loadingDiff": "Carregando diff…",
  "changes.noChanges": "Sem alterações",
  "changes.refresh": "Atualizar",
  "changes.notRepo": "Não é um repositório git",
  "changes.selectFile": "Selecione um arquivo",
  "changes.binary": "Arquivo binário — diff por linha indisponível",
  "changes.commitTitle": (hash: string) => `Commit ${hash}`,

  "git.staged": "Preparado",
  "git.changes": "Alterações",
  "git.untracked": "Arquivos não rastreados",
  "git.committed": "Alterações commitadas",
  "git.stage": "Preparar",
  "git.unstage": "Remover do preparo",
  "git.stageAll": "Preparar tudo",
  "git.unstageAll": "Remover tudo do preparo",
  "git.discard": "Descartar",
  "git.deleteFile": "Excluir",
  "git.viewAll": "Ver tudo",
  "git.detached": "(destacado)",
  "git.aheadBehind": "Commits à frente e atrás do branch upstream",
  "git.commitPlaceholder": "Mensagem do commit",
  "git.amend": "Corrigir o último commit",
  "git.amendCommit": "Corrigir commit",
  "git.commitCount": (n: number) =>
    n === 1 ? "Commitar 1 arquivo" : `Commitar ${n} arquivos`,
  "git.commitNoFiles": "Este commit não altera arquivos",
  "git.noCommits": "Ainda não há commits",
  "git.loadMore": "Carregar mais",
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
  "tree.moveGroupToWorktree": "Mover para um worktree…",
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
  "settings.termLineHeight": "Altura da linha do terminal",
  "settings.chatTypography": "Visualização da conversa",
  "settings.chatTypographyHint": "Estas configurações de fonte são independentes do terminal e entram em vigor imediatamente.",
  "settings.chatFont": "Fonte da conversa",
  "settings.chatFontSize": "Tamanho da fonte da conversa",
  "settings.chatLineHeight": "Altura da linha da conversa",
  "settings.fontDefault": "Default", // TODO translate
  "settings.fontCustom": "Custom…", // TODO translate
  "settings.fontListUnavailable": "Não foi possível obter a lista de fontes do sistema. Você pode digitar o nome de uma fonte manualmente.",
  "settings.fontUnconfirmed": "Não foi possível confirmar se esta fonte está disponível.",
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
  "settings.notifyUnsupported":
    "Notifications aren't available in this environment.", // TODO translate
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
  "settings.scHint":
    "Clique em um atalho e pressione uma nova combinação (Cmd/Ctrl obrigatório).", // hint
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
  "remote.ipLabel": "IP", // IP address
  "remote.ipAuto": "Automático (primeiro endereço LAN)", // Automatic (first LAN address)
  "remote.ipVpn": "VPN", // VPN
  "remote.qrHint":
    "Escaneie com o celular para abrir o link de pareamento no endereço selecionado.", // Scan with your phone to open the pairing link on the selected address.
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
  "remote.autoRestartHint":
    'O acesso remoto reinicia automaticamente ao reabrir o aplicativo. "Parar servidor" desativa isso.', // Remote access restarts automatically when the app is reopened. Stop Server turns this off.
  "remote.autostartFailed": "Falha no início automático:", // Automatic start failed:
  "remote.mirror": "Espelhar o layout em todos os dispositivos", // Mirror layout across devices
  "remote.mirrorHint":
    "Abas, divisões e a sessão ativa ficam iguais em todos os dispositivos conectados. O foco do teclado permanece onde está em cada um.", // Tabs, splits, and the active session stay the same on every connected device. Keyboard focus stays put on each one.

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
  "connect.sshFingerprintLabel": (kt: string) =>
    `Impressão digital da chave do host SSH (${kt})`,
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
  "connect.showPassword": "Mostrar senha",
  "connect.hidePassword": "Ocultar senha",
  "connect.urlPasswordPlaceholder": "Senha de login",
  "connect.mirror": "Espelhar o app de desktop remoto", // Mirror the remote desktop app
  "connect.mirrorHint":
    "Abas, divisões e a sessão ativa ficam iguais às do app de desktop na máquina remota; alterações de qualquer lado aparecem em ambos. Se o app de desktop não estiver em execução, esta conexão abre diretamente o banco de dados dele, ou um banco de dados separado se não houver nenhum.", // Same tabs, splits, and active session as the desktop app on the remote machine; changes on either side show on both. If the desktop app is not running, this connection opens its database directly, or a separate database when there is none.
  "connect.shareDesktopDb": "Usar o banco de dados do app de desktop remoto",
  "connect.shareDesktopDbHint":
    "Compartilha um banco de dados com o app de desktop da máquina remota (melhor quando ambos têm a mesma versão). Desativado = banco de dados isolado.",

  // ── Sidebar ──
  "tree.newSession": "Nova sessão", // New Session
  "tree.newTerminalSession": "Novo terminal", // New Terminal
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
  "tree.collectionInfo": "Informações da coleção", // Collection Info
  "tree.projectInfo": "Informações do projeto", // Project Info
  "info.branch": "Ramo", // Branch
  "info.path": "Caminho", // Path
  "info.recentCommits": "Commits recentes", // Recent Commits
  "info.noCommits": "Sem commits", // No commits
  "tree.killProcess": "Encerrar processo", // Kill Process
  "tree.killProcessConfirm": (name: string) => `Encerrar o processo de “${name}”? A tarefa atual será interrompida. O histórico da conversa e os arquivos salvos serão mantidos.`,
  "tree.archiveSession": "Arquivar sessão", // Archive Session
  "tree.archiveGroup": "Arquivar grupo", // Archive Group
  // Temporary (draft) sessions
  "tree.scratchTag": "temp", // scratch
  "tree.persistSession": "Tornar sessão permanente…", // Make Permanent Session…
  "tree.persistDoc": "Salvar no disco…", // Save to Disk…
  "tree.closeScratch": "Fechar rascunho", // Close Scratch
  "tree.importProject": "Importar projeto", // Import Project
  "tree.createProject": "Criar projeto",
  // New Collection / Collection name / research / Create Collection / No folder / Delete Collection
  "tree.newCollection": "Nova coleção",
  "tree.deleteCollection": "Excluir coleção",
  "collection.title": "Nova coleção",
  "collection.name": "Nome da coleção",
  "collection.namePlaceholder": "research",
  "collection.submit": "Criar coleção",
  "collection.tag": "Sem pasta",
  "collection.deleteTitle": "Excluir coleção",
  "collection.deleteBody": (name) =>
    `Excluir a coleção "${name}"? Todos os seus grupos e sessões também serão excluídos. Isso não pode ser desfeito.`,
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
  "clone.slowHint":
    "Sem progresso há 30 segundos. Verifique a rede ou o proxy da máquina remota; você pode cancelar e tentar novamente.",
  "clone.submit": "Clonar", // Clone
  "tree.globalSearch": "Pesquisar em todas as sessões", // Search All Sessions
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
  "tree.noProjectsPre":
    "Nenhum projeto ainda. Clique no ícone de pasta ou pressione ", // No projects yet. Click the folder button, or press
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
  "tree.engineLabel": "Abre em",
  "tree.engineTui": "Visão de terminal",
  "tree.engineChat": "Visão de conversa",
  // The agent runs its own terminal interface.
  "tree.engineTuiHint": "O agente usa a própria interface de terminal.",
  // Messages and tool cards, with buttons for permission questions.
  "tree.engineChatHint": "Apresentação com mensagens e cartões de ferramentas; as solicitações de permissão são respondidas na interface.",
  "tree.agentArgsLabel": "Argumentos de inicialização (opcional)", // Launch args (optional)
  // Working directory / Leave empty for the default
  "tree.workingDirLabel": "Diretório de trabalho",
  "tree.workingDirPlaceholder": "Deixe vazio para usar o padrão",
  "preset.execPathLabel": "Executável (opcional)",
  "preset.execPathPlaceholder": "/usr/local/bin/claude",
  "preset.execPathHint":
    "Deixe vazio para usar o comando configurado do agente. Informe para que apenas esta sessão use um substituto compatível.",
  "preset.saveLabel": "Salvar como predefinição",
  "preset.namePlaceholder": "Nomeie esta predefinição",
  "preset.iconChoose": "Escolher ícone",
  "preset.iconClear": "Remover",
  "preset.iconHint":
    "Imagens quadradas funcionam melhor; as demais são cortadas e reduzidas para 64x64.",
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
  "info.notYetCaptured":
    "Ainda não gerado (capturado após a primeira execução)", // Not yet generated (captured after first run)
  "info.sessionId": "ID da sessão", // Session ID
  "info.projectId": "ID do projeto", // Project ID
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
  "importSessions.results": ({ count }: { count: number }) => `Resultados: ${count}`,
  "importSessions.selected": ({ count }: { count: number }) => `Selecionadas: ${count}`,
  "importSessions.clearSelection": "Limpar seleção",
  "importSessions.clearSearch": "Limpar busca",
  "importSessions.noHistory": "Nenhuma sessão anterior foi encontrada para o diretório deste projeto.",
  "importSessions.title": "Importar sessões",
  "importSessions.description": "Encontre sessões existentes do Codex, Claude e OpenCode cujo diretório de trabalho corresponda a este projeto. Selecione as sessões para adicioná-las ao projeto e abra uma delas para continuar a conversa.",
  "importSessions.search": "Pesquisar por título, agente ou ID da sessão",
  "importSessions.empty": "Nenhuma sessão correspondente encontrada.",
  "importSessions.imported": "Já importada",
  "importSessions.confirm": ({ count }: { count: number }) => `Importar (${count})`,
  "importSessions.success": ({ count }: { count: number }) => `Sessões adicionadas ao projeto: ${count}.`,
  "resume.title": "Retomar sessão", // Resume Session
  "resume.desc":
    "Escolha o tipo de agente e informe o session id próprio do agente; ao abrir, a conversa original é retomada.", // Pick the agent type and enter the agent's own session id…
  "resume.agentType": "Tipo de agente", // Agent type
  "resume.sessionIdPlaceholder": "Session id da conversa", // Conversation session id
  "resume.confirm": "Retomar e abrir", // Resume & Open

  // New worktree-session dialog
  "tree.newWorktreeSession": "Nova sessão de worktree…", // New Worktree Session…
  "worktree.worktreeNameLabel": "Nome do worktree", // Worktree name
  "worktree.worktreeNameHint":
    "Usado como nome do diretório e do branch do worktree.", // Used as the worktree directory and branch name.
  "worktree.createFailed": "Não foi possível criar o worktree", // Couldn't create the worktree
  "worktree.noRepoRoot":
    "Este projeto não tem um caminho de repositório git utilizável.", // This project has no usable git repository path.
  // ── Worktree selector for custom session creation ──
  "worktreeSel.label": "Worktree",
  "worktreeSel.modeNone": "Nenhum", // None
  "worktreeSel.modeNew": "Novo", // New
  "worktreeSel.modeExisting": "Existente", // Existing
  "worktreeSel.loading": "Carregando worktrees…", // Loading worktrees…
  "worktreeSel.empty": "Nenhum worktree existente neste repositório.", // No existing worktrees in this repository.
  "worktreeSel.loadFailed":
    "Não foi possível listar os worktrees (não é um repositório git?).", // Couldn't list worktrees (not a git repository?).
  "group.worktreeHint":
    "As sessões criadas neste grupo usarão este worktree por padrão.", // Sessions created in this group will use this worktree by default.
  "worktree.moveGroupTitle": "Mover o grupo para um worktree",
  "worktree.moveGroupHint":
    "As sessões criadas neste grupo a partir de agora usarão este worktree. As já existentes mantêm o diretório atual.",

  // ── Archive panel ──
  "archive.title": "Sessões arquivadas", // Archived Sessions
  "archive.empty1": "Nenhuma sessão arquivada.", // No archived sessions.
  "archive.empty2":
    'Clique com o botão direito em uma sessão na barra lateral e escolha "Arquivar sessão" para guardá-la aqui.', // Right-click a session in the sidebar…
  "archive.restore": "Restaurar como sessão normal", // Restore to normal session
  "archive.export": "Exportar contexto completo como Markdown", // Export full context as Markdown
  "archive.deleteForever": "Excluir permanentemente (com a gravação)", // Delete permanently (with recording)
  "archive.pickOne":
    "Selecione uma sessão arquivada à esquerda para ver a transcrição", // Select an archived session on the left…
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
  "search.hint":
    'Pesquise o conteúdo das sessões. As arquivadas são excluídas por padrão — marque "Incluir arquivadas" para incluí-las.', // Search session content. Archived sessions are excluded by default.
  "search.includeArchived": "Incluir arquivadas", // Include archived
  "search.includeArchivedHint":
    "Pesquisar também em sessões arquivadas (desativado por padrão)", // Also search archived sessions (off by default)
  "search.searching": "Pesquisando…", // Searching…
  "search.noResults": "Nenhuma correspondência encontrada", // No matches found
  "search.sessionCount": (n) => (n === 1 ? "1 sessão" : `${n} sessões`), // n sessions
  "search.matchCount": (n) =>
    n === 1 ? "1 correspondência" : `${n} correspondências`, // n matches
  "search.pickSession":
    "Selecione uma sessão à esquerda para ver as correspondências", // Select a session on the left to see its matches
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
  "center.noSessionHintPre":
    "Escolha uma sessão na barra lateral ou pressione ", // Pick a session from the sidebar, or press
  "center.noSessionHintPost": " para criar um terminal", // to create a terminal
  "center.createTerminal": "Criar terminal", // Create Terminal
  "tab.unsavedDot": "Alterações não salvas", // Unsaved changes
  "tab.newTerminal": "Novo terminal", // New terminal
  "tab.newDocument": "Novo documento", // New document
  "tab.bgTitle": (n) =>
    `Abas em segundo plano: ${n} (processos ainda em execução)`, // Background keep-alive tabs: {n}…
  "tab.bgLabel": (n) => `Fundo ${n}`, // Background {n}
  "tab.scratchFallback": "(terminal temporário)", // (scratch terminal)
  "tab.killBgTab":
    "Encerrar esta aba em segundo plano (seus processos serão encerrados)", // Kill this background tab…
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
  "browser.desktopOnly":
    "As abas do navegador abrem apenas no aplicativo de desktop.", // Browser tabs open in the desktop app only.
  "browser.stop": "Parar o carregamento", // Stop loading
  "browser.openExternal": "Abrir no navegador do sistema", // Open in system browser
  "browser.addressPlaceholder": "Digite uma URL ou termos de busca", // Enter URL or search terms
  "browser.quickAccess": "Acesso rápido", // Quick access
  "browser.loading": "Carregando…", // Loading…
  // Application-exit confirmation and dormant restored sessions.
  "quit.title": "Sair do VelaTerm?", // Quit VelaTerm?
  "quit.body":
    "Todas as sessões de terminal e de agente em execução serão encerradas.", // Any running terminal and agent sessions will be stopped.
  "quit.saveWorkspace": "Salvar espaço de trabalho", // Save workspace
  "quit.saveWorkspaceHint":
    "Abrir as mesmas abas e divisões na próxima vez. Os terminais são restaurados, mas não reiniciados.", // Reopen the same tabs and splits next time. Terminals are restored but not restarted.
  "quit.confirm": "Sair", // Quit
  "dormant.body":
    "Restaurado do espaço de trabalho salvo. Nenhum processo em execução ainda.", // Restored from your saved workspace. No process is running yet.
  "dormant.start": "Iniciar", // Start
  "overlimit.title": (max) => `Limite de segundo plano excedido (${max})`, // Background keep-alive over limit ({max})
  "overlimit.body":
    "All background tabs are working or awaiting your reply. Choose one to end:", // All background tabs are working or awaiting your reply. Choose one to end:
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
  "term.mirrorBadge": (dims) =>
    `⤢ Espelho${dims} · clique para ajustar a esta janela`, // ⤢ Mirror{dims} · click to fit this window
  "term.mirrorBadgeMobile": (dims) =>
    `⤢ Espelho${dims} · ajustar a esta janela`, // ⤢ Mirror{dims} · fit this window
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
  "agentInstall.pathSaved": (label: string) =>
    `Caminho do executável de ${label} salvo nas Configurações:`, // executable path saved to Settings
  "agentInstall.doneTitle": (label: string) => `${label} está instalado`, // {label} is installed
  "agentInstall.doneDesc": "Reinicie esta sessão para começar a usá-lo.", // Relaunch this session to start using it.
  "agentInstall.restartNow": "Reiniciar agora", // Relaunch now
  "agentInstall.later": "Mais tarde", // Later
  "agentInstall.pathLabel": "Caminho do executável", // Executable path
  "agentInstall.pathPlaceholder": (bin: string) => `~/.local/bin/${bin}`,
  "agentInstall.pathHint": "Já instalado fora do PATH? Informe o caminho completo do executável.", // Already installed outside PATH?
  "agentInstall.pathSave": "Usar este caminho", // Use this path
  "agentInstall.pathBrowse": "Procurar…", // Browse…
  "search.placeholder": "Buscar no terminal", // Search in terminal

  // ── Document tabs ──
  "doc.wysiwyg": "WYSIWYG", // WYSIWYG
  "doc.visual": "Visual",
  "doc.source": "Código-fonte",
  "doc.compare": "Comparação",
  "doc.editorLoadFailed": "Não foi possível carregar o editor Markdown.",
  "doc.imageOnly": "Só é possível inserir arquivos de imagem aqui.",
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
  "doc.overwriteConfirm":
    "Já existe um arquivo com esse nome. Clique em “Substituir” para substituí-lo.", // A file with this name already exists. Click "Overwrite" to replace it.
  "doc.saveTooltip": "Salvar", // Save
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
  "doc.imgDecodeFailed":
    "Não é possível exibir esta imagem (formato não suportado ou arquivo corrompido).", // Cannot display this image (unsupported or corrupted format).
  "doc.imgFit": "Ajustar", // Fit
  "doc.imgActual": "1:1", // 1:1
  "doc.exportPdf": "Exportar PDF", // Export PDF
  "doc.diagramError": "Erro de diagrama", // Diagram error

  // ── Right information panel ──
  "panel.noSession": "Nenhuma sessão selecionada", // No session selected
  "panel.collapseSection": "Recolher seção", // Collapse section
  "panel.expandSection": "Expandir seção", // Collapse section
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
  "files.dblClickOpen": "Clique duas vezes para abrir",
  "files.deleteConfirm": (name) =>
    `Excluir "${name}"? Isso não pode ser desfeito.`, // Delete "{name}"? This can't be undone.

  // ── File transfer (remote access) ──
  "transfer.uploadsTitle": "Envios", // Uploads
  "transfer.download": "Baixar", // Download
  "transfer.upload": "Enviar arquivos…", // Upload Files…
  "transfer.uploadTooltip": "Enviar arquivos para esta pasta", // Upload files to this folder
  "transfer.clear": "Limpar", // Clear
  "transfer.cancelled": "Cancelado", // Cancelled
  "transfer.failed": "Falhou", // Failed
  "transfer.stalled": "Reconectando…", // Reconnecting…
  "transfer.foldersUnsupported": "Não é possível enviar pastas.", // Folders can't be uploaded.

  // ── Status bar ──
  "statusbar.sessions": (n) => (n === 1 ? "1 sessão" : `${n} sessões`), // {n} sessions
  "statusbar.filterTooltip": (label) =>
    `Clique para mostrar apenas sessões "${label}" na barra lateral (clique novamente para limpar)`, // Click to show only "X" sessions…
  "statusbar.bgCount": (n, max) => `Fundo ${n}/${max}`, // Background {n}/{max}
  "statusbar.bgTooltip": (max) =>
    `Abas em segundo plano (limite ${max}; ao exceder, a aba inativa mais antiga é encerrada automaticamente)`, // Background keep-alive tabs (limit {max}…)
  "statusbar.bgEvicted": (name) =>
    `Aba em segundo plano encerrada: ${name} (limite excedido)`, // Ended background tab: {name} (over keep-alive limit)
  "statusbar.webTooltip": (url) =>
    `Acesso remoto pelo navegador ativado: ${url}`, // Browser remote access enabled: {url}
  "statusbar.permAsk": "Permissões: Perguntar", // Perms: Ask
  "statusbar.permSkip": "Permissões: Ignorar", // Perms: Skip
  "statusbar.notifyOn": "Notify: On", // TODO translate
  "statusbar.notifyOff": "Notify: Off", // TODO translate
  "statusbar.permTooltip":
    "Modo de permissão desta sessão · clique para alterar (somente esta sessão)", // This session's permission mode · click to change (this session only)
  "statusbar.permMenuTitle": "Permissões desta sessão", // This session's permissions
  "statusbar.permOptAsk": "Perguntar sempre (padrão)", // Ask each time (default)
  "statusbar.permScopeHint":
    "Aplica-se apenas a esta sessão. Para configurações globais, acesse Configurações ▸ Agentes.", // Applies to this session only. For global defaults, go to Settings ▸ Agents.
  "statusbar.permRestartMsg":
    "Permissão alterada. A sessão precisa ser reiniciada para aplicar. Reiniciar retoma a conversa atual, mas interrompe qualquer tarefa em andamento. Reiniciar agora?", // Permission changed. The session must restart to apply. Restart resumes the current conversation but interrupts any task in progress. Restart now?
  "statusbar.permRestartNow": "Reiniciar agora", // Restart now
  "statusbar.permRestartLater": "Mais tarde", // Later
  "statusbar.permScopeTitle": "Aplicar a?", // Apply to?
  "statusbar.permScopeSession": "Somente esta sessão", // This session only
  "statusbar.permScopeGlobal": "Padrão global", // Global default
  "statusbar.permScopeGlobalHint":
    "Aplica-se agora a esta sessão e passa a ser o padrão para futuras sessões deste tipo (sincronizado com as Configurações).", // Applies now to this session and becomes the default for future sessions of this kind (synced with Settings).

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
  "err.uncaughtDesc":
    "As informações abaixo podem ajudar a localizar o problema.", // The information below can help locate the problem.

  // ── transport ──
  "transport.noReplayInBrowser":
    "A reprodução de gravações ainda não é suportada no navegador", // Recording playback is not yet supported in the browser
  "transport.imgUploadHttp": (status) => `Falha no envio da imagem (${status})`, // Image upload failed ({status})

  // ── Login gate, directory selection, and connection banner ──
  "login.showPassword": "Mostrar",
  "login.hidePassword": "Ocultar",
  "login.passwordSaveFailed": "Conexão estabelecida, mas não foi possível salvar a senha neste dispositivo. Tente novamente.",
  "login.connecting": "Conectando…", // Connecting…
  "login.remoteAccess": "Acesso remoto", // Remote Access
  "login.desc": "Digite a senha de acesso para se conectar a este terminal.", // Enter the access password to connect to this terminal.
  "login.passwordPlaceholder": "Senha de acesso", // Access password
  "login.connect": "Conectar", // Connect
  "login.wrongPassword": "Senha incorreta", // Wrong password
  "login.rateLimited":
    "Muitas tentativas. Aguarde um minuto e tente novamente.", // Too many attempts. Please wait a minute and try again.
  "login.failed": "Falha no login, tente novamente", // Login failed, please try again
  "login.pairingRequired":
    "Este servidor requer um link de emparelhamento. Abra o link gerado no painel de Acesso remoto do app de desktop.", // This server requires a pairing link
  "login.authFailed":
    "Falha na autenticação. Verifique a senha de acesso ou abra um novo link de emparelhamento se ele foi gerado novamente.", // Authentication failed, check password or use a new pairing link
  "dir.title": "Escolher diretório do projeto", // Choose Project Directory
  "dir.pathPlaceholder":
    "Pesquise, ou digite um caminho e pressione Enter (aceita ~)", // Search, or type a path and press Enter (supports ~)
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
  "conn.sshDown":
    'A conexão SSH caiu — clique em "Reconectar agora" para tentar novamente', // SSH link is down — press Reconnect now to try again
  "reqerr.title": "Falha na solicitação", // Request failed
  "reqerr.dismiss": "Fechar", // Dismiss
  // ── Error Log panel ──
  "errlog.title": "Registro de erros", // Error Log
  "errlog.empty": "Nenhum erro registrado.", // No errors recorded.
  "errlog.copyAll": "Copiar tudo", // Copy all
  "errlog.clear": "Limpar", // Clear
  "errlog.close": "Fechar", // Close

  // ── Mobile ──
  "mobile.backConnections": "Voltar às conexões",
  "mobile.loadSlow": "O carregamento está demorando mais que o esperado. Você pode tentar novamente ou voltar às suas conexões.",
  "mobile.connectionUnavailable": "Conexão indisponível",
  "mobile.pushTitle": "Notificações de tarefas",
  "mobile.pushHint": "As notificações mostram o nome da sessão e uma breve prévia da resposta, inclusive em segundo plano ou com a tela bloqueada. Esse texto é enviado ao velaterm.com e ao serviço de notificações push. Senhas de conexão e chaves privadas SSH não são enviadas.",
  "mobile.pushEnable": "Ativar notificações",
  "mobile.pushDisable": "Desativar notificações",
  "mobile.pushTest": "Enviar notificação de teste",
  "mobile.pushTestSent": "A notificação de teste foi adicionada à fila. Confira a central de notificações do sistema.",
  "mobile.pushDisabled": "As notificações em segundo plano estão desativadas.",
  "mobile.pushEnabled": "As notificações em segundo plano estão ativadas.",
  "mobile.pushNotConfigured": "Esta compilação não tem um serviço de notificações push configurado.",
  "mobile.pushDenied": "Permita as notificações nas configurações do sistema.",
  "mobile.pushRegistrationFailed": "Não foi possível registrar o dispositivo. Tente novamente.",
  "mobile.pushRelayUnavailable": "O serviço de notificações está indisponível. Tente novamente.",
  "mobile.pushHostUnavailable": "O servidor remoto ainda não ativou as notificações em segundo plano. Atualize-o e conecte-se novamente.",
  "mobile.pushDisclosure": "As notificações em segundo plano usam o Getui e o serviço push do fabricante do dispositivo. Para entregá-las, esses serviços processam identificadores do dispositivo, informações de rede, nomes de sessões e breves prévias das respostas. Senhas de conexão e chaves privadas SSH não são enviadas.",
  "mobile.pushConnectHint": "Após ativar as notificações, abra cada conexão uma vez para concluir a inscrição.",
  "mobile.pushTarget": "Conexão de teste",
  "mobile.copyConnection": "Copiar e editar",
  "mobile.copyConnectionHint": "Edite as configurações a partir desta conexão. As credenciais salvas são mantidas com segurança. A conexão original permanece inalterada; se as configurações forem idênticas, a conexão existente será mantida.",
  "mobile.copyConnectionReused": "Estas configurações já estão salvas. A conexão existente foi mantida.",
  "mobile.inputOptions": "Opções da mensagem",
  "mobile.connections": "Gerenciar conexões",
  "mobile.more": "Mais ações",
  "mobile.toDesktop": "Mudar para a versão desktop", // Switch to desktop
  "mobile.empty1": "Nenhuma sessão.", // No sessions.
  "mobile.noMatch": "Nenhuma sessão correspondente", // No matching sessions
  "mobile.empty2":
    "Crie uma no aplicativo desktop ou no navegador de um computador e ela aparecerá aqui automaticamente.", // Create one on the desktop app or a computer browser…
  "mobile.back": "‹ Voltar", // ‹ Back
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
  "splitter.dragToResize": "Arraste para redimensionar", // Drag to resize
  "transport.wsDisconnected": "WebSocket desconectado", // WebSocket disconnected
  "transport.wsConnectFailed": "Falha na conexão WebSocket", // WebSocket connection failed
  "transport.cmdFailed": "O comando falhou", // Command failed
  "transport.remoteCmdForbidden": (cmd: string) =>
    `Comando não disponível para clientes remotos: ${cmd}`, // Command not available to remote clients
  "transport.remoteSettingForbidden": (key: string) =>
    `Chave de configuração não gravável por clientes remotos: ${key}`, // Settings key not writable by remote clients
  "transport.remotePathForbidden": (path: string) =>
    `Clientes remotos não podem acessar arquivos no diretório de dados do aplicativo: ${path}`, // Remote clients cannot access files in the app data directory

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
  "info.collection": "Coleção", // Collection
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

  // ── Visão de conversa (a sessão do agente lida como diálogo) ──
  "session.showConversation": "Visão de conversa",
  "session.showTerminal": "Visão de terminal",
  "session.switchTitle": "Trocar de visão reinicia o agente",
  "session.switchBody": "O turno em andamento será interrompido. A conversa é preservada.",
  "session.switchConfirm": "Trocar",
  "session.terminalViewHint": "Clique aqui para voltar à visão de terminal.",
  "session.loading": "Lendo a conversa…",
  "session.unavailable": "Esta conversa ainda não pode ser lida",
  "session.working": "Trabalhando…",
  "session.thinking": "Raciocínio",
  "session.toolRunning": "em andamento",
  "session.toolUnknown": "Ferramenta",
  "session.toolFailed": "Falhou",
  "session.toolNoDetail": "Nada mais foi registrado",
  "session.showMore": (n: number) => `Mostrar mais ${n} caracteres`,
  "session.showLess": "Mostrar menos",
  "session.composerHint": "Escreva para o agente · Enter envia, Shift+Enter quebra a linha",
  "session.send": "Enviar",

  // ── Motor de conversa (uma sessão conduzida por protocolo) ──
  "chat.empty": "Escreva no campo abaixo para iniciar a conversa.",
  "chat.interrupt": "Parar",
  "chat.interruptTooltip": "Parar · Esc",
  "chat.allow": "Permitir",
  "chat.deny": "Negar",
  "chat.permissionAsk": (tool: string) => `${tool} quer ser executado`,
  "chat.exited": (code: number) => `O agente parou (código ${code})`,
  "chat.modeNextTurn": "No próximo turno",
  "chat.modePendingHint": (current: string, next: string) =>
    `Permissões atuais: ${current}. ${next} será aplicado no próximo turno; o turno atual continuará sem alterações.`,
  "chat.modeTooltip": "Modo de permissão",
  "chat.collaborationModeTooltip": "Modo de colaboração",
  "chat.collaborationMode.default": "Padrão",
  "chat.collaborationMode.defaultHint":
    "Avança diretamente e só pergunta quando uma decisão é necessária",
  "chat.collaborationMode.plan": "Planejamento",
  "chat.collaborationMode.planHint":
    "Analisa a tarefa e prepara um plano; as perguntas podem usar cartões interativos",
  "chat.moreOptions": "Mais",
  "chat.modelTooltip": "Modelo",
  "chat.keepChoice": "Definir como padrão",
  "chat.keepChoiceFor": (model) => `Definir como padrão para ${model}`,
  "chat.followModelDefault": (agent: string) => `Usar o modelo padrão do ${agent}`,
  "chat.followModelDefaultHint": "Usa o modelo definido nas configurações do agente.",
  "chat.savedModelDefault": "Padrão do aplicativo",
  "chat.catalogWebsite": "Catálogo de modelos do site",
  "chat.catalogCache": "Catálogo de modelos em cache",
  "chat.catalogBundled": "Catálogo de modelos incluído",
  "chat.catalogChecked": (time: string) => `Última verificação: ${time}`,
  "chat.catalogFailed": "Falha na atualização. O catálogo anterior continua disponível.",
  "chat.catalogRefresh": "Atualizar",
  "chat.modelDefault": "Modelo padrão",
  "chat.mode.default": "Sempre perguntar",
  "chat.mode.agentDefault": "Padrão do agente",
  "chat.mode.acceptEdits": "Aceitar edições",
  "chat.mode.plan": "Modo plano",
  "chat.permissionRestart.unconfirmed": "A conexão foi perdida. Não foi possível confirmar a alteração de permissão. Reconecte-se para verificar as permissões atuais da sessão.",
  "permission.stateUnavailable": "Estado das permissões indisponível",
  "permission.currentUnknown": "Permissões atuais não confirmadas",
  "permission.notRunning": "Não está em execução",
  "permission.applied": "Aplicado",
  "permission.nextTurn": "Aplica-se à próxima mensagem",
  "permission.restart": "Aplica-se após reiniciar esta sessão",
  "permission.nextStart": "Na próxima inicialização",
  "permission.defaultHint": "Permissão padrão para novas sessões. As sessões existentes mantêm suas próprias configurações de permissão.",
  "chat.permissionRestart.title": "Reiniciar para ignorar as confirmações?",
  "chat.permissionRestart.body": "O Claude precisa reiniciar para ignorar as confirmações. A resposta atual será interrompida, e o histórico da conversa será mantido. Após a alteração, as confirmações de permissão serão ignoradas.",
  "chat.permissionRestart.confirm": "Reiniciar e aplicar",
  "chat.permissionRestart.busy": "Reiniciando…",
  "chat.permissionRestart.failed": (detail: string) => "Não foi possível alterar a permissão. O modo anterior foi mantido. " + detail,
  "chat.permissionRestart.tasks": "Processe ou remova as mensagens na fila e interrompa as tarefas em segundo plano antes de reiniciar.",
  "chat.permissionRestart.stale": "O processo da sessão mudou. Selecione “Sem perguntar” novamente.",
  "chat.permissionRestart.noHistory": "Ainda não é possível retomar esta conversa. Aguarde a conclusão da inicialização e tente novamente.",
  "chat.mode.bypassPermissions": "Sem perguntar",
  "chat.mode.readOnly": "Somente leitura",
  "chat.mode.fullAccess": "Acesso total",
  "chat.placeholder": "Escreva para o agente, ou use /comandos, /habilidades e @arquivos",
  "chat.command.clearDescription": "Arquivar esta sessão e iniciar uma nova conversa",
  "chat.command.rewindDescription": "Escolher o que desfazer a partir da última mensagem do usuário",
  "chat.command.rewindUnavailable":
    "Para desfazer, é preciso haver uma mensagem do usuário concluída e nenhum turno ativo, mensagem na fila ou pedido de permissão.",
  "chat.effortTooltip": "Esforço de raciocínio",
  "chat.effortDefault": "Raciocínio",
  "chat.effort.auto": "Automático",
  "chat.effort.low": "Baixo",
  "chat.effort.medium": "Médio",
  "chat.effort.high": "Alto",
  "chat.effort.xhigh": "Muito alto",
  "chat.effort.max": "Máximo",
  "chat.effort.ultra": "Extremo",
  "chat.effort.ultracode": "Ultra Code",
  "chat.agentTooltip": "Agente",
  "chat.effort.minimal": "Mínimo",
  "chat.filterPlaceholder": "Filtrar",
  "chat.placeholderOpencode": "Escreva para o agente; use /comandos e @arquivos, ou comece com ! para executar um comando de shell",
  "chat.command.compactDescription": "Resumir a conversa para liberar contexto",
  "chat.command.undoDescription": "Reverter a última mensagem e as alterações de arquivos que ela causou",
  "chat.command.redoDescription": "Restaurar o que o último desfazer reverteu",
  "chat.command.shareDescription": "Criar um link para compartilhar esta conversa",
  "chat.command.unshareDescription": "Parar de compartilhar esta conversa",
  "chat.mode.auto": "Modo automático",

  // ── Uma pergunta do agente, respondida como formulário ──
  "chat.question.heading": "O agente tem uma pergunta",
  "chat.question.submit": "Enviar",
  "chat.question.next": "Próxima",
  "chat.question.dismiss": "Dispensar",
  "chat.question.answerPlaceholder": "Digite sua resposta",
  "chat.question.otherPlaceholder": "Outra resposta",
  "chat.question.answeredHeading": (n: number) =>
    n === 1 ? "1 pergunta respondida" : `${n} perguntas respondidas`,
  "chat.question.blankAnswer": "Sem resposta",

  // ── Um plano aguardando aprovação ──
  "chat.plan.heading": "Plano aguardando aprovação",
  "chat.plan.implement": "Aprovar e executar",
  "chat.plan.reject": "Rejeitar",

  // ── Mensagens escritas enquanto o agente trabalha ──
  "chat.placeholderBusy": "Digite uma mensagem; ela será enviada quando este turno terminar",
  "chat.queueTooltip": (combo: string) => `Será enviada quando este turno terminar · ${combo} envia agora`,
  "chat.queue.pending": "Mensagens pendentes",
  "chat.queue.view": "Ver mensagem completa",
  "chat.queue.edit": "Editar",
  "chat.queue.remove": "Remover",

  // ── Imagens coladas ou arrastadas para o campo de mensagem ──
  "chat.attach.remove": "Remover esta imagem",
  "chat.attach.tooMany": (max: number) => `Uma mensagem pode incluir até ${max} imagens`,
  "chat.attach.tooLarge": (name: string, mb: number) => `${name} excede ${mb} MB e não foi anexada`,
  "chat.attach.unreadable": (name: string) => `Não foi possível ler ${name}`,
  // Compacting the conversation… / Context compacted / Context compacted automatically
  "chat.compaction.running": "Compactando a conversa…",
  "chat.compaction.manual": "Contexto compactado",
  "chat.compaction.auto": "Contexto compactado automaticamente",
  "chat.compaction.from": (tokens: string) => `de ${tokens} tokens`,
  // N steps
  "chat.subagent.steps": (n: number) => (n === 1 ? "1 passo" : `${n} passos`),
  "chat.subagent.tokens": (tokens: string) => `${tokens} tokens`,
  "chat.rewind.edit": "Editar",
  "chat.rewind.editSend": "Revisar e reenviar",
  "chat.rewind.editConfirm": "Excluir e reenviar",
  "chat.rewind.editWarning": "A mensagem original e todas as mensagens posteriores serão excluídas permanentemente. A mensagem editada será enviada a partir deste ponto. As alterações nos arquivos não serão desfeitas.",
  "chat.rewind.inactive": "O processo da conversa não está em execução. Estas ações estarão disponíveis após a inicialização.",
  "chat.rewind.unsupported": "O agente conectado não oferece esta ação no momento.",
  "chat.rewind.title": "Voltar para este ponto",
  "chat.rewind.warning": "Esta ação não pode ser desfeita.",
  "chat.rewind.conversation": "Reverter conversa",
  "chat.rewind.files": "Restaurar arquivos",
  "chat.rewind.both": "Reverter conversa e restaurar arquivos",
  "chat.rewind.confirm.conversation": "Remover esta mensagem e tudo o que veio depois?",
  "chat.rewind.confirm.files": "Restaurar os arquivos ao estado anterior a esta mensagem?",
  "chat.rewind.confirm.both": "Remover este turno e restaurar os arquivos que ele alterou?",
  "chat.rewind.unavailable": "Não há um ponto de restauração de arquivos para esta mensagem.",
  "chat.rewind.previewing": "Verificando o ponto de restauração…",
  "chat.rewind.cancel": "Manter como está",
  "chat.rewind.apply": "Reverter",
  "chat.rewind.applying": "Revertendo…",
  "chat.rewind.fileSummary": (files: number, insertions: number, deletions: number) =>
    `${files} ${files === 1 ? "arquivo será alterado" : "arquivos serão alterados"}: +${insertions} −${deletions}. Esta ação não pode ser desfeita.`,
  // ── Regras permanentes oferecidas por uma pergunta de permissão, adotadas com um clique ──
  "chat.suggest.modeSession": (mode: string) => `${mode} nesta sessão`,
  "chat.suggest.mode": (mode: string) => `Mudar para ${mode}`,
  "chat.suggest.allowSession": (rule: string) => `Permitir ${rule} nesta sessão`,
  "chat.suggest.allowAlways": (rule: string) => `Permitir ${rule} sempre`,
  "chat.suggest.dirSession": (dirs: string) => `Permitir acesso a ${dirs} nesta sessão`,
  "chat.suggest.dirAlways": (dirs: string) => `Permitir acesso a ${dirs} sempre`,
  // ── Codex: regras de rede permanentes, intervir, comandos próprios e os chips de velocidade e tom ──
  "chat.suggest.networkAlways": (host: string) => `Sempre permitir acesso de rede a ${host}`,
  "chat.steer": "Adicionar instrução",
  "chat.stopping": "Parando o turno atual…",
  "chat.stopped": "Turno atual interrompido",
  "chat.steerAccepted": "Instrução enviada",
  "chat.steerTooltip": (combo: string) => `${combo} adiciona ao turno em andamento`,
  "chat.command.reviewDescription": "Revisar o código e relatar o que precisa de atenção",
  "chat.command.reviewHint": "[branch <nome> | commit <sha> | instruções]",
  "chat.command.startTimeout": "O agente não abriu a sessão a tempo",
  "chat.serviceTierTooltip": "Velocidade",
  "chat.serviceTier.default": "Velocidade padrão",
  "chat.personalityTooltip": "Tom",
  "chat.personality.default": "Tom padrão",
  "chat.personality.none": "Neutro",
  "chat.personality.friendly": "Cordial",
  "chat.personality.pragmatic": "Pragmático",
  // ── Conversa longa: sequências de chamadas de ferramenta cabem em uma linha, com volta ao fim ──
  "chat.toolRun.count": (n: number) => `${n} chamadas de ferramenta`,
  "chat.toolRun.tooltip": "Ver cada chamada",
  "chat.backToEnd": "Voltar para a mensagem mais recente",
  "chat.turnFold.hide": "Ocultar passos",
  "chat.turnFold.show": (n: number) => (n === 1 ? "Mostrar 1 passo" : `Mostrar ${n} passos`),
  "chat.turnFold.hideAll": "Ocultar todos os passos",
  "chat.turnFold.showAll": "Mostrar todos os passos",
  "chat.elicitation.heading": (server: string) => `${server} solicita informações`,
  "chat.elicitation.cancel": "Cancelar",
  "chat.elicitation.decline": "Recusar",
  "chat.elicitation.submit": "Enviar",
  "chat.elicitation.done": "Concluído",
  "chat.elicitation.choose": "Escolher…",
  "chat.effort.off": "Desativado",
  "chat.effort.offHint": "Sem raciocínio estendido",
  "chat.fastMode.label": "Rápido",
  "chat.fastMode.on": "O modo rápido está ativado",
  "chat.fastMode.off": "O modo rápido está desativado",
  "chat.auth.login": "Entrar",
  "chat.auth.logout": "Sair",
  "chat.auth.confirmLogout": "Confirmar saída",
  "chat.auth.logoutConfirm": (provider: string) => `Sair do ${provider} neste host? As credenciais compartilhadas serão removidas, o que afetará outras sessões que as utilizam. O histórico das conversas será mantido.`,
  "chat.auth.signingOut": "Saindo…",
  "chat.auth.signedOut": (provider: string) => `Você saiu do ${provider}. Entre para continuar esta conversa.`,
  "chat.auth.logoutFailed": "Não foi possível confirmar a saída. Tente novamente.",
  "chat.auth.wait": "Aguarde a conclusão da tarefa atual antes de trocar de conta.",
  "chat.auth.title": (provider: string) => `Conta do ${provider}`,
  "chat.auth.start": "Entrar novamente",
  "chat.auth.required": (provider: string) => `Seu login no ${provider} não é mais válido. Entre novamente para continuar.`,
  "chat.auth.starting": "Preparando o login…",
  "chat.auth.pending": "Abra a página de autorização e insira este código. Esta tela será atualizada quando o login for concluído.",
  "chat.auth.success": "Login realizado. Você pode enviar uma mensagem para continuar esta conversa.",
  "chat.auth.failed": "Não foi possível concluir o login. Tente novamente. Verifique se a autenticação por código de dispositivo está ativada no ChatGPT e se sua versão do Codex CLI oferece suporte a ela.",
  "chat.auth.canceled": "Login cancelado. Você pode tentar novamente a qualquer momento.",
  "chat.auth.scope": (provider: string) => `O login atualiza a conta do ${provider} usada neste host. Outras sessões que compartilham estas credenciais também usarão essa conta.`,
  "chat.auth.canceling": "Cancelando o login…",
  "chat.auth.submitting": "Verificando o código de autorização…",
  "chat.auth.claude.pending": "Abra a página de autorização, entre na sua conta e cole o código completo exibido.",
  "chat.auth.claude.failed": "Não foi possível concluir o login. Tente novamente e verifique se sua versão do Claude CLI oferece suporte à autorização de contas.",
  "chat.auth.claude.code": "Código de autorização",
  "chat.auth.claude.submit": "Enviar código",
  "chat.auth.claude.invalidCode": "Cole o código completo desta tentativa de autorização, incluindo a parte após #.",
  "chat.auth.claude.externalAuth": "As chaves de API e os outros métodos de autenticação configurados não serão alterados.",
  "chat.auth.open": "Abrir página de autorização",
  "chat.resetCredits.label": (n: string) => `Créditos de redefinição: ${n}`,
  "chat.resetCredits.title": "Créditos para redefinir os limites do Codex",
  "chat.resetCredits.unknown": "Não foi possível obter a quantidade de créditos de redefinição.",
  "chat.resetCredits.confirm": "Usar um crédito para redefinir os limites de uso elegíveis do Codex. Esta ação não pode ser desfeita.",
  "chat.resetCredits.reset": "Limites de uso redefinidos.",
  "chat.resetCredits.alreadyRedeemed": "Esta solicitação já foi concluída com sucesso.",
  "chat.resetCredits.nothingToReset": "Nenhum limite de uso está elegível para redefinição.",
  "chat.resetCredits.noCredit": "Nenhum crédito de redefinição disponível.",
  "chat.resetCredits.error": "A solicitação falhou ou o saldo atual está indisponível. Atualize o saldo ou tente novamente a redefinição pendente.",
  "chat.resetCredits.busy": "Processando…",
  "chat.resetCredits.retry": "Tentar redefinição novamente",
  "chat.resetCredits.use": "Usar um crédito",
  "chat.resetCredits.refresh": "Atualizar",
  "chat.usage.context": (used: string, max: string, pct: number) =>
    `Contexto: ${used} de ${max} tokens (${pct}%)`,
  "chat.usage.cost": (usd: string) => `Custo da sessão: $${usd}`,
  "chat.usage.rateLimited": (resets: string) => `Limite de uso atingido; redefinição ${resets}`,
  "chat.usage.rateWarning": (pct: number, resets: string) =>
    `Limite de uso: ${pct}% consumido; redefinição ${resets}`,
  "chat.autoContinue.fiveHour": (time: string) => `Limite de uso de 5 horas atingido. Retomada automática da tarefa: ${time}.`, // 5-hour usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.weekly": (time: string) => `Limite de uso semanal atingido. Retomada automática da tarefa: ${time}.`, // Weekly usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.generic": (time: string) => `Limite de uso atingido. Retomada automática da tarefa: ${time}.`, // Usage limit reached. The task will continue automatically at ${time}.
  "chat.autoContinue.unknownReset": "Limite de uso atingido. O horário de redefinição é desconhecido, então a tarefa não continuará automaticamente.", // Usage limit reached. The reset time is unknown, so the task will not continue automatically.
  "chat.autoContinue.repeated": "O limite de uso foi atingido novamente. A tarefa não continuará mais automaticamente.", // The usage limit was reached again. The task will no longer continue automatically.
  "chat.autoContinue.failed": "Não foi possível continuar a tarefa automaticamente. Envie uma mensagem para continuar.", // The task could not continue automatically. Send a message to continue.
  "chat.mcp.codexScope": "Esta ação altera sua configuração de usuário do Codex e afeta outras conversas que a utilizam. Continuar?",
  "chat.mcp.tooltip": "Servidores MCP",
  "chat.mcp.loading": "Lendo a lista de servidores…",
  "chat.mcp.backendUnsupported": "O servidor VelaTerm conectado não oferece suporte ao gerenciamento de MCP. Atualize e reinicie esse servidor e tente novamente.",
  "chat.mcp.none": "Nenhum servidor MCP configurado",
  "chat.mcp.tools": (n: number) => (n === 1 ? "1 ferramenta" : `${n} ferramentas`),
  "chat.mcp.reconnect": "Reconectar",
  "chat.mcp.disable": "Desativar",
  "chat.mcp.enable": "Ativar",
  "chat.mcp.status.connected": "Conectado",
  "chat.mcp.status.disabled": "Desativado",
  "chat.mcp.status.failed": "Com falha",
  "chat.mcp.status.pending": "Conectando",
  "chat.mcp.status.disconnected": "Desconectado",
  "chat.mcp.status.other": "Desconhecido",
  "chat.tasks.label": "Tarefas",
  "chat.tasks.tooltip": "Tarefas em segundo plano",
  "chat.tasks.backgroundAll": "Mover o trabalho em andamento para o segundo plano",
  "chat.tasks.none": "Nenhuma tarefa em segundo plano",
  "chat.tasks.stop": "Parar",
  "chat.retry.line": (attempt: number, max: number, seconds: number, message: string) =>
    `Nova tentativa (${attempt}/${max}) em ${seconds} s: ${message}`,
  "chat.notify.dismiss": "Fechar",
  "settings.completionMode": "Sugestões de comandos",
  "settings.completionAuto": "Automáticas",
  "settings.completionTab": "Com Tab",
  "settings.completionOff": "Desativadas",
  "settings.completionUnavailable": "Não foi possível carregar ou salvar as configurações.",
  "settings.completionHint": "Aplica-se a novos terminais Zsh, Bash 4+, Fish e PowerShell. O CMD mantém o comportamento padrão de Tab. Tab insere a sugestão selecionada; Enter executa o comando atual sem aplicar sugestões.",


};

export default ptBR;
