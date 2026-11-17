# desktop

Shell Tauri 2. Embute o crate do agente e mostra a UI React numa janela (ícone na bandeja é a issue #14).

```bash
pnpm install          # na raiz do repo
pnpm dev:desktop      # desenvolvimento
pnpm --filter easylobby-desktop build   # instalador NSIS, sem o pacote de atualização
```

O instalador oficial e o auto-update saem do workflow de release; veja [Como lançar uma versão](../docs/release.md).

O código Rust do shell fica em `src-tauri/` e faz parte do workspace do Cargo, mas fora dos `default-members`: compile com `cargo build -p easylobby-desktop`.

## Janela e bandeja

Ao iniciar, o app abre a janela principal e cria um ícone na bandeja. No Windows,
o ícone pode ficar no grupo de ícones ocultos próximo ao relógio.

- O botão **X** oculta a janela e mantém o app na bandeja.
- Clique esquerdo no ícone ou **Abrir easylobby** no menu restaura e foca a janela, inclusive se estiver minimizada.
- Clique direito abre o menu; **Sair** reabre a interface oficial do LoL (`launch-ux`), se o agente estiver conectado, e encerra o aplicativo. Se a LCU não responder em 3 s, o app fecha mesmo assim (#89).
- Abrir o easylobby com ele já aberto mostra a janela da instância que está rodando, em vez de abrir outra (#90).
- Quando a partida é encontrada, a janela vem para a frente (mesmo minimizada ou na bandeja) e a UI toca um som. Se o Windows não deixar a janela tomar o foco, o ícone pisca na barra de tarefas.
- **Abrir junto com o LoL** (#511): o app inicia com o Windows com `--bandeja`, só com o ícone na bandeja e sem a janela. Quando o LoL abre e o agente conecta, a janela aparece (se estava escondida ou minimizada) e o `kill-ux` tira a UI oficial. A opção fica em Configurações, na barra lateral, e mexe na chave `Run` do registro pelo `tauri-plugin-autostart`. Ela é ligada na primeira vez que um build de release abre (um arquivo `abrir-com-lol-padrao` na pasta de configuração lembra disso); o `tauri dev` nunca liga sozinho.
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

## Atualizações

O app procura versão nova ao abrir e a cada 6 horas, pelo `tauri-plugin-updater`
e o `latest.json` do GitHub Releases (#35). Com versão nova, um ponto aparece no
botão de Configurações, e a seção Atualizações mostra a versão, as notas e o botão
**Atualizar e reiniciar**, liberado só fora da fila, da seleção e da partida. A
chave pública que confere a assinatura fica em `plugins.updater.pubkey` do
`tauri.conf.json`.

## Log de erros

O app grava os erros num arquivo de texto por dia em
`%LOCALAPPDATA%\io.github.rmuratah.easylobby\logs\easylobby-AAAA-MM-DD.log`
(#534). Ficam os 7 dias mais recentes, e cada arquivo para em 5 MB. Entram
os erros do agente, as chamadas que a LCU recusa ou responde com erro, as
quedas da conexão com o LoL e com o agente e os erros da interface. Não
entram a senha do lockfile, tokens, a chave da API da Riot nem o texto das
mensagens do chat. O botão "Abrir pasta de logs" em Configurações abre a
pasta no Explorer.

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
