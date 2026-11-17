# Registro do easylobby na Riot

Material para registrar o easylobby no [Riot Developer Portal](https://developer.riotgames.com) (#37, #217). O passo a passo e os textos do formulário estão no guia "Registro do easylobby na Riot"; este arquivo guarda a parte que muda junto com o código: a lista de endpoints e serviços que o app usa.

**Atualize esta lista no mesmo PR que fizer o agente chamar um endpoint novo**, e atualize a página do produto no portal antes de publicar a versão, porque a Riot pede que funções novas passem pela página do produto.

Para conferir a lista com o código:

```sh
# caminhos da LCU, da Live Client Data API e da API da Riot fora dos testes
for f in agent/src/*.rs; do awk '/^#\[cfg\(test\)\]/{exit} {print}' "$f"; done \
  | grep -oE '"/(riotclient|lol-[a-z0-9-]+|liveclientdata|lol/)[^"]*"' | sort -u
# serviços externos
for f in agent/src/*.rs; do awk '/^#\[cfg\(test\)\]/{exit} {print}' "$f"; done \
  | grep -oE '"https?://[^"]+"' | sort -u
```

`agent/src/fixtures.rs` é o gravador de fixtures, usado só no desenvolvimento, e fica de fora da lista.

## O que mudou desde o primeiro guia

O guia foi escrito em 5 de novembro de 2026. Desde então o agente passou a usar duas coisas que mudam o registro:

- **API da Riot com chave** (`spectator-v5`, #436, aba "Em jogo" do perfil). Uma Personal API Key não pode ir no app distribuído nem atender outros jogadores, então essa função precisa de uma **Production API Key** guardada num servidor nosso (`EASYLOBBY_RIOT_API_URL`), ou fica desligada no alfa. O registro pode começar como Personal API Key, desde que a aba "Em jogo" não vá para os amigos sem a chave de produto.
- **Live Client Data API** (`127.0.0.1:2999`, #268), que o jogo abre durante a partida. Ela é oficial e documentada, mas a frase "does nothing during a match" do texto antigo deixou de ser verdade: o app lê, só para mostrar, as runas dos jogadores e o tempo de jogo.

## Endpoints used (para colar no portal)

The list below is in English so it can be pasted into the product page as is.

### League Client API (LCU), local, authenticated with the lockfile

| Area | Endpoints | Why |
| --- | --- | --- |
| Official UI | `POST /riotclient/kill-ux`, `POST /riotclient/launch-ux`, `POST /riotclient/ux-minimize`, event `/riotclient/ux-state/request`, `GET /riotclient/region-locale` | Close the official UI (optional), bring it back with one click, follow the client's requests to show it, read the player's region |
| Player | `GET /lol-summoner/v1/current-summoner`, `GET /lol-summoner/v2/summoners/puuid/{puuid}`, `GET /lol-summoner/v1/alias/lookup`, `GET /lol-summoner/v1/summoners?name=` | Show the logged-in player, open a friend's or teammate's profile, find a player by Riot ID typed by the user (to invite or add) |
| Patch and game data | `GET /lol-patch/v1/game-version`, `GET /lol-game-data/assets/...` (`champion-summary.json`, `perks.json`, `perkstyles.json`, `summoner-spells.json`, icons and splashes) | Current patch, champion, rune and spell names and images |
| Game flow | `/lol-gameflow/v1/gameflow-phase`, `/lol-gameflow/v1/session`, `POST /lol-end-of-game/v1/state/dismiss-stats` | Know which screen to show; leave the post-game screen when the player clicks Leave |
| Modes and lobby | `GET /lol-game-queues/v1/queues`, `/lol-lobby/v2/lobby` and its sub-resources (`members`, `invitations`, `position-preferences`, `partyType`, `team`, `subteamData`, `custom/available-bots`, `matchmaking/search`), `/lol-lobby/v2/party/{id}/join`, `/lol-lobby/v2/received-invitations`, `/lol-lobby/v2/play-again`, `/lol-lobby/v2/play-again-decline`, `/lol-lobby/v2/ags/...` (lobby link), `/lol-lobby/v1/parties`, `/lol-lobby/v1/lobby/members/localMember/player-slots` (Quickplay), `/lol-lobby/v1/lobby/custom/*` and `/lol-lobby/v1/custom-games` (custom games and bots) | Create and join lobbies, pick a queue and roles, invite friends, answer invites, custom games, Play Again |
| Queue and ready check | `/lol-matchmaking/v1/search`, `GET /lol-matchmaking/v1/ready-check`, `POST .../ready-check/accept`, `POST .../ready-check/decline` | Enter and leave the queue; accept or decline only when the player clicks |
| Champion select | `/lol-champ-select/v1/session`, `.../session/actions/{id}` (ban and pick on click), `.../session/my-selection` (skin and spells), `.../session/my-selection/reroll` and `.../session/bench/swap/{id}` (ARAM), `.../session/{champion,pick-order,position}-swaps/{id}/{request,accept,decline,cancel}`, `GET /lol-champ-select/v1/pickable-champion-ids`, `GET /lol-champ-select/v1/bannable-champion-ids`, `/lol-lobby-team-builder/champ-select/v1/subset-champion-list`, `/lol-lobby/v1/autofill-displayed` | Manual ban and pick, skins, summoner spells, ARAM rerolls and bench, trades |
| Champions and collection | `GET /lol-champions/v1/owned-champions-minimal`, `GET /lol-champions/v1/inventories/{summonerId}/{champions-minimal,skins-minimal,champions/{id}/skins}`, `GET /lol-collections/v1/inventories`, `GET /lol-champion-mastery/v1/local-player/champion-mastery` | Show owned champions, skins and mastery (read only) |
| Runes | `/lol-perks/v1/pages`, `/lol-perks/v1/pages/{id}`, `/lol-perks/v1/currentpage`, `GET /lol-perks/v1/inventory`, `GET /lol-perks/v1/recommended-champion-positions`, `GET /lol-perks/v1/recommended-pages/champion/{id}/position/{pos}/map/{map}` | View, edit and switch rune pages; load the client's own recommended page when the player asks |
| Friends and chat | `/lol-chat/v1/me`, `/lol-chat/v1/friends`, `/lol-chat/v1/friend-groups`, `/lol-chat/v1/friend-counts`, `/lol-chat/v1/conversations` and `.../{id}/messages`, `/lol-chat/v1/conversations/active`, `GET /lol-chat/v1/settings`, `GET /lol-suggested-players/v1/suggested-players` | Friend list, status, chat with friends, lobby and champ select chat, invite suggestions |
| Profile and history | `GET /lol-ranked/v1/current-ranked-stats`, `GET /lol-ranked/v1/ranked-stats/{puuid}`, `GET /lol-match-history/v1/products/lol/{current-summoner or puuid}/matches`, `GET /lol-match-history/v1/games/{id}`, `GET /lol-match-history/v1/recently-played-summoners/{id}`, `GET /lol-honor-v2/v1/latest-eligible-game` | Official rank only, match history and scoreboard, post-game honor |
| Wallet | `GET /lol-inventory/v1/wallet/{currency}`, `GET /lol-loot/v1/player-loot/CURRENCY_cosmetic` | Show Blue Essence, RP and Orange Essence balances (no store, no purchases) |
| Training | `GET /lol-npe-tutorial-path/v1/tutorials` | Tutorial list in Training |
| Events (WAMP) | Subscription to `OnJsonApiEvent`, filtered client side to: `/lol-gameflow/v1/`, `/lol-lobby/v2/`, `/lol-matchmaking/v1/`, `/lol-champ-select/v1/`, `/lol-perks/v1/`, `/lol-chat/v1/` (except settings), `/lol-summoner/v1/current-summoner`, `/lol-ranked/v1/current-ranked-stats`, `/riotclient/ux-state/request`, the ARAM subset list, the custom games list and the wallet | Real-time updates |

### Live Client Data API, local, during a match

| Endpoint | Why |
| --- | --- |
| `GET https://127.0.0.1:2999/liveclientdata/allgamedata` | Show the runes of each player and the game time on the "game in progress" screen, read only |

### Riot Games API

| Endpoint | Why |
| --- | --- |
| `GET https://{regional}.api.riotgames.com/riot/account/v1/accounts/by-riot-id/{gameName}/{tagLine}` | Turn the Riot ID of the profile the player opened into the API puuid used by spectator-v5. Same server and key as below |
| `GET https://{platform}.api.riotgames.com/lol/spectator/v5/active-games/by-summoner/{puuid}` | "In game" tab of another player's profile: whether a friend is in a match and the two teams. Only with a Production API Key kept on our own server; off when there is no key |

### Other public sources

| Source | Why |
| --- | --- |
| `ddragon.leagueoflegends.com` (Data Dragon) | Champion, item, rune and spell images |
| `raw.communitydragon.org` | Images the client has and Data Dragon does not (fallback) |
| `www.leagueoflegends.com/pt-br` | Link and title of the latest patch notes |
| `gg.riotgames.com/LOL?joinCode=` | Build the official lobby invite link |

## Aviso legal

Texto que a Riot pede, já na tela Configurações do app (seção "Sobre") e no README:

> easylobby isn't endorsed by Riot Games and doesn't reflect the views or opinions of Riot Games or anyone officially involved in producing or managing Riot Games properties. Riot Games, and all associated properties are trademarks or registered trademarks of Riot Games, Inc.
