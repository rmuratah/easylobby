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

O shell inicia o agente e o canal `/ws` em `127.0.0.1:47800` ao abrir.
O canal permanece ativo ao ocultar a janela e termina com o processo ao usar Sair.
Não há reinício interno do serviço (a correção de cancelamento é acompanhada na #58).
Esta integração não fecha nem reabre a interface oficial do LoL.

### Validação manual no Windows

Na raiz, execute `pnpm install --frozen-lockfile` e `pnpm dev:desktop`.
Confira a janela e o ícone; feche pelo X, reabra pelo clique esquerdo e pelo menu,
minimize e restaure pela bandeja. Use **Sair**, confirme que janela e ícone sumiram,
e inicie novamente. Repita o ciclo para conferir que uma única janela é restaurada.

O navegador (`pnpm dev:ui`) não oferece a bandeja nativa. No Linux, suporte a
ícones e eventos de clique depende do ambiente desktop; use o menu quando o clique
esquerdo não estiver disponível.

## Conexão com o agente

Não inicie o example `canal_ws` junto com o desktop: ambos usam a porta 47800.
Se ela estiver ocupada, o desktop falha ao iniciar em vez de reutilizar outro processo.

Para testar no mock, rode `cargo run -p mock-lcu` em um terminal. Em outro,
na raiz do projeto, configure o modo mock antes de iniciar o desktop:

```powershell
# PowerShell
$env:EASYLOBBY_MOCK_LCU = "1"
pnpm dev:desktop
# Depois de encerrar, volte ao LoL real:
Remove-Item Env:EASYLOBBY_MOCK_LCU
```

```bash
# Git Bash / Linux / macOS
EASYLOBBY_MOCK_LCU=1 pnpm dev:desktop
```

Sem essa variável, o agente descobre o LoL real. Abra o client e faça login:
a tela deve passar de espera para conectado. Pare/reinicie o mock ou o LoL e
confira a atualização do status sem reiniciar o easylobby. Nome e nível serão
exibidos na entrega da #7; esta etapa valida o canal e os estados de conexão.
