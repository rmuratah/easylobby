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
