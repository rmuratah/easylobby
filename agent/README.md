# agent

Crate Rust do agente. Acha o processo do LoL, lê o `lockfile`, conecta na LCU API (REST com certificado `riotgames.pem` fixado + WebSocket WAMP) e expõe o canal interno `/ws` tipado para a UI.

```bash
cargo test -p easylobby-agent
```

Fixtures gravadas de sessões reais ficam em `tests/fixtures/<patch>/` (issue #5) e alimentam o `mock-lcu`.

## Descoberta da LCU (`lockfile`)

`lockfile::discover()` acha os processos `LeagueClientUx`/`LeagueClient`, lê o `lockfile` (`nome:pid:porta:senha:protocolo`) na pasta do `--install-directory=` ou do executável e, se não achar, usa `--app-port=` e `--remoting-auth-token=` dos argumentos do `LeagueClientUx`. O `Debug` das credenciais esconde a senha.

Para conferir no LoL real:

```bash
cargo run -p easylobby-agent --example descobrir_lcu
```
