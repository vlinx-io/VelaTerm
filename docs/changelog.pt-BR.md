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
