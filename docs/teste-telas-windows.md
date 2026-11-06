# Telas Home, Lobby e Fila (#21)

## Roteiro manual Windows

Pré-requisitos: Rust estável, ferramentas de compilação/WebView2 do Tauri, Node 22,
pnpm indicado no package.json e LoL instalado. Rodar `pnpm install --frozen-lockfile`
e `pnpm dev:desktop` na raiz. Estes passos precisam ser executados com o LoL real;
os testes automatizados da UI simulam o WebSocket do agente.

1. Abrir o app com LoL fechado: conferir Home, orientação de conexão e emergência
   visível (desabilitada sem conexão). Abrir LoL sem concluir login, depois entrar:
   conferir espera pelo login, Riot ID e nível. Reiniciar o agente e o LoL e
   verificar que dados antigos desaparecem e o fluxo confirmado volta após reconectar.
2. Na Home, criar lobby. Durante a operação, navegar para Runas e voltar: o comando
   deve continuar pendente, sem permitir duplicação. Conferir transição automática
   ao Lobby, membros, líder, fila selecionada e troca de fila.
3. Escolher cargos e salvar; navegar durante o envio e conferir bloqueio até a
   resposta. Testar ARAM (sem cargos) e lobby como membro (sem troca de fila/início).
4. Iniciar busca: conferir transição para Fila, tempo e estimativa. Navegar por Home
   e Runas e voltar. Cancelar, incluindo clique duplo e navegação enquanto pendente:
   conferir um comando e retorno ao estado confirmado. Simular queda durante busca
   e reconectar; não exibir tempo ou comandos baseados na sessão antiga.
5. Encontrar partida enquanto Runas estiver aberta: conferir destaque imediato e
   acesso ao botão oficial. Enquanto #106 não for integrado, aceitar/recusar pelo
   client oficial. Após integrar #106, repetir usando suas ações no destaque.
6. Entrar em seleção de campeões/partida e terminar: conferir indicação da fase,
   ausência de comandos visíveis de lobby/fila e acesso ao client oficial.
7. Nas runas, aplicar, criar, editar e excluir página. Navegar durante um envio e
   voltar: conferir rascunho/operação preservados e ausência de comandos duplicados.
8. Usar emergência em Home, Lobby, Fila, Runas e fase sem tela, inclusive durante
   outra operação. Conferir retorno de sucesso/erro e proteção contra clique duplo.
9. Reduzir janela a 800×560: rolar os painéis/editor, conferir que emergência fica
   visível, textos não escapam horizontalmente e todos os controles são alcançáveis
   por Tab/Shift+Tab, acionáveis pelo teclado e têm foco visível. Durante uma mudança
   automática de tela, conferir retorno da rolagem ao topo e foco fora de conteúdo oculto.

## Integrações pendentes

Base: main em `579e94c` (inclui runas, PR #104). PRs #106 e #103 estavam abertos na
inspeção inicial e não foram copiados, mesclados nem reimplementados nesta entrega.

- **#106 / #20:** integrar o painel de aceitar/recusar no destaque global de partida
  encontrada em App.tsx, fora dos painéis ocultos de Lobby/Fila e recebendo a mesma
  conexão. Preservar suas guardas de comandos, timers e tratamento de respostas.
  Possíveis conflitos em App.tsx e testes de integração de Lobby/Queue; os testes
  antigos agora também inspecionam controles ocultos para manter cobertura das guardas.
- **#103 / #30:** integrar FriendsList como acesso social recebendo a conexão já
  compartilhada. Possíveis conflitos em App.tsx e agent/assets.ts (runas já trouxe
  esse helper à main). #103 já apresentava conflito antes desta entrega. Não há
  botão de amigos que leve a uma funcionalidade ainda indisponível.

## Validação desta entrega

- Vitest: 185 testes passaram; 1 integração real ignorada por depender de configuração externa.
- ESLint, TypeScript + Vite e `git diff --check`: passaram.
- Chromium headless, estado simulado, viewport 800×560: Home/Lobby/Fila/partida
  encontrada/Runas, sem transbordamento horizontal, cabeçalho de emergência fora da
  área rolável e foco por teclado. Capturas de Lobby e partida encontrada inspecionadas.
- Não verificados: Tauri nativo, LoL real, mock-lcu/Rust e roteiro Windows acima.
