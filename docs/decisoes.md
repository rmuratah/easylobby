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

## Responsabilidades e contribuição

Rafa Murata (`rmuratah`) cuida do agente e do mock-lcu. Victor Ricco (`Victor-Ricco`) cuida da UI e do desktop. Documentação e CI são compartilhados; os responsáveis de cada tarefa estão em **Assignees** no GitHub.

Leia [AGENTS.md](../AGENTS.md) para a arquitetura, fases e limites de atuação, e [CONTRIBUTING.md](../CONTRIBUTING.md) para atribuição de issues, branches, contratos compartilhados, revisão cruzada e merge por squash.
