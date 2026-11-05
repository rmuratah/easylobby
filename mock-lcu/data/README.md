# Dados do jogo do mock-lcu

`perkstyles.json` e `perks.json` imitam `/lol-game-data/assets/v1/perkstyles.json`
e `/lol-game-data/assets/v1/perks.json` da LCU (as árvores e os nomes das runas, #26).
As gravações em `agent/tests/fixtures/` ainda não têm esses arquivos, então o mock
responde com estes quando a gravação não tiver.

São **dados sintéticos**, montados à mão no formato da LCU: os `id`s e as linhas
seguem o patch 16.22 (e batem com as páginas gravadas), mas as descrições são de
mentira e os nomes podem não bater com o client. Quando o `gravar_fixtures`
gravar os arquivos de verdade, eles passam na frente.
