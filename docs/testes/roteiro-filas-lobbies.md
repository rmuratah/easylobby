# Roteiro de teste no LoL real: filas e lobbies (#323)

Roteiro para testar, no League of Legends real, a criação de cada fila e modo pela tela **Jogar** e as funções de cada lobby. Feito a partir do código da `main` em 09/11/2026 (último commit `d429be9`).

Em cada passo, faça a ação, confira o resultado esperado e marque a coluna **Resultado** com `OK` ou `Falhou`. Se falhar, escreva em **Obs.** o que apareceu (texto do erro, print, o que o client oficial mostrava). No fim tem um quadro de resumo e o jeito de reportar.

## Dependências de PR

| Item | Depende de | Situação em 09/11 |
| --- | --- | --- |
| Bots aparecendo nas equipes da personalizada e do Modo de Treino (seções 5.3 e 6.2) | #322 | **Aberto.** Sem ele, o bot é adicionado no LoL, mas a vaga aparece vazia no app. Teste essas linhas depois do merge, ou rode a branch `fix/313-bots-personalizada`. |
| Ordem das 5 posições no Summoner's Rift Clássico (passo 2.8) | #230 (só o agente saiu, no #297) | **Sem tela.** O agente já lê e manda a ordem, mas a UI ainda não tem a tela de ordenar. Só anote o que aparece. |

Todo o resto já está na `main`: #282 (card Summoner's Rift Clássico), #290 e #307 (Arena em 6 equipes de 3), #292 (Tornar líder e Remover), #293 (Arena liberada), #306 e #309 (Entrar numa personalizada), #310 (abrir perfil pelo lobby), #276 (elo no card), #274 e #272 (criar personalizada), #269 (aba Personalizada e espectadores), #287 (chat escondido sozinho), #298 e #295 (ícones dos mapas e dos modos).

## 0. Preparação

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 0.1 | Atualize a `main` (`git pull`), rode `pnpm install --frozen-lockfile`. No PowerShell, tire o mock: `Remove-Item Env:EASYLOBBY_MOCK_LCU -ErrorAction SilentlyContinue`. | Instalação sem erro. | | |
| 0.2 | Abra o LoL e faça login. Rode `pnpm dev:desktop`. | O app abre, conecta, a UI oficial fecha sozinha (kill-ux) e o Início mostra seu invocador. | | |
| 0.3 | Separe uma **segunda conta** (um amigo online ou uma smurf em outro PC). Para a Arena, se der, uma **terceira**. | Os passos marcados com 👥 precisam dela. | | |
| 0.4 | Deixe o botão **Abrir client oficial** à mão para conferir o que o client mostra quando algo parecer errado. Feche o client de novo antes de seguir. | O client abre e fecha sem derrubar o app. | | |

## 1. Tela Jogar (sem lobby)

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 1.1 | Clique em **Jogar** na barra lateral, sem estar em lobby. | Título "Escolha o modo de jogo" e "Você não está em um lobby." Abas: **PvP**, **Co-op versus IA**, **Treinamento**, **Personalizada**. | | |
| 1.2 | Olhe os cards da aba PvP. | Cards com ícone (não sigla): **Summoner's Rift**, **ARAM**, **Arena** e, se a Riot tiver outras filas abertas, **Summoner's Rift Clássico**. | | |
| 1.3 | Confira os submodos do card Summoner's Rift. | Partida Dinâmica, Partida Alternada, Ranqueada Flex, Ranqueada Solo/Duo, cada um com descrição curta. | | |
| 1.4 | Confira ARAM e Arena. | ARAM: ARAM Normal e ARAM Desordem. Arena: Arena 3x6 (e outras Arenas abertas, se houver, no mesmo card). | | |
| 1.5 | Compare com o client oficial quais filas estão fechadas agora. | Fila fechada pela Riot aparece cinza com "Fechada agora." e não dá para marcar. Card sem nenhuma fila aberta aparece apagado com "Fechado agora." | | |
| 1.6 | Escolha um submodo qualquer e olhe o rodapé. | Rodapé mostra "Modo — Submodo" e o botão **Criar lobby** habilitado. | | |
| 1.7 | Clique em **Criar lobby** duas vezes bem rápido. | Mostra "Criando lobby…", só cria um lobby, depois abre a tela do lobby. | | |
| 1.8 | Troque de aba e volte. | A escolha da aba volta para o primeiro submodo aberto, sem erro. | | |

## 2. PvP: criar cada fila e conferir o lobby

Para cada linha: na tela Jogar (sem lobby), escolha a fila, clique **Criar lobby**, confira o lobby e saia pelo **X** (Sair do lobby) antes da próxima. Em todas, o topo do lobby deve mostrar o nome da fila, "1 / N jogadores", "Você é o líder", **Lobby aberto/fechado**, **Copiar link**, **Trocar modo** e o **X**.

| # | Fila (queueId) | Resultado esperado no lobby | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 2.1 | Partida Dinâmica (480) | "1 / 5 jogadores". No seu card aparecem os 2 slots de posição + campeão (Jogo Dinâmico). Embaixo, o quadro "Suas posições e campeões". Escolha 2 posições e um campeão para cada uma: salva sozinho ("Salvando posições e campeões…"). Posição prioritária mostra o losango. Confira no client oficial que ficou igual. | | |
| 2.2 | Partida Alternada (400) | "1 / 5". Quadro **Suas posições** com Posição primária e secundária. Escolha primária e secundária: salva sozinho em ~0,5 s e aparece no seu card. Com primária "Preencher", a secundária fica bloqueada. Só primária escolhida: aparece "Escolha a posição secundária para salvar." | | |
| 2.3 | Ranqueada Flex (440) | "1 / 5". Posições como no 2.2. O selo de elo no seu card mostra o elo da **Flex**. | | |
| 2.4 | Ranqueada Solo/Duo (420) | "1 / **2**". Posições como no 2.2. O selo de elo mostra o elo da **Solo/Duo**. Só uma vaga livre. | | |
| 2.5 | ARAM Normal (450) | "1 / 5". Sem quadro de posições. | | |
| 2.6 | ARAM Desordem (2400) | "1 / 5". Sem posições. Se a fila estiver fechada, ela aparece cinza no 1.4 e este passo fica "não se aplica". | | |
| 2.7 | Arena 3x6 (1750) | "1 / **18**". No lugar da lista aparecem **6 equipes de 3** (Equipe 1 a 6), você numa delas com "Sua equipe". Sem posições. Detalhes da Arena na seção 8. | | |
| 2.8 | Summoner's Rift Clássico (4310, se aberta) | Card **Summoner's Rift Clássico** na aba PvP, com a fila aberta pela Riot. Cria o lobby com o nome da fila. **Posições:** o LoL pede a ordem das 5 posições, mas a tela de ordenar ainda não existe (#230). Anote o que o app mostra (primária/secundária ou nada) e o que o client oficial mostra. | | |
| 2.9 | Outras filas no card Summoner's Rift Clássico (se houver evento) | Cada fila aberta aparece como submodo, com nome legível (não código interno). Cria o lobby. | | |

## 3. Co-op versus IA

| # | Fila (queueId) | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 3.1 | Aba **Co-op versus IA** | Card **Summoner's Rift** (Introdução, Iniciante, Intermediário) e, se aberta, **Summoner's Rift Clássico** ("O 5 contra 5 clássico contra bots"). | | |
| 3.2 | Introdução (870) | Cria o lobby "1 / 5", sem posições. | | |
| 3.3 | Iniciante (880) | Igual ao 3.2. | | |
| 3.4 | Intermediário (890) | Igual ao 3.2. | | |
| 3.5 | Clássico (Coop vs IA) (4320) no card Summoner's Rift Clássico | O submodo usa o nome que a Riot dá (ex.: "Clássico (Coop vs IA)"), com a descrição "Seu time contra bots." Cria o lobby. Anote se aparecem posições. | | |
| 3.6 | Em um lobby de Co-op, clique **Iniciar busca** e depois **Cancelar busca**. | Estado muda para "Buscando partida", com tempo na fila; cancelar volta a "Fora da fila". | | |

## 4. Treinamento

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 4.1 | Aba **Treinamento** | Dois cards: **Tutorial** e **Modo de Treino**. O botão do rodapé vira **Iniciar**. | | |
| 4.2 | Escolha Tutorial. | Aparecem as 3 partes; as concluídas têm "Concluída"; a escolhida por padrão é a primeira liberada que falta. Rodapé "Tutorial — Parte N". | | |
| 4.3 | Escolha uma parte e clique **Iniciar** (pode sair da partida logo depois). | "Iniciando o Tutorial…", depois "Tutorial iniciado." e o LoL abre a parte certa. | | |
| 4.4 | Escolha **Modo de Treino** e clique **Iniciar**. | "Iniciando o Modo de Treino…", depois abre o lobby com título "Modo de Treino", Equipe 1 e Equipe 2, e o botão **Iniciar partida**. | | |
| 4.5 | No lobby do Modo de Treino, adicione um bot (veja 6.2). ⚠️ #322 | O bot aparece na equipe com campeão, posição e dificuldade. | | |
| 4.6 | Clique **Iniciar partida**. | "Iniciando a partida…" e o jogo abre no Summoner's Rift. Saia da partida. | | |

## 5. Personalizada: criar

Aba **Personalizada**, opção **Criar**.

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 5.1 | Abra a aba. | No topo, os 3 mapas com ícone: **Summoner's Rift**, **Mapa aleatório**, **Rift Clássico**. Embaixo, à esquerda: Nome, Tamanho da equipe, Senha, "Esconder da lista de saguões públicos". À direita: Tipo de jogo, Espectadores, Intervalo de espectador. Botão **Confirmar**. | | |
| 5.2 | Confira o nome sugerido. | "Partida de \<seu nome\>". Se o app abriu antes do login, o nome atualiza sozinho quando o invocador chega. | | |
| 5.3 | Apague o nome todo. | Aparece "Dê um nome para a partida." e **Confirmar** fica desabilitado. Nome aceita até 64 caracteres. | | |
| 5.4 | Summoner's Rift: confira o Tipo de jogo. | Escolha às Cegas, Modo alternado, Todos aleatórios, Alternada Torneio. | | |
| 5.5 | Mapa aleatório: confira Tipo de jogo e a seleção de mapas. | Tipos: Todos aleatórios, ARAM: Desordem, Escolha às Cegas, Alternada Torneio, Modo alternado. Aparece a escolha do mapa: Howling Abyss, Travessia de Koeshin, Ponte da Carnificina, Mapa aleatório. | | |
| 5.6 | Rift Clássico: confira Tipo de jogo. | Escolha Alternada e Escolha às Cegas. | | |
| 5.7 | Tamanho da equipe. | Opções de 1 a 5. | | |
| 5.8 | Espectadores e intervalo. | Espectadores: Somente do saguão, Somente da lista de amigos, Todos, Nenhum. Intervalo: Adicionar intervalo, Sem intervalo. O rodapé resume as escolhas e diz "Com senha" quando há senha. | | |
| 5.9 | Crie: Summoner's Rift, Escolha às Cegas, 5, **sem senha**, espectadores Todos. **Confirmar**. | "Criando a partida personalizada…" e abre o lobby da personalizada: título com o nome, resumo (mapa, tipo, tamanho), Equipe 1 e Equipe 2 com "1 / 5" e "0 / 5", você na Equipe 1. Confira no client oficial que as opções batem. Saia. | | |
| 5.10 | Crie: Mapa aleatório, Todos aleatórios, Ponte da Carnificina, 3, **com senha** `teste123`, espectadores Somente da lista de amigos. | Lobby abre com o resumo certo e equipes de 3. Confira no client oficial. **Não saia**: use este lobby no 6 e no 7.👥 | | |
| 5.11 | Crie com Rift Clássico, Escolha Alternada (depois de sair do anterior). | Lobby abre com o mapa certo. Saia. | | |
| 5.12 | Crie com "Esconder da lista de saguões públicos" marcado. | 👥 O amigo **não** vê a partida na lista do Entrar (seção 7). | | |

## 6. Lobby da personalizada

Use o lobby do 5.10 (ou crie um igual).

### 6.1 Equipes e espectadores

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 6.1.1 | Olhe as vagas livres da Equipe 2. | Na primeira vaga livre da outra equipe aparece **Juntar-se**. Como líder, cada vaga livre tem **Adicionar bot**. | | |
| 6.1.2 | Clique **Juntar-se** na Equipe 2. | "Trocando de equipe…", você passa para a Equipe 2. Volte para a 1 do mesmo jeito. | | |
| 6.1.3 | Seção **Espectadores**: clique **Assistir**. | Mostra "Quem pode assistir: Somente da lista de amigos", "1 / N", você aparece em "Jogadores assistindo" e a frase "Você está assistindo. Use Juntar-se para voltar a uma equipe." Volte com **Juntar-se**. | | |
| 6.1.4 | Crie uma com espectadores **Nenhum**. | A seção Espectadores some (se ninguém estiver assistindo). | | |
| 6.1.5 | Repare no topo do lobby. | Tem "Você é o líder", **Lobby aberto/fechado** e o **X**. Não tem Copiar link nem Trocar modo (normal na personalizada). | | |

### 6.2 Bots ⚠️ depende do #322

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 6.2.1 | Clique **Adicionar bot** na Equipe 2. | Abre o pop-up "Novo bot na Equipe 2" com busca e grade de campeões (só campeões que podem ser bot). | | |
| 6.2.2 | Busque um nome (ex.: "ann") e escolha. | O bot aparece na Equipe 2 com o campeão, posição livre e dificuldade **Introdução**. | | |
| 6.2.3 | Clique no campeão do bot e troque. | "Mudando o bot…" e o campeão muda. | | |
| 6.2.4 | Troque a posição e a dificuldade do bot. | As listas salvam sozinhas; confira no client oficial. | | |
| 6.2.5 | Adicione um bot **pelo client oficial** e feche o client. | O bot aparece no app (é o bug do #322; antes do merge a vaga aparece vazia). | | |
| 6.2.6 | Clique no **X** do bot. | "Tirando o bot…" e a vaga fica livre. | | |
| 6.2.7 | 👥 Com o amigo no lobby (como membro), peça para ele olhar. | Para ele, os bots aparecem sem controles e sem "Adicionar bot". | | |

### 6.3 Líder e membros na personalizada 👥

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 6.3.1 | Com o amigo dentro (via seção 7), clique no **⋯** do card dele. | Menu com **Tornar líder** e **Remover do grupo** (vermelho). | | |
| 6.3.2 | Tornar líder → Cancelar; depois Tornar líder → confirmar. | A pergunta "Passar a liderança para X? Só o novo líder pode devolver." O cancelar não faz nada. Confirmando, a coroa passa para ele e o seu topo muda para "Você é membro". Peça para ele devolver. | | |
| 6.3.3 | Remover do grupo → confirmar. | "Remover X do grupo?" e o amigo sai do lobby. | | |
| 6.3.4 | Como membro, olhe o rodapé. | "Aguarde o líder iniciar a partida." e sem botão Iniciar partida. | | |

## 7. Personalizada: Entrar 👥

O amigo cria uma personalizada **com senha** (ou você cria e ele entra; teste dos dois lados se der).

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 7.1 | Aba Personalizada, opção **Entrar**. | Lista "Personalizadas abertas" com colunas Nome, Dono, Mapa, Jogadores, Espect. Se ainda não carregou: "A lista ainda não foi carregada. Use Atualizar." | | |
| 7.2 | Clique **Atualizar**. | A lista é a mesma que o client oficial mostra (compare). Sem nenhuma: "Nenhuma personalizada aberta agora." | | |
| 7.3 | Busque pelo nome ou pelo dono da partida do amigo. | Filtra; sem resultado: "Nenhuma personalizada encontrada para …". | | |
| 7.4 | Escolha a partida com senha. | O rodapé mostra dono, mapa e "N/M jogadores" e aparece o campo **Senha**. **Entrar** fica desabilitado até digitar a senha. | | |
| 7.5 | Digite uma senha **errada** e clique **Entrar**. | Aparece erro ("Não foi possível alterar o lobby: …"), sem travar o app. | | |
| 7.6 | Digite a senha certa e aperte **Enter**. | "Entrando na partida personalizada…", depois "Você entrou na partida personalizada." e abre o lobby da personalizada como membro. | | |
| 7.7 | Entre numa partida **sem senha**. | Sem campo de senha, entra direto. | | |
| 7.8 | Partida cheia na lista. | Aparece marcada como cheia (dica "Partida cheia"). | | |
| 7.9 | Partida com "Esconder da lista" (5.12). | Não aparece na lista. | | |

## 8. Arena 3x6 (6 equipes) 👥

Crie a Arena 3x6 (passo 2.7) e chame o amigo.

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 8.1 | Olhe as equipes. | 6 equipes (2 colunas em janela estreita, 3 em larga), cada uma com 3 vagas. Sua equipe com "Sua equipe" e você com "Você · Nível N". | | |
| 8.2 | Clique no ícone de **trocar** numa vaga livre de outra equipe. | "Trocando para a vaga X da Equipe Y…" e você muda de vaga. Confira no client oficial. | | |
| 8.3 | Clique no **+** de uma vaga livre. | Abre o pop-up de convite (seção 9.3). Convide o amigo. | | |
| 8.4 | O amigo entra. | Ele aparece numa vaga com nome e nível; o contador vira "2 / 18". | | |
| 8.5 | Coloque você e o amigo na mesma equipe (cada um troca de vaga). | Os dois ficam na mesma equipe; ele vê o mesmo. | | |
| 8.6 | Clique na vaga do amigo (fora do ⋯). | Abre o perfil dele (seção 9.8). Volte ao lobby. | | |
| 8.7 | Abra o **⋯** na vaga dele. | O menu abre para a direita e cabe no card. Tornar líder e Remover funcionam como no 6.3. | | |
| 8.8 | Se alguém estiver sem vaga. | Aparece "Aguardando vaga: nome." | | |
| 8.9 | Clique **Iniciar busca** e depois **Cancelar busca**. | Durante a busca o botão de trocar de vaga fica desabilitado. | | |

## 9. Funções comuns do lobby

Faça numa **Partida Alternada (400)**, salvo quando a linha diz outra fila.

### 9.1 Trocar modo

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 9.1.1 | Clique **Trocar modo**. | Título "Trocar modo de jogo", "Você está no lobby de Partida Alternada." Sem as abas Treinamento e Personalizada. | | |
| 9.1.2 | Olhe a Partida Alternada. | Bloqueada, com "Você já está nesta fila." Já vem marcada outra fila aberta do mesmo card. | | |
| 9.1.3 | **Voltar ao lobby**. | Volta sem trocar nada. | | |
| 9.1.4 | Trocar modo → ARAM Normal → **Trocar fila**. | "Trocando fila…", volta ao lobby já em ARAM, sem quadro de posições. | | |
| 9.1.5 | Troque para Ranqueada Solo/Duo com **3 pessoas** no lobby. 👥 | A LCU recusa: aparece erro e o lobby continua como estava. | | |
| 9.1.6 | Troque para Arena 3x6 e depois de volta para Alternada. | As 6 equipes aparecem e depois a lista normal com posições. | | |
| 9.1.7 | Como **membro** (não líder). 👥 | **Trocar modo** desabilitado, com "Somente o líder pode trocar a fila. Você pode sair do lobby." | | |

### 9.2 Lobby aberto/fechado e Copiar link

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 9.2.1 | Clique no **Lobby fechado** para abrir. | "Abrindo o lobby…", vira **Lobby aberto**. | | |
| 9.2.2 | 👥 Com o lobby aberto, o amigo entra pela sua lista de amigos (pelo client dele) sem convite. | Ele entra direto. | | |
| 9.2.3 | Feche o lobby. 👥 O amigo sai e tenta entrar de novo. | Sem convite, não entra. | | |
| 9.2.4 | Como membro. 👥 | O botão mostra o estado, mas fica desabilitado ("Só o líder abre ou fecha o lobby."). | | |
| 9.2.5 | Clique **Copiar link**. | "Gerando o link…", o botão vira "Link copiado" por ~2 s. Cole no bloco de notas: é um link. Se a cópia falhar, o link aparece embaixo do botão para copiar à mão. | | |
| 9.2.6 | 👥 Mande o link para o amigo e ele abre. | Ele entra no seu lobby (anote como o link se comporta). | | |
| 9.2.7 | Lobby aberto/fechado e Copiar link no Tutorial. | Não aparecem no lobby do Tutorial. | | |

### 9.3 Convites 👥

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 9.3.1 | Clique no **+** de uma vaga livre. | Abre "Convidar amigos": filtro com Nome de jogo e Tagline, "Jogadores disponíveis" com **Convidar por nome**, **Recentes** e as pastas de amigos como no client. | | |
| 9.3.2 | Abra Recentes. | Lista quem jogou com você recentemente (com o campeão), ou "Nenhum jogador recente para convidar." | | |
| 9.3.3 | Digite o Riot ID do amigo (nome e tagline). | As pastas filtram. Em **Convidar por nome** aparece o Riot ID com o botão **Convidar**. | | |
| 9.3.4 | Clique **Convidar** no Convidar por nome. | Vira "Convite enviado" e o amigo recebe o convite. | | |
| 9.3.5 | Convide por nome alguém que **não existe** (ex.: `naoexiste#0000`). | Aparece erro, sem travar. | | |
| 9.3.6 | Marque 2 amigos nas pastas e clique **Enviar convites**. | O contador mostra "2 selecionados"; os dois recebem. Com mais marcados que vagas: "Limite de vagas: N". | | |
| 9.3.7 | Card **Grupo** (coluna da direita). | Mostra cada convidado com o status: "Convite pendente", "No grupo" (✓), "Recusou o convite" (✕). Peça para um recusar e outro aceitar. | | |
| 9.3.8 | Arraste um amigo da lista de amigos para cima do lobby. | O lobby ganha borda de destaque e soltar manda o convite. | | |
| 9.3.9 | Na Ranqueada Solo/Duo com o amigo dentro. | Sem vaga livre e sem +. | | |

### 9.4 Chat do grupo 👥

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 9.4.1 | Sozinho no lobby. | O card **Chat do grupo** não aparece. | | |
| 9.4.2 | O amigo entra. | O card aparece ("Aguardando a sala do grupo abrir…" e depois "Nenhuma mensagem ainda."). | | |
| 9.4.3 | Mande uma mensagem e peça uma de volta. | As duas aparecem nos dois lados, na ordem, com o nome. O botão mostra "Enviando…" no envio. | | |
| 9.4.4 | O amigo sai. | O card some de novo. | | |

### 9.5 Menu do líder (⋯) 👥

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 9.5.1 | Como líder, olhe os cards. | **⋯** só no card do amigo, nunca no seu. Como membro, não aparece em nenhum. | | |
| 9.5.2 | ⋯ → Tornar líder → confirmar. | Coroa passa para o amigo, você vira "Você é membro" e perde Trocar modo. Ele devolve. | | |
| 9.5.3 | ⋯ → Remover do grupo → confirmar. | Amigo sai; o card Grupo mostra "Saiu do grupo". | | |
| 9.5.4 | Abra o ⋯ e aperte **Esc**; abra e clique fora. | O menu fecha nos dois casos. | | |
| 9.5.5 | Entre na busca com o amigo no lobby. | O ⋯ fica desabilitado ("Disponível na fase de lobby"); se estava aberto, fecha. | | |

### 9.6 Posições (Alternada, Flex, Solo/Duo) 👥

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 9.6.1 | Troque as posições várias vezes rápido com as setas do teclado. | Só a última vai para o LoL; nada pisca de volta. | | |
| 9.6.2 | O amigo troca as posições dele. | O card dele atualiza no seu app, com ícone e "Primária"/"Secundária". | | |
| 9.6.3 | Mude as posições pelo client oficial e feche o client. | O app mostra as novas. | | |
| 9.6.4 | Tente mudar durante a busca. | Bloqueado, com "A escolha de posições está disponível na fase de lobby." | | |

### 9.7 Elo no card

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 9.7.1 | Em qualquer lobby, olhe o selo ao lado do nível. | Mostra o elo da Solo/Duo (na Flex, o da Flex), igual ao perfil. Sem ranque: anote o que aparece. | | |
| 9.7.2 | 👥 Card do amigo. | Mostra o elo dele. | | |

### 9.8 Abrir perfil pelo lobby 👥

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 9.8.1 | Clique no card do amigo (fora do ⋯). | Abre o perfil dele (dica "Ver perfil de X"). | | |
| 9.8.2 | Clique no seu próprio card. | Não abre nada. | | |
| 9.8.3 | Clique no ⋯ do card dele. | Abre só o menu, não o perfil. | | |
| 9.8.4 | Volte para Jogar. | O lobby continua igual. | | |

### 9.9 Busca e saída

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 9.9.1 | Como líder, **Iniciar busca**. | "Buscando partida", tempo na fila e estimativa. Trocar modo e o X somem; posições ficam bloqueadas. | | |
| 9.9.2 | **Cancelar busca**. | Volta a "Fora da fila" e os botões voltam. | | |
| 9.9.3 | Como membro. 👥 | Não consegue buscar: "Somente o líder do lobby pode iniciar a busca." | | |
| 9.9.4 | Clique no **X**. | "Saindo do lobby…", "Saída do lobby confirmada." e volta para a tela Jogar. | | |

## 10. Robustez

| # | Passo | Resultado esperado | Resultado | Obs. |
| --- | --- | --- | --- | --- |
| 10.1 | Com um lobby aberto, feche o LoL. | O lobby mostra "Aguarde a conexão com o agente e o League of Legends…". O app não fecha. | | |
| 10.2 | Abra o LoL de novo e faça login. | O app reconecta e mostra o lobby (ou a tela Jogar). Nenhum comando é repetido sozinho. | | |
| 10.3 | Crie um lobby pelo client oficial, depois feche o client. | O app mostra o lobby criado lá. | | |
| 10.4 | 👥 Aceite pelo app um convite do amigo. | Você entra no lobby dele como membro, com as restrições de membro. | | |

## Resumo

| Seção | OK | Falhou | Não testado |
| --- | --- | --- | --- |
| 1. Tela Jogar | | | |
| 2. PvP | | | |
| 3. Co-op versus IA | | | |
| 4. Treinamento | | | |
| 5. Personalizada: criar | | | |
| 6. Lobby da personalizada | | | |
| 7. Personalizada: entrar | | | |
| 8. Arena 3x6 | | | |
| 9. Funções do lobby | | | |
| 10. Robustez | | | |

**Testado por:** \_\_\_\_ **Data:** \_\_\_\_ **Patch do LoL:** \_\_\_\_ **Commit da main:** \_\_\_\_

## Como reportar uma falha

Abra uma issue por falha, com o número do passo no título (ex.: "Arena: trocar de vaga não muda a equipe (passo 8.2)"), print do app e do client oficial, e o log do agente do terminal do `pnpm dev:desktop`. Se o passo for de um PR aberto (#322), comente no próprio PR.
