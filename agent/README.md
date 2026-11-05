# agent

Crate Rust do agente. Acha o processo do LoL, lê o `lockfile`, conecta na LCU API (REST com certificado `riotgames.pem` fixado + WebSocket WAMP) e expõe o canal interno `/ws` tipado para a UI.

```bash
cargo test -p easylobby-agent
```


## Descoberta da LCU (`lockfile`)

`lockfile::discover()` acha os processos `LeagueClientUx`/`LeagueClient`, lê o `lockfile` (`nome:pid:porta:senha:protocolo`) na pasta do `--install-directory=` ou do executável e, se não achar, usa `--app-port=` e `--remoting-auth-token=` dos argumentos do `LeagueClientUx`. O `Debug` das credenciais esconde a senha.

Para conferir no LoL real:

```bash
cargo run -p easylobby-agent --example descobrir_lcu
```

## Cliente REST da LCU (`rest`)

`rest::LcuClient` fala com a LCU por HTTPS com Basic Auth `riot:<senha>` e o certificado `certs/riotgames.pem` fixado como única raiz confiável: as raízes do sistema ficam de fora e **a verificação TLS nunca é desligada**. Como o `riotgames.pem` é X.509 v1 e o rustls recusa, o TLS é o nativo do sistema (`native-tls`).

```rust
let credentials = lockfile::discover()?;
let lcu = LcuClient::new(&credentials)?;
let phase: String = lcu.get("/lol-gameflow/v1/gameflow-phase").await?;
lcu.post::<_, ()>("/riotclient/kill-ux", &serde_json::Value::Null).await?;
```

`get`, `post`, `put`, `patch` e `delete` convertem o corpo em qualquer tipo `Deserialize` (`()` para respostas vazias) e transformam status fora de `2xx` em `LcuError::Status`, com o `message` do erro da LCU. `request` devolve status e corpo sem tratar erro. Para o mock-lcu, use `LcuClient::with_root_certificate(&credentials, pem)` com o `mock-lcu-ca.pem` que ele grava ao lado do lockfile. Os testes em `mock-lcu/tests/cliente_rest.rs` sobem o mock e conferem GET, POST, 401, 404 e que um certificado que não bate com o fixado é recusado.

## Invocador logado (`summoner`)

`summoner::current_summoner(&lcu)` lê `/lol-summoner/v1/current-summoner` e devolve um `Summoner` com Riot ID (`gameName#tagLine`), nível, ícone, puuid e id. Antes do login a LCU responde 404.

```bash
# LoL real, aberto e logado
cargo run -p easylobby-agent --example invocador

# mock-lcu (com `cargo run -p mock-lcu` em outro terminal)
cargo run -p easylobby-agent --example invocador -- --mock
```

Com `--mock` (ou `--lockfile <arquivo>`), o example lê o lockfile do mock e fixa o `mock-lcu-ca.pem` que fica ao lado dele.

## Eventos em tempo real (`wamp`)

`wamp::LcuEvents` abre o WebSocket WAMP da LCU (`wss://127.0.0.1:<porta>/`), com o mesmo Basic Auth e o mesmo `riotgames.pem` fixado do cliente REST, e se inscreve em `OnJsonApiEvent`. Cada `next().await` devolve o próximo `LcuEvent` (`uri`, `event_type` `Create`/`Update`/`Delete` e `data`) que passa no `EventFilter`, na ordem em que a LCU mandou.

```rust
let credentials = lockfile::discover()?;
let mut events = LcuEvents::connect(&credentials, EventFilter::easylobby()).await?;
while let Some(event) = events.next().await {
    let event = event?;
    // event.uri == "/lol-gameflow/v1/gameflow-phase", event.data == "ReadyCheck"...
}
// `None`: a LCU fechou a conexão (LoL fechado). Para reconectar, veja `connection`.
```

`EventFilter::easylobby()` guarda gameflow, lobby, fila e ready check, champ select, runas, chat e o invocador logado, e tira `/lol-chat/v1/settings`. O filtro olha a `uri` antes de interpretar o `data`, então eventos grandes que não interessam custam pouco. `EventFilter::prefixes([...])` e `EventFilter::all()` servem para outros casos. Senha errada vira `WampError::Unauthorized`; LoL fechado, certificado que não bate e tempo esgotado têm `is_connect()`.

```bash
# LoL real, aberto e logado
cargo run -p easylobby-agent --example eventos

# mock-lcu (com `cargo run -p mock-lcu` em outro terminal)
cargo run -p easylobby-agent --example eventos -- --mock
```

`--todos` mostra também os eventos fora do filtro. Os testes em `mock-lcu/tests/cliente_wamp.rs` reproduzem a gravação `16.22` e conferem que todos os eventos filtrados chegam na ordem, o filtro, o modo `http`, o 401, o certificado errado e o `None` quando o mock para.

## Reconexão automática (`connection`)

`connection::LcuConnection` junta descoberta, REST e WAMP numa conexão que sobrevive ao LoL fechar e reabrir (#10). Cada `next().await` devolve um `ConnectionEvent`:

| Evento | Quando |
| --- | --- |
| `Connected(session)` | Conectou num client (o primeiro ou um que reabriu). `session.client` é o `LcuClient` para os comandos; os eventos de antes se perderam, então releia pelo REST o estado que acompanha. |
| `Event(event)` | Um `LcuEvent` que passou no filtro. |
| `Disconnected(reason)` | A conexão caiu (LoL fechou ou reiniciou). O `LcuClient` antigo não serve mais. |
| `Retrying { attempt, delay, error }` | Uma tentativa falhou (LoL fechado, lockfile velho...). A próxima sai depois de `delay`: 0,5 s, 1 s, 2 s, 4 s e depois a cada 5 s. |

```rust
let mut lcu = LcuConnection::new(ReconnectConfig::default());
loop {
    match lcu.next().await {
        ConnectionEvent::Connected(session) => { /* reler o estado */ }
        ConnectionEvent::Event(event) => { /* ... */ }
        ConnectionEvent::Disconnected(_) | ConnectionEvent::Retrying { .. } => {}
    }
}
```

`next` nunca termina por conta própria e pode ser cancelado num `tokio::select!` sem perder evento nem reiniciar a espera. Cada tentativa redescobre as credenciais (`CredentialSource::Discover`) ou relê o lockfile e a CA do mock ao lado dele (`CredentialSource::Lockfile`), porque porta, senha e certificado mudam quando o LoL reabre. `error.is_client_not_running()` separa o LoL fechado dos outros erros.

O example `eventos` usa a reconexão: deixe-o rodando e feche e reabra o LoL (ou o `cargo run -p mock-lcu`). Os testes em `mock-lcu/tests/reconexao.rs` sobem e derrubam o mock com a gravação `16.22` e conferem a espera pelo LoL, a reconexão com porta, senha e CA novas no meio do champ select, o lockfile velho e o cancelamento do `next`.

## Abrir o LoL pelo Riot Client (`launcher`)

`launcher::launch_league(&LaunchOptions::default()).await` abre o LoL quando ele não está rodando (#11) e volta sem esperar; quem espera é a `LcuConnection`, que já tenta de novo até conectar. A ordem:

1. Se já existe `LeagueClient`/`LeagueClientUx` (inclusive depois do `kill-ux`), devolve `LaunchOutcome::AlreadyRunning` e não faz nada.
2. Se o Riot Client está aberto, pede o LoL pela API local dele: `POST /product-launcher/v1/products/league_of_legends/patchlines/live`, com porta e senha do lockfile do Riot Client (`%LOCALAPPDATA%\Riot Games\Riot Client\Config\lockfile`), Basic Auth e o `riotgames.pem` fixado (`RequestedFromRiotClient`). Chamar o executável com o Riot Client já aberto só mostrava a janela dele.
3. Senão, ou se a API falhar, chama `RiotClientServices.exe --launch-product=league_of_legends --launch-patchline=live` (`Launched`, com o `api_error` quando a API falhou). Se ninguém fez login, o Riot Client mostra a tela de login e abre o LoL depois.

O `RiotClientServices.exe` é procurado nesta ordem: `LaunchOptions::riot_client`, se dado; o executável de um `RiotClientServices` já aberto; `rc_default`, `rc_live`, `associated_client` e `rc_beta` do `%ProgramData%\Riot Games\RiotClientInstalls.json`; e `C:\Riot Games\Riot Client\`. Sem nenhum, `LaunchError::RiotClientNotFound` lista onde procurou.

```bash
# Windows, com o LoL fechado
cargo run -p easylobby-agent --example abrir_lol
cargo run -p easylobby-agent --example abrir_lol -- --riot-client "D:\Riot Games\Riot Client\RiotClientServices.exe"
```

O example abre o LoL, espera conectar e mostra o invocador logado. `--patchline pbe` abre o PBE. A decisão de abrir ou não e a ordem de busca são testadas com processos e arquivos falsos (`plan_launch`/`find_riot_client`); no Linux, um teste também dispara um "Riot Client" falso e confere os argumentos. O pedido pela API (`launcher::request_launch`) é testado em `mock-lcu/tests/abrir_lol.rs`, com o mock no papel do Riot Client: ele registra os `POST /product-launcher/...` em `MockLcu::launches()`.

## Fechar a UI oficial (`ux`)

A cada `Connected`, depois de REST e WAMP abrirem, a `LcuConnection` chama `POST /riotclient/kill-ux` (#12): fecha o `LeagueClientUx` e o client continua rodando, então a LCU segue respondendo. Isso vale também quando o LoL reabre. Se o agente não consegue conectar, a UI oficial fica aberta.

O resultado vem em `session.kill_ux`: `Killed`, `Disabled` (opção desligada) ou `Failed(erro)`. Uma falha não derruba a conexão. Para desligar, use `ReconnectConfig { kill_ux: false, .. }` ou `lcu.set_kill_ux(false)`, que vale a partir da próxima conexão e não reabre a UI agora. Para fechar ou reabrir na hora (o botão de emergência), use `ux::kill_ux(&session.client)` e `ux::launch_ux(&session.client)`.

O example `eventos` fecha a UI ao conectar e a reabre com `launch-ux` ao sair com Ctrl+C; `--manter-ui` não chama o `kill-ux`. Os testes em `mock-lcu/tests/kill_ux.rs` conferem o kill-ux ao conectar, o `launch-ux`, a opção desligada, o kill-ux de novo quando o LoL reabre e a troca da opção entre conexões.

## Canal `/ws` para a UI (`ws`)

O agente expõe um WebSocket em `ws://127.0.0.1:47800/ws` (#15). A UI nunca fala com a LCU: ela recebe o estado e as mudanças por aqui e manda comandos por aqui. As mensagens são JSON, definidas em `ws::protocol`, e os tipos TypeScript saem desses tipos com `ts-rs` em `ui/src/bindings/`.

Do agente para a UI (`ServerMessage`, campo `type`):

| Mensagem | Quando |
| --- | --- |
| `hello` | Ao conectar, com `protocol`, `agentVersion` e o estado inteiro (`state`: `lcu`, `summoner`, `gameflowPhase`). Volta a vir se a UI ficar para trás ou pedir `getState`: troque todo o estado pelo dele. |
| `lcu` | A conexão com o LoL mudou: `starting`, `connected` (com `session`), `disconnected` (com `reason`) ou `retrying` (com `attempt`, `retryInMs`, `clientNotRunning` e `error`). |
| `summoner` | O invocador logado mudou; `null` com o LoL fechado ou antes do login. |
| `gameflowPhase` | A fase mudou (`None`, `Lobby`, `Matchmaking`, `ReadyCheck`, `ChampSelect`, `InProgress`...); `null` com o LoL fechado. |
| `response` | Resposta a um comando, com o mesmo `id` e `result` `ok` ou `error` (`code` e `message`). |
| `invalidMessage` | A UI mandou algo que não é um `ClientMessage` válido. |

Da UI para o agente (`ClientMessage`): `{"id": 1, "command": {"type": "ping"}}`. Comandos por enquanto: `ping` e `getState`. Os comandos das próximas issues (como `launch-ux`, #13) entram em `ws::protocol::Command`.

`ws::run(lcu, listener, config)` liga uma `LcuConnection` ao canal. Por partes: `Hub` guarda o estado, `ws::drive` o atualiza a partir da conexão (relendo invocador e fase pelo REST a cada `Connected` e limpando tudo no `Disconnected`) e `ws::serve` serve o `/ws`. O servidor recusa com 403 pedidos com `Origin` fora de `WsConfig::allowed_origins` (padrão: a janela do Tauri e o Vite em `localhost:5173`), para uma página aberta no navegador não conseguir falar com o agente.

Para desenvolver a UI contra o mock-lcu:

```bash
cargo run -p mock-lcu                                    # terminal 1
cargo run -p easylobby-agent --example canal_ws -- --mock  # terminal 2
pnpm --filter easylobby-ui dev                           # terminal 3
```

O example mostra no terminal cada mensagem que as UIs recebem. Sem `--mock`, ele acha o LoL real.

Depois de mudar um tipo de `ws::protocol` (ou o `Summoner`), regenere os tipos TypeScript e comite `ui/src/bindings/`:

```bash
EASYLOBBY_UPDATE_BINDINGS=1 cargo test -p easylobby-agent bindings
```

O teste `bindings_estao_atualizados` falha no CI se os arquivos estiverem desatualizados. Os testes em `mock-lcu/tests/canal_ws.rs` sobem o mock com a gravação `16.22` e o agente, e conferem o `hello`, a partida inteira pelas mudanças de fase, o invocador, os comandos, o LoL fechando e reabrindo e o 403 para outras origens.

## Fixtures da LCU

Respostas REST e eventos WAMP gravados de sessões reais ficam em `tests/fixtures/<patch>/` e alimentam o `mock-lcu` (#6). Cada pasta tem:

| Arquivo | Conteúdo |
| --- | --- |
| `manifest.json` | Patch, versão do jogo, data da gravação e `synthetic` (escrito à mão ou gravado) |
| `rest.json` | Respostas de `GET`, cada uma com a fase do gameflow em que foi tirada (`None`, `Lobby`, `Matchmaking`, `ReadyCheck`, `ChampSelect`...) |
| `events.jsonl` | Eventos de `OnJsonApiEvent`, um por linha: `tMs` desde o início, `uri`, `eventType` e `data` |

O módulo `fixtures` lê e grava esse formato (`FixtureSet::load`/`save`). A pasta `exemplo/` é um conjunto **sintético**, escrito à mão, para desenvolver antes da primeira gravação real: normal às cegas em duo, mensagem no chat, fila, ready check e champ select até travar o campeão.

Para gravar uma sessão real, com o LoL aberto e logado:

```bash
cargo run -p easylobby-agent --example gravar_fixtures
```

Jogue a sessão pela UI oficial (lobby, fila, ready check, champ select, chat) e aperte Ctrl+C. O gravador tira uma foto dos endpoints no início e a cada mudança de fase, grava os eventos dos tópicos usados pelo easylobby e salva em `tests/fixtures/<patch>/` (`--patch`, `--saida` e `--segundos` mudam isso). A conexão usa o certificado `certs/riotgames.pem` fixado, nunca desliga a verificação TLS.

Antes de salvar, o `Sanitizer` troca puuid, ids (inclusive o `gameId` das partidas), nomes, tags e nomes de grupos de amigos por valores `fake-...` (o mesmo valor real vira sempre o mesmo falso, então as relações continuam valendo), troca o texto das mensagens de chat e apaga tokens, senhas e a senha do lockfile. Para passar uma gravação antiga pelo sanitizer atual, use `--ressanitizar tests/fixtures/<patch>`. O teste `fixtures_validas` falha se algum conjunto tiver dado que o sanitizer trocaria ou texto com cara de id ou token real. Mesmo assim, **confira os arquivos antes de commitar.**
