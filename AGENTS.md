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

| Pasta | Conteúdo |
| --- | --- |
| `agent/` | Crate Rust do agente |
| `mock-lcu/` | Servidor falso da LCU |
| `ui/` | React + TypeScript + Vite + Tailwind (testes com Vitest) |
| `desktop/` | Shell Tauri 2 |
| `.github/`, `docs/` | CI e documentação |

Os dois devs mexem em todas as pastas. Quando a UI precisar de algo novo do agente (evento, comando, campo), faça a mudança no agente e gere os tipos de novo. Na UI, use só os tipos gerados; não invente formato de mensagem do `/ws`.

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
- **Aceite automático de partida fora do alfa e da beta** (issue #39, Depois da beta).
- Nada que automatize jogadas ou dê vantagem dentro da partida; o app só substitui a interface do client.
- O app será publicado: antes do alfa fechado, registro no **Riot Developer Portal** e revisão da política da Riot (issue #37).

## Etapas (milestones)

| Etapa | Pronto quando |
| --- | --- |
| Alfa fechado | O fluxo de jogar inteiro funciona sem a UI oficial (modo e lobby, fila, seleção de campeões nos 6 fluxos, partida, pós-jogo com honra e Jogar de novo), testado no LoL real, com instalador e registro na Riot; 3 a 5 amigos usam o app |
| Beta | Todas as funções escolhidas para o app estão prontas: perfil completo, estatísticas no estilo op.gg, detalhe da partida e paridade com o client oficial |
| Depois da beta | Sem data: celular, aceite automático, Clash, chat de voz e outros extras |

As Fases 0 a 8 do plano original foram substituídas por essas etapas em novembro de 2026 (#472); veja o [registro de decisões](docs/decisoes.md).

## Quem faz cada tarefa

Não há divisão de trabalho por pasta nem por label: rmuratah e Victor-Ricco fazem UI e agente. Quem faz cada issue está no campo **Assignees** dela no GitHub; quem pega a issue faz todas as partes dela (agente, tipos gerados, mock-lcu e tela). As labels `agente`, `ui` e `infra` dizem só onde a issue mexe.

## Como trabalhar

Siga o [CONTRIBUTING.md](CONTRIBUTING.md). Em resumo:

1. Comece por uma issue atribuída ao dev com quem você está trabalhando. Pergunte qual é, se não souber. A issue pode envolver agente, UI ou os dois.
2. Branch a partir da `main` atualizada: `tipo/numero-da-issue-descricao-curta` (`feat`, `fix`, `docs`, `chore`, `refactor`, `test`).
3. Commits em Conventional Commits: `tipo(escopo): descrição no imperativo (#issue)`, escopos `agente`, `ui`, `desktop`, `mock-lcu`, `ci`, `docs`.
4. PR para a `main` com título no mesmo formato, `Closes #N` e o template preenchido. CI verde, aprovação do outro dev, merge por squash.
5. Mudanças pequenas, um PR por issue, com testes quando fizer sentido. Uma issue grande pode virar mais de um PR (veja "Issues com agente e UI" no [CONTRIBUTING.md](CONTRIBUTING.md)).
6. Na revisão, só bug, falha de segurança ou quebra do contrato do `/ws` bloqueiam o merge. Melhorias, refatoração e estilo viram issue com a label `melhoria`. Uma rodada de revisão por PR; a segunda só confere os pontos da primeira. Veja "Revisão de PRs" no [CONTRIBUTING.md](CONTRIBUTING.md).
7. Se faltar contexto, peça o arquivo ou a issue em vez de supor.
