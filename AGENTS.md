# AGENTS.md

Instruções para assistentes de IA (Codex, ChatGPT, Claude e outros) que trabalham neste repositório. Leia tudo antes de propor ou escrever código. Responda em português.

## O projeto

O **easylobby** é um client leve para o League of Legends. O client oficial (LeagueClientUx, em Chromium/CEF) consome muita memória só para mostrar lobby, fila e seleção de campeões. O easylobby fecha a interface oficial e oferece uma interface própria e leve para as mesmas tarefas. Medimos cerca de **885 MB de RAM economizados** fechando a UI oficial.

## Arquitetura

```
LoL (LCU API: REST + WAMP)  <-->  agente Rust  <-->  /ws tipado  <-->  UI React (dentro do Tauri 2)
                                     ^
                              mock-lcu (fixtures) no lugar do LoL durante o desenvolvimento
```

1. O LoL roda um servidor local, a **LCU API** (REST + WebSocket WAMP), protegido por senha. Porta e senha ficam no `lockfile` da pasta de instalação do LoL.
2. O **agente** (Rust) acha o processo do LoL, lê o lockfile e conecta na LCU: REST com `reqwest`, Basic Auth e o certificado `riotgames.pem` fixado (**nunca** desligar a verificação TLS); WebSocket WAMP inscrito em `OnJsonApiEvent`, filtrando só os tópicos usados.
3. Ao conectar, o agente chama `POST /riotclient/kill-ux` (fecha a UI oficial, mantém o client rodando). O botão de emergência chama `POST /riotclient/launch-ux`.
4. O agente expõe um canal interno `/ws` (`axum`) com eventos e comandos tipados. Os tipos TypeScript são **gerados a partir do Rust** (`ts-rs` ou `specta`). A UI nunca fala direto com a LCU, só com o `/ws`.
5. O app desktop é **Tauri 2**: embute o crate do agente e mostra a UI React numa janela com ícone na bandeja.
6. O **mock-lcu** é um servidor falso que responde com respostas gravadas de uma sessão real (`agent/tests/fixtures/<patch>/`) e reproduz os eventos, para desenvolver sem abrir o LoL.

## Monorepo

| Pasta | Conteúdo | Dono |
| --- | --- | --- |
| `agent/` | Crate Rust do agente | rmuratah |
| `mock-lcu/` | Servidor falso da LCU | rmuratah |
| `ui/` | React + TypeScript + Vite + Tailwind (testes com Vitest) | Victor-Ricco |
| `desktop/` | Shell Tauri 2 | Victor-Ricco |
| `.github/`, `docs/` | CI e documentação | os dois |

**Não altere pastas do outro dev sem combinar.** Se a UI precisar de algo novo do agente (evento, comando, campo), peça num comentário da issue em vez de implementar no agente. Na UI, use só os tipos gerados; não invente formato de mensagem do `/ws`.

## Stack

- Agente: Rust, `tokio`, `reqwest`, WebSocket WAMP, `axum`, `ts-rs`/`specta`.
- Desktop: Tauri 2.
- UI: React, TypeScript, Vite, Tailwind, Vitest.
- CI (GitHub Actions): `cargo fmt --check`, `clippy` e `cargo test` em runner Windows; lint + Vitest na UI.

## Decisões já tomadas

Não reabra sem conversar com os dois devs. Consulte também o [registro de decisões](docs/decisoes.md).

- Tauri 2, núcleo em Rust, front em React + TypeScript.
- Fechar a UI oficial com `kill-ux` (com opção para desligar) e reabrir com `launch-ux`.
- **Sem loja**: nada de comprar skins ou itens pelo app.
- **Aceite automático de partida fora do MVP** (issue #39, backlog).
- Nada que automatize jogadas ou dê vantagem dentro da partida; o app só substitui a interface do client.
- O app será publicado: antes da beta, registro no **Riot Developer Portal** e revisão da política da Riot (issue #37).

## Fases (milestones)

| Fase | Pronto quando |
| --- | --- |
| 0 · Fundação | O agente lê o lockfile e mostra o invocador logado, no LoL real e no mock |
| 1 · Núcleo do agente e janela Tauri | O app abre o LoL, fecha a UI oficial sozinho e reabre pelo botão; fechar e reabrir o LoL não derruba o agente |
| 2 · Fila e partida | Uma normal do início ao fim sem abrir a UI oficial |
| 3 · Seleção de campeões e runas | Ranqueada solo com ban e troca de runas sem a UI oficial (MVP pronto) |
| 4 · Social | Duo montada e chat com amigos só pelo app |
| 5 · Redesign | Todas as telas do app seguem o novo visual |
| 6 · Refinamento e testes | Todas as telas testadas no LoL real, sem bug aberto de criticidade média ou alta |
| 7 · Celular e beta | 3 a 5 amigos usam a beta por uma semana |

## Divisão de tarefas

Quem faz cada issue está no campo **Assignees** dela no GitHub. A regra geral:

- Label só `agente`: rmuratah.
- Label só `ui` ou `infra`: Victor-Ricco.
- Label `agente,ui` (os dois atribuídos): rmuratah faz o comando e os tipos no agente; Victor-Ricco faz a tela. Veja "Issues divididas" no [CONTRIBUTING.md](CONTRIBUTING.md).

Na Fase 0, o #1 (esqueleto do monorepo) sai primeiro, porque quase tudo depende dele. Enquanto isso, o #2 (docs) não depende de código.

## Como trabalhar

Siga o [CONTRIBUTING.md](CONTRIBUTING.md). Em resumo:

1. Comece por uma issue atribuída ao dev com quem você está trabalhando. Pergunte qual é, se não souber.
2. Branch a partir da `main` atualizada: `tipo/numero-da-issue-descricao-curta` (`feat`, `fix`, `docs`, `chore`, `refactor`, `test`).
3. Commits em Conventional Commits: `tipo(escopo): descrição no imperativo (#issue)`, escopos `agente`, `ui`, `desktop`, `mock-lcu`, `ci`, `docs`.
4. PR para a `main` com título no mesmo formato, `Closes #N` e o template preenchido. CI verde, aprovação do outro dev, merge por squash.
5. Mudanças pequenas, um PR por issue (ou por metade de uma issue dividida), com testes quando fizer sentido.
6. Na revisão, só bug, falha de segurança ou quebra do contrato do `/ws` bloqueiam o merge. Melhorias, refatoração e estilo viram issue com a label `melhoria`. Uma rodada de revisão por PR; a segunda só confere os pontos da primeira. Veja "Revisão de PRs" no [CONTRIBUTING.md](CONTRIBUTING.md).
7. Se faltar contexto, peça o arquivo ou a issue em vez de supor.
