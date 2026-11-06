# Como contribuir com o easylobby

Este guia vale para todo mundo que mexe no código, inclusive assistentes de IA (Claude, ChatGPT etc.). Leia antes de abrir a primeira branch.

Antes de começar, leia [AGENTS.md](AGENTS.md) e [docs/decisoes.md](docs/decisoes.md). Ao abrir uma issue, escolha o template de tarefa ou de bug e preencha escopo, critérios de conclusão e validação. Defina os responsáveis em **Assignees**, as labels e o milestone conforme a tarefa.

## Fluxo de trabalho

1. **Escolha uma issue.** Todo trabalho começa numa issue. Atribua a issue a você antes de começar, para os dois não pegarem a mesma tarefa. Se a tarefa não tem issue, crie uma.
2. **Crie a branch a partir da `main` atualizada.**
   ```bash
   git checkout main
   git pull origin main
   git checkout -b feat/17-criar-lobby
   ```
3. **Faça commits pequenos** seguindo o padrão abaixo.
4. **Abra um pull request** para a `main` assim que tiver algo para mostrar (pode ser rascunho).
5. **Revisão cruzada:** o outro dev revisa e aprova. O CI precisa estar verde.
6. **Merge por squash** e a branch é apagada.

## Branch principal

- A `main` é protegida: ninguém faz push direto nela.
- Toda mudança entra por pull request, com CI verde e aprovação do outro dev.

## Nome da branch

Formato: `tipo/numero-da-issue-descricao-curta`

- Tudo em minúsculas, palavras separadas por hífen, sem acento e sem espaço.
- Uma branch por issue, com uma exceção: issues divididas (veja abaixo).

| Tipo | Quando usar | Exemplo |
| --- | --- | --- |
| `feat` | Funcionalidade nova | `feat/17-criar-lobby` |
| `fix` | Correção de bug | `fix/10-reconexao` |
| `docs` | Só documentação | `docs/2-protocolo-de-branches` |
| `chore` | Configuração, CI, dependências | `chore/3-ci-github-actions` |
| `refactor` | Mudança interna sem alterar comportamento | `refactor/22-estado-champ-select` |
| `test` | Só testes ou fixtures | `test/5-fixtures-lcu` |

## Issues divididas (label `agente,ui`)

Quando uma issue tem parte no agente e parte na UI, os dois devs ficam atribuídos e ela vira dois PRs:

1. A parte do agente vai numa branch com sufixo `-agente`, por exemplo `feat/17-criar-lobby-agente`: o comando, o evento e os tipos TypeScript gerados. Isso é o contrato.
2. A parte da UI vai numa branch com sufixo `-ui`, por exemplo `feat/17-criar-lobby-ui`. Ela pode começar assim que o PR do agente estiver aberto, programando contra os tipos gerados e testando no mock-lcu.
3. O PR do agente entra primeiro e usa `Refs #17`; o PR da UI usa `Closes #17`.

## Mensagens de commit

Seguimos [Conventional Commits](https://www.conventionalcommits.org/pt-br/v1.0.0/):

```
tipo(escopo): descrição curta no imperativo (#issue)
```

- `tipo`: os mesmos da tabela de branches.
- `escopo` (opcional): `agente`, `ui`, `desktop`, `mock-lcu`, `ci`, `docs`.
- Exemplos:
  - `feat(agente): ler lockfile do LoL (#4)`
  - `fix(ui): corrigir timer da fila (#19)`
  - `chore(ci): rodar clippy nos PRs (#3)`

## Pull requests

- Título no mesmo formato do commit, por exemplo `feat(agente): ler lockfile do LoL (#4)`.
- Na descrição, `Closes #N` para fechar a issue automaticamente no merge.
- Preencha o template: o que mudou, como testar e o que ficou de fora.
- Merge sempre por **squash**; o título do PR vira a mensagem do commit na `main`.
- Depois do merge, a branch é apagada.

## Evitando conflitos entre PRs

Os arquivos do canal `/ws` são os que mais conflitam, porque quase toda feature mexe neles. Eles estão divididos por assunto (conexão, lobby e fila, seleção de campeões, runas, social) para que PRs de assuntos diferentes não toquem nas mesmas linhas:

- **Agente:** cada assunto do `Hub` tem o seu arquivo em `agent/src/ws/hub/`, com os próprios testes. Comandos, mensagens e campos do estado ficam em blocos por assunto em `ws/protocol.rs` e `ws/commands.rs`; algo novo entra no fim do bloco do seu assunto, não no fim do enum.
- **Tipos gerados:** `ui/src/bindings/` tem uma variante ou um campo por linha. Num conflito nesses arquivos, resolva o Rust e rode `EASYLOBBY_UPDATE_BINDINGS=1 cargo test -p easylobby-agent bindings` de novo em vez de editar à mão.
- **Testes da UI:** monte o estado do agente com `makeState({ ... })` de `ui/src/test/state.ts`, passando só os campos que o teste usa. Campo novo no `AgentState` muda só o `emptyState`.
- **Ordem:** no máximo um PR aberto por vez mexendo no protocolo do agente, e merge logo que estiver aprovado.

## Revisão de PRs

A revisão serve para achar problemas que impedem o merge, não para deixar o código perfeito. Para não virar um ciclo sem fim:

- **Só bloqueia o merge:** bug, falha de segurança ou quebra do contrato do `/ws` (tipos, eventos e comandos).
- **Não bloqueia:** melhorias, refatoração e estilo. Viram issue com a label `melhoria` e ficam para a rodada de melhorias depois que o app estiver completo, junto com o UAT da #62.
- **Uma rodada por PR:** quem revisa junta todos os pontos numa revisão só. A segunda rodada só confere se aqueles pontos foram resolvidos, sem procurar itens novos.
- **Revisão com IA:** quem usa uma IA (ChatGPT, Claude etc.) filtra os pontos antes de pedir, em vez de colar a resposta inteira, e separa o que é bloqueante do que é `melhoria`.
- **Aprovação com ressalvas:** liste as ressalvas no comentário da aprovação ou abra as issues `melhoria` correspondentes.

## Verificações automáticas

O workflow `CI` roda em cada PR e nos pushes para `main`. Acompanhe os resultados na aba **Checks** do PR ou em **Actions**:

- **Rust (Windows)**: Rust estável, `cargo +stable fmt --all --check`, `cargo +stable clippy --locked --all-targets -- -D warnings` e `cargo +stable test --locked`. A formatação cobre o workspace; Clippy e testes usam os membros padrão (`agent` e `mock-lcu`).
- **UI**: Node.js 22 e a versão do pnpm definida em `package.json`, instalação com `pnpm install --frozen-lockfile`, `pnpm lint` e `pnpm test` (Vitest sem modo watch).

Esses comandos também podem ser executados na raiz do repositório antes de abrir o PR. O CI não executa o LoL real nem compila o instalador Tauri. Informe no PR os testes manuais pertinentes à entrega.

Uma falha deve ser investigada nos logs do passo correspondente antes do merge. Para tornar os checks obrigatórios, um administrador deve selecionar **Rust (Windows)** e **UI** na regra de proteção da `main`; o arquivo do workflow não altera essa configuração.
