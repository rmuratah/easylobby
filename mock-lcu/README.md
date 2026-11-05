# mock-lcu

Servidor falso da LCU para desenvolver e testar sem abrir o LoL (#6). Responde aos `GET` com as respostas gravadas em `agent/tests/fixtures/<patch>/` (#5) e reproduz os eventos WAMP no ritmo da gravação.

```bash
cargo run -p mock-lcu                                   # gravação real mais nova, HTTPS
cargo run -p mock-lcu -- --fixtures exemplo --velocidade 10 --repetir
cargo run -p mock-lcu -- --http --porta 2999 --verbose
```

Ao subir, ele mostra a porta, o lockfile e o certificado da CA. Pare com Ctrl+C; o lockfile e o certificado são apagados.

## O que imita da LCU

- Escuta em `127.0.0.1`, numa porta livre (ou `--porta`), com REST e WebSocket na mesma porta.
- Basic Auth `riot:<senha>`. Sem a senha certa, 401. A senha é aleatória a cada execução (ou `--senha`).
- Escreve um **lockfile** no formato do LoL, `LeagueClient:<pid>:<porta>:<senha>:<protocolo>`, em `<temp>/easylobby-mock-lcu/lockfile` (ou `--lockfile`). O agente lê com `Credentials::from_lockfile_path` (#4).
- **HTTPS** com um certificado para `127.0.0.1` gerado na hora e assinado por uma CA também gerada na hora. O PEM da CA fica em `mock-lcu-ca.pem`, ao lado do lockfile. O cliente fixa esse PEM no lugar do `riotgames.pem`, do mesmo jeito; **a verificação TLS nunca é desligada**. Com `--http` o servidor fala HTTP puro e o lockfile diz `http`.
- `POST /riotclient/kill-ux` e `POST /riotclient/launch-ux` respondem 204 (e só registram o pedido).
- `POST /lol-matchmaking/v1/ready-check/accept` e `.../decline` respondem 204 com um ready check `InProgress` aberto, trocam o `playerResponse` para `Accepted` ou `Declined` e mandam o evento `Update` pelo WAMP, como a LCU. Sem ready check aberto, 404. A reprodução segue com os eventos gravados.
- `POST /lol-lobby/v2/lobby` com `{"queueId": N}` cria o lobby (ou troca a fila do lobby atual) a partir do lobby gravado, e `DELETE /lol-lobby/v2/lobby` sai dele (#17). Os dois mandam pelo WAMP os eventos que a LCU mandaria: a fase vai para `Lobby` (ou volta para `None`) e o lobby é criado, atualizado ou apagado. Fila que não está em `/lol-game-queues/v1/queues` recebe 400.
- `POST /lol-lobby/v2/lobby/matchmaking/search` entra na fila com o lobby atual e `DELETE` no mesmo caminho sai dela (#19). Entrar muda a fase para `Matchmaking` e cria a busca em `/lol-matchmaking/v1/search` a partir da gravada; enquanto procura, o `timeInQueue` sobe um segundo por segundo (acelerado por `--velocidade`); sair apaga a busca e volta a fase para `Lobby`. Fora de lobby, o `POST` recebe 400. A partida nunca é encontrada: o resto do caminho vem da reprodução da gravação.
- `PUT /lol-lobby/v2/lobby/members/localMember/position-preferences` com `{"firstPreference": "MIDDLE", "secondPreference": "JUNGLE"}` troca os cargos do jogador no lobby e manda o lobby atualizado pelo WAMP (#18). Fora de lobby recebe 404; em fila sem escolha de cargos (ARAM, Jogo Dinâmico) ou com cargo inválido, 400.
- `PATCH /lol-champ-select/v1/session/actions/{id}` com `{"championId": N}` marca o campeão na ação e `POST .../actions/{id}/complete` confirma a ação em andamento (#23). Os dois mudam a sessão atual e mandam o `Update` dela pelo WAMP. Num pick do jogador fora da vez, o `PATCH` vira a intenção (`championPickIntent`). Fora da seleção ou com ação desconhecida, 404; ação já confirmada, fora da vez (no `complete`) ou sem campeão, 400. O draft não anda sozinho depois: a vez seguinte vem da reprodução da gravação.
- WebSocket WAMP em `/` (subprotocolo `wamp`): `[5, "OnJsonApiEvent"]` inscreve em tudo, `[5, "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase"]` num recurso e nos sub-recursos dele, `[6, tópico]` cancela. Os eventos chegam como `[8, tópico, {"data", "eventType", "uri"}]`. Quando o servidor para, os WebSockets abertos caem sem handshake de fechamento, como quando o LoL fecha.

## Como as respostas mudam

- O mock começa na fase do gameflow da primeira resposta gravada (normalmente `None`).
- A reprodução começa na **primeira inscrição WAMP**, para o cliente ler o estado inicial e não perder evento nenhum (`--iniciar-ja` começa na hora; `--parado` nunca começa). `--velocidade 10` reproduz 10 vezes mais rápido; `--repetir` recomeça no fim, mandando um evento de volta para a fase inicial.
- Um `GET` devolve a resposta gravada na fase atual ou, se não houver, na fase visitada mais recente que tenha uma.
- Cada evento muda o recurso: `Create`/`Update` viram a resposta do `GET` (200 com o `data`), `Delete` faz o recurso responder 404. Quando a fase muda, as respostas gravadas na nova fase voltam a valer para os recursos que elas cobrem.
- Caminho sem resposta gravada nem evento: 404 no formato de erro da LCU. Outros métodos (`POST`, `PUT`, `PATCH`, `DELETE`) fora das rotas acima: 501, porque as fixtures só têm `GET`.

## Usar nos testes

O crate também é uma biblioteca: `MockLcu::start(Config::new(fixtures))` sobe o servidor dentro do teste, numa porta livre. Veja `tests/servidor.rs`, que conecta como o agente vai conectar (lockfile, TLS nativo com o certificado fixado, Basic Auth e WAMP).

```bash
cargo test -p mock-lcu
```
