# Decisões de arquitetura e produto

Este documento registra as decisões acordadas do easylobby. Elas orientam a implementação; não significam que todas as funcionalidades já estejam prontas. Consulte as issues e seus milestones para acompanhar a entrega. Mudanças nestas decisões precisam ser discutidas com os dois desenvolvedores antes da implementação.

## Arquitetura

- **Tauri 2** é o shell desktop, com janela e ícone na bandeja. Ele embute o agente Rust.
- **Rust** concentra a integração com o LoL: descoberta do processo, leitura do lockfile, REST e eventos WebSocket WAMP da LCU.
- **React + TypeScript**, com Vite e Tailwind, compõem a interface.
- A UI se comunica somente com o canal interno `/ws` do agente. Os tipos TypeScript são gerados a partir do Rust com `ts-rs` ou `specta`; a escolha entre essas ferramentas ainda não está definida neste documento. A UI não acessa a LCU diretamente nem inventa formatos de mensagens.
- O **mock-lcu** permite desenvolver sem abrir o LoL, usando fixtures de sessões reais em `agent/tests/fixtures/<patch>/`.

## Interface oficial e recuperação

O agente usa `POST /riotclient/kill-ux` para fechar a interface oficial e reduzir o consumo de memória, mantendo o client em execução. Esse comportamento terá opção para desligar. Um botão de emergência usa `POST /riotclient/launch-ux` para reabrir a interface oficial.

A medição inicial foi de cerca de **885 MB de RAM economizados** ao fechar a UI oficial; o valor não é uma garantia para todas as máquinas ou versões do client.

## Segurança da conexão

A LCU é um servidor local autenticado: porta e senha são obtidas do lockfile. O agente usa Basic Auth e o certificado `riotgames.pem` fixado. **Nunca desative a verificação TLS.** Não publique senhas, conteúdo de lockfiles ou cabeçalhos de autenticação em issues, logs compartilhados ou fixtures.

## Escopo do produto

- O app substitui a interface do client para lobby, fila e seleção de campeões; não automatiza jogadas nem oferece vantagem dentro da partida.
- **Não haverá loja:** compras de skins ou itens ficam fora do app.
- O MVP termina na fase 3: ranqueada solo com ban e troca de runas sem a interface oficial.
- O aceite automático de partida é opcional e está **fora do MVP**, no [backlog #39](https://github.com/rmuratah/easylobby/issues/39). Isso não exclui o aceite ou a recusa manual da [issue #20](https://github.com/rmuratah/easylobby/issues/20).
- Antes da beta, é necessário registrar o app no **Riot Developer Portal** e revisar a política da Riot, conforme a [issue #37](https://github.com/rmuratah/easylobby/issues/37). Este documento não representa aprovação da Riot.

## Redesign (Fase 5)

O redesign dá ao app um visual próprio em todas as telas, sem mudar o agente. A referência visual é o protótipo aprovado na [issue #122](https://github.com/rmuratah/easylobby/issues/122).

- **Estilo:** claro e minimalista, com identidade própria. O app não imita o client oficial: nada de molduras douradas, da fonte Beaufort ou de assets da interface da Riot. Dados e imagens do jogo (ícones de campeão, runa e feitiço vindos do client) podem ser usados.
- **Temas:** claro e escuro, feitos com tokens de cor (variáveis CSS). Por padrão o app segue o tema do Windows; o usuário escolhe Sistema, Claro ou Escuro, e a escolha fica salva no app.
- **Cor de destaque:** teal da paleta fornecida por Victor (`#008498` no claro, `#21ABB8` no escuro). Os valores originais ficam em `ui/src/theme/themeColors.ts`; papéis semânticos ajustam contraste quando necessário.
- **Fonte:** Figtree, embutida no build. Nada é carregado da internet.
- **Estrutura da janela:** barra de título própria, sem a moldura do Windows, com minimizar, fechar, "Abrir client oficial" e o seletor de tema. A área principal muda sozinha conforme o estado do jogo; amigos, convites e chat ficam numa coluna lateral recolhível.
- **Lobby unificado:** fila e partida encontrada aparecem dentro da tela do lobby. Ao aceitar a partida, a seleção de campeões ocupa a tela.
- **Escopo:** visual e melhorias de UX só na UI (atalhos, busca de campeão, estados vazios, de carregamento e de erro). O agente e o contrato do `/ws` não mudam nesta fase; o que precisar do agente vai para a Fase 6.
- **Limites:** a janela abre em 1100×700 e o mínimo é 960×640; contraste WCAG AA nos dois temas; a RAM no início, no lobby e na seleção pode subir no máximo 10% sobre a referência medida na [issue #121](https://github.com/rmuratah/easylobby/issues/121).

### Como o redesign convive com o trabalho em andamento

O redesign é feito por rmuratah, em `ui/` e `desktop/`, que são pastas do Victor-Ricco. Combinado entre os dois: o Victor termina e mergeia a parte funcional de cada tela, e o redesign daquela tela só começa depois disso. O Victor revisa todos os PRs da fase. As mudanças entram aos poucos na `main`, um PR por issue: base visual, moldura e depois uma tela por vez.

## Responsabilidades e contribuição

Rafa Murata (`rmuratah`) cuida do agente e do mock-lcu. Victor Ricco (`Victor-Ricco`) cuida da UI e do desktop. Documentação e CI são compartilhados; os responsáveis de cada tarefa estão em **Assignees** no GitHub.

Leia [AGENTS.md](../AGENTS.md) para a arquitetura, fases e limites de atuação, e [CONTRIBUTING.md](../CONTRIBUTING.md) para atribuição de issues, branches, contratos compartilhados, revisão cruzada e merge por squash.
