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
- Clique direito abre o menu; **Sair** reabre a interface oficial do LoL (`launch-ux`), se o agente estiver conectado, e encerra o aplicativo. Se a LCU não responder em 3 s, o app fecha mesmo assim (#89).
- Abrir o easylobby com ele já aberto mostra a janela da instância que está rodando, em vez de abrir outra (#90).
- Quando a partida é encontrada, a janela vem para a frente (mesmo minimizada ou na bandeja) e a UI toca um som. Se o Windows não deixar a janela tomar o foco, o ícone pisca na barra de tarefas.
- A janela libera o áudio sem clique (`--autoplay-policy=no-user-gesture-required` em `additionalBrowserArgs`), para o som tocar mesmo quando a fila foi iniciada por outro jogador do lobby. Os outros argumentos da linha são os padrões do Tauri, que essa opção substitui.

O shell inicia o agente e o canal `/ws` em `127.0.0.1:47800` ao abrir.
O canal permanece ativo ao ocultar a janela e termina com o processo ao usar Sair.
Se o canal cair, o shell o sobe de novo (até 3 tentativas seguidas, com espera crescente);
se não conseguir, mostra uma caixa de erro com o motivo em vez de deixar a UI em
"Reconectando" (#91). O agente fecha a interface oficial ao conectar (`kill-ux`).

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
Se ela estiver ocupada por outro programa, o desktop mostra uma caixa de erro e fecha
ao confirmar, em vez de reutilizar outro processo.

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
