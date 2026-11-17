//! Cédula de honra do fim de partida: honrar jogadores ou pular (#182).
//!
//! Como no client oficial, o jogador tem várias honras por partida
//! (`votePool.votes`, que muda de partida para partida) e pode honrar
//! aliados e adversários, cada um uma vez.
//!
//! Depois do jogo, a LCU abre a cédula em `GET /lol-honor-v2/v1/ballot` e
//! avisa pelo WAMP no mesmo URI (`Create` e `Update` com a cédula, `Delete`
//! quando ela fecha, na volta ao lobby). Fora do pós-jogo a LCU responde uma
//! cédula vazia (`gameId: 0`, sem aliados), que o agente trata como "sem
//! cédula".
//!
//! Nas gravações do LoL real (16.22) a cédula fica aberta durante todo o
//! `EndOfGame`, inclusive depois de o agente encerrar a tela `honor-vote`
//! do `PreEndOfGame` (#180), então a votação acontece na tela de resultado.
//!
//! O voto é `POST /lol-honor/v1/honor` com `{recipientPuuid, honorType}`
//! (`LolHonorV2ApiHonorPlayerServerRequestV3` no `/help` da LCU, example
//! `ajuda_lcu`, 16.22), o mesmo formato de `honoredPlayers` da cédula. O
//! endpoint antigo, `POST /lol-honor-v2/v1/honor-player`, chama
//! `honor-edge/v2/honorPlayer` no servidor da Riot, que recusou o voto (403)
//! nas quatro partidas de teste, com `honorCategory` e com `honorType`.
//!
//! A LCU responde 200 antes de saber o que o servidor disse. O voto só vale
//! quando o aliado aparece em `honoredPlayers` da cédula
//! (`{recipientPuuid, honorType}`), e é isso que a UI usa para confirmar.
//!
//! Pular não fala com o servidor: só encerra a tela `honor-vote` do
//! `PreEndOfGame` (`crate::pre_end_of_game`), como quem fecha a votação sem
//! votar.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) da cédula de honra.
pub const BALLOT_PATH: &str = "/lol-honor-v2/v1/ballot";

/// Endpoint do voto.
pub const HONOR_PATH: &str = "/lol-honor/v1/honor";

/// Tipo do voto (`honorType`). Desde a mudança da honra há um só.
pub const HONOR_TYPE: &str = "HEART";

/// Tela do `PreEndOfGame` da votação, encerrada ao pular.
pub const HONOR_VOTE_SEQUENCE: &str = "honor-vote";

/// Cédula de honra aberta no fim de uma partida.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HonorBallot {
    /// Partida da cédula.
    #[ts(type = "number")]
    pub game_id: u64,
    /// Aliados que podem receber honra, na ordem da LCU (por rota).
    pub eligible_allies: Vec<HonorCandidate>,
    /// Adversários que podem receber honra, na ordem da LCU (por rota).
    pub eligible_opponents: Vec<HonorCandidate>,
    /// `puuid` de quem o jogador já honrou nesta partida.
    pub honored_puuids: Vec<String>,
    /// Honras da partida, contando as já dadas (`votePool.votes` da
    /// primeira cédula da partida). As que sobram são `votes` menos
    /// `honoredPuuids`.
    pub votes: u32,
}

/// Um jogador na cédula, aliado ou adversário.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HonorCandidate {
    pub puuid: String,
    #[ts(type = "number")]
    pub summoner_id: u64,
    pub champion_id: u32,
    pub champion_name: String,
    /// Rota da LCU (`TOP`, `JUNGLE`, `MIDDLE`, `BOTTOM`, `UTILITY`), ou
    /// vazia fora de Summoner's Rift.
    pub role: String,
    /// Nome que a LCU manda (o `gameName` do Riot ID).
    pub summoner_name: String,
    pub bot: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBallot {
    #[serde(default)]
    game_id: u64,
    #[serde(default)]
    eligible_allies: Vec<RawCandidate>,
    #[serde(default)]
    eligible_opponents: Vec<RawCandidate>,
    #[serde(default)]
    honored_players: Vec<Value>,
    #[serde(default)]
    vote_pool: RawVotePool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCandidate {
    #[serde(default)]
    puuid: String,
    #[serde(default)]
    summoner_id: u64,
    #[serde(default)]
    champion_id: u32,
    #[serde(default)]
    champion_name: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    summoner_name: String,
    #[serde(default)]
    bot_player: bool,
}

#[derive(Default, Deserialize)]
struct RawVotePool {
    #[serde(default)]
    votes: u32,
}

impl HonorBallot {
    /// Lê o `data` de um evento ou a resposta do REST. `None` para a cédula
    /// vazia de fora do pós-jogo.
    pub fn from_json(value: &Value) -> Option<Self> {
        let raw: RawBallot = serde_json::from_value(value.clone()).ok()?;
        if raw.game_id == 0 || raw.eligible_allies.is_empty() {
            return None;
        }
        Some(HonorBallot {
            game_id: raw.game_id,
            eligible_allies: candidates(raw.eligible_allies),
            eligible_opponents: candidates(raw.eligible_opponents),
            // `{recipientPuuid, honorType}` no 16.22; aceita também só o
            // puuid ou `puuid`, de versões antigas.
            honored_puuids: raw
                .honored_players
                .iter()
                .filter_map(|p| {
                    p.as_str()
                        .or_else(|| p["recipientPuuid"].as_str())
                        .or_else(|| p["puuid"].as_str())
                })
                .filter(|p| !p.is_empty())
                .map(str::to_owned)
                .collect(),
            votes: raw.vote_pool.votes,
        })
    }

    /// O jogador `puuid`, aliado ou adversário, se ele pode receber honra.
    pub fn candidate(&self, puuid: &str) -> Option<&HonorCandidate> {
        self.eligible_allies
            .iter()
            .chain(&self.eligible_opponents)
            .find(|c| c.puuid == puuid)
    }

    /// Mantém o total de honras da partida entre uma cédula e a seguinte.
    ///
    /// Não se sabe se a LCU desconta de `votePool.votes` as honras já dadas;
    /// guardar o maior valor visto na mesma partida dá o total nos dois
    /// casos, e as que sobram saem de `honoredPuuids`.
    pub fn keep_total_votes(mut self, previous: Option<&HonorBallot>) -> Self {
        if let Some(previous) = previous.filter(|p| p.game_id == self.game_id) {
            self.votes = self.votes.max(previous.votes);
        }
        self
    }

    /// Honras que ainda dá para dar.
    pub fn votes_left(&self) -> u32 {
        self.votes
            .saturating_sub(u32::try_from(self.honored_puuids.len()).unwrap_or(u32::MAX))
    }
}

fn candidates(raw: Vec<RawCandidate>) -> Vec<HonorCandidate> {
    raw.into_iter()
        .filter(|c| !c.puuid.is_empty())
        .map(|c| HonorCandidate {
            puuid: c.puuid,
            summoner_id: c.summoner_id,
            champion_id: c.champion_id,
            champion_name: c.champion_name,
            role: c.role,
            summoner_name: c.summoner_name,
            bot: c.bot_player,
        })
        .collect()
}

/// Corpo do voto em `candidate`.
pub fn vote_body(candidate: &HonorCandidate) -> Value {
    json!({
        "honorType": HONOR_TYPE,
        "recipientPuuid": candidate.puuid,
    })
}

/// Cédula atual (`GET /lol-honor-v2/v1/ballot`). `None` fora do pós-jogo.
pub async fn current_ballot(client: &LcuClient) -> Result<Option<HonorBallot>, LcuError> {
    let value: Value = client.get(BALLOT_PATH).await?;
    Ok(HonorBallot::from_json(&value))
}

/// Honra `candidate` na cédula aberta.
pub async fn vote(client: &LcuClient, candidate: &HonorCandidate) -> Result<(), LcuError> {
    client
        .post::<_, Option<Value>>(HONOR_PATH, &vote_body(candidate))
        .await
        .map(|_| ())
}

/// Pula a votação: encerra a tela `honor-vote` sem votar.
pub async fn skip(client: &LcuClient) -> Result<(), LcuError> {
    crate::pre_end_of_game::complete(client, HONOR_VOTE_SEQUENCE).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gravada() -> Value {
        // Recorte da cédula gravada em `16.22-pos-jogo` (#177).
        json!({
            "eligibleAllies": [
                {"botPlayer": false, "championId": 897, "championName": "K'Sante",
                 "puuid": "fake-puuid-0092", "role": "TOP",
                 "skinSplashPath": "/lol-game-data/assets/x.jpg",
                 "summonerId": 1134, "summonerName": "fake-name-0129"},
                {"botPlayer": false, "championId": 234, "championName": "Viego",
                 "puuid": "fake-puuid-0093", "role": "JUNGLE",
                 "skinSplashPath": "/lol-game-data/assets/y.jpg",
                 "summonerId": 1132, "summonerName": "fake-name-0130"}
            ],
            "eligibleOpponents": [
                {"botPlayer": false, "championId": 41, "championName": "Gangplank",
                 "puuid": "fake-puuid-0091", "role": "TOP", "summonerId": 1131,
                 "summonerName": "fake-name-0133"}
            ],
            "gameId": 1124,
            "honoredPlayers": [],
            "votePool": {"fromGamePlayed": 1, "fromHighHonor": 0, "fromRecentHonors": 1,
                         "fromRollover": 3, "votes": 4}
        })
    }

    #[test]
    fn le_a_cedula_gravada() {
        let ballot = HonorBallot::from_json(&gravada()).unwrap();
        assert_eq!(ballot.game_id, 1124);
        assert_eq!(ballot.votes, 4);
        assert_eq!(ballot.eligible_allies.len(), 2);
        let ksante = &ballot.eligible_allies[0];
        assert_eq!(ksante.puuid, "fake-puuid-0092");
        assert_eq!(ksante.summoner_id, 1134);
        assert_eq!(ksante.champion_id, 897);
        assert_eq!(ksante.role, "TOP");
        assert!(!ksante.bot);
        assert!(ballot.honored_puuids.is_empty());
        assert!(ballot.candidate("fake-puuid-0093").is_some());
        // Adversários também recebem honra.
        assert_eq!(ballot.eligible_opponents.len(), 1);
        assert_eq!(ballot.eligible_opponents[0].champion_name, "Gangplank");
        assert!(ballot.candidate("fake-puuid-0091").is_some());
        assert!(ballot.candidate("fake-puuid-9999").is_none());
        assert_eq!(ballot.votes_left(), 4);
    }

    #[test]
    fn cedula_vazia_fora_do_pos_jogo() {
        let vazia = json!({"eligibleAllies": [], "eligibleOpponents": [], "gameId": 0,
            "honoredPlayers": [], "votePool": {"votes": 1}});
        assert_eq!(HonorBallot::from_json(&vazia), None);
        assert_eq!(HonorBallot::from_json(&Value::Null), None);
    }

    #[test]
    fn quem_ja_foi_honrado() {
        let mut value = gravada();
        value["honoredPlayers"] = json!([
            {"recipientPuuid": "fake-puuid-0092", "honorType": "HEART"},
            {"puuid": "fake-puuid-0093"}
        ]);
        let ballot = HonorBallot::from_json(&value).unwrap();
        assert_eq!(
            ballot.honored_puuids,
            ["fake-puuid-0092", "fake-puuid-0093"]
        );
    }

    #[test]
    fn guarda_o_total_de_honras_da_partida() {
        let antes = HonorBallot::from_json(&gravada()).unwrap();
        let mut value = gravada();
        value["votePool"]["votes"] = json!(3);
        value["honoredPlayers"] =
            json!([{"recipientPuuid": "fake-puuid-0092", "honorType": "HEART"}]);
        let depois = HonorBallot::from_json(&value)
            .unwrap()
            .keep_total_votes(Some(&antes));
        assert_eq!(depois.votes, 4);
        assert_eq!(depois.votes_left(), 3);

        // Outra partida não herda o total.
        value["gameId"] = json!(2000);
        let outra = HonorBallot::from_json(&value)
            .unwrap()
            .keep_total_votes(Some(&antes));
        assert_eq!(outra.votes, 3);
    }

    #[test]
    fn corpo_do_voto() {
        let ballot = HonorBallot::from_json(&gravada()).unwrap();
        assert_eq!(
            vote_body(&ballot.eligible_allies[0]),
            json!({"honorType": "HEART", "recipientPuuid": "fake-puuid-0092"})
        );
    }
}
