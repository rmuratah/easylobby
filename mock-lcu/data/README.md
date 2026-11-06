# Dados do jogo do mock-lcu

`perkstyles.json` e `perks.json` imitam `/lol-game-data/assets/v1/perkstyles.json`
e `/lol-game-data/assets/v1/perks.json` da LCU (as árvores e os nomes das runas, #26).
As gravações em `agent/tests/fixtures/` ainda não têm esses arquivos, então o mock
responde com estes quando a gravação não tiver.

São **dados sintéticos**, montados à mão no formato da LCU: os `id`s e as linhas
seguem o patch 16.22 (e batem com as páginas gravadas), mas as descrições são de
mentira e os nomes podem não bater com o client. Quando o `gravar_fixtures`
gravar os arquivos de verdade, eles passam na frente.

`summoner-spells.json` imita `/lol-game-data/assets/v1/summoner-spells.json` (os
feitiços de invocador, #24), também **sintético**: os `id`s e os modos de jogo
seguem o client, mas nomes, descrições e níveis mínimos podem não bater. O
`gravar_fixtures` já grava o arquivo de verdade, que passa na frente.

`champion-summary.json` imita `/lol-game-data/assets/v1/champion-summary.json` (os
campeões da grade da seleção, #28), também **sintético**: os `id`s cobrem os
campeões da gravação do patch 16.22 com os nomes em inglês, e `roles` vem vazio.
Os `id`s que não reconhecemos aparecem como "Campeão N". O `gravar_fixtures` já
grava o arquivo de verdade, que passa na frente.
