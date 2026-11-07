# Validação da Fase 5 · Redesign (#133)

Registro do critério de pronto da fase. A parte automática já foi conferida e está aqui. A parte que exige o LoL real (RAM e partida completa) tem os campos para preencher.

## 1. Estilos antigos

- Nenhuma classe de cor fixa (`slate`, `sky`, `amber`, `emerald`, `rose`, `red`) sobra nas telas. O bloco de compatibilidade que mapeava essas classes para os tokens saiu do `index.css`.
- Nenhum tamanho de texto fora da escala do tema (`text-sm`, `text-xs`, `text-2xl` e afins). Tudo usa `text-caption`, `text-body`, `text-section` ou `text-title`.
- A única cor fixa que ficou é proposital: o fundo vermelho `#c42b1c` com texto branco no hover do botão Fechar da barra de título. É o padrão do Windows e passa AA (5,7:1).
- O `.app-shell` forçava `color-scheme: dark`. No tema claro, isso deixava escuros os campos nativos, como a lista aberta de um `select` e as barras de rolagem. Agora o `color-scheme` vem do tema.

## 2. Contraste (WCAG AA)

**Tokens.** `ui/src/theme/tokens.test.ts` confere cada par de cores usado nas telas, nos dois temas:

- 4,5:1 para texto.
- 3:1 para bordas, ícones e indicadores de status.

A validação acrescentou os pares que as telas usam de fato sobre o destaque suave e sobre o fundo: texto secundário sobre `accent-soft` (jogador da vez, página de runas em uso, partida encontrada), `accent-ink`, `warn` e `danger` sobre `bg` e `raised`.

Um par falhava: o texto secundário sobre `accent-soft`, no tema claro, dava 4,44:1. O gerador da paleta (`ui/scripts/generate-palette.mjs`) agora também garante 4,5:1 desse texto sobre o destaque suave. A cor `--el-muted` do tema claro mudou de `#666e79` para `#656d78`.

**Telas renderizadas.** A regra `color-contrast` do axe-core 4.10 rodou na build de produção, com o agente simulado, em 36 combinações. O mesmo teste conferiu se havia rolagem horizontal na janela, na área principal ou na coluna de amigos.

| Tela | 960×640 claro | 960×640 escuro | 1100×700 claro | 1100×700 escuro |
| --- | --- | --- | --- | --- |
| LoL fechado | ok | ok | ok | ok |
| Início | ok | ok | ok | ok |
| Lobby | ok | ok | ok | ok |
| Fila | ok | ok | ok | ok |
| Partida encontrada | ok | ok | ok | ok |
| Seleção de campeões | ok | ok | ok | ok |
| Runas (editor aberto, vindo da seleção) | ok | ok | ok | ok |
| Coluna com conversa aberta | ok | ok | ok | ok |
| Coluna recolhida com aviso | ok | ok | ok | ok |

"ok" quer dizer zero violação de contraste, de rótulo (`label`, `button-name`) e de ARIA, e nenhuma rolagem horizontal.

O axe deixou dois casos sem decidir, e nenhum é um problema real:

- O botão "Editar runas" na dobra da tela em 960×640, que o axe não consegue medir por estar cortado pela janela.
- O botão "←" da conversa, que é só um símbolo e já tem o rótulo "Voltar às conversas".

## 3. Tamanho da janela

Em 960×640 (mínimo) e 1100×700 (padrão), todas as telas acima ficam usáveis e sem rolagem horizontal. Os botões de ação que precisam estar sempre à vista ficam presos embaixo:

- Iniciar ou cancelar a busca.
- Aceitar ou recusar a partida.
- Salvar ou cancelar no editor de runas.

## 4. RAM (preencher no Windows, LoL real)

Use o mesmo método da [#121](https://github.com/rmuratah/easylobby/issues/121#issuecomment-6692208608):

1. Abra o app e vá até a tela.
2. Espere 10 segundos parado.
3. Cole o script inteiro no PowerShell. Ele mede três vezes, com 5 segundos entre uma e outra, e mostra a mediana.
4. Anote também a tela em que o app estava.

O script só conta o `easylobby-desktop` e os `msedgewebview2` filhos dele. A primeira versão somava tudo o que descendia do app. Quando o LoL é aberto pelo botão do easylobby, o `LeagueClient` e o `Riot Client` viram filhos do app, e o total passava de 2 GB.

| Tela | Referência (#121) | Limite (+10%) | Medições (MB) | Mediana | Dentro? |
| --- | --- | --- | --- | --- | --- |
| Início | 134,4 MB | 147,8 MB | | | |
| Lobby | 137,5 MB | 151,3 MB | | | |
| Seleção de campeões | 146,3 MB | 160,9 MB | | | |

```powershell
# Conta só o easylobby e os WebView2 dele. Quando o app abre o LoL, o LeagueClient e o Riot Client
# viram filhos do easylobby; o filtro por nome deixa esses processos (e os filhos deles) de fora.
function Medir-Easylobby {
  $procs = Get-CimInstance Win32_Process
  $root = $procs | Where-Object { $_.Name -like 'easylobby*.exe' }
  if (-not $root) { Write-Host 'O easylobby nao esta aberto.'; return $null }
  $ids = [System.Collections.Generic.HashSet[int]]::new()
  $root | ForEach-Object { [void]$ids.Add([int]$_.ProcessId) }
  do { $added = 0; foreach ($p in $procs) { if ($p.Name -eq 'msedgewebview2.exe' -and $ids.Contains([int]$p.ParentProcessId) -and $ids.Add([int]$p.ProcessId)) { $added++ } } } while ($added)
  $perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Where-Object { $ids.Contains([int]$_.IDProcess) }
  $perf | Sort-Object WorkingSetPrivate -Descending | Format-Table @{n='Processo';e={$_.Name}}, @{n='PID';e={$_.IDProcess}}, @{n='MB';e={[math]::Round($_.WorkingSetPrivate/1MB,1)}} -AutoSize | Out-Host
  $total = ($perf | Measure-Object WorkingSetPrivate -Sum).Sum / 1MB
  "Total: {0:N1} MB em {1} processos" -f $total, $perf.Count | Out-Host
  return $total
}
# Três medições com 5 s de intervalo e a mediana.
$totais = 1..3 | ForEach-Object { if ($_ -gt 1) { Start-Sleep -Seconds 5 }; Medir-Easylobby }
if ($totais -notcontains $null) { "Medições: {0}  |  Mediana: {1:N1} MB" -f (($totais | ForEach-Object { '{0:N1}' -f $_ }) -join ' / '), (($totais | Sort-Object)[1]) }
```

## 5. Partida normal no LoL real (preencher)

Faça uma partida normal do início ao fim no tema claro e outra no escuro. Marque o que funcionou sem nada quebrado ou ilegível.

| Etapa | Claro | Escuro |
| --- | --- | --- |
| App abre, fecha a UI oficial e mostra o início | | |
| Cria o lobby, escolhe as posições, convida um amigo | | |
| Busca, partida encontrada e aceitar (botão ou Enter) | | |
| Seleção: busca digitando, ban, pick e confirmação | | |
| Troca de runas e de feitiços na seleção | | |
| Chat do time e com um amigo | | |
| Janela em 960×640 durante a seleção | | |
| Fim da partida: o app volta ao lobby ou ao início | | |
