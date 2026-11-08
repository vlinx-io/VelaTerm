## v0.3.0 — 2026-11-08

- 🔏 A versão para macOS agora é assinada com o certificado da Vlinx Software, Inc. em vez do anterior; após a atualização, talvez seja preciso conceder novamente algumas permissões do macOS, e os itens salvos nas Chaves, como senhas de conexões remotas, precisam ser autorizados de novo: quando solicitado, digite sua senha de login e escolha “Sempre Permitir”.
- 🗂️ As sessões agora têm três tipos, “Tarefa”, “Projeto” e “Chat”, escolhidos no topo da página “Nova sessão”, e a barra lateral esquerda passa a ser organizada nas coleções fixas “Projetos”, “Terminais”, “Tarefas” e “Chats”, sendo que “Terminais” lista todos os terminais em execução e os agentes detectados neles.
- 🧭 Uma nova barra de atividades na borda esquerda alterna entre “Projetos”, “Terminais”, “Tarefas”, “Chats”, “Sessões arquivadas” e “Base de conhecimento”, e reúne na parte inferior “Configurações”, “Conta”, “Feedback”, “Compartilhar” e “Central de jogos”.
- 📦 Coleções podem ser arquivadas por inteiro com “Arquivar coleção” no menu de contexto, e restaurar qualquer uma de suas sessões traz a coleção de volta.
- 🆕 Na página “Nova sessão”, o projeto, o agente, o modelo, o esforço de raciocínio e as permissões são escolhidos em um cartão de configurações acima da caixa de entrada, e a lista de projetos permite criar, importar ou clonar um projeto sem sair da página.
- 🩺 Ao abrir uma conversa, o VelaTerm verifica se o agente dela está instalado e conectado à conta: caso não esteja, aparece um aviso de instalação ou um painel para entrar, e o botão “Enviar” fica desativado até que o agente esteja pronto.
- 🔧 Um agente só é considerado instalado quando realmente é executado; assim, uma instalação inacabada ou uma atualização com defeito mostra o guia de instalação, as instalações em um clique só são concluídas quando o agente é iniciado, e uma atualização que deixa o agente sem conseguir iniciar é informada como incompleta.
- 🪟 O novo comando `vartifact <file|URL>` abre um arquivo em um painel ao lado da conversa, com prévia ao vivo para HTML e visualizações editáveis para Markdown, imagens e código, que podem ser salvas com ⌘S, enquanto `vopen` agora sempre abre uma aba.
- 💤 As sugestões em cinza em linha no terminal agora vêm desativadas por padrão e o preenchimento volta a ser “Apenas Tab”; esta atualização muda uma única vez a configuração “Em linha + Tab” existente para “Apenas Tab”, e você pode reativá-las nas configurações.
- ⌨️ No modo “Em linha + Tab”, a nova configuração “Tecla Tab” permite aceitar a sugestão em cinza com Tab, e um segundo Tab desfaz a aceitação e lista as correspondências; enquanto houver uma sugestão visível, a barra de título do terminal mostra quais teclas estão disponíveis.
- 🎨 Editar uma linha de comando do Zsh não deixa mais cinza o texto já digitado, e a descrição de “Sugestões de comandos” em “Configurações” corresponde ao modo selecionado.
- 📡 O preenchimento com Tab em conexões remotas responde em uma única ida e volta.
- 💬 As sessões do ZCode permitem adicionar uma instrução durante a resposta com Option/Alt+Enter, e a mensagem entra no turno em andamento depois da próxima chamada de ferramenta.
- 🔑 No ZCode 3.15 e versões posteriores, é possível entrar com OAuth, e um GLM Coding Plan adquirido não é mais informado como ausente ou esgotado.
- 📥 É possível importar sessões do Pi, do OMP, do ZCode e do DeepSeek Harness, e a caixa de diálogo “Importar sessões” permite filtrar por agente.
- 🔤 A interface usa Inter em todos os lugares, enquanto código, saída de ferramentas, caminhos e terminais continuam em JetBrains Mono, e a configuração de fonte da interface foi removida.
- 📚 A base de conhecimento exibe as entradas em uma coluna centralizada, lista as sessões arquivadas em uma árvore de projetos, grupos e sessões pai, com uma visão “Por data”, e permite alternar a base de conhecimento local entre cartões e lista.
- 🔔 No macOS e no Windows, clicar em uma notificação traz para a frente a janela de onde ela veio, de modo que uma notificação de uma janela remota não coloca mais a janela principal por cima dela.
- 📋 No macOS, colar não trava mais o aplicativo quando o aplicativo de origem da área de transferência para de responder: o VelaTerm desiste após 5 segundos e avisa você.
- 📝 No editor de código-fonte, “Selecionar tudo” no menu do macOS ou no menu de contexto seleciona o arquivo inteiro, e copiar não deixa mais de fora as linhas fora da tela.
- 📏 A caixa de entrada não aumenta mais e depois volta a diminuir quando você envia uma mensagem.
- 📂 “Terminais”, “Tarefas” e “Chats” mostram seu conteúdo expandido, independentemente do que estiver recolhido em “Projetos”.
- 🔄 As novas sessões não falham mais ao sincronizar com `cursor_expired` depois que você abre a conversa de uma subtarefa.
- 🧯 Reconectar após uma longa desconexão com muito conteúdo acumulado não deixa mais a conversa travada com o botão “Enviar” desativado.
- 🪞 Conexões remotas com hosts Windows que usam Cygwin ou Git Bash passam a usar o aplicativo de desktop do VelaTerm já em execução lá, e enviar uma mensagem não falha mais enquanto esse aplicativo executa a sessão.
- ⚡ A indexação de conversas longas do Codex consome cerca da metade do tempo de CPU de antes.
- 🔗 Os links para artefatos do Claude abrem no seu navegador padrão.
- 🗄️ O botão “Sessões arquivadas” saiu da barra de abas e foi para a barra de atividades.

## v0.2.15 — 2026-11-03

- 🆕 A aba “Nova sessão” permite escolher o projeto, o agente, o modelo, o esforço de raciocínio, as permissões, a visão e um worktree isolado em torno de uma caixa de entrada centralizada, e inicia a sessão com a sua primeira tarefa; ela pode ser aberta pelo item “Nova sessão…” dos menus de nova sessão, pelo botão de estrela na barra de abas ou pelo painel central vazio.
- 🐋 O DeepSeek Harness (`dsh`) passa a funcionar como agente na visão de conversa, com cartões de ferramentas, raciocínio, solicitações de permissão, troca de modelo e de esforço, uso do contexto e um histórico que é retomado ao reabrir.
- 🧩 Copilot, Cursor, Kimi e Grok agora podem ser executados na visão de conversa.
- 🤖 O Zhipu ZCode passa a funcionar como agente na visão de conversa: quando o aplicativo de desktop ZCode está instalado, ele usa o agente integrado desse aplicativo, e é possível entrar com Z.ai ou Zhipu BigModel, ou alternar entre os dois, sem sair da conversa.
- 📊 No painel Info, as sessões do ZCode mostram os tokens, a taxa de acertos de cache e a velocidade de geração deste turno, o uso do contexto e o uso de 5 horas e semanal do GLM Coding Plan.
- 📦 O ZCode (no macOS e no Windows) e o DeepSeek Harness podem ser instalados pela própria conversa com “Instalar agora”, e a mensagem que falhou é enviada novamente assim que a instalação termina.
- ⬆️ Clicar em “Atualização” no selo de versão de um agente abre uma caixa de diálogo que executa o comando de atualização do próprio agente com a saída ao vivo e oferece “Reiniciar agente” para que a conversa continue na nova versão.
- 🔌 As conexões remotas com muitos terminais em atividade não caem mais repetidamente depois de se reconectarem.
- 🔄 Depois que um servidor remoto é reiniciado, as sessões não ficam mais presas na sincronização enquanto seus terminais são iniciados.
- 🔁 Quando a sincronização de uma conversa falha, ela é repetida automaticamente, e a área abaixo da caixa de entrada mostra o status com um botão “Tentar novamente”.
- 🐢 As conexões SSH com um host remoto muito sobrecarregado não expiram mais enquanto aguardam a resposta do servidor.
- 🔗 “Reconectar agora” em uma janela remota recria o túnel SSH imediatamente, em vez de esperar a próxima tentativa automática.
- 🪟 No macOS e no Windows, fechar a janela principal pede confirmação e mantém as janelas remotas abertas; no macOS, o aplicativo permanece no Dock, e reabrir a janela principal restaura o espaço de trabalho salvo.
- 🧾 Os arquivos alterados na lista “This turn” e no painel Git abrem seus diffs em uma aba, e não mais em uma caixa de diálogo.
- ⌨️ As sugestões de comandos do terminal não aparecem mais enquanto você digita: no Zsh, o comando mais recente do histórico que corresponde ao texto aparece em cinza na própria linha e pode ser aceito com → ou End, e a lista só aparece quando você pressiona Tab; escolha “Em linha + Tab”, “Apenas Tab” ou “Desativadas” em Configurações.
- 💬 Nas prévias de Artifact, é possível clicar em elementos da página para deixar comentários para a sessão, e as opções marcadas na página são listadas acima da prévia para que você navegue entre elas e escolha uma.
- 🔍 A busca em uma conversa usa pesquisa de texto completo, que encontra todas as suas palavras em qualquer ordem, e mostra “Sem resultados” em vez de “Carregando a conversa…” quando não há correspondências.
- 🧰 As habilidades incluídas, como vspawn e vrefer, agora são instaladas na pasta compartilhada `~/.agents/skills`, para que o Pi, o ZCode e o DeepSeek Harness possam usá-las, e o Codex não lista mais cada habilidade duas vezes.
- 📋 Na visão de conversa, os resultados de `/code-review` aparecem como uma lista de problemas encontrados com locais de arquivo clicáveis.
- 🌐 Quando a tradução de respostas está desativada em uma sessão, os cartões de pergunta oferecem um botão “Tradução” que traduz apenas aquele cartão.
- ✂️ As perguntas e as opções dos cartões de pergunta podem ser selecionadas e copiadas.
- 🌳 A árvore de sessões coloca sob a linha de um worktree os grupos e as sessões pai que pertencem a ele, inclusive as sessões iniciadas diretamente na pasta do worktree.
- 🌲 Para novas sessões, ações do Git e merges, os grupos aninhados usam o worktree do grupo pai mais próximo que tiver um.
- 🔎 Renomear um item enquanto a árvore de sessões está filtrada não faz mais com que ele desapareça dos resultados.
- ⏳ Quando o OpenCode atinge um limite de uso, uma única linha de erro indica qual é o limite, e a conversa continua automaticamente após a redefinição quando o OpenCode informa o horário em que ela ocorre.
- 📸 A sobreposição de captura de tela aparece mais rápido depois que você pressiona o atalho.
- 🚀 As janelas mostram um esboço do layout assim que abrem e carregam na inicialização menos da metade do código de antes.
- ⚡ `vspawn --yes` cria a sessão filha diretamente, sem mostrar o cartão de inicialização.
- 🛡️ As auditorias de código usam o Codex Security 0.8.2 e podem ser executadas com o DeepSeek, e os formulários de auditoria e de “Adicionar à base de conhecimento” usam os mesmos campos de agente, modelo e esforço das novas sessões, aceitando modelos que não estão na lista.
- 🌗 No macOS, quando o tema acompanha o sistema, a barra de título fica escura junto com o restante da janela.
- 📥 As instalações em um clique só informam a conclusão depois que o comando de instalação termina, e uma instalação incompleta do Zoo Code é tratada como não instalada, em vez de causar uma falha na inicialização.
- 🖥️ Mudar uma sessão para a visão de terminal não falha mais com “The initial task was already claimed”.
- 🧟 As sessões não informam mais “The previous agent is still running” depois que o ID de processo do agente antigo é reutilizado por outro programa.
- 🧯 Depois que o navegador se reconecta, as conversas não mantêm mais uma linha TransportError da desconexão.
- 🪞 Com o espelhamento ativado, abrir e fechar abas não transforma mais as sessões de conversa em espaços reservados “Restaurado do espaço de trabalho salvo”.
- 📜 As conversas do Claude que contêm uma resposta descartada voltam a carregar o histórico normalmente.
- 🧠 Os modelos integrados do Claude que não constam do catálogo de modelos do site continuam aparecendo nos formulários de nova sessão e de inicialização.

## v0.2.14 — 2026-10-23

- 🔌 As conexões remotas não caem nem se reconectam mais a cada segundo quando várias conversas longas estão abertas, e essas conversas voltam a abrir normalmente.
- ⏹️ As conexões SSH e WSL mostram um botão “Parar” durante a conexão, para interrompê-la em qualquer etapa, inclusive enquanto o servidor é preparado; durante uma conexão, “Cancelar” na caixa de diálogo de senha também a interrompe.
- 🌿 A árvore de sessões e os rótulos de branch não acumulam mais processos do Git em repositórios grandes, e as leituras do Git em segundo plano são encerradas após 30 segundos em vez de ficarem travadas.
- 🧩 O painel Git atualiza muito mais rápido em repositórios com submódulos: alterações dentro de um submódulo não fazem mais com que ele apareça no repositório principal, e um submódulo que aponta para um novo commit continua aparecendo.
- 📁 Os grupos podem ser arrastados para outros projetos junto com seus grupos aninhados e suas sessões, que passam a iniciar na pasta do novo projeto.
- ⏳ Ao abrir uma conversa longa, aparece um pequeno indicador “Carregando a conversa…” em vez de “O histórico ainda está sendo indexado”.
- 🖱️ O menu de contexto da sessão mostra “Encerrar processo” enquanto o processo da sessão está em execução, inclusive quando foi iniciado por outro cliente, e deixa de mostrá-lo quando o processo termina.

## v0.2.13 — 2026-10-22

- 📱 No celular, agentes em execução na visão de terminal abrem direto no terminal, com uma caixa de entrada flutuante para escrever ou ditar mensagens inteiras; os pequenos botões “Transcrição” e “Terminal” no cabeçalho alternam entre os dois, e a transcrição não sugere mais mudar para a visão de conversa.
- 🔑 Em conexões compartilhadas, o proprietário do host pode abrir o terminal de agentes em execução na visão de terminal e digitar nele, e a lista de sessões também mostra agentes sem visão de conversa, como o Kiro; contas convidadas continuam vendo uma transcrição somente leitura.
- 🌐 Traduzir manualmente uma resposta em uma sessão ociosa mostra a tradução em vez de “A tradução está indisponível”.
- 📜 Enquanto você rola para cima em uma conversa longa, as novas respostas não ficam mais presas atrás de “Carregar mensagens mais recentes”, e rolar para baixo a partir do histórico anterior carrega automaticamente as mensagens mais recentes.
- 🧭 A visão para celular não oferece mais “Mudar para a versão desktop”, e os celulares que mudaram antes voltam automaticamente à visão para celular.
- 🧹 Os registros de execução do `vrun` são excluídos automaticamente 24 horas depois que a tarefa termina.

## v0.2.12 — 2026-10-20

- 🛑 Interromper um turno do Claude na visão de conversa tem efeito imediato, e a próxima mensagem é enviada na hora, mesmo quando o computador está muito sobrecarregado.
- 🪟 Atalhos de menu como Cmd+D e cliques em notificações agem apenas na janela correspondente, então as janelas remotas não são mais divididas junto com a janela principal.
- 🔎 A busca global retorna resultados enquanto a indexação ainda está em andamento e mostra quantas sessões ainda estão sendo indexadas.
- ⚡ A busca global indexa as conversas do Claude, Codex, Pi e OMP diretamente dos arquivos, de forma muito mais rápida e sem encher o cache do histórico de conversas, e abrir um resultado leva exatamente à mensagem correspondente.
- 🧠 O histórico de conversas é lido diretamente dos registros originais dos agentes, em vez de uma cópia separada no disco, e as pastas `chat-index` e `chat-spool` são removidas na inicialização para liberar espaço em disco.
- 📱 No celular, sessões em execução na visão de terminal sugerem mudar para a visão de conversa; depois da confirmação, o aplicativo desktop também muda a sessão.
- 👀 Conexões compartilhadas listam as sessões em execução na visão de terminal como somente leitura, e só o proprietário do host pode mudá-las para a visão de conversa.
- 🌐 As configurações de tradução ganham “Ativar tradução por padrão”, que define o estado inicial da tradução nas sessões sem uma escolha salva.
- 🌳 A árvore de sessões agrupa sob uma única linha de worktree as sessões que usam o mesmo worktree.
- 📄 Respostas curtas na visão de conversa, inclusive mensagens com imagens, não exibem mais o link “Ler o conteúdo completo”.
- 🔕 O botão que limpa todos os indicadores de notificação passou para a extremidade esquerda da barra inferior da árvore de sessões.
- 🤖 As caixas de diálogo de nova sessão e de retomar sessão mostram o ícone de cada agente na lista de tipos de agente, e a lista de permissões abre para cima quando não há espaço suficiente abaixo.
- 🩹 A visão de conversa não informa mais que o histórico contém registros corrompidos quando o armazenamento está ocupado, e continua carregando logo em seguida.
- 📝 No editor de Markdown, Backspace e Shift+Tab removem a numeração de um item de lista vazio ou de um parágrafo sem numeração dentro de uma lista.
- 🔵 O chip “Tarefas” na visão de conversa fica ciano, a cor que a árvore de sessões usa para trabalho em segundo plano, enquanto ainda houver tarefas em segundo plano em execução, e na lista de tarefas as que estão em execução também aparecem em ciano com o rótulo “Em execução”.
- 🗑️ Excluir uma sessão não oferece mais a remoção de worktrees do git; para remover um, use “Delete worktree…”.
- 🔁 O status da tradução é atualizado quando muda, e não mais a cada 10 segundos, e as configurações de tradução verificam novamente os agentes instalados sempre que são abertas.
- 💤 O painel Info pausa a coleta de recursos do processo enquanto a janela está oculta.
- ⏱️ O indicador de trabalho e o cronômetro do turno na visão de conversa não somem mais nem ficam visíveis indevidamente quando um turno começa ou termina enquanto você troca de sessão.
- 🔗 `vrefer` numera as mensagens da mesma forma que `vsearch`, e nas sessões do Codex bifurcadas, `vrefer` e a busca global incluem o histórico da conversa de origem.

## v0.2.11 — 2026-10-18

- 🧪 As prévias HTML experimentais abrem ao lado das conversas ou em abas, atualizam com as alterações dos arquivos e oferecem controles de código, console e versões; a validação nativa e entre plataformas ainda está pendente.
- 📜 Conversas longas carregam mensagens antigas em páginas e expandem conteúdos extensos sob demanda, sem duplicar mensagens ou ferramentas nas conversas ao vivo do Claude e do Codex.
- 🔎 A busca global atualiza conversas alteradas mais rapidamente, retoma a indexação após interrupções e ignora conversas ilegíveis enquanto seus arquivos não mudam.
- 📊 A contagem de arquivos alterados no painel Info abre uma lista dos arquivos editados neste turno com as linhas adicionadas e removidas, quando disponíveis.
- 🗂️ Conversas arquivadas seguem seus projetos, grupos e sessões superiores originais em linhas compactas, com projetos recolhíveis e controles para expandir ou recolher tudo.
- 🧩 As habilidades dos plugins da conta Codex aparecem nas sugestões de preenchimento, e os menus permitem selecionar com as teclas de seta.
- ⌨️ No Windows e no Linux, Ctrl+Shift+C copia o texto selecionado no terminal, e o atalho pode ser alterado em Configurações ▸ Atalhos.
- 🧹 Após `/clear` no terminal do Claude, reabrir a sessão ou mudar para a visualização de conversa não retoma mais a conversa apagada.
- 📏 O painel Info mostra o limite de contexto do modelo nas conversas do OpenCode.
- 🌐 A tradução inclui Bing Translator sem chave de API, configurações compactas de idioma e tradução antes de reenviar mensagens editadas do Claude e do Codex.
- 🔤 As traduções existentes continuam visíveis após mudar o mecanismo ou modelo de tradução e podem ser traduzidas novamente.
- ⏱️ `vrun` informa o progresso a cada 180 segundos com o código de saída 75, permite continuar a espera com `--wait` e protege tarefas em execução contra a reutilização do mesmo nome.
- 🧵 Respostas do Claude iniciadas pela conclusão de tarefas em segundo plano têm seu próprio resumo da tarefa e separação de conversa, inclusive no histórico.
- ⚠️ A troca entre as visualizações de terminal e conversa pede confirmação enquanto houver tarefas em segundo plano em execução.
- 🏠 Sessões sem diretório de trabalho ou cujo diretório não existe mais iniciam na pasta pessoal em ambas as visualizações.
- 🪟 No Windows, iniciar o VelaTerm ou abrir o painel de conexão não solicita mais a instalação do WSL quando não há distribuições registradas.
- 🔐 Falhas de login remoto contam para o limite de tentativas mesmo quando a conexão fecha durante a verificação da senha.
- 🏷️ Os indicadores de versão dos agentes mostram um link explícito `Update: vX` quando há uma versão mais recente do CLI.
- 🔵 Os indicadores de tarefas em segundo plano ficam em ciano contínuo, e os pontos de status permanecem nas abas e na barra lateral, sem aparecer nos títulos dos painéis.
- ✍️ As bordas da entrada de conversa estão mais nítidas, com um contorno externo ao receber foco, e os valores do painel Info têm o mesmo brilho dos rótulos no tema escuro.
- 🌿 Os botões para clonar projetos usam um ícone de ramificação com um marcador de projeto.

## v0.2.10 — 2026-10-13

- 🧩 O OpenCode v2 é compatível junto com o v1, e a lista de modelos carrega corretamente no OpenCode v2.0.26.
- 🏷️ O cabeçalho da sessão mostra a versão da CLI do agente; quando há uma versão mais nova, o número da versão leva às instruções oficiais de atualização.
- 🗂️ O painel Git mostra até 1.000 arquivos alterados e sugere adicionar pastas grandes, como `node_modules`, ao `.gitignore`.
- 🔤 O painel Git exibe corretamente nomes de arquivo com caracteres não ASCII, prepara, remove da preparação e descarta apenas os arquivos escolhidos, e agrupa os conflitos separadamente.
- 📜 O histórico do Git mantém o intervalo carregado, os itens expandidos e a posição de rolagem ao ser atualizado.
- 🌿 O branch exibido no painel direito, na barra de status, no título do terminal e na barra lateral é atualizado assim que você troca de branch, sem reiniciar.
- 📁 A árvore de arquivos atualiza automaticamente as pastas abertas e marca as que contêm alterações do Git.
- 🧹 Windows: as atualizações automáticas não deixam mais instaladores na pasta Temp, e as sobras de atualizações anteriores são removidas.
- 🪝 Windows: Codex, Crush, Kiro, Cursor, Kimi, Antigravity, Copilot e Grok informam o status da sessão diretamente ao VelaTerm, sem scripts do PowerShell.
- 🖱️ Windows: programas de terminal em tela cheia, como o vim, recebem cliques e rolagem do mouse; a roda do mouse não vira mais teclas de seta.
- 🕘 Os terminais zsh voltam a ler e gravar o seu próprio `~/.zsh_history`, então `history` e a seta para cima também mostram os comandos de outros terminais.
- 🖥️ Novas sessões do Claude e do Codex abrem na visualização de conversa; os demais agentes abrem no terminal por padrão.
- 🚀 O `vspawn` aceita `--pi` e `--agent <nome>` e pode iniciar qualquer agente que aceite tarefas.
- ⏱️ Se a lista de modelos não carregar em 30 segundos, aparece um erro com Retry.
- 🗑️ Predefinições salvas podem ser excluídas em Configurações ▸ Agentes.
- ⌨️ A dica do botão de captura de tela mostra o atalho atual, e o ícone ficou mais fácil de distinguir do botão de compartilhar.
- 🧭 O painel direito volta a alternar de Conhecimento para Info e outras abas.
- ✍️ macOS: digitar `/` no terminal com um método de entrada não insere mais o caractere duas vezes.

## v0.2.9 — 2026-10-07

- ✨ A renomeação com IA lembra o modelo e o nível de raciocínio escolhidos para cada agente.
- ⛔ A geração de títulos com IA pode ser cancelada com Cancel, Esc ou um clique fora da caixa de diálogo, e você pode tentar novamente depois.
- 🧭 Novos fluxos Plan/Execute têm o papel Review independente desativado por padrão e permitem ativá-lo na caixa de diálogo de início ou com `--review`.
- 🤖 O seletor de modelos do Chat mostra os estados de carregamento, erro e lista vazia com Retry, e as opções do Codex correspondem à conta e à configuração da sessão atual.
- 📊 O painel Info mostra uma estimativa da velocidade média de saída, incluindo os tokens de raciocínio informados, ou um traço quando a estimativa não está disponível.
- 🩺 As configurações avançadas oferecem um diagnóstico opcional da latência de entrada com limite ajustável, sem registrar o texto digitado.
- ✂️ A ferramenta experimental de captura de tela tem um botão na barra de título das plataformas de desktop Tauri compatíveis, mesmo quando seu atalho de teclado está desativado.
- 🌬️ Os indicadores ciano de tarefas em segundo plano usam a mesma animação de pulsação na árvore de sessões, nos filtros de status e na barra de status.
- 📁 Grupos criados com um filtro de status ativo permanecem visíveis até atualizar o status ou alterar o filtro, desde que também correspondam à busca por nome.
- 🎨 Os logs de comandos em segundo plano exibem cores, estilos de texto e a última linha de progresso, e a barra de execução mantém visível o indicador de foco do painel.

## v0.2.8 — 2026-10-06

- 🪟 Windows: ao voltar para a janela, o foco do teclado permanece onde está quando já se encontra dentro da página.

- 🛡️ Windows: o antivírus Huorong não detecta mais o VelaTerm nem o vela-server como `Trojan/MSIL.ShellLoader.q`. Era um falso positivo: para executar comandos de shell na visualização de conversa, o VelaTerm criava cada processo em estado suspenso e depois o retomava, uma sequência de chamadas que softwares maliciosos também usam para injetar código. Agora os processos são iniciados normalmente, e o código que causava a detecção foi removido.

- 🔕 Windows: criar sessões e atualizar a lista de modelos não executa mais o PowerShell em segundo plano, e o status das sessões do Cursor passa a ser determinado pela saída do terminal, o que pode ser um pouco menos preciso.

- ⛔ Windows: na visualização de conversa, entradas que começam com `!` não são mais executadas como comandos de shell; um aviso é exibido e a entrada é mantida, enquanto os comandos anteriores no histórico continuam visíveis.

- 🔐 Conexões SSH com hosts Windows que usam o servidor OpenSSH integrado não iniciam mais o PowerShell com `-EncodedCommand`.

- 🚀 As sessões de agentes TUI iniciam corretamente mesmo com um PATH longo, sem exibir um comando de inicialização truncado no terminal.

- ⬇️ As sessões de agentes TUI voltam ao final quando um turno termina ou o agente solicita aprovação, exceto quando você está lendo a saída anterior.

- 💾 A recuperação experimental de conversas da v0.2.7 foi removida: o Chat não exibe mais mensagens Saved submission duplicadas nem avisos de interrupção, e as mensagens na fila não enviadas não são mais mantidas após uma reinicialização.

- 🔁 Os fluxos de trabalho de planejamento e execução ganham uma função Review independente e opcional, ativada por padrão nos novos fluxos, com agente, modelo e nível de raciocínio próprios; `vspawn --plan-execute` aceita `--review`, `--no-review`, `--review-agent`, `--review-model` e `--review-effort`.

- 🏷️ "Renomear com IA…" pede confirmação antes de começar e permite escolher o agente, o modelo e o nível de raciocínio para essa renomeação; a caixa de diálogo aparece centralizada enquanto as opções são carregadas.

- 🔍 Sessões, grupos e projetos podem ser arrastados enquanto uma busca por nome, um filtro de status ou um filtro de marca está ativo.

- ➕ Com um filtro de status ativo, uma nova sessão permanece sob seu grupo ou sessão pai até que o status seja atualizado ou o filtro seja alterado.

- 🪟 As janelas do aplicativo abrem maiores por padrão, de acordo com a área disponível da tela, e as janelas de conexão abrem centralizadas na tela da janela principal.

- 🎨 As coleções têm um novo ícone de camadas empilhadas, os ícones da barra de título escura ficaram um pouco mais suaves, os quatro botões da barra lateral vazia têm a mesma largura e as caixas de diálogo de formulário usam um único estilo de rótulo.

---

## v0.2.7 — 2026-10-05

- 🪐 Antigravity chega à visualização de conversa como agente experimental com texto, ferramentas, mensagens na fila e histórico nativo; imagens, instruções durante uma resposta, aprovações interativas, bifurcação e retrocesso continuam indisponíveis, e a validação completa da recuperação em todas as plataformas ainda está pendente.

- 🗂️ As coleções podem conter subcoleções, projetos e grupos de sessões, ser criadas ou importadas no espaço de trabalho e preservar seus projetos e sessões arquivadas quando são excluídas.

- ⚠️ Versões anteriores não devem usar um banco de dados cujas pastas foram migradas para coleções; para voltar a uma versão anterior, é necessário restaurar um backup feito antes da migração.

- 🏷️ A renomeação inteligente nomeia a sessão com base na conversa usando as configurações do agente atual e permite escolher um agente quando essas configurações não podem ser usadas.

- 🌱 Novas sessões usam o ambiente shell atualizado e os agentes recém-instalados sem reiniciar o VelaTerm.

- 💾 Após uma reinicialização, a recuperação experimental de conversas restaura mensagens, imagens e entradas na fila salvas anteriormente e pausa o trabalho interrompido até a retomada explícita; a validação completa em ambientes nativos e em todas as plataformas ainda está pendente.

- 📸 A captura de tela experimental nos aplicativos Tauri para macOS e Windows permite selecionar uma área, fazer anotações, copiá-la ou salvar o PNG original; a validação dos aplicativos nativos ainda está pendente, e Electron, navegador e visualizações remotas não são compatíveis.

- 🧵 As abas de tarefas do Claude em segundo plano exibem todo o histórico nativo da conversa, incluindo chamadas de ferramentas em paralelo.

- 🖼️ Cada mensagem de conversa pode conter até 20 imagens, com limite de 5 MiB por imagem.

- 📍 Os marcadores de mensagens permitem visualizar uma prévia e ir diretamente para mensagens anteriores do usuário.

- 🧭 Os seletores de diretório para projetos, clonagem e Salvar como oferecem edição de caminhos e preenchimento automático de locais de forma uniforme.

- 🪪 `vself` lê as configurações salvas das sessões e as relações entre sessões pai e filhas, enquanto `vflow list` lista os fluxos de planejamento e execução relacionados sem alterá-los.

- ⌨️ Selecionar tudo no terminal pode ser configurado nos atalhos, usa Cmd+A no macOS e Ctrl+Shift+A nas demais plataformas por padrão e só se aplica quando o terminal está em foco.

- 🐚 O preenchimento automático do Bash aceita caminhos que começam com `~`, e versões antigas do Bash abrem sessões sem erros de PS0.

- 🛑 Os hooks das tarefas filhas não alteram mais o estado da conversa da sessão pai, e fechar uma aba de conversa interrompe seu processo Chat ativo.

- 🪟 O Windows restaura o foco do teclado do terminal ao voltar à janela e remove a linha azul durante a composição de texto em chinês.

- 🎨 O tema escuro clássico tem contraste mais nítido, os ícones do desktop acompanham o tema do sistema, os ícones de projetos são azuis e as coleções não exibem mais a quantidade de projetos.

- 📚 Os menus de sessões e coleções usam um rótulo mais claro para adicionar conteúdo à base de conhecimento.

- 🌐 O acesso ao VelaTerm por HTTP sem criptografia em uma rede local abre a página corretamente.

- 🌍 As verificações de atualização incluem o idioma da interface para exibir as notas de versão correspondentes, e o serviço aceita o atualizador Electron do Linux.

- 🧰 Os nomes dos agentes Kimi Code e Grok Build não incluem mais os números de versão do modelo.

---

## v0.2.6 — 2026-09-30

- 🐧 A versão para Linux agora é baseada em Electron. O AppImage mantém o nome do arquivo e seus dados, não exige mais WebKitGTK nem libfuse2, e as instalações existentes são atualizadas para ela pelo atualizador integrado.

- 🛰️ Máquinas que não podem ser acessadas por SSH, como o WSL em outro computador ou um contêiner Docker, podem ser vinculadas à sua conta após a instalação do `vela-server` com um único comando e passam a aparecer na lista Remote para conversas com a IA. Depois que o proprietário concede acesso completo no host, o aplicativo para desktop também pode usar o terminal, os arquivos e o painel Git dessa máquina.

- 📂 Pastas arrastadas para a barra lateral a partir do Finder, do Explorador de Arquivos ou de um gerenciador de arquivos do Linux são adicionadas como projetos.

- 📝 O editor de Markdown ganhou os atalhos do Typora para títulos, listas, blocos de código e tabelas, fechamento automático de parênteses e aspas, abertura de links com Cmd/Ctrl+clique, os modos foco e máquina de escrever, e a contagem de palavras e caracteres na barra de status. O front matter YAML aparece em uma caixa separada acima do documento e permanece inalterado enquanto você não o editar.

- 🗃️ Quando uma pasta não é um repositório Git, mas contém vários, o painel Git oferece um seletor de repositório e trabalha com o repositório escolhido.

- ⏳ Na visualização de conversa, uma sessão cuja resposta terminou enquanto tarefas em segundo plano continuam em execução mostra o novo status "Tarefas ativas", com um ponto ciano, em vez de permanecer em "Em andamento". Sessões que executam tarefas longas com `vrun` mostram o mesmo status.

- 🍴 Na visualização de conversa, uma sessão bifurcada inicia a própria conversa na primeira execução, e a sessão de origem deixa de receber as mensagens dela.

- 🛡️ Na visualização de conversa, o rótulo de modo de uma sessão do Claude acompanha o modo de permissão que o Claude realmente usa, por exemplo depois de entrar no modo Plan, e os cartões de permissão mostram o motivo que o Claude informa para a solicitação.

- 📏 A caixa de entrada da visualização de conversa pode ser redimensionada arrastando a borda superior, e um clique duplo na borda restaura a altura padrão. A altura vale para todos os painéis e é mantida após reiniciar.

- 🗂️ Quando várias pastas estão selecionadas na barra lateral, o menu de contexto permite arquivá-las junto com suas sessões.

- 📚 As coleções sempre aparecem acima dos projetos na barra lateral.

- 🧠 Quando um registro de processamento da base de conhecimento criou apenas uma entrada, o título abre essa entrada diretamente, e os botões de entrada mostram os títulos em vez de números.

- 📊 O painel Info mostra o limite de contexto correto para modelos mais recentes do Claude, como o Opus 5.5, em vez de 200k.

- 🎨 Mudanças menores: as três opções de abertura em painel dividido do menu de contexto das sessões foram agrupadas no submenu "Abrir em painel dividido", a confirmação de saída avisa que as janelas remotas abertas também serão fechadas, e as listas suspensas dos formulários se fecham assim que uma opção é escolhida.

---

## v0.2.5 — 2026-09-28

- 🐧 O aplicativo para Windows permite abrir espaços de trabalho separados no WSL1 e no WSL2, usando os agentes, arquivos e histórico de sessões da distribuição Linux selecionada.

- 🔄 Os espaços de trabalho do WSL permitem reconectar após a parada do servidor e, ao fechar uma janela, escolher entre parar o servidor ou manter as sessões em execução.

- 🌐 As sessões do Claude ganham a opção “Chrome” na área de entrada, que ativa ou desativa o Claude in Chrome sem reiniciar a conversa. Cada sessão guarda a própria escolha, e as sessões sem escolha seguem o padrão.

- 📟 A aba de uma tarefa de shell em segundo plano mostra o comando executado e a saída mais recente, atualizada enquanto a tarefa está em execução.

- 🧩 Quando as Vela Skills não estão instaladas, a barra de status oferece a instalação. A caixa de diálogo descreve cada uma, instala todas diretamente e pode ser configurada para não lembrar novamente.

- 🪪 As sessões filhas criadas com `vspawn` recebem o ID da sessão pai em `VLX_PARENT_SESSION_ID`, o que permite que um agente se comunique com ela por meio de `vrefer` e `vtell`. O novo comando `vself` mostra a sessão atual e as sessões acima dela.

- 🖥️ O AppImage para Linux não abre mais uma janela em branco em distribuições recentes como o Fedora 44.

- 🎨 Mudanças menores: “Tarefas em segundo plano” aparece por padrão abaixo da área de entrada e continua no menu “Mais” se você a mover de volta para lá; quando um link não pode ser aberto, uma mensagem informa que é possível copiar o endereço com o botão direito.

---

## v0.2.4 — 2026-09-26

- 📋 Copiar da conversa entrega o texto como ele aparece na tela: sem crases em torno do código em linha, sem asteriscos no destaque, links apenas com seu texto, blocos de código sem cercas, células de tabela separadas por tabulações e listas com os marcadores que você vê. O formato com estilos continua indo junto para a área de transferência, e o menu de contexto ganha "Copiar como Markdown" para obter o código Markdown da seleção.

- ⏳ A continuação automática após o limite de uso ser reiniciado agora vem ligada. Se você já tinha alterado essa opção, sua escolha é mantida.

- ↩️ Reverter uma conversa não fica mais bloqueado por uma tarefa que já terminou: uma tarefa em primeiro plano é considerada concluída quando seu turno acaba, e o progresso que chega depois não volta a marcá-la como em execução. Quando de fato há uma tarefa em segundo plano rodando, a mensagem informa o nome dela.

- 🧹 As threads de subagentes do Codex não aparecem mais na lista do histórico de sessões, assim como já acontece com as sidechains do Claude.

- ⌨️ Autocompletar no terminal: depois de percorrer os candidatos com as setas, o Enter aceita o item destacado da mesma forma que o Tab. O Enter continua indo para o shell se você não moveu a seleção, se continuou digitando após escolhê-la ou se há uma tecla modificadora pressionada.

- 📱 O Android é compilado em dois canais. A versão padrão traz os canais de push da Getui, Huawei, Xiaomi, OPPO, vivo, Meizu e Honor; a versão da Play vem sem eles e indica que as notificações de tarefa estão sem canal de push configurado.

- 🎨 Mudanças menores de interface: listas de tarefas não repetem mais a caixa de seleção, a barra de notificações e o aviso de continuação automática ficam entre a conversa e o campo de escrita com a mesma largura do texto das mensagens, e no celular o botão de enviar permanece ao lado do campo quando as opções estão recolhidas.

---

## v0.2.3 — 2026-09-24

- 🪟 Sessões existentes podem ser movidas para os painéis divididos. O menu de contexto da barra lateral abre uma sessão em um painel à direita, um painel abaixo ou no painel em foco; ao arrastar uma sessão da barra lateral até a borda de um painel, a divisão segue aquela direção, e soltá-la no meio substitui a sessão ali exibida. De duas a quatro sessões selecionadas podem ser dispostas lado a lado em uma mesma aba dividida igualmente, e as sessões presentes nos demais painéis da aba atual ficam sinalizadas na barra lateral.

- 🗂️ As tarefas em segundo plano iniciadas por um agente abrem em abas próprias ao lado da conversa, com status, tempo decorrido, tokens, chamadas de ferramentas, a última ferramenta informada e as fases de cada agente. Cada tarefa tem seu próprio endereço e, ao sair dela, a visualização volta ao painel de onde foi aberta.

- 💬 Na visualização de conversa, uma mensagem que começa com `!` é executada no shell da sessão. A saída aparece conforme chega, o código de saída é exibido, o comando pode ser cancelado enquanto executa e permanece no histórico de leitura das sessões Claude, Codex, OpenCode, Pi e OMP.

- ⏱️ O novo comando `vrun` inicia um comando demorado e aguarda por ele em uma única chamada, de modo que o agente descobre quando o trabalho realmente terminou. Os comandos iniciados assim aparecem acima do terminal com o tempo de execução, uma janela de log e um botão de parada que pede confirmação.

- ⌨️ A criação de sessões de agente ganhou página própria e atalho de teclado: pesquise agentes e predefinições, reaproveite a última utilizada e escolha se a sessão será criada no mesmo nível da atual ou abaixo dela.

- 🧰 A barra de ferramentas de digitação é configurável nas configurações: escolha quais itens ficam ao lado da mensagem e em que ordem. Os itens desativados, assim como os que não cabem na largura, continuam disponíveis no menu Mais.

- 📥 Ao baixar um arquivo em uma janela de conexão por URL ou SSH, primeiro escolhe-se onde salvá-lo nesta máquina e, em seguida, o progresso do download é exibido, com um botão para cancelá-lo.

- 🗃️ Sessões do Kiro podem ser importadas para um projeto e consultadas: a importação associa sessões pelo diretório de trabalho e cada uma abre em uma visualização de histórico somente leitura com busca. Por enquanto há suporte a registros apenas de texto.

- 🧠 A organização da base de conhecimento funciona com Grok, OpenCode, Pi e OMP além de Claude e Codex, e o agente é escolhido no mesmo tipo de lista suspensa usada no restante do aplicativo.

- 📱 iOS e Android: a página inicial guarda suas conexões SSH e URL, permite entrar em uma conta VelaTerm e lista os dispositivos que compartilham conteúdo nela. A impressão digital de um host é confirmada uma vez e fica memorizada, um QR code preenche o endereço do serviço, e todas as telas nativas e avisos do sistema estão traduzidos nos 11 idiomas da interface.

- 🔐 As sessões do Claude iniciam no modo de permissão escolhido: os argumentos de inicialização adicionais mantêm a precedência prevista, o modo é conferido com o que a CLI informa na inicialização, e uma sessão iniciada sem confirmações permanece marcada dessa forma.

- 🧩 O menu de modelos lista o Opus 5.5 e acrescenta os modelos adicionais informados pela CLI selecionada, preservando a ordem original; um modelo que chega com uma atualização da CLI aparece sem reiniciar o aplicativo.

- 🌱 As solicitações de sessão derivada resistem a interrupções: uma solicitação cuja resposta se perdeu pode ser recuperada depois de reconectar, uma inicialização já confirmada reutiliza a mesma sessão e as mesmas configurações ao ser repetida, e uma árvore de trabalho que não pôde ser criada é revertida sem tocar no que já existia.

- ⚡ O aplicativo inicia mais rápido: o código carregado na inicialização tem cerca de metade do tamanho anterior, e as páginas de base de conhecimento, auditoria de segurança, importação de sessões e projetos compartilhados são carregadas ao serem abertas.

- 🖼️ Imagens coladas ou arrastadas para uma mensagem são reduzidas a 1568 pixels no lado maior antes do envio.

- 🐚 As sessões Bash carregam o autocompletar do shell a partir de um arquivo de inicialização, então uma nova sessão Bash não abre mais com um comando já digitado. Os arquivos de perfil de login continuam sendo lidos na ordem própria do Bash.

- ✍️ Markdown: um til isolado não risca mais o restante da linha, de modo que um prompt de shell colado continua legível, e o negrito ou itálico que termina junto a caracteres chineses, japoneses ou coreanos fecha corretamente, sem deixar asteriscos na tela.

- 📨 `vtell --steer` entrega a mensagem dentro do turno que o destinatário está executando, em vez de esperar esse turno terminar. Se o destinatário está parado em uma pergunta, o retorno é blocked, pois a mensagem só é lida depois que essa pergunta for respondida.

- 🔁 Na visualização de conversa, o status de uma sessão termina junto com o turno, a marca de não lido some quando o trabalho realmente começa, e uma sessão com trabalho em segundo plano em andamento continua marcada como ativa.

- 🪟 Windows: os hooks do Cursor iniciam corretamente, e a camada de janelas foi atualizada em resposta aos problemas de entrada de teclado relatados após uma reconexão RDP ou troca de área de trabalho virtual.

- 🛡️ Auditoria de segurança: a lista de modelos e os nomes dos agentes vêm do mesmo catálogo de inicialização usado no restante do aplicativo, de modo que execuções anteriores mostram o nome atual de cada agente.

- 🩹 Outras correções: as mensagens de erro da conversa se alinham à coluna central; as duas caixas de diálogo de inicialização sempre podem ser fechadas e uma inicialização confirmada que falhou pode ser cancelada; mensagens e mensagens na fila mostram quem as enviou; uma fila longa rola dentro de uma altura fixa; âncoras HTML vazias não exibem mais uma marca durante a edição de um documento; e o filtro de status no celular passa a incluir as sessões que começam a corresponder a ele depois.

---

## v0.2.2 — 2026-09-15

- ⏳ Retomada automática após limites de uso, desativada por padrão nas configurações: quando o Claude ou o Codex para em um limite de 5 horas ou semanal, a sessão continua sozinha assim que o limite é redefinido. Acima da área de escrita aparece um aviso com o horário da redefinição e um botão "Cancelar"; a espera sobrevive ao reinício do aplicativo e termina ao enviar uma mensagem, reverter, limpar a sessão ou desligar a configuração.

- 🪟 Windows: as instalações em um clique de OpenCode, Grok e Crush passam `--allow-scripts`, e o npm gera o executável; Cursor, OMP e Antigravity são procurados na pasta de instalação real em `%LOCALAPPDATA%`, e o OMP também respeita `PI_INSTALL_DIR`.

- 🪟 Windows: arquivos ainda sendo baixados ou copiados não contam mais como instalados, o cartão de instalação espera a instalação terminar de fato antes de informar sucesso, e um caminho salvo desatualizado é substituído pelo recém-localizado.

- 🧩 Visão de conversa: o raciocínio, as chamadas de ferramentas e as respostas intermediárias de um turno podem ser ocultados com "Ocultar passos", deixando só a resposta final; a barra de ferramentas oculta ou mostra todos os turnos de uma vez, e a busca expande um turno oculto para localizar uma ocorrência.

- 🔐 O botão de permissões agora mostra o modo com que a sessão foi iniciada em vez de "Permissões atuais não confirmadas"; a linha duplicada de "ajuste de início" saiu, e uma sessão iniciada com as confirmações ignoradas volta a ficar destacada na barra de status.

- 🔐 Uma sessão sem permissão própria segue o padrão global do seu tipo de agente nas duas visões, e editar uma sessão não transforma mais o valor herdado em uma escolha fixa da sessão.

- ↩️ As páginas da base de conhecimento ganham um botão "Subir um nível": uma entrada volta para a sessão, depois para o projeto e para o início, e sessões arquivadas, notas e pastas sobem do mesmo jeito.

- 🌱 Uma sessão filha abre na mesma visão da sessão pai: a partir de uma sessão na visão de conversa, a tarefa chega como primeira mensagem e as imagens como anexos; a partir de uma sessão no terminal, ela inicia no terminal com a tarefa passada como argumento.

- 🎨 macOS: a janela e a faixa da barra de título assumem as cores do tema antes de a janela aparecer, e trocar de tema em execução as redesenha na hora.

- 🗜️ As sessões do Claude mantêm no histórico de leitura os turnos anteriores a uma compactação, enquanto reverter continua descartando o ramo abandonado.

- 🔎 A busca na conversa mantém a ocorrência selecionada no lugar enquanto carrega histórico mais antigo; Enter vai para a ocorrência anterior e Shift+Enter para a seguinte.

- 🩹 Voltar para uma conversa parada no meio do histórico restaura a posição de leitura em vez de mostrar o painel em branco.

- 🧭 As informações de diagnóstico do catálogo de modelos só aparecem ao abrir o menu de modelos com a tecla Option pressionada; as linhas de erro se alinham ao texto das mensagens e a base de conhecimento em inglês mostra "Archived Sessions".

---

## v0.2.1 — 2026-09-14

- 🔎 A página inicial da base de conhecimento ganha uma caixa de busca: uma consulta pesquisa ao mesmo tempo o conhecimento de sessão e as notas locais, agrupando os resultados por origem. As correspondências exatas vêm primeiro; a busca aproximada (abreviações, subsequências e erros de digitação) só entra quando não há nenhuma correspondência exata, e consultas em chinês continuam por subcadeia.

- 🗂️ As sessões arquivadas passam para a base de conhecimento. O botão da barra lateral leva até lá em vez de abrir um painel próprio, a árvore ganha uma raiz "Sessões arquivadas" agrupada pelo projeto de origem, e a área principal lista todas as sessões arquivadas com restaurar, reorganizar, exportar e excluir no próprio lugar.

- 🔍 As sessões arquivadas têm busca em texto completo: os resultados são agrupados por sessão com a contagem de ocorrências, e o painel de prévia percorre cada ocorrência com o mesmo realce da busca global. Ao abrir uma sessão, você alterna entre a conversa e as entradas de conhecimento.

- 🤖 Visão de conversa: quando o executável de um agente está ausente, um guia de instalação aparece sob a mensagem em vez do erro de inicialização bruto, e "Instalar agora" passa para a visão de terminal, que executa o comando recomendado. Se o agente estiver instalado fora do PATH, o guia aceita o caminho do executável diretamente, com seletor de arquivos do sistema no desktop.

- 🩹 Instalações que parecem presentes, mas estão quebradas, agora são detectadas: um wrapper npm global cujo destino foi removido ou substituído é tratado como não instalado, e um caminho configurado que aponta para esse wrapper abre o guia de instalação sem reescrever sua configuração.

- 🔐 O item do modo de permissão mostra direto o modo escolhido, e o menu marca em cada linha se a opção já está valendo ou aguarda o próximo turno.

- 🔽 Todos os menus suspensos agora usam o componente Select interno em vez do controle nativo, então ficam iguais no macOS 15 e 26, sem a sobreposição do controle do sistema.

- ℹ️ Projetos e coleções têm uma caixa de informações no menu de contexto.

- 🪟 Windows: os subprocessos iniciados pela visão de conversa (agentes, catálogo de modelos e verificações do git) não piscam mais janelas de console.

- 💡 O botão de feedback da barra de título passa para depois do botão de compartilhar.

---

## v0.2.0 — 2026-09-13

- 📱 Aplicativo VelaTerm para iOS e Android (versão preliminar): conecte-se a uma máquina por SSH ou URL após confirmar a impressão digital do host, carregue a interface remota completa dentro do aplicativo, preencha os dados de conexão lendo um código QR e entre em uma conta remota.

- 🛡️ Auditoria de código experimental: inicie pelo menu de contexto de um projeto uma auditoria do repositório inteiro, de um diretório ou arquivo, ou das alterações não confirmadas na árvore de trabalho, com o Codex ou o Claude Code; revise os achados em relação ao código-fonte e exporte um relatório em Markdown ou JSON.

- 📓 Base de conhecimento local: abra uma pasta de notas Markdown no painel direito, edite as notas no editor WYSIWYG, pesquise por caminho e em texto completo, gerencie etiquetas e favoritos e restaure notas excluídas da lixeira. Os agentes podem consultar as notas com `vkb`; importações de pastas rodam em segundo plano e deixam um registro que você pode expandir, revisar ou cancelar.

- 🤖 Sessões de planejamento e execução: `vspawn --plan-execute` abre uma sessão de planejamento que divide a tarefa em sessões de execução; `vflow` propõe a divisão, `vtell --report` devolve o resultado de cada execução para aceite e um retrabalho reutiliza a sessão de execução original. Os worktrees podem ser compartilhados por todos os papéis ou criados por sessão.

- 🔗 Projetos e sessões compartilhados agora carregam a interface real do host atrás da URL compartilhada, encaminhada por um túnel de saída e limitada ao projeto ou à sessão autorizados. Dispositivos e autorizações podem ser gerenciados nas páginas da conta.

- 🧠 A base de conhecimento de sessão ganha grupos de projetos, sessões e entradas com arrastar e soltar, renomear e excluir; uma nova organização da mesma sessão substitui a que estava na fila.

- 💬 Visualização de conversa: históricos longos carregam página a página até a primeira mensagem, todos os agentes compatíveis com o mecanismo de conversa (incluindo OMP) abrem a visualização de conversa por padrão, e turnos consecutivos do mesmo agente compartilham uma única linha de autor.

- ⌨️ Terminal: as sugestões nativas do shell oferecem preenchimento com Tab para zsh, bash, fish e PowerShell no macOS e no Linux, as setas continuam recuperando comandos anteriores com a lista aberta, e o núcleo do terminal passa para o xterm 6.

- 📊 O painel Info mostra estatísticas do turno atual para Claude, Codex, Grok, OpenCode, Pi e OMP: tokens de entrada e saída, taxa de acertos de cache, velocidade de geração, chamadas de ferramentas e modificações de arquivos registradas. Os controles menos usados da área de escrita passam para "Mais".

- 🔐 A área de escrita mostra o modo de permissão configurado, o vigente e o pendente, e pede confirmação quando é preciso reiniciar para aplicar uma mudança.

- 🔔 As notificações mostram o nome da sessão e uma prévia curta; clicar abre a sessão, e o aplicativo móvel pode recebê-las pelo serviço de notificações push do sistema.

- 🌐 A lista de modelos do Claude agora vem do catálogo de modelos publicado no site, com cache local e atualização a cada seis horas, combinada com os modelos informados pela CLI.

- 🧵 As setas para cima e para baixo na área de escrita recuperam suas mensagens anteriores, incluindo as enfileiradas e as pendentes de confirmação, e restauram no fim o rascunho não enviado.

- ↩️ Reverter uma mensagem devolve suas imagens à área de escrita para que possam ser enviadas novamente.

- 🔑 Agentes iniciados no macOS herdam o ambiente completo do shell de login, então ferramentas instaladas fora do PATH padrão também são encontradas.

- 💡 Uma entrada de feedback na barra de título abre a página de feedback.

- 🕹️ Uma entrada da Central de jogos na barra de abas abre a central de jogos do site (PIXEL WING); no desktop, o jogo abre no navegador integrado.

---

## v0.1.108 — 2026-09-08

- 💬 Visualização experimental de conversa para Claude, Codex e OpenCode, com respostas em streaming, raciocínio, detalhes de ferramentas, permissões e formulários de perguntas. Novas configurações continuam usando o terminal por padrão.

- 🎛️ Os controles da conversa oferecem configurações de modelo, fila de mensagens, intervenções, preenchimento de arquivos e imagens anexadas, conforme os recursos de cada mecanismo.

- 🔎 Busca na conversa, links de arquivos, ações em imagens e fontes independentes facilitam a leitura. O histórico remoto carrega progressivamente e os detalhes de ferramentas são obtidos sob demanda.

- 📨 Os recibos de envio ajudam a conferir resultados após desconexões. Envios incertos permanecem aguardando confirmação, sem reenvio automático.

- 🤝 Os novos comandos `vrefer` e `vsearch` leem e pesquisam outras sessões; `vrefer --ask` delega a leitura a um agente. `vorch` e `vstat` oferecem coordenação de vários agentes e consulta de status.

- 🧠 As entradas de memória aceitam etiquetas, edição direta, proteção de alterações não salvas e configurações de organização. Os filtros e a seleção na importação de histórico e a navegação do grafo foram melhorados.

- 🌐 O cliente de compartilhamento público aceita vinculação de conta e dispositivo, acesso limitado a sessões AI e conexões de retransmissão criptografadas. Uploads de imagens por convidados e formulários MCP complexos ainda não são suportados.

- 🔄 Os IDs de retomada do Codex são conferidos com o histórico persistido. A ausência confirmada do histórico gera um erro explícito em vez de iniciar uma sessão vazia; terminais já em execução continuam acessíveis para reconexão.

- ↩️ A reversão segue os recursos do mecanismo: o Codex restaura apenas a conversa, sem restaurar arquivos. O tratamento do escopo e as verificações prévias de diretório no OpenCode ainda têm limitações conhecidas.

- 📦 As mudanças de versão preservam as versões travadas das dependências. A validação de instalação e atualização multiplataforma e da integração AI real continua pendente; verificações automatizadas não a substituem.

---

## v0.1.107 — 2026-09-05

- 🧠 Memória global (experimental): o Claude ou o Codex transforma suas conversas em uma wiki compartilhada organizada por tema
- 🕸️ Grafo de código (experimental): indexe um diretório de trabalho, navegue pelas relações entre símbolos e vincule-as às memórias
- 🔎 Os agentes podem consultar código e memória durante a sessão com `vknowledge`
- 🖥️ O SSH pode espelhar o app de desktop remoto: as mesmas abas, divisões e sessão ativa nas duas máquinas
- 🪟 As máquinas remotas por SSH agora podem ser Windows
- 📥 Importe as sessões do Codex, Claude e OpenCode que já existem em uma pasta de projeto
- 🤖 Novo agente: OMP
- 🎚️ O `vspawn` pode escolher o modelo e o esforço de raciocínio de uma sessão filha
- 🌿 O `vspawn-tree` funciona em coleções, e as sessões de worktree em execução exibem um ícone de branch
- 🔤 A busca prioriza palavras inteiras e destaca exatamente o que encontrou
- 🖱️ Clique com o botão do meio para fechar uma aba
- ⌨️ Navegadores no macOS: ⌘D e ⌘⇧D dividem o painel
- 💬 A digitação com IME não desloca mais a tela, e o cursor fica visível durante a composição
- 🪓 Uma divisão feita pelo menu afeta apenas a janela em foco, e toda divisão é registrada em `logs/split.log`
- 📁 O seletor de pastas remoto mantém o caminho que você digita
- ℹ️ Painel de informações: horário de início e tempo de execução na mesma linha, com a carga média no macOS
- 🔑 Os campos de senha não exibem mais o botão de revelar do próprio navegador

---

## v0.1.106 — 2026-09-02

### Espaço de trabalho

- **A barra lateral agora conta com coleções: contêineres de nível superior que não estão vinculados a uma pasta no disco.** Algumas sessões nunca pertenceram a um repositório — como sessões remotas, páginas do navegador ou um terminal aberto apenas para fazer um teste rápido. Até agora, o único lugar para colocá-las era dentro de um projeto, onde ficavam deslocadas. Uma coleção é uma linha independente com um nome próprio e sem nenhuma pasta correspondente no disco. Como não há uma raiz de projeto, ações irrelevantes agora ficam ocultas logo de início em vez de falharem depois: "New Worktree Session", "Move to Worktree…" e o seletor de worktree na janela de nova sessão ficam todos ocultos para coleções, em vez de exibirem um erro ao serem clicados. No lugar deles, o diálogo de nova sessão traz um campo de diretório de trabalho (Working directory) cujo padrão é o seu diretório pessoal (home), acompanhado de um seletor nativo de pastas no desktop. Sessões iniciadas sem um diretório especificado agora abrem no seu diretório home em vez de herdarem o caminho de inicialização do app (que era `/` no macOS).

- **O painel de Recursos agora mostra toda a máquina, não apenas a sessão atual.** Antes, o painel informava apenas o uso de CPU e memória da árvore de processos da própria sessão, tornando impossível saber se a sessão estava consumindo todos os seus recursos ou se a máquina já estava sobrecarregada. Agora o painel é dividido em THIS SESSION e SYSTEM. A seção SYSTEM exibe barras de progresso para CPU, memória e swap (ficando em âmbar acima de 70% e vermelhas acima de 90%), além de uma linha específica para cada plataforma: pressão de memória (memory pressure) no macOS, médias de carga (load averages) normalizadas pela contagem de núcleos no Linux, e nenhuma das duas no Windows (que não possui uma métrica equivalente). Se você só quiser ver a sua sessão, desmarque "system" no cabeçalho do painel — as linhas do sistema desaparecem e a amostragem em segundo plano é interrompida por completo. Essa preferência é salva e aplicada ao painel de todas as sessões.

- **O Windows e o Linux ganharam uma barra de menus, acessível pela tecla Alt.** O macOS já conta com menus de sistema nativos, mas no Windows e no Linux, configurações, verificação de atualizações e divisão de painéis antes só ficavam acessíveis pelo ícone da barra de título. Pressionar e soltar apenas a tecla Alt agora alterna a visibilidade da barra de menus. Combinações de teclas com Alt continuam funcionando normalmente — como o Alt funciona como o prefixo Meta nos terminais, atalhos como Alt+B e Alt+F continuam sendo passados para o shell; apenas o pressionamento isolado do Alt, que não envia nada ao terminal, aciona o menu. A barra inclui File (configurações, verificar atualizações), Terminal (novo terminal, dividir à direita, dividir abaixo) e Help (site, feedback, compartilhar), exibindo dicas de atalhos com base nas suas combinações de teclas personalizadas. Ela suporta navegação tanto por mouse quanto por teclado: pressionar Esc fecha o menu aberto, oculta a barra e devolve o foco ao terminal.

- **A opção "Clear notification badges" no menu remove um badge que nenhuma outra ação conseguia tirar.** O contador no ícone do Dock soma as sessões não lidas e os cards de confirmação de criação pendentes. No entanto, o botão de limpar na barra lateral só aparecia quando havia sessões não lidas e limpava apenas essas sessões. Se sobrasse algum card de solicitação não respondido — como o de uma janela fechada ou de uma sessão excluída onde o card não podia mais ser renderizado —, o ícone no dock exibia o número 1 sem nenhuma forma de removê-lo. O novo item de menu marca todas as sessões como lidas, resolve todas as solicitações de criação pendentes como recusadas e redefine o contador de notificações do sistema operacional diretamente para zero. Isso é especialmente importante no macOS, que mantém a contagem do badge do Dock mesmo após reiniciar o aplicativo.

- **A janela não trava mais enquanto comandos leem arquivos ou aguardam o sistema.** Comandos síncronos do Tauri são executados na thread principal, congelando a interface do usuário durante toda a sua duração. Uma auditoria de todos os 43 comandos revelou 13 realizando operações de I/O na thread principal: solicitar permissões de notificação no macOS podia bloquear a interface por até um minuto aguardando a confirmação do usuário; ler gravações de sessões longas percorria dezenas de megabytes em blocos de 64 KB; colar capturas de tela gravava diretamente no disco; e buscar o diretório de trabalho de uma sessão executava o `lsof`. Todos os 13 comandos agora rodam fora da thread principal. O processamento de teclas digitadas permanece no caminho rápido (fast path), sem alterações.

### Acesso remoto

- **Os hosts agora mostram quais clientes remotos estão conectados.** Como o espelhamento de sessão é bidirecional, um cliente remoto pode reorganizar o layout do host — mas antes o host não tinha como saber se havia alguém conectado, muito menos quem. A barra de título agora exibe um badge "Mirrored by N"; clicar nele revela uma lista de clientes conectados com seus nomes, endereços IP e horários de conexão. O nome do cliente é informado durante o handshake criptografado (recorrendo a "Unnamed" se nenhum for fornecido).

### Agentes de IA

- **O uso da conta agora é consultado uma vez por máquina, em vez de uma vez por sessão.** Os limites de uso pertencem à conta, mas antes o painel Info de cada sessão fazia essa consulta de forma independente. Ter dez sessões abertas gerava dez requisições duplicadas para obter os mesmos números — o que costumava atingir os limites de taxa (rate limits) rigorosos do Claude e deixar o painel em branco. Agora, um serviço de consulta (poller) centralizado no backend mantém um único snapshot lido por todas as sessões, transmitindo atualizações sempre que os dados mudam. Ele só faz consultas aos provedores que você realmente utiliza — detectados pela presença de `~/.claude`, `~/.codex` ou `~/.grok/auth.json` — sem acessar o chaveiro do sistema (keychain) para evitar solicitações desnecessárias de permissão. Os snapshots são salvos em disco para que o painel mostre os números em cache imediatamente ao reiniciar, sem precisar esperar pela primeira consulta. Tirar o notebook do modo de suspensão também atualiza os dados na hora, em vez de esperar pelo próximo intervalo agendado. Falhas consecutivas de consulta aplicam recuo exponencial (backoff) de até 8x. Nas Configurações, o intervalo de atualização agora conta com um botão liga/desliga explícito ao lado do campo de intervalo, substituindo a configuração ambígua de 0 segundos.

- **Falhas na atualização de uso agora são marcadas claramente como desatualizadas (stale), em vez de parecerem em dia.** Antes, quando uma solicitação de atualização falhava, o cliente retornava silenciosamente a cópia em cache enquanto registrava uma atualização bem-sucedida — avançando o horário, limpando erros e redefinindo o temporizador de backoff. Ao atingir limites de taxa, isso fazia o painel parecer em ordem enquanto continuava fazendo consultas a cada cinco minutos. Agora, as solicitações que falham mantêm os valores em cache, mas exibem um badge âmbar `stale` ao lado do provedor. Passar o mouse sobre o badge mostra uma dica de tela (tooltip) com o horário da falha, o motivo do erro e há quanto tempo aquela leitura foi obtida. O horário "Updated" passa a refletir a última sincronização bem-sucedida, e as solicitações de "retry-after" de servidores com limite de taxa são respeitadas, embora atualizações manuais ainda sejam disparadas imediatamente.

- **As cotas semanais por modelo agora aparecem no painel de uso.** Além dos limites de 5 horas e 7 dias para toda a conta, o endpoint de uso informa cotas semanais específicas por modelo. Antes, esses dados eram ignorados, o que fazia com que usuários de modelos como o Fable vissem apenas o uso total da conta em vez da cota restante para aquele modelo específico. Essas linhas de detalhamento agora são exibidas abaixo dos totais, categorizadas por família de modelos.

- **Reabrir uma sessão do OpenCode agora restaura o histórico do chat.** Antes, o aplicativo verificava se uma sessão ainda existia usando `opencode session list`, que só busca sessões no diretório de trabalho atual. Como o app desktop roda a partir do seu diretório de inicialização (`/` no macOS), os IDs de sessões existentes nunca eram encontrados, fazendo com que as tentativas de retomada abrissem silenciosamente sessões em branco. A verificação de sessão agora consulta o ID específico diretamente e só considera a sessão excluída se o comando confirmar isso de forma explícita. Outros erros são tratados como indeterminados e o app prossegue com a retomada, evitando a perda acidental do histórico da sessão.

### Interface

- **Digitar com um Editor de Método de Entrada (IME) agora exibe o texto em composição.** O texto digitado por meio de um IME (como Pinyin ou Kana) ficava invisível até ser confirmado com Enter em todas as três plataformas. Isso era causado por uma regra de CSS personalizada: o xterm renderiza o texto de composição dentro de um contêiner de sobreposição (overlay) sem restrições, no qual a nossa restrição `right` adicional (feita para quebrar linhas longas no terminal) resultava em largura zero, enquanto `overflow: hidden` cortava o texto por completo. Essa regra foi removida, e a única coisa ainda sobrescrita é o esquema de cores padrão preto sobre branco do overlay, que não combinava com nenhum tema. Um problema relacionado também foi corrigido: o xterm expandia sua textarea oculta para coincidir com o tamanho do overlay no posicionamento de candidatos do IME, mas nunca a reduzia de volta, deixando uma sobreposição invisível que bloqueava cliques do mouse e seleção de texto naquela linha.

- **No macOS, atalhos com Ctrl não acionam mais as combinações de teclas do Command.** A verificação de teclas modificadoras antes tratava Cmd e Ctrl como equivalentes no macOS. Com isso, atalhos padrão do terminal entravam em conflito com os atalhos do aplicativo: Ctrl+D (EOF) dividia painéis, Ctrl+W (apagar palavra) fechava painéis, e combinações como Ctrl+F, Ctrl+T e Ctrl+O eram interceptadas pelo app. Como novos painéis são abertos silenciosamente, muitas vezes parecia que as divisões surgiam do nada. Agora, o aplicativo desktop para macOS usa exclusivamente Cmd para seus atalhos, enquanto outras plataformas (assim como navegadores web conectados a um Mac) usam exclusivamente Ctrl.

- **Menus suspensos não reabrem mais imediatamente ao selecionar uma opção.** Um elemento `<label>` envolvente estava propagando cliques da linha de volta para o botão acionador, fazendo com que o menu suspenso reabrisse assim que um item era selecionado.

### Correções

- **Linux: o AppImage voltou a abrir.** O AppImage da versão 0.1.105 travava no Ubuntu 22.04 antes mesmo de abrir uma janela, incapaz de iniciar os processos auxiliares do WebKit. As ferramentas de empacotamento do AppImage reescrevem caminhos `/usr` literais em binários para `././` (preservando o tamanho da string para aplicação de patch in-place), exigindo que o script inicializador mude o diretório de trabalho para `$APPDIR/usr` para que esses caminhos sejam resolvidos. Na versão 0.1.105, nosso script inicializador personalizado — introduzido para evitar vazamento de variáveis de ambiente para shells secundários — repassava as variáveis de ambiente, mas esquecia dessa mudança de diretório. Agora, o inicializador muda de diretório corretamente, com o suporte de uma asserção em tempo de compilação para evitar regressões. As versões 0.1.104 e anteriores não foram afetadas.

- **Windows: as notificações voltaram a tocar som e clicar nelas leva direto à sessão correspondente.** As notificações do Windows antes enviavam um identificador de som do macOS para o plugin de notificações. Como o nome não podia ser interpretado, ele virava silêncio — ou seja, as notificações do Windows nunca emitiam som, independentemente das configurações. As notificações do Windows haviam sido redirecionadas pelo plugin duas versões atrás como contorno para um problema já resolvido naquela mesma versão. Esse desvio foi desfeito: as notificações voltaram ao canal nativo, restaurando tanto o som quanto a navegação direta para a sessão ao clicar.

- **Windows: caracteres de desenho de caixas e blocos agora ficam alinhados.** As bordas e molduras do terminal eram renderizadas anteriormente com uma fonte de fallback proporcional, causando distorções visuais. Mesmo após corrigir isso, caracteres de bloco (usados em barras de progresso e no mascote do Claude Code) ainda exibiam uma fresta vertical finíssima na borda direita de cada célula, porque a fonte de fallback era ligeiramente mais estreita que a fonte primária do terminal. Ambas as faixas de caracteres agora são fornecidas por um subconjunto integrado da fonte JetBrains Mono, garantindo largura idêntica para todos os glifos.

- **Windows: o `git` agora funciona na variante completa do Git Bash.** Embora a instalação completa do Git Bash fosse concluída com sucesso, executar `git` no terminal retornava "command not found", solicitando repetidamente aos usuários que instalassem a versão completa do Git Bash. O Git Bash só adiciona `mingw64/bin` ao `PATH` quando sabe em qual árvore de diretórios está sendo executado. Como o nosso inicializador de shell omitia esse parâmetro, binários localizados exclusivamente nesse diretório — como `git` e `curl` — não eram encontrados. Além disso, o aviso de download agora seleciona o instalador compatível com a arquitetura do sistema host, em vez de usar 64 bits por padrão.

- **Windows: a remoção de worktrees agora funciona e a reinstalação de hooks do Kiro não duplica mais entradas.** Os comandos de remoção de worktree antes eram executados de dentro do próprio diretório do worktree alvo. Como o Windows impede a exclusão do diretório de trabalho atual de um processo ativo, a operação falhava constantemente com erro de permissão; agora os comandos do Git são executados a partir da raiz do repositório principal. Além disso, a lógica de detecção de hooks comparava caminhos com barras normais contra caminhos com barras invertidas do Windows, fazendo a verificação falhar e duplicar as entradas de hooks a cada nova instalação.

---

## v0.1.105 — 2026-08-26

### Espaço de trabalho

- **O estado de uma sessão, as marcas de não lida e se a sessão ainda está em execução agora são decididos pelo backend, e todos os clientes veem a mesma resposta.** Antes cada cliente deduzia isso do que por acaso tivesse observado, então "não lida" na verdade queria dizer "não lida nesta janela": ler uma sessão no navegador deixava a cópia do desktop como não lida, e o filtro dinâmico de estado insistia em devolver aquela linha para a lista compartilhada, de onde nem "Atualizar estados" conseguia tirá-la. Os eventos de estado também só eram registrados depois que o próprio cliente tivesse criado uma sessão, então um navegador recém-conectado mostrava pontos nas sessões que ele mesmo tinha aberto e nada nas demais. Agora o backend mantém um registro com autoridade por sessão — o agente, seu estado, se o processo está vivo e a marca de não lida —, responde a uma consulta em lote quando um cliente conecta ou reconecta, e transmite cada mudança para todo mundo. Os clientes relatam o que observam; quem tira as conclusões é o backend. Ler uma sessão no celular limpa a marca no desktop, um navegador recém-conectado mostra os pontos certos em sessões que ele nunca abriu, e as regras de arbitragem que antes viviam no frontend foram junto, com seus motivos e seus testes: um hook que já reportou uma vez bloqueia qualquer coisa deduzida da saída bruta, saída contínua só significa ocupado para uma sessão que tem agente, não é Codex e ainda não recebeu nenhum relato com autoridade, e uma espera de 1200 ms impede que uma tarefa recém-iniciada seja cancelada por um evento de término que chega logo atrás. A leitura de tela é a exceção, porque depende da grade já renderizada, e ela só existe dentro de um cliente: o cliente dono do tamanho do terminal relata o que leu, e o backend decide se aceita — o relato de qualquer outro cliente é recusado. Se algo aí der errado, definir `vlx-arbitration` como `frontend` no localStorage devolve a arbitragem à antiga cadeia do frontend.

- **Reiniciar uma sessão não fecha mais a aba dela no outro cliente.** O `pty://killed` não carregava nenhum dado, então o outro lado não sabia distinguir um reinício de um fechamento: tratava tudo como fechamento, removia o painel e depois espelhava esse layout de volta. Agora o evento diz qual cliente encerrou o processo e por quê, de modo que um reinício mantém o painel e espera o novo processo chegar. Um motivo ausente ou desconhecido continua valendo como fechamento — deixar um painel aberto para uma sessão que nunca vai voltar coloca um terminal morto na tela, o que é pior do que fechar um que estava prestes a reiniciar.

- **Um navegador que se conecta a um desktop recém-restaurado não inicia mais todas as sessões de verdade.** Restaurar um espaço de trabalho desenha cartões de espaço reservado em vez de iniciar processos, mas essa decisão só existia no desktop; o navegador seguia o layout espelhado, não sabia distinguir "não está rodando" de "está rodando, só nunca foi aberta aqui", e montava os terminais — e montar um terminal é iniciá-lo. Uma folha que chega com o layout de outra pessoa agora vira um cartão quando o backend diz que não há processo por trás dela. Abrir uma sessão você mesmo continua valendo como intenção de iniciá-la, e um terminal que você está olhando neste momento nunca é substituído por um cartão quando o processo dele termina, porque você ainda pode querer ler o que ele imprimiu.

- **Uma mudança de configuração chega na hora aos outros clientes.** O backend sempre teve as configurações com autoridade, mas as alterava sem avisar ninguém, então o outro cliente só descobria na inicialização seguinte. Em uma conexão remota isso é mais do que uma diferença de tela: desligar "expandir o filtro de estado dinamicamente" em um cliente não adiantava nada enquanto outro continuava acrescentando linhas à lista compartilhada, e o cliente com o menor limite de abas ativas descartava abas em segundo plano por baixo de todos os outros. Gravar uma configuração agora transmite qual chave mudou, e cada cliente a relê pelo mesmo caminho que usa na inicialização, então as regras que escondem valores protegidos de quem chama remotamente continuam valendo — a transmissão leva só nomes de chave, nunca valores. Os celulares, que antes não enviavam nem recebiam configurações, agora também participam.

- **Um navegador espera o layout espelhado antes de restaurar o seu próprio.** Uma janela remota tem duas fontes de layout — a do próprio localStorage e a que o anfitrião envia no modo espelho — e a que chegasse primeiro era sobrescrita pela outra. Colocar a local na frente custava mais do que um piscar de tela: montar uma folha de terminal inicia um processo de verdade, e para uma sessão cujo processo já tinha acabado isso significava abrir um shell que ninguém jamais olharia, e que depois ficava lá, porque um navegador desanexa seus terminais em vez de encerrá-los. Agora o navegador espera o primeiro alinhamento assentar antes de restaurar qualquer coisa, por no máximo dois segundos; se o backend estiver lento ou inacessível, ele recorre ao layout local em vez de ficar com uma janela vazia. Os celulares, e qualquer cliente com o espelhamento desligado, passam direto.

### Acesso remoto

- **Uma conexão SSH pode ligar o modo espelho no serviço que ela inicia.** A chave fica no painel de acesso remoto, mas o SSH inicia um serviço sem interface na máquina do outro lado, e lá não há painel nenhum para clicar. Segure Option ao clicar em "Conectar remoto" e o formulário de SSH passa a oferecer a caixa "Espelhar a interface entre os clientes", desmarcada por padrão. O valor viaja junto com a conexão e fica na memória do serviço, em vez de ser gravado no banco de dados da máquina remota: quando você também reaproveita o banco do desktop remoto, uma conexão SSH não deve virar em silêncio uma chave ligada no painel de outra pessoa. A escolha é lembrada por host, então tirar a mesma máquina do histórico traz o valor de volta. Reaproveitar um serviço que já esteja rodando agora exige que a versão, o modo de dados e o modo espelho batam todos — se qualquer um dos três for diferente, o serviço antigo é substituído, o que encerra as sessões que rodavam nele, e por isso essa opção fica atrás do Option, ao lado da chave do banco de dados.

- **Um cliente que está sendo espelhado avisa isso.** As abas e as divisões de um cliente que segue outro se reorganizavam sozinhas, sem nada na tela para explicar de onde vinha a mudança. Agora a barra de título traz um selo de Espelhado, com uma explicação ao passar o mouse. O anfitrião não mostra selo nenhum — a chave está com ele.

- **Agora dá para mover arquivos entre a sua máquina e a que executa o terminal.** O acesso remoto mostrava os arquivos da outra máquina, mas não oferecia como trazer um nem como deixar um lá: só restava um comando no terminal. O painel de arquivos passa a ter Baixar no menu de contexto de um arquivo e Enviar no cabeçalho, e arrastar arquivos da sua área de trabalho até a linha de uma pasta os manda para lá. As duas direções usam a mesma conexão autenticada de todo o resto, então funciona igual em um navegador na rede local, no celular e em uma janela de conexão remota. As transferências avançam em blocos, com uma fila de progresso abaixo da árvore, e continuam enquanto você olha outro painel. Baixar é um link de download comum, cuidado pelo gerenciador de downloads do próprio navegador: ele grava em disco conforme recebe, mostra velocidade e tempo restante, e pode ser pausado e retomado, em qualquer tamanho de arquivo e em todos os navegadores, inclusive no celular. O link carrega um tíquete emitido para aquele único arquivo e válido por alguns minutos, porque este servidor guarda credenciais em um cabeçalho e um navegador que abre um link não envia nenhum. Um envio é escrito com um nome temporário e só é renomeado ao terminar, de modo que uma transferência interrompida nunca deixa um arquivo pela metade onde deveria haver um inteiro; um nome já ocupado é recusado antes de qualquer transferência. Os envios mostram velocidade e tempo restante e sobrevivem à queda da conexão: um bloco que falha espera e tenta de novo por cerca de um minuto, perguntando ao servidor até onde o arquivo temporário realmente chegou em vez de reenviar um bloco que talvez já tenha chegado. Ao desistir, esses bytes ficam: arraste o mesmo arquivo para a mesma pasta e ele continua de onde parou, mesmo depois de recarregar, porque só cancelar descarta o arquivo parcial.

### Agentes de IA

- **As sessões do Antigravity e do Copilot passam a ser nomeadas pela primeira mensagem.** Os dois faltavam na renomeação automática, o que deixava fileiras de "Antigravity 1, 2, 3" na barra lateral. Os eventos de hook do Antigravity não trazem texto do usuário nenhum, só um id de conversa e o caminho da transcrição, então a primeira mensagem é lida da própria transcrição; o bloco de metadados que vem logo depois dela fica de fora do título. Os eventos do Copilot também não trazem nome de evento, e são distinguidos pelo formato, então um corpo com um prompt e sem nome de ferramenta agora conta como um envio — o que deixa de fora, corretamente, o prompt com que a sessão é iniciada e as chamadas de ferramenta.

- **Uma sessão do Antigravity reabre com o histórico dela.** Retomar depende do id da conversa, e o analisador que extrai um id de sessão dos argumentos de inicialização não reconhecia a grafia que o Antigravity usa, então `--conversation=<id>` nunca tinha uma âncora para apontar e toda sessão reaberta vinha vazia.

- **`vspawn --yes` cria a sessão sem o cartão de confirmação.** Quem mantém ligada a confirmação antes de criar tinha que clicar em um cartão para cada sessão filha de uma execução. A flag — que também pode ser escrita `-y` ou `--no-confirm` — pula o cartão naquela chamada e inicia a sessão com as configurações padrão. Ela não altera a configuração em si, então a próxima criação sem a flag pergunta de novo.

- **O campo de modelo do cartão de tarefa filha aceita qualquer coisa que você digitar.** Ele era um menu suspenso simples, então só dava para escolher os modelos da lista — e um agente entende muito mais identificadores do que isso: nomes com data como `claude-opus-4-6`, nomes com prefixo do fornecedor, apelidos configurados localmente. Agora é um campo de texto, com os modelos conhecidos pendurados em um menu suspenso ao lado como atalhos. A lista é uma sugestão, não uma lista de permissões: o que você digitar é o que vai ser passado, um campo vazio significa nenhum `--model`, e a lista filtra conforme você digita e se recolhe quando um identificador personalizado não casa com nada.

### Interface

- **Um único menu suspenso, usado em toda parte.** Os menus suspensos espalhados pelo aplicativo tinham sido copiados do mesmo código mais de uma dúzia de vezes e depois foram se afastando uns dos outros: três fundos de painel, quatro sombras, três cores de destaque ao passar o mouse, botões de abrir com 26, 28 e 32 pixels de altura, e marcações nas linhas selecionadas de que só uma lista de seleção múltipla precisa. Um único componente agora sustenta os seletores de branch da mesclagem, os seletores de idioma, de shell padrão e de fonte nas configurações, o seletor de agente, os diálogos de worktree, o tipo de agente em sessões novas e restauradas, e o último select nativo do modal de formulário. Ele traz junto o controle pelo teclado, que nenhum deles tinha: setas para mover, Enter para escolher, Esc para fechar sem fechar o diálogo atrás dele, Home e End para saltar. Os dois menus da barra de status continuam como eram, e o filtro de estado da barra lateral mantém suas marcações, porque ele é mesmo de seleção múltipla.

- **A senha do painel de acesso remoto pode ser revelada.** Era um campo de senha puro, então não dava para ver o que você tinha digitado; o botão de olho existia, mas só dentro do arquivo do próprio painel de conexão. Agora os dois compartilham um mesmo componente, e o estado revelado é redefinido quando o painel fecha.

- **O seletor de IP não parece mais um controle do sistema.** Ele era um select nativo, e o WKWebView veste esses controles com a aparência do sistema, que fica mal em um painel escuro — a mesma reclamação dos menus suspensos substituídos acima. Agora ele usa o componente compartilhado, e seu rótulo foi encurtado para "IP", já que o texto ao lado dele já diz para que ele serve.

### Correções

- **A verificação de atualizações pergunta ao servidor todas as vezes.** Um cliente deixado aberto ficava preso na primeira versão que tinha visto: depois de encontrar a 0.1.101, continuava oferecendo a 0.1.101 mesmo depois de a 0.1.104 sair, e "Verificar atualizações" só reabria o mesmo diálogo, porque o código antigo retornava logo de cara sempre que já houvesse um aviso pendente. Agora toda verificação é uma requisição de verdade. Uma versão mais nova substitui o aviso na tela, a mesma versão ou um download em andamento deixam o aviso como está, e um servidor que não reporta atualização nenhuma derruba um aviso que ficou obsoleto — a versão foi retirada, ou você mesmo a instalou nesse meio-tempo. O botão "Baixar manualmente" agora abre a página de download no site; antes ele entregava o pacote do próprio atualizador, que se desempacota no lugar e não dá para instalar à mão.

- **Um overlay de erro em tela cheia não aparece mais quando os terminais são destruídos rápido demais.** A viewport do xterm agenda uma sincronização da área de rolagem quando é construída e outra quando é reiniciada, e não cancela nenhuma das duas ao ser descartada, então um terminal aberto e fechado dentro da mesma tarefa — que é exatamente o que reconstruir a árvore de sessões durante uma conexão remota faz — ainda executava esses callbacks, encontrava um renderizador já limpo e lançava um erro. O erro vem de um temporizador, onde nem try/catch nem um error boundary alcançam, então ele é capturado globalmente e reconhecido de forma restrita: só uma pilha ou mensagem que cite aquela sincronização, junto com uma menção ao renderizador ou às dimensões dele, é engolida como inofensiva e registrada no log de requisições. Falhas de verdade continuam levantando o overlay.

---

## v0.1.104 — 2026-08-25

### Agentes de IA

- **O cartão de tarefa filha agora oferece os modelos reais de cada agente e a flag de esforço que aquele agente realmente entende.** O cartão montava seus argumentos de inicialização com `--model` e `--effort` para tudo, mas só Claude, Kiro e Antigravity escrevem assim o esforço de raciocínio — Grok e Zoo chamam de `--reasoning-effort`, e Cline chama de `--thinking`. Escolher um nível de esforço para qualquer um dos outros entregava à CLI uma flag da qual ela nunca tinha ouvido falar, e a sessão não chegava a iniciar. Agora cada agente contribui com os próprios nomes de flag e os próprios valores. O controle de modelo segue o que cada CLI consegue nos contar: as que sabem listar o catálogo (OpenCode, Grok, Crush, Antigravity, Cursor, pi, Kiro) são consultadas e oferecem a lista real, as que têm um conjunto fixo (Claude, Codex, Kimi Code) oferecem esse conjunto, e as demais dão um campo de texto com um exemplo do formato que esperam. Um agente que não esteja instalado ou em que você não tenha feito login avisa, em vez de ficar girando para sempre. Escolher "Padrão" agora limpa um valor herdado em vez de deixar o antigo no lugar, e escolher um nível de esforço não descarta mais o modelo herdado da sessão pai.

- **Uma tarefa filha criada a partir de uma sessão Kimi Code continua sendo Kimi Code.** O Kimi Code faltava na lista que o caminho de criação usa para herdar o agente do pai, então suas sessões filhas voltavam silenciosamente como o agente padrão.

### Espaço de trabalho

- **Um grupo pode ser movido para um worktree depois de criado.** O worktree era escolhido na criação do grupo e ficava fixo dali em diante; mudar de ideia significava apagar o grupo e montá-lo de novo. Clique com o botão direito em um grupo e escolha "Move to Worktree…" para criar um worktree novo, vincular um existente ou reapontar um grupo que já esteja vinculado. Só o grupo em si muda: as sessões que já estão dentro mantêm o diretório com que foram criadas — uma sessão em execução não pode ser movida para outro diretório debaixo de si mesma —, enquanto as sessões criadas depois começam no worktree.

- **O modo espelho agora cobre toda a árvore da barra lateral.** Ele compartilhava a seleção e os painéis recolhidos; a caixa de busca e os filtros de estado e de marcador ficavam locais, pela ideia de que sincronizá-los atrapalha quem está procurando alguma coisa. Esse raciocínio estava invertido: espelhar significa que as duas janelas mantêm o mesmo estado, não que uma reproduza as teclas digitadas na outra — um filtro que está ligado aqui está ligado lá. O que de fato atrapalha as pessoas é os dois lados mostrarem árvores diferentes. Agora toda projeção da barra lateral viaja: o layout dividido, o nome de cada projeção, seu texto de busca, seus filtros de estado e de marcador, e seu próprio estado de recolhimento. O formato do instantâneo passou para a versão 2, e um cliente rodando uma versão mais antiga para de espelhar em vez de aplicar meio quadro, então recarregue qualquer janela que você tenha deixado aberta durante a atualização.

### Interface

- **Fechar a janela no macOS faz a mesma pergunta que sair.** ⌘Q e o item de menu passavam pela confirmação do próprio aplicativo, mas o botão vermelho de fechar destruía a janela na hora — e é essa janela que contém a webview onde o diálogo de confirmação vive. Ou você não recebia confirmação nenhuma, ou recebia o substituto nativo reduzido, sem a caixa de "salvar o espaço de trabalho" e com o texto sem tradução. As três plataformas agora seguram a janela aberta até você responder.

- **"Salvar o espaço de trabalho" vem marcado por padrão, e fica como você deixou.** Perder um layout custa mais do que um instantâneo indesejado, então a caixa começa marcada. Ela também costumava esquecer: a configuração era gravada no banco de dados com um atraso de 400 ms, e sair matava o processo dentro dessa janela, então a inicialização seguinte se reconciliava com o valor antigo e desfazia sua alteração. A gravação agora é descarregada antes da saída, com um teto de 600 ms para que um backend travado não deixe o botão de confirmar girando. (Contribuição de FarhadGSRX.)

- **As senhas do painel de conexão remota podem ser reveladas.** Tanto a senha da URL quanto a senha SSH têm um botão de olho que alterna entre texto oculto e texto plano. O estado revelado é local ao campo e é redefinido quando o painel fecha, então uma senha nunca fica exposta na tela.

- **O selo de filtros da barra lateral conta todos os filtros ligados.** Um filtro de marcador apenas acendia o botão sem dizer mais nada, então o selo podia marcar 1 com dois filtros ativos. Agora ele soma estados e marcadores juntos e bate com as marcações do menu suspenso; um único filtro de estado mantém seu ponto colorido.

### Correções

- **As atualizações automáticas no macOS voltaram a funcionar.** Os pacotes da v0.1.103 carregavam entradas complementares do AppleDouble (`._VelaTerm.app`), das quais o atualizador remove o primeiro componente do caminho — deixando um caminho vazio —, e então ele se recusava a desempacotar o arquivo. As duas arquiteturas foram afetadas, então todo usuário de macOS na v0.1.103 ficava preso nela. O empacotamento não grava mais essas entradas.

- **Os controles nativos seguem o tema do aplicativo quando ele difere do do sistema.** Aplicar um tema definia as cores do próprio aplicativo, mas nunca atualizava `color-scheme`, que era definido uma única vez na inicialização a partir da preferência do sistema e nunca mais mudava, então caixas de seleção, menus suspensos e barras de rolagem continuavam escuros sob um aplicativo claro em um sistema escuro. (Contribuição de FarhadGSRX.)

- **Um clone novo volta a compilar.** O crate Rust embute `../dist` em tempo de compilação, e o comando de desenvolvimento não o produz, então um repositório recém-clonado falhava ao compilar antes mesmo de conseguir rodar. O script de build agora cria o diretório quando ele não existe. (Contribuição de FarhadGSRX.)

---

## v0.1.103 — 2026-08-24

### Correções

- **Sessões Codex no Windows não recusam mais iniciar.** Cada sessão Codex falhava imediatamente com `unexpected argument '--codex-hook'` porque a tabela TOML dos hooks de ciclo de vida, passada pela linha de comando, contém espaços e aspas duplas, e o `codex.cmd` instalado pelo npm reprocessa isso através do cmd.exe, que remove as aspas e divide o valor em múltiplos argumentos. A injeção de hooks agora é ignorada no Windows; a detecção de estado recorre às heurísticas existentes notify / screen / busy, que continuam reportando estados de repouso e atividade, embora com menos precisão que os hooks. macOS e Linux não são afetados e continuam usando hooks.

- **Revertida a correção de pré-edição IME do Windows da v0.1.102.** A correção que restaurou o overlay de composição para entrada em chinês, japonês e coreano também adicionou cor de fundo, borda de 1px e cantos arredondados, desenhando uma pequena caixa ao redor do texto de pré-edição dentro do terminal — algo que não deveria aparecer ali. Como o dimensionamento do overlay, a geometria do container auxiliar e a limpeza do textarea eram interdependentes, toda a alteração precisou ser revertida. O problema subjacente — digitação cega de CJK no Windows — permanece aberto e é rastreado na issue #6.

---

## v0.1.102 — 2026-08-23

### Agentes de IA

- **Predefinições de agente: várias CLIs compatíveis lado a lado.** Cada tipo de agente estava preso a um único executável, então um fork, uma compilação noturna ou uma segunda CLI que fala o mesmo protocolo não tinham por onde entrar — você editava os argumentos de inicialização de um tipo existente e perdia o original. Uma predefinição agora indica o próprio executável, o próprio ícone e os próprios argumentos de inicialização, e aparece no menu de nova sessão ao lado dos tipos embutidos. As sessões registram qual predefinição as criou, de modo que bifurcar uma mantém o mesmo executável, e uma predefinição criada no desktop aparece também nos navegadores pareados e nos clientes remotos, ícone incluído, porque o ícone viaja como dado e não como um caminho de uma máquina só. As sessões existentes ficam intactas: um banco de dados de uma versão anterior inicia exatamente como antes.

- **O cartão de tarefa filha escolhe o modelo e o nível de esforço, e uma única resposta resolve em toda parte.** Quando um agente pede para criar uma tarefa filha, o cartão de confirmação agora oferece o modelo e — quando o agente aceita — o esforço de raciocínio, já preenchidos a partir dos argumentos de inicialização do próprio pai, para que o caso comum seja um clique só. Trocar o agente no cartão deduz os dois de novo, então o nome de um modelo de uma CLI não consegue mais ir parar na linha de comando de outra. O cartão aparece em todos os clientes conectados, e responder em um agora o dispensa nos outros; a primeira resposta também reivindica a tarefa no servidor, de modo que confirmar no celular e no desktop dentro do mesmo segundo cria um worktree e uma sessão filha, em vez de dois.

### Espaço de trabalho

- **Modo espelho: um mesmo layout em todos os clientes.** O fluxo do terminal sempre foi compartilhado — um PTY, um fluxo de bytes —, mas o arranjo em volta dele vivia apenas no armazenamento do navegador de cada cliente, então um navegador aberto pela LAN mostrava as próprias abas e divisões, e reorganizar uma tela não fazia nada na outra. Com o modo espelho ligado, as abas, as divisões, a sessão ativa, a seleção da barra lateral e os painéis recolhidos são publicados para todos os clientes e seguidos por todos eles. O anfitrião controla a chave no painel de acesso remoto. Reorganizar de qualquer um dos lados vale no outro; uma sessão que sai do layout desta janela é desanexada, não encerrada, então seguir outro cliente nunca termina o processo de ninguém; e aplicar o layout de outro cliente não rouba o teclado de quem está digitando localmente. Os celulares ficam de fora: a navegação de dois níveis do celular é uma interface de outro formato, e copiar nela uma árvore de divisões de desktop não ajuda ninguém.

- **A aba Git da barra lateral direita virou um cliente Git de verdade.** Antes ela só listava os arquivos modificados. Agora ela adiciona à área de preparação arquivos individuais ou grupos inteiros e os remove de lá, descarta alterações, escreve um commit (com a opção de emendar) e mostra o histórico com os arquivos e os diffs de cada commit — agrupados em seções recolhíveis de preparados, modificados, não rastreados e confirmados. Os caminhos são tratados a partir da raiz do repositório, então uma sessão aberta em um subdiretório age sobre os arquivos que diz agir, e um HEAD desanexado é identificado como tal em vez de mostrar um branch chamado HEAD.

### Interface

- **⌘Q agora faz a mesma pergunta que fechar a janela.** O item "Sair" do menu do aplicativo era o do próprio sistema, que encerra o processo na hora: apertar ⌘Q pulava a confirmação de "salvar o espaço de trabalho" que o botão de fechar mostra, então a mesma intenção se comportava de um jeito diferente conforme a forma de expressá-la. Os dois caminhos agora passam por uma única confirmação. Se a janela que a mostra tiver recarregado ou travado nesse meio-tempo, apertar ⌘Q de novo repete a pergunta e recorre a um diálogo nativo, em vez de deixar o aplicativo sem como sair.

- **As dicas de atalho mostram as teclas que realmente funcionam.** Os padrões variam conforme a plataforma, e um navegador reserva para si as combinações de ⌘/Ctrl com letras — ⌘D salva um favorito, ⌘T abre uma aba —, então no macOS os atalhos com ⌘ do próprio aplicativo nunca chegavam à página quando o VelaTerm era aberto como uma URL. Abas comuns de navegador agora usam as combinações com Ctrl+Alt em todos os sistemas operacionais, enquanto o aplicativo de desktop e as janelas de conexão remota mantêm o ⌘. As dicas exibidas ao passar o mouse e a dica da aba vazia mostram a combinação que estiver valendo, inclusive uma que você mesmo tenha reatribuído, em vez de uma combinação com ⌘ fixa no código; e o terminal bloqueia exatamente as combinações que o aplicativo reivindicou, de modo que reatribuir uma ação leva aquela tecla junto.

- **As predefinições de fonte cobrem Nerd Fonts e CJK, e uma fonte personalizada que não esteja instalada avisa.** A lista de predefinições ganhou as famílias Nerd Font e CJK mais comuns, e uma fonte digitada à mão é devolvida na tela e verificada: se o sistema não a tiver, a página de configurações avisa, em vez de cair silenciosamente em um padrão que não se parece em nada com o que você pediu.

- **Os campos de texto no macOS não colocam mais maiúsculas nem corrigem o que você digita.** As maiúsculas automáticas, a autocorreção e o corretor ortográfico do sistema se aplicavam a todos os campos do aplicativo, inclusive nomes de sessão e campos de comando, onde "npm" virava "Npm". Agora estão desligados em toda parte.

### Windows

- **Digitar em chinês, japonês ou coreano volta a mostrar o texto em composição e a janela de candidatos.** Os dois estavam invisíveis — você digitava às cegas e só via o resultado depois de apertar Enter. A culpa era de duas regras CSS nossas: o contêiner que segura a camada de composição encolhia para largura zero, e dentro dele o deslocamento `right` dessa camada não resolvia para nada. A janela de candidatos ia junto, porque o sistema operacional a posiciona a partir do retângulo dessa camada. A camada volta a ser desenhada e adota as cores do tema do aplicativo, e o elemento de entrada invisível sobre o qual ela se apoia libera sua geometria assim que a composição termina, de modo que clicar e arrastar sobre aquele trecho chega ao terminal, e não a um elemento vazio que antes continuava cobrindo-o.

- **A barra de título nativa segue a configuração de tema claro ou escuro.** O aplicativo mantém a barra de título do sistema, e o Windows a pinta em claro enquanto não lhe disserem o contrário, então uma interface escura carregava uma faixa branca em cima. Agora ela combina com o aplicativo, inclusive nas janelas abertas depois, como as de SSH e as de conexão remota. Escolher "seguir o sistema" devolve o controle ao sistema operacional em vez de fixar um valor.

- **Sumiu o quadradinho solto na inicialização a frio.** O plugin de instância única cria uma janela de mensagens oculta e nunca lhe deu a transparência que o próprio estilo prometia, então o Windows às vezes aumentava essa janela de tamanho zero até o mínimo e pintava um quadradinho durante a inicialização. Agora ela é de fato transparente; o comportamento de instância única não mudou.

### Desempenho

- **O acesso remoto carrega bem menos na primeira exibição.** Os recursos estáticos agora são comprimidos sob demanda e servidos com validadores de cache, então uma segunda visita revalida em vez de baixar tudo de novo, e os pacotes de idioma e os renderizadores opcionais do terminal só carregam quando algo precisa deles, em vez de fazerem parte do primeiro envio. Somando tudo, a transferência inicial cai para cerca de um quinto do que era.

### Correções

- **Uma sessão filha agora inicia igual à sessão que a pediu.** As filhas não herdavam nem o modo de permissões do pai nem seus argumentos de inicialização, então a filha de uma sessão que pulava as confirmações vinha pedindo confirmação, e um modelo fixado no pai era descartado. Agora os dois são herdados, com os padrões globais do tipo de agente como reserva — os mesmos que o menu de "nova sessão de agente" aplica.

- **Reconectar uma janela remota não relata mais uma "falha de autenticação" falsa.** Quando a janela reconectava, o novo WebSocket e o que ele substituía corriam um contra o outro; o encerramento do perdedor era relatado como falha de autenticação, e o aviso acusava de rejeição um pareamento perfeitamente válido.

- **Um grupo não pode mais ser arrastado para dentro da própria subárvore.** Soltar um grupo sobre um de seus próprios descendentes desprendia todo aquele ramo da árvore, e as sessões dentro dele sumiam da barra lateral até o banco de dados ser reparado à mão. O movimento agora é recusado.

- **A saída dos agentes mantém as cores quando o VelaTerm é iniciado por outra ferramenta.** Um terminal herda o ambiente de quem o iniciou, então abrir a partir de uma IDE ou de um ambiente de agentes que exporta `NO_COLOR`, `CI` ou `FORCE_COLOR=0` fazia toda TUI de agente ficar monocromática dentro do VelaTerm, mesmo com o terminal anunciando cor completa. Esses valores herdados são descartados quando uma sessão começa; as mesmas variáveis exportadas do seu próprio perfil de shell continuam valendo, porque esse perfil roda dentro da sessão.

## v0.1.101 — 2026-08-15

### Acesso remoto

- **Escolha qual endereço o link de compartilhamento usa — endereços do Tailscale agora aparecem.** A lista de endereços só aceitava as faixas IPv4 privadas clássicas; por isso, malhas VPN como o Tailscale, que atribuem endereços da faixa de NAT de operadora (100.64.0.0/10), eram descartadas silenciosamente do painel de acesso remoto e do link de pareamento — embora o servidor já estivesse acessível por elas. Esses endereços agora são listados; túneis VPN ficam por último, para nunca virarem o padrão. Um novo seletor de IP no painel — visível antes de iniciar e com o servidor em execução — mostra cada candidato com o nome da interface e marca túneis VPN; ao escolher um, sua URL vai para a frente e o link de pareamento é gerado novamente com exatamente esse host, de modo que o link copiado funciona em um dispositivo que só alcança esta máquina pela VPN, sem editar a URL manualmente. Um código QR abaixo do link de pareamento pode ser escaneado diretamente com o celular. A escolha é lembrada; se a interface escolhida desaparecer, o painel volta para “Automático” sem esquecê-la. O servidor em si permanece inalterado e continua escutando em todas as interfaces. Escolher um endereço que só apareceu depois de o servidor iniciar — uma VPN conectada mais tarde, por exemplo — agora também atualiza imediatamente a URL copiada e o código QR, em vez de só o link de pareamento até a próxima reinicialização; os túneis VPN ficam atrás dos endereços LAN em todas as plataformas, um endereço escolhido com o servidor parado determina o primeiro link de pareamento após o início, e regenerações de link sobrepostas não podem mais sobrescrever um link mais novo com um mais antigo.

- **O compartilhamento agora sobrevive a uma reinicialização.** O token de pareamento era gerado de novo a cada inicialização do servidor, então fechar e reabrir o VelaTerm invalidava silenciosamente todos os links compartilhados, e cada telefone precisava ser pareado outra vez. O token, os dispositivos pareados e a lista de dispositivos bloqueados agora são salvos em um arquivo do diretório de dados legível apenas pelo proprietário: um dispositivo já pareado se reconecta com sua URL salva após uma reinicialização — a senha de acesso continua sendo um segundo fator obrigatório — e um dispositivo revogado permanece revogado. O VelaTerm também lembra que o compartilhamento estava ativo: feche o aplicativo com o servidor em execução e a próxima inicialização o retoma na mesma porta, tanto no aplicativo de desktop quanto em um servidor sem interface com `--serve`; se você mesmo o parar, nada inicia automaticamente. Se o início automático falhar, por exemplo porque a porta está ocupada, o aplicativo abre normalmente e o painel de acesso remoto mostra o motivo. O campo de porta agora lembra a porta realmente usada em vez de voltar ao padrão, e "Regenerar link" continua sendo o interruptor de emergência explícito: ele emite um novo token na hora, invalida todos os links antigos e sobrescreve o estado salvo. A senha de acesso em si nunca é gravada no disco — apenas um hash de alto custo de memória (Argon2id) é armazenado.

### Segurança

- **Um dispositivo pareado não pode mais administrar o próprio compartilhamento.** Qualquer navegador pareado podia invocar os mesmos comandos de administração que o aplicativo de desktop — criar um novo link de pareamento (o que também esvazia a lista de bloqueio de dispositivos), listar e revogar outros dispositivos, ou parar e reconfigurar o servidor — e o armazenamento de configurações entregava a cada cliente o mapa completo de configurações, incluindo o hash de memória rígida da senha de acesso e as configurações de início automático que a próxima inicialização lê. Os comandos de administração agora são reservados ao aplicativo de desktop e ao shell do Electron; a API de configurações filtra as chaves de acesso remoto e o token do Gitea de toda leitura vinda de um dispositivo pareado e recusa gravações nelas. Um dispositivo pareado mantém aquilo para que o pareamento existe — suas sessões de terminal com acesso completo ao shell —, mas não pode mais ler o verificador da senha, convidar ou expulsar outros dispositivos, nem redirecionar a porta que a próxima inicialização usará. Comandos que leem, gravam ou excluem segredos armazenados — o token do Gitea e as senhas de host lembradas — agora também são recusados para um dispositivo pareado, e os comandos que recebem caminhos — ler, pré-visualizar, gravar, criar, renomear e excluir, assim como mostrar o diff do git de um arquivo ou escolher a pasta em que um repositório é clonado — resolvem primeiro os links simbólicos e rejeitam caminhos dentro do próprio diretório de dados do VelaTerm, onde vivem o estado de pareamento e as chaves; qualquer outro caminho continua funcionando, de modo que a navegação e a edição remota de arquivos permanecem intactas. Um teste enumera cada comando remoto que aceita um caminho, de modo que um comando novo não consegue escapar dessa verificação sem ser notado. Quando uma dessas proteções recusa uma requisição, o navegador agora mostra uma mensagem devidamente traduzida em vez de um erro bruto em inglês.

- **Uma revogação ou um link regenerado agora sobrevive também à configuração de instância dupla.** Em um servidor sem interface (`--serve`) com início automático ativado, duas instâncias do servidor mantinham cada uma sua própria cópia do estado de pareamento salvo e o regravavam por inteiro: uma revogação ou um link novo feito por uma podia ser desfeito em silêncio pela outra. Todas as instâncias de um processo agora compartilham um único estado de pareamento por diretório de dados: revogação e rotação valem em toda parte imediatamente, e exatamente um gravador persiste o arquivo, que continua sendo a fonte de verdade entre reinicializações reais.

- **Logins com falhas repetidas são freados.** A verificação da senha de acesso usa Argon2id, caro de propósito — e qualquer um que alcance a porta pode tentar. Após cinco tentativas falhas de um mesmo endereço, as seguintes são rejeitadas por um minuto antes de qualquer trabalho de hash, e o próprio hash agora roda fora do loop de eventos do servidor, com um teto rígido de verificações simultâneas: uma enxurrada de senhas erradas não consegue mais saturar o servidor com hashing de memória rígida nem deixá-lo lento para os dispositivos já conectados. O freio vive na memória e é zerado com o servidor; o token de pareamento e a senha continuam sendo a barreira real. O limite agora é compartilhado por todas as instâncias do servidor que usam o mesmo diretório de dados — a configuração de instância dupla com `--serve` não dobra mais o orçamento de tentativas — e cada tentativa é reservada antes de a verificação da senha começar, para que requisições paralelas de um mesmo endereço não escapem por baixo do limite. Um navegador limitado agora vê uma mensagem própria de limite de tentativas na tela de login em vez de ouvir que a senha estava errada; além disso, o freio não é mais lembrado como uma senha errada: passada a pausa, a próxima tentativa volta a ser processada sem recarregar a página. Uma tentativa abandonada no meio — a aba fechada enquanto a senha ainda era verificada — agora libera sua reserva imediatamente em vez de contar contra o endereço pelo resto do minuto, e um login bem-sucedido libera apenas a própria reserva em vez de apagar todo o registro do endereço: atrás de um endereço de rede compartilhado, alguém entrar corretamente não zera mais o orçamento de tentativas de um atacante, e as falhas registradas só expiram com o seu minuto.

- **Segredos no disco e nos logs são tratados com mais cuidado.** O arquivo com o estado de pareamento e a chave de criptografia de ponta a ponta agora são criados legíveis apenas pelo proprietário desde o início, em vez de restringidos após a primeira gravação, e o banco de dados de sessões — que contém o hash da senha — também fica restrito ao proprietário. Um servidor sem interface (`--serve`) não imprime mais nos logs o segredo de longa duração do link de pareamento: se a saída não for um terminal, o link é retido e uma orientação aparece no lugar; `--print-pairing` reativa isso explicitamente. O registro de dispositivos é limitado a 32 entradas com nomes de comprimento limitado, para que um cliente pareado não possa fazer o arquivo salvo crescer sem limite, e se salvar uma revogação ou um novo link falhar, o erro agora chega a quem chamou em vez de ficar numa linha de log. O início automático não substitui mais um servidor já iniciado manualmente, e um erro de início automático obsoleto some assim que você mesmo para o servidor.

### Correções

- **O emparelhamento agora pode ser gerenciado no shell Electron.** Criar um link de emparelhamento, listar os dispositivos emparelhados e revogar um dispositivo só existiam como comandos de desktop (Tauri); o despachante WebSocket usado pelo shell Electron e pelos clientes de navegador respondia "Unknown command", deixando o painel de acesso remoto inoperante ali. Os três comandos agora passam pelas mesmas funções centrais nos dois transportes, de modo que não podem divergir, e testes de regressão cobrem as novas rotas de despacho — incluindo a criação de um link de emparelhamento real contra um servidor local em execução.

## v0.1.100 — 2026-08-10

### Agentes de IA

- **O Kiro CLI passa a ser um tipo de sessão de primeira classe.** As sessões do Kiro ganham seu próprio nó na árvore, um indicador de status de trabalho/aguardando com autoridade, alimentado pelos próprios lifecycle hooks do Kiro, notificações no fim de cada turno, retomada automática da mesma conversa ao reabrir o nó, argumentos de inicialização e uma opção para pular confirmações, além de inicialização via vspawn — tudo o que os outros agentes já tinham. O VelaTerm clona o seu agente Kiro padrão em um agente `vlx-term` próprio, acrescenta a essa cópia lifecycle hooks que apenas observam e inicia essa cópia — o seu arquivo de agente nunca é editado, e seu prompt, suas ferramentas e seus servidores MCP vêm junto sem alteração. O Kiro não tem hook de pedido de permissão, então o indicador continua em trabalho enquanto ele espera sua aprovação.

### Correções

- **Programas iniciados pelo terminal não herdam mais o ambiente do próprio AppImage (Linux).** O lançador do AppImage aponta `PYTHONHOME`, `PYTHONPATH`, `PERLLIB`, `QT_PLUGIN_PATH` e os caminhos de plugins do GStreamer para o diretório de montagem temporária do pacote, e coloca os diretórios do pacote à frente de todo o resto em `PATH` e `LD_LIBRARY_PATH`. Um terminal entrega todo o seu ambiente ao shell que inicia, então o `python3` do sistema procurava sua biblioteca padrão dentro do pacote e se recusava a rodar, e outros programas com ligação dinâmica carregavam a cópia de uma biblioteca do pacote em vez da do sistema. Agora o VelaTerm remove esses caminhos do pacote antes de iniciar um shell ou uma ferramenta externa, e não mexe nos valores que você mesmo definiu. `APPDIR` e `APPIMAGE` continuam visíveis, de modo que programas que verificam se estão rodando a partir de um AppImage seguem tendo sua resposta. Só as builds AppImage eram afetadas; o pacote deb, o macOS e o Windows se comportam como antes.

## v0.1.99 — 2026-08-09

### Terminal

- **Shift+Enter escreve uma nova linha em vez de enviar.** Terminais não têm codificação para Enter com tecla modificadora, então CLIs de agentes como Claude Code e Codex recebiam apenas um retorno de carro comum e enviavam o texto enquanto ainda estava sendo escrito. Agora o VelaTerm emite ESC+CR, a mesma sequência que essas ferramentas esperam de um mapeamento de teclas do iTerm2, o que torna a entrada de várias linhas utilizável — inclusive no macOS, onde o tratador de teclas personalizado sequer era instalado. A composição em um método de entrada continua intacta: Enter ainda confirma o candidato.

### Projetos e organização

- **Atualizar o status de uma única sessão.** Em um painel com filtro de status, as sessões ganham a ação «Atualizar status», que reavalia somente aquela sessão segundo as condições do próprio painel, incluindo-a ou removendo-a enquanto todas as outras permanecem no lugar. A ação pertence ao painel de onde o menu foi aberto, de modo que divisões aninhadas nunca tomam emprestado o filtro de outro painel. O resultado é guardado por painel e restaurado após reiniciar.
- **Remover uma marca exige um clique.** Escolher o emoji já aplicado passa a removê-lo, então o item dedicado à remoção e seu separador foram retirados. O selo de emoji no botão de filtro também saiu: o destaque já mostra que há um filtro de marca ativo, e o menu mostra qual é.

### Correções

- **A integração de área de trabalho do AppImage do Linux instala em qualquer máquina.** O ícone incluído era um link simbólico para um caminho absoluto da máquina de compilação, de modo que ferramentas como Gear Lever e AppImageLauncher não conseguiam extraí-lo, embora o próprio aplicativo funcionasse normalmente. O link agora é relativo. O requisito de glibc divulgado também foi corrigido para 2.35 depois de medir as bibliotecas distribuídas junto, e não apenas o executável, o que torna o Ubuntu 22.04 a distribuição mais antiga compatível com o aplicativo de área de trabalho.

## v0.1.98 — 2026-08-02

### Agentes de IA

- **O Grok Build passa a ser um agente de primeira classe no VelaTerm.** Instale, inicie e retome o Grok 4.5 com IDs de sessão estáveis, lifecycle hooks oficiais, estados precisos de trabalho e permissão, transcrições unificadas, detalhes de uso e um ícone oficial que acompanha o tema nas visualizações de desktop, navegador e celular.

### Projetos e organização

- **Divida a barra lateral de projetos em visualizações de trabalho independentes.** Qualquer painel da árvore pode ser dividido novamente para baixo e restaura, após a reinicialização, sua própria pesquisa, filtros de status e emoji, estado de recolhimento e proporção de tamanho. Todos os painéis continuam sendo projeções da mesma árvore de projetos mantida pelo backend, portanto as edições ficam sincronizadas sem duplicar dados de negócio.
- **Marque e filtre nós sem perder o contexto.** Projetos, grupos e sessões podem receber marcadores de emoji. Um contêiner marcado mantém toda a sua subárvore visível, a associação de status permanece estável durante o trabalho, tanto a inclusão dinâmica quanto a atualização manual estão disponíveis, e as condições de status e emoji são combinadas como uma união.
- **Crie um projeto vazio no próprio lugar.** Escolha o diretório pai, valide o nome e crie e importe a pasta em um único fluxo. Se houver uma falha parcial, somente a importação será repetida, sem criar diretórios duplicados.

### Interface

- **Compartilhe o VelaTerm onde sua comunidade está.** A janela de compartilhamento agora inclui WeChat Moments, Weibo, Xiaohongshu, X, Reddit, Hacker News, LinkedIn, Facebook, Telegram e WhatsApp, com um fluxo por QR code para o WeChat e um convite para compartilhar na janela de atualização.
- **Pequenas interações ficaram mais cuidadosas.** As abas temporárias do terminal podem ser renomeadas antes de virarem sessões salvas. Os campos de entrada comuns desativam a capitalização automática dos teclados móveis sem alterar a entrada do terminal.

## v0.1.97 — 2026-07-25

### Agentes de IA

- **As sessões não ficam mais presas em “trabalhando”.** O Codex informava a atividade das ferramentas e o fim do turno por processos efêmeros separados, cujos callbacks podiam chegar fora de ordem e deixar um turno concluído exibido como ainda em execução. Agora os relatos intermediários que chegam após o fim do próprio turno são descartados, e um novo hook de fim de sessão cobre sessões que terminam sem evento de conclusão.
- **Turnos interrompidos se resolvem em segundos.** Pressionar Esc, ou um erro de streaming, encerra um turno do Claude ou do Codex sem nenhum callback de conclusão. Seis segundos de silêncio no terminal agora corrigem discretamente essa sessão para aguardando, sem gerar uma notificação de “respondeu”.

### Interface

- **Atalhos de divisão confiáveis no macOS.** Dividir à direita (Cmd+D) e dividir abaixo (Cmd+Shift+D) agora são registrados como comandos do menu Terminal nativo, de modo que o macOS não intercepta mais a combinação antes do VelaTerm.
- **Um salvamento por tecla pressionada.** O Cmd+S era tratado tanto pelo atalho global quanto pelo editor em foco, o que podia gravar o mesmo arquivo duas vezes em um único pressionamento.

## v0.1.96 — 2026-07-23

### Agentes de IA

- **O status do Codex confia em lifecycle hooks, não em suposições do terminal.** Sessões modernas do Codex agora usam apenas os lifecycle hooks oficiais como fonte de atividade. Um handshake `SessionStart` verifica a integração, callbacks ausentes aparecem como “Status indisponível” e o texto ou a atividade do terminal não pode mais sobrescrever estados de trabalho, confirmação ou conclusão.
- **Uso do Codex mais atualizado após cada turno.** O painel Info mostra imediatamente o snapshot rollout local, compara-o com os limites ao vivo, atualiza novamente depois que o Codex grava o snapshot token final e ignora respostas atrasadas de uma sessão anterior.

### Interface

- **Seleção confiável na árvore de projetos no macOS.** As linhas virtuais não dependem mais de transform do compositor, impedindo que coordenadas de hit-test obsoletas do WKWebView enviem ações de passar o mouse, clicar ou arrastar para outra linha após rolagem ou atualização da árvore.

## v0.1.95 — 2026-07-21

### Agentes de IA

- **Kimi Code e Zoo Code chegam à árvore de sessões.** O VelaTerm agora inicia, retoma, instala e configura os dois agentes. O Kimi usa lifecycle hooks oficiais para informar com autoridade os estados de trabalho, permissão e espera; o Zoo Code mantém uma identidade de tarefa estável e usa detecção do terminal quando não há hooks externos.
- **Atualização ao vivo do uso do Codex.** O painel Info consulta o Codex app server para obter os limites atuais e mantém o snapshot rollout local como fallback compatível.

### Projetos e terminais

- **Abra projetos com `vela <path>`.** Builds empacotadas podem instalar um comando shell no estilo do VS Code. Uma segunda chamada envia o projeto à janela VelaTerm existente, sem abrir uma instância duplicada.
- **Clone Git visível e cancelável.** Clone Project mostra etapas, porcentagem e tempo decorrido, alerta quando o progresso para e cancela toda a árvore de processos Git sem deixar um destino incompleto. Credenciais e query tokens são ocultados em erros e logs de auditoria.
- **Terminais WSL no Windows.** Todas as distribuições WSL instaladas aparecem ao lado de PowerShell, cmd e Git Bash para terminais comuns. Sessões de agentes continuam no shell host do Windows para manter hooks e caminhos de executáveis confiáveis.

### Interface e confiabilidade

- **Controle mais claro das sessões em segundo plano.** Os menus mostram o estado ao vivo de cada sessão e o diálogo de limite permite encerrar várias abas selecionadas de uma vez.
- **Ciclo de vida mais seguro e notas multilíngues.** Há confirmação antes de parar sessões ativas; a identidade lifecycle exata do Codex prevalece sobre scans rollout ambíguos; as notas de atualização suportam todos os idiomas incluídos.

## v0.1.94 — 2026-07-12

### Localização

- **Interface em vietnamita.** Tiếng Việt agora está disponível no seletor de idiomas e é selecionado automaticamente quando o sistema usa uma localidade vietnamita.

### Navegador

- **Inicialização mais rápida do navegador integrado.** Cada aba do navegador agora tem atalhos de um clique para ChatGPT, Claude, Gemini e Google. Os menus de contexto de projetos e grupos também podem criar uma página permanente do navegador diretamente na parte correspondente da árvore de sessões.

### Imagens e documentos

- **Colagem confiável de caminhos de imagem no macOS.** Quando o WebKit não expõe uma imagem copiada como arquivo, o VelaTerm passa a lê-la da área de transferência nativa e ainda a envia como caminho de arquivo, em vez de recorrer silenciosamente ao espaço reservado de imagem nativo do agente. As janelas remotas sempre exibem a configuração de colagem de imagens, explicam por que o modo de caminho de arquivo é necessário e desativam a opção nativa indisponível.
- **Colagem de imagens em documentos-fonte.** O editor de código-fonte agora aceita imagens da área de transferência. Documentos Markdown salvos armazenam as imagens ao lado do documento em `assets/` e inserem uma sintaxe de imagem Markdown portátil; rascunhos não salvos incorporam os dados da imagem para que eles não sejam perdidos na limpeza dos arquivos temporários.

### Interface

- **Os menus de contexto permanecem visíveis e apontam para o item correto.** Menus abertos perto da borda direita são medidos e reposicionados corretamente. Clicar com o botão direito em um nó da árvore agora destaca apenas o alvo do menu, sem alterar a seleção existente; os menus de grupo também incluem um terminal limitado àquele grupo.
- **Edição e rótulos de status mais limpos.** O texto-fonte não renderiza mais ligaduras de fonte em forma de seta para sequências como comentários HTML, as porcentagens de uso são explicitamente rotuladas como usadas e o menu de contexto nativo não relacionado do WebView hospedeiro não aparece mais atrás dos menus do VelaTerm.

### Correções

- **Codex permanece no histórico normal do terminal.** As sessões do Codex iniciadas pelo VelaTerm agora usam o modo de terminal inline. Assim, pressionar Esc para interromper ou voltar não alterna mais os buffers de tela do terminal nem leva a visualização do histórico para o topo. A configuração do Codex do usuário não é alterada.
