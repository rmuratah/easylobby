# easylobby

Client leve para o League of Legends. Fecha a interface oficial (LeagueClientUx) e oferece uma interface própria e leve para lobby, fila e seleção de campeões, conversando direto com a LCU API. Medimos cerca de **885 MB de RAM economizados** fechando a UI oficial.

Leia [AGENTS.md](AGENTS.md) para a visão geral da arquitetura e das decisões, e [CONTRIBUTING.md](CONTRIBUTING.md) para o fluxo de branches, commits e PRs.

Consulte também o [registro de decisões](docs/decisoes.md) para os limites do produto e as escolhas de arquitetura.

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

### Aba "Em jogo" do perfil (API da Riot)

A partida em andamento de outro jogador vem da API oficial da Riot (`spectator-v5`, #436), que precisa de chave. A chave **nunca** entra no repositório nem no app distribuído. Para testar, gere uma chave de desenvolvimento no [Riot Developer Portal](https://developer.riotgames.com/) (ela expira em 24 horas) e use uma destas opções:

- a variável `EASYLOBBY_RIOT_API_KEY` no terminal que abre o app; ou
- o arquivo `%APPDATA%\easylobby\riot-api-key.txt` (no Linux/macOS, `~/.config/easylobby/riot-api-key.txt`), só com a chave.

`EASYLOBBY_RIOT_API_URL` troca o endereço da API (com `{platform}` no lugar de `br1`), para o servidor com a chave de produto antes do alfa fechado (#37). Sem chave e sem servidor, a aba não aparece.

## Etapas

| Etapa | Pronto quando |
| --- | --- |
| Alfa fechado | O fluxo de jogar inteiro funciona sem a UI oficial (modo e lobby, fila, seleção de campeões nos 6 fluxos, partida, pós-jogo com honra e Jogar de novo), testado no LoL real, com instalador e registro na Riot; 3 a 5 amigos usam o app |
| Beta | Todas as funções escolhidas para o app estão prontas: perfil completo, estatísticas no estilo op.gg, detalhe da partida e paridade com o client oficial |
| Depois da beta | Sem data: celular, aceite automático, Clash, chat de voz e outros extras |

As Fases 0 a 8 do plano original foram substituídas por essas etapas em novembro de 2026 (#472); veja o [registro de decisões](docs/decisoes.md).

## Autores

- [Rafa Murata](https://github.com/rmuratah)
- [Victor Ricco](https://github.com/Victor-Ricco)
