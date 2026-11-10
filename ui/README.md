# ui

Interface do easylobby em React + TypeScript + Vite + Tailwind, testada com Vitest.

```bash
pnpm install          # na raiz do repo
pnpm --filter easylobby-ui dev
pnpm lint
pnpm test
```

A UI só fala com o canal `/ws` do agente. Use só os tipos gerados em `src/bindings/`; não invente formato de mensagem.

## Testar a conexão no navegador

Na raiz do repositório, use três terminais:

```bash
# 1: LCU simulada
cargo run -p mock-lcu
# 2: agente, único ponto de acesso da UI
cargo run -p easylobby-agent --example canal_ws -- --mock
# 3: interface
pnpm dev:ui
```

Abra o endereço do Vite na porta 5173 (localhost ou 127.0.0.1). Para LoL real,
remova `--mock` do comando do agente e faça login no client. Não rode simultaneamente
o desktop, que já inicia seu próprio agente na porta 47800.

A UI aguarda `hello` antes de considerar o canal pronto, aplica os eventos usando
os tipos gerados e substitui todo o estado ao receber outro `hello`. Ao perder o
agente, limpa os dados e tenta reconectar com intervalos de 1, 2, 4 e até 8 segundos.
Protocolos incompatíveis exigem atualizar UI/agente e recarregar a página.
O cliente também oferece `send` para os comandos gerados, com correlação por ID e
limite de 10 segundos por resposta; comandos pendentes não são reenviados após queda.

Validação: pare o agente, confira "Reconectando ao agente", reinicie e confira a
recuperação. Pare apenas o mock e confirme a espera pelo LoL. A #7 adicionará nome e nível.

```bash
pnpm test
pnpm lint
pnpm build:ui
```

## Cargos no lobby (#18): roteiro Windows

Com Node.js 22, pnpm 10.28.0 e Rust estável instalados, rode `pnpm install
--frozen-lockfile` na raiz. Abra três terminais PowerShell nessa mesma pasta:

1. `cargo run -p mock-lcu -- --parado` (sem reprodução de eventos gravados).
2. `cargo run -p easylobby-agent --example canal_ws -- --mock`.
3. `pnpm dev:ui`; abra `http://localhost:5173`.

Crie um lobby Normal alternada ou Ranqueada. Escolha Meio e Selva, clique em
**Salvar cargos** e confira os cargos na lista de jogadores. Durante o envio,
os controles ficam bloqueados; clicar duas vezes não deve repetir o comando.
A mensagem de sucesso confirma o comando; os cargos exibidos vêm do estado do
agente. Confira também:

- Primário Preenchimento ou Não selecionado deixa o secundário Não selecionado;
  o mesmo cargo não pode ser primário e secundário. Preenchimento secundário é
  permitido com um primário específico.
- ARAM e Jogo Dinâmico mostram que a fila não permite escolher cargos.
- Pare o agente durante um envio, reinicie e confira a reconexão sem reenvio
  automático. Repita parando somente o mock.
- No LoL real, use o agente sem `--mock` (e sem o mock rodando): entre no lobby
  de outra pessoa e confira que pode escolher seus cargos sem ser líder.
  O jogador local é identificado por `puuid`, não pela posição na lista.

Para executar a integração automática real no Windows:

```powershell
cargo build --locked -p mock-lcu
cargo build --locked -p easylobby-agent --example canal_ws
$env:EASYLOBBY_INTEGRATION = '1'
pnpm --filter easylobby-ui exec vitest run src/agent/integration.test.ts
Remove-Item Env:EASYLOBBY_INTEGRATION
```

Feche as instâncias manuais do agente/mock antes dessa integração. Ela valida
`setPositions` e o estado confirmado nas filas com cargos, além do fluxo de
lobby e reconexão. Os testes de UI com WebSocket simulado também cobrem erros,
timeout, membro sem liderança e respostas tardias. Testes simulados não
substituem a validação visual no Windows/LoL real.

## Paleta dos temas

`src/theme/themeColors.ts` preserva a paleta fornecida por Victor (claro e escuro),
incluindo gradientes, bronze rosado, status e avatares. Após editar esse arquivo, gere
as variáveis CSS na raiz do repositório:

```bash
node ui/scripts/generate-palette.mjs
```

`tokens.css` mantém os valores originais em `--el-palette-*` e os papéis existentes
em `--el-*`. Texto, controles e indicadores usam variantes ajustadas para os
contrastes já exigidos pelos testes. O seletor Sistema/Claro/Escuro continua sendo
o responsável pela troca. As classes slate das telas ainda em redesign são
mapeadas para os tokens para acompanhar o tema.
