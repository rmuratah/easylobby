# Como contribuir com o easylobby

Este guia vale para todo mundo que mexe no código, inclusive assistentes de IA (Claude, ChatGPT etc.). Leia antes de abrir a primeira branch.

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
- Uma branch por issue.

| Tipo | Quando usar | Exemplo |
| --- | --- | --- |
| `feat` | Funcionalidade nova | `feat/17-criar-lobby` |
| `fix` | Correção de bug | `fix/10-reconexao` |
| `docs` | Só documentação | `docs/2-protocolo-de-branches` |
| `chore` | Configuração, CI, dependências | `chore/3-ci-github-actions` |
| `refactor` | Mudança interna sem alterar comportamento | `refactor/22-estado-champ-select` |
| `test` | Só testes ou fixtures | `test/5-fixtures-lcu` |

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
