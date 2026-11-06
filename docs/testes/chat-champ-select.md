# Chat da seleção — validação da UI (#27)

Esta entrega completa o componente do PR #115, já integrado à branch de #114.
Base: `feat/25-trocas-ui` (`4af9fe0`). Ordem de integração: #112 (inclui #113)
→ #114 (inclui #115) → PR desta branch. Retarget para `main` após as bases
entrarem; não fazer merge das dependências só para testar este PR.

## Contrato e limites

- A UI usa `useAgent` existente e `sendChampSelectMessage { body }`.
- `champSelectChat` é um snapshot, não um delta. A UI substitui a lista e
  mantém a ordem recebida, sem inserção otimista ou reordenação por timestamp.
- O agente deduplica por ID e mantém até 200 mensagens. Recusa texto vazio
  após `trim`, sala ausente (`invalidCommand`), LoL desconectado (`notConnected`)
  ou recusa da LCU (`lcu`). Não há limite de caracteres definido no agente.
- Autor: `fromMe` → Você; `fromCellId` → jogador em `myTeam`, usando nome/tag
  disponíveis ou Aliado N; célula desconhecida → Aliado. Não tenta revelar nomes.
- O timeout de 10 segundos vem do cliente existente. Não prova falha de entrega:
  o texto permanece, há aviso para conferir o histórico e nenhum reenvio automático.
- Fechamento da sala, troca/encerramento de seleção, troca de conexão LCU e saída
  da fase invalidam o formulário. Eventos de chat fora da seleção são descartados.
- **Limitação do contrato:** a sala não expõe ID/geração no `/ws`. Não é possível
  reconhecer uma substituição direta de sala dentro da mesma seleção sem `null`,
  nem atribuir um snapshot atrasado à sala anterior após a nova estar aberta.
  A filtragem dessas mensagens depende do agente (que confere conexão e ID de sala).
  Uma garantia adicional na UI exigiria ID/geração no contrato, fornecido por rmuratah;
  não foram inventados campos nem editados os bindings.

## Automatizado

Na raiz, com Node 22 e pnpm do `package.json`:

```powershell
corepack enable
pnpm install --frozen-lockfile
pnpm --filter easylobby-ui exec vitest run --maxWorkers=2
pnpm lint
pnpm build:ui
cargo test -p mock-lcu --test chat_champ_select --locked
```

Os testes de integração da UI usam `App`, `useAgent`, parser e cliente reais, com
transporte WebSocket simulado. Cobrem recebimento/ordem, autores, envio, espaços,
submissões repetidas, confirmação antes da resposta, snapshot repetido, erro,
timeout, rascunho novo durante envio, fechamento/reabertura, mudança de seleção,
conexão LCU, queda do socket, mensagens/respostas atrasadas e rolagem.
O parser testa campos inválidos em evento e `hello`.

Executado neste ambiente: suíte completa (352 aprovados, 1 ignorado) e, após
os dois últimos casos de regressão, os três arquivos afetados (107 aprovados),
lint e build. O pnpm disponibilizado pelo ambiente é 11.25.0; o lockfile foi
instalado com `--frozen-lockfile` e não mudou. O CI deve validar com a versão
fixada no projeto. Mock-lcu não executado: `cargo: command not found`.

## Windows com mock-lcu

Instale Rust estável e os pré-requisitos do Tauri 2 (Build Tools C++ e WebView2).
Em um PowerShell, na raiz:

```powershell
cargo run -p mock-lcu -- --fixtures 16.22 --repetir
```

Em outro:

```powershell
$env:EASYLOBBY_MOCK_LCU = "1"
pnpm dev:desktop
# Após encerrar o desktop:
Remove-Item Env:EASYLOBBY_MOCK_LCU
```

Não rode `canal_ws` junto do desktop: usam a mesma porta. Aguarde a gravação
chegar à seleção. Confira o histórico gravado, autores e ordem. Envie texto com
Enter e botão; deve aparecer uma única vez após confirmação do agente. Texto
vazio não habilita envio. Role para cima e receba mensagens: a leitura deve ficar
no lugar; volte ao fim para acompanhar as próximas. Confira também o histórico
quando alcançar 200 mensagens (o agente descarta as mais antigas).

Pare/reinicie o mock durante um envio; o chat antigo deve desaparecer e não
reaparecer com o rascunho ou erro anterior. Na próxima seleção, o formulário deve
começar vazio. Teste navegar para Runas e voltar, e o chat privado com um amigo.
Os cenários precisos de timeout e resposta tardia são reproduzidos pelos testes
automatizados; o mock não oferece um controle específico de atraso para essa UI.

## Windows com LoL real — pendente

Sem `EASYLOBBY_MOCK_LCU`, inicie `pnpm dev:desktop` e entre numa seleção com um
aliado. Troquem mensagens nos dois sentidos, confirme o autor visível/anônimo,
acentos, texto longo e rolagem em janela pequena. Troque a ordem de escolha e
confira a autoria após o agente atualizar as células. Complete a seleção ou saia
dela e entre novamente: não deve sobrar texto, erro ou envio pendente.
Confira que seleção/ban, feitiços, trocas, runas e chat privado continuam operando.

CI (UI e Rust Windows), revisão cruzada de rmuratah e teste real permanecem
necessários antes do merge. Seguir uma rodada de revisão; só bugs, segurança e
quebras do contrato bloqueiam. Melhorias ficam para issues próprias.
