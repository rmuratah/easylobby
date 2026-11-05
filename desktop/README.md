# desktop

Shell Tauri 2. Embute o crate do agente e mostra a UI React numa janela (ícone na bandeja é a issue #14).

```bash
pnpm install          # na raiz do repo
pnpm dev:desktop      # desenvolvimento
pnpm --filter easylobby-desktop build   # instalador
```

O código Rust do shell fica em `src-tauri/` e faz parte do workspace do Cargo, mas fora dos `default-members`: compile com `cargo build -p easylobby-desktop`.
