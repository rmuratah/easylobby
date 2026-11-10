//! Live Client Data API (#268): o servidor local que o **jogo** (não o
//! client) abre em `https://127.0.0.1:2999` depois da tela de carregamento.
//!
//! O agente lê só `GET /liveclientdata/allgamedata`, para as runas de cada
//! jogador (`allPlayers[].runes`) e o tempo de jogo (`gameData.gameTime`).
//! Não tem senha; o certificado é assinado pela mesma raiz da LCU, o
//! `riotgames.pem`, que fica fixado como única raiz confiável. A
//! verificação TLS nunca é desligada.

use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use crate::rest::RIOT_ROOT_PEM;

/// Endereço da Live Client Data API no PC do jogador.
pub const LIVE_CLIENT_URL: &str = "https://127.0.0.1:2999";

/// Caminho com tudo da partida.
pub const ALL_GAME_DATA_PATH: &str = "/liveclientdata/allgamedata";

/// Tempo máximo de uma leitura: o jogo responde na hora ou não está
/// aberto.
const TIMEOUT: Duration = Duration::from_secs(3);

/// Cliente da Live Client Data API, com o certificado da Riot fixado.
#[derive(Debug, Clone)]
pub struct LiveClient {
    http: reqwest::Client,
    base_url: String,
}

impl LiveClient {
    /// Cliente para o jogo de verdade.
    pub fn new() -> Result<Self, reqwest::Error> {
        Self::with_root_certificate(LIVE_CLIENT_URL, RIOT_ROOT_PEM)
    }

    /// Cliente para outro endereço, com outra raiz fixada (testes).
    pub fn with_root_certificate(base_url: &str, root_pem: &[u8]) -> Result<Self, reqwest::Error> {
        let root = reqwest::Certificate::from_pem(root_pem)?;
        let http = reqwest::Client::builder()
            .use_native_tls()
            .tls_built_in_root_certs(false)
            .add_root_certificate(root)
            .no_proxy()
            .timeout(TIMEOUT)
            .build()?;
        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_owned(),
        })
    }

    /// Lê a partida. `None` com o jogo fechado ou carregando, ou numa
    /// resposta que não dá para ler.
    pub async fn game_data(&self) -> Option<LiveClientData> {
        let response = self
            .http
            .get(format!("{}{ALL_GAME_DATA_PATH}", self.base_url))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let bytes = response.bytes().await.ok()?;
        let body: Value = serde_json::from_slice(&bytes).ok()?;
        LiveClientData::from_json(&body)
    }
}

/// O que o agente usa da Live Client Data API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveClientData {
    /// Segundos de jogo.
    pub game_time_secs: u32,
    pub players: Vec<LiveClientPlayer>,
}

/// Um jogador como a Live Client Data API manda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveClientPlayer {
    /// `100` (`ORDER`) ou `200` (`CHAOS`).
    pub team_id: u32,
    /// `alias` do campeão (`MonkeyKing`), tirado de `rawChampionName`
    /// (`game_character_displayname_MonkeyKing`).
    pub champion_alias: String,
    pub game_name: String,
    pub tag_line: String,
    pub keystone_id: u32,
    pub primary_style_id: u32,
    pub sub_style_id: u32,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawData {
    all_players: Vec<RawPlayer>,
    game_data: RawGameData,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawGameData {
    game_time: f64,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawPlayer {
    team: String,
    raw_champion_name: String,
    riot_id: String,
    riot_id_game_name: String,
    riot_id_tag_line: String,
    runes: RawRunes,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawRunes {
    keystone: RawId,
    primary_rune_tree: RawId,
    secondary_rune_tree: RawId,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawId {
    id: u32,
}

impl LiveClientData {
    /// Lê o JSON de `allgamedata`. `None` sem a lista de jogadores (o jogo
    /// ainda carregando responde outra coisa).
    pub fn from_json(value: &Value) -> Option<Self> {
        value.get("allPlayers")?.as_array()?;
        let raw = RawData::deserialize(value).ok()?;
        let players = raw
            .all_players
            .into_iter()
            .map(|p| {
                let (game_name, tag_line) = if p.riot_id_game_name.is_empty() {
                    match p.riot_id.split_once('#') {
                        Some((name, tag)) => (name.to_owned(), tag.to_owned()),
                        None => (p.riot_id.clone(), String::new()),
                    }
                } else {
                    (p.riot_id_game_name, p.riot_id_tag_line)
                };
                LiveClientPlayer {
                    team_id: if p.team.eq_ignore_ascii_case("CHAOS") {
                        200
                    } else {
                        100
                    },
                    champion_alias: p
                        .raw_champion_name
                        .rsplit('_')
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                    game_name,
                    tag_line,
                    keystone_id: p.runes.keystone.id,
                    primary_style_id: p.runes.primary_rune_tree.id,
                    sub_style_id: p.runes.secondary_rune_tree.id,
                }
            })
            .collect();
        let time = raw.game_data.game_time;
        let game_time_secs = if time.is_finite() && time > 0.0 {
            // Até a partida mais longa cabe folgado num `u32`.
            time.floor().min(f64::from(u32::MAX)) as u32
        } else {
            0
        };
        Some(Self {
            game_time_secs,
            players,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn le_runas_e_tempo() {
        let body = json!({
            "activePlayer": {"riotId": "Eu#BR1"},
            "allPlayers": [
                {"championName": "Wukong", "rawChampionName": "game_character_displayname_MonkeyKing",
                 "riotId": "Eu#BR1", "riotIdGameName": "Eu", "riotIdTagLine": "BR1",
                 "team": "ORDER", "isBot": false,
                 "runes": {"keystone": {"id": 8010, "displayName": "Conquistador"},
                           "primaryRuneTree": {"id": 8000}, "secondaryRuneTree": {"id": 8400}}},
                {"championName": "Ahri", "rawChampionName": "game_character_displayname_Ahri",
                 "riotId": "Outro#000", "team": "CHAOS"}
            ],
            "gameData": {"gameMode": "CLASSIC", "gameTime": 125.73}
        });
        let data = LiveClientData::from_json(&body).unwrap();
        assert_eq!(data.game_time_secs, 125);
        assert_eq!(
            data.players[0],
            LiveClientPlayer {
                team_id: 100,
                champion_alias: "MonkeyKing".into(),
                game_name: "Eu".into(),
                tag_line: "BR1".into(),
                keystone_id: 8010,
                primary_style_id: 8000,
                sub_style_id: 8400,
            }
        );
        let other = &data.players[1];
        assert_eq!(other.team_id, 200);
        assert_eq!(
            (other.game_name.as_str(), other.tag_line.as_str()),
            ("Outro", "000")
        );
        assert_eq!(other.keystone_id, 0);
    }

    #[test]
    fn carregando_nao_tem_dados() {
        assert!(LiveClientData::from_json(&json!({"errorCode": "RESOURCE_NOT_FOUND"})).is_none());
        assert!(LiveClientData::from_json(&json!([])).is_none());
    }

    #[tokio::test]
    async fn jogo_fechado_responde_none() {
        // Porta de outro processo nenhum: a conexão é recusada na hora.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let client =
            LiveClient::with_root_certificate(&format!("https://127.0.0.1:{port}"), RIOT_ROOT_PEM)
                .unwrap();
        assert!(client.game_data().await.is_none());
    }
}
