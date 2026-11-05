# easylobby

Client leve para o League of Legends. Fecha a interface oficial (LeagueClientUx) e oferece uma interface própria e leve para lobby, fila e seleção de campeões, conversando direto com a LCU API. Medimos cerca de **885 MB de RAM economizados** fechando a UI oficial.

Leia [AGENTS.md](AGENTS.md) para a visão geral da arquitetura e das decisões, e [CONTRIBUTING.md](CONTRIBUTING.md) para o fluxo de branches, commits e PRs.

## Estrutura

```
agent/          crate Rust do agente (LCU API <-> canal /ws tipado)
mock-lcu/       servidor falso da LCU, responde com fixtures gravadas
ui/             React + TypeScript + Vite + Tailwind (testes com Vitest)
desktop/        shell Tauri 2 (embute o agente e mostra a UI)
docs/           documentação e decisões
```

Os crates Rust formam um workspace do Cargo (`Cargo.toml` na raiz); `ui/` e `desktop/` formam um workspace do pnpm (`pnpm-workspace.yaml`).

## Pré-requisitos

- [Rust](https://rustup.rs/) estável (`cargo`, `rustfmt`, `clippy`).
- [Node.js](https://nodejs.org/) 22 e [pnpm](https://pnpm.io/) 10 (`corepack enable` instala a versão do `package.json`).
- Para o desktop, os [pré-requisitos do Tauri 2](https://tauri.app/start/prerequisites/) do seu sistema. No Windows: Visual Studio Build Tools (C++) e WebView2, que já vem no Windows 10/11.

## Como rodar

### Agente e mock-lcu (Rust)

```bash
cargo build                 # compila agente e mock-lcu
cargo test                  # testes
cargo fmt --all --check     # formatação
cargo clippy --all-targets -- -D warnings
cargo run -p mock-lcu       # sobe o servidor falso (issue #6)
```

Na raiz, `cargo build` e `cargo test` rodam só `agent` e `mock-lcu`; o shell Tauri fica de fora por padrão porque precisa da UI compilada e das dependências do sistema.

### UI (React)

```bash
pnpm install
pnpm --filter easylobby-ui dev     # http://localhost:5173
pnpm lint                          # eslint
pnpm test                          # vitest
pnpm build:ui                      # gera ui/dist
```

A UI nunca fala direto com a LCU, só com o canal `/ws` do agente, usando os tipos gerados em `ui/src/bindings/` (issue #15).

### Desktop (Tauri 2)

```bash
pnpm install
pnpm dev:desktop        # abre a janela apontando para a UI em modo dev
pnpm --filter easylobby-desktop build   # gera o instalador (formato do instalador é o #35)
```

`pnpm dev:desktop` roda `pnpm --filter easylobby-ui dev` sozinho antes de abrir a janela.

## Fases

| Fase | Pronto quando |
| --- | --- |
| 0 · Fundação | O agente lê o lockfile e mostra o invocador logado, no LoL real e no mock |
| 1 · Núcleo do agente e janela Tauri | O app abre o LoL, fecha a UI oficial sozinho e reabre pelo botão |
| 2 · Fila e partida | Uma normal do início ao fim sem abrir a UI oficial |
| 3 · Seleção de campeões e runas | Ranqueada solo com ban e troca de runas (MVP) |
| 4 · Social | Duo montada e chat com amigos só pelo app |
| 5 · Celular e beta | 3 a 5 amigos usam a beta por uma semana |

## Autores

- [Rafa Murata](https://github.com/rmuratah)
- [Victor Ricco](https://github.com/Victor-Ricco)
