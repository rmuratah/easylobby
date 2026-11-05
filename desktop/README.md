# desktop

Shell Tauri 2. Embute o crate do agente e mostra a UI React numa janela (ícone na bandeja é a issue #14).

```bash
pnpm install          # na raiz do repo
pnpm dev:desktop      # desenvolvimento
pnpm --filter easylobby-desktop build   # instalador
```

O código Rust do shell fica em `src-tauri/` e faz parte do workspace do Cargo, mas fora dos `default-members`: compile com `cargo build -p easylobby-desktop`.

## Janela e bandeja

Ao iniciar, o app abre a janela principal e cria um ícone na bandeja. No Windows,
o ícone pode ficar no grupo de ícones ocultos próximo ao relógio.

- O botão **X** oculta a janela e mantém o app na bandeja.
- Clique esquerdo no ícone ou **Abrir easylobby** no menu restaura e foca a janela, inclusive se estiver minimizada.
- Clique direito abre o menu; **Sair** encerra o aplicativo e remove o ícone.

O crate do agente está embutido no shell; a conexão e o canal `/ws` ainda dependem
 das respectivas issues. Esta entrega não fecha nem reabre a interface do LoL.

### Validação manual no Windows

Na raiz, execute `pnpm install --frozen-lockfile` e `pnpm dev:desktop`.
Confira a janela e o ícone; feche pelo X, reabra pelo clique esquerdo e pelo menu,
minimize e restaure pela bandeja. Use **Sair**, confirme que janela e ícone sumiram,
e inicie novamente. Repita o ciclo para conferir que uma única janela é restaurada.

O navegador (`pnpm dev:ui`) não oferece a bandeja nativa. No Linux, suporte a
ícones e eventos de clique depende do ambiente desktop; use o menu quando o clique
esquerdo não estiver disponível.
