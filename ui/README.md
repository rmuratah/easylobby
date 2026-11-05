# ui

Interface do easylobby em React + TypeScript + Vite + Tailwind, testada com Vitest.

```bash
pnpm install          # na raiz do repo
pnpm --filter easylobby-ui dev
pnpm lint
pnpm test
```

A UI só fala com o canal `/ws` do agente. Use só os tipos gerados em `src/bindings/`; não invente formato de mensagem.
