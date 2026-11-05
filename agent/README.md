# agent

Crate Rust do agente. Acha o processo do LoL, lê o `lockfile`, conecta na LCU API (REST com certificado `riotgames.pem` fixado + WebSocket WAMP) e expõe o canal interno `/ws` tipado para a UI.

```bash
cargo test -p easylobby-agent
```

Fixtures gravadas de sessões reais ficam em `tests/fixtures/<patch>/` (issue #5) e alimentam o `mock-lcu`.
