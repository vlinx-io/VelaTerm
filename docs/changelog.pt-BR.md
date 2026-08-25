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
