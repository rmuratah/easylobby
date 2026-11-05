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
