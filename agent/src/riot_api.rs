//! API oficial da Riot (#436): a partida em andamento de **outro** jogador,
//! que a LCU não tem.
//!
//! O agente só usa o `spectator-v5`
//! (`GET /lol/spectator/v5/active-games/by-summoner/{puuid}`), que devolve a
//! partida com as duas equipes, os campeões, os feitiços e as runas, ou 404
//! quando o jogador não está em partida. Elo, Riot ID e partidas recentes
//! continuam vindo da LCU, como na partida ao vivo do próprio jogador.
//!
//! A Riot não deixa embutir a chave no app distribuído. Por enquanto a
//! chave de desenvolvimento fica na máquina de quem testa, na variável
//! [`API_KEY_ENV`] ou no arquivo [`KEY_FILE`] da pasta de configuração do
//! easylobby, fora do git. Antes da beta, [`API_URL_ENV`] aponta para um
//! servidor nosso que guarda a chave de produto (#37). Sem chave e sem
//! servidor, o recurso fica desligado.
//!
//! A verificação TLS usa as raízes do sistema e nunca é desligada.

use std::path::PathBuf;
use std::time::Duration;

use reqwest::StatusCode;
use serde_json::Value;

/// Variável com a chave da API da Riot.
pub const API_KEY_ENV: &str = "EASYLOBBY_RIOT_API_KEY";
/// Variável com o endereço da API, com `{platform}` no lugar da plataforma
/// (`br1`). Serve para trocar a Riot pelo servidor do easylobby.
pub const API_URL_ENV: &str = "EASYLOBBY_RIOT_API_URL";
/// Endereço padrão: a API da Riot da plataforma do jogador.
pub const DEFAULT_API_URL: &str = "https://{platform}.api.riotgames.com";
/// Arquivo com a chave na pasta de configuração do easylobby.
pub const KEY_FILE: &str = "riot-api-key.txt";
/// Partida em andamento de um jogador, pelo `puuid`.
pub const ACTIVE_GAME_PREFIX: &str = "/lol/spectator/v5/active-games/by-summoner";
/// Região do client logado (`{"region": "BR", ...}`).
pub const REGION_LOCALE_PATH: &str = "/riotclient/region-locale";

/// Tempo máximo de uma chamada.
const TIMEOUT: Duration = Duration::from_secs(8);

/// Erro de uma chamada à API.
#[derive(Debug, thiserror::Error)]
pub enum RiotApiError {
    #[error("falha ao falar com a API da Riot: {0}")]
    Http(#[from] reqwest::Error),
    #[error("a API da Riot respondeu {0}")]
    Status(u16),
    #[error("resposta da API da Riot que não dá para ler")]
    InvalidBody,
}

/// Onde e com que chave chamar a API.
#[derive(Clone, PartialEq, Eq)]
pub struct RiotApiConfig {
    /// Endereço com `{platform}` (ou sem, para um servidor só).
    pub url_template: String,
    /// Chave da Riot. Vazia quando o servidor do easylobby guarda a chave.
    pub key: String,
}

// A chave nunca vai para os logs.
impl std::fmt::Debug for RiotApiConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RiotApiConfig")
            .field("url_template", &self.url_template)
            .field("key", &if self.key.is_empty() { "" } else { "***" })
            .finish()
    }
}

impl RiotApiConfig {
    /// Lê a configuração das variáveis e do arquivo da chave. `None` sem
    /// chave e sem servidor próprio: o recurso fica desligado.
    pub fn load() -> Option<Self> {
        let env = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        };
        let key = env(API_KEY_ENV).or_else(|| {
            let path = config_dir()?.join(KEY_FILE);
            std::fs::read_to_string(path)
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        });
        Self::from_parts(env(API_URL_ENV), key)
    }

    /// Junta endereço e chave. Sem endereço, o da Riot, que exige chave.
    pub fn from_parts(url: Option<String>, key: Option<String>) -> Option<Self> {
        match (url, key) {
            (Some(url), key) => Some(Self {
                url_template: url,
                key: key.unwrap_or_default(),
            }),
            (None, Some(key)) => Some(Self {
                url_template: DEFAULT_API_URL.to_owned(),
                key,
            }),
            (None, None) => None,
        }
    }

    /// Endereço base para a `platform` (`br1`), sem `/` no fim.
    pub fn base_url(&self, platform: &str) -> String {
        self.url_template
            .replace("{platform}", platform)
            .trim_end_matches('/')
            .to_owned()
    }
}

/// Pasta de configuração: `%APPDATA%\easylobby` no Windows,
/// `$XDG_CONFIG_HOME/easylobby` (ou `~/.config/easylobby`) no resto.
pub fn config_dir() -> Option<PathBuf> {
    let env_dir = |name: &str| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        return env_dir("APPDATA").map(|dir| dir.join("easylobby"));
    }
    env_dir("XDG_CONFIG_HOME")
        .map(|dir| dir.join("easylobby"))
        .or_else(|| env_dir("HOME").map(|home| home.join(".config").join("easylobby")))
}

/// Plataforma da API (`br1`) pela região do client (`BR`), de
/// [`REGION_LOCALE_PATH`].
pub fn platform_from_region(region: &str) -> Option<&'static str> {
    Some(match region.trim().to_ascii_uppercase().as_str() {
        "BR" | "BR1" => "br1",
        "NA" | "NA1" => "na1",
        "LAN" | "LA1" => "la1",
        "LAS" | "LA2" => "la2",
        "EUW" | "EUW1" => "euw1",
        "EUNE" | "EUN1" => "eun1",
        "TR" | "TR1" => "tr1",
        "RU" => "ru",
        "ME" | "ME1" => "me1",
        "OCE" | "OC1" => "oc1",
        "JP" | "JP1" => "jp1",
        "KR" => "kr",
        "PH" | "PH2" => "ph2",
        "SG" | "SG2" => "sg2",
        "TH" | "TH2" => "th2",
        "TW" | "TW2" => "tw2",
        "VN" | "VN2" => "vn2",
        _ => return None,
    })
}

/// Cliente da API da Riot.
#[derive(Debug, Clone)]
pub struct RiotApi {
    http: reqwest::Client,
    config: RiotApiConfig,
}

impl RiotApi {
    pub fn new(config: RiotApiConfig) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .use_native_tls()
            .timeout(TIMEOUT)
            .build()?;
        Ok(Self { http, config })
    }

    /// A partida em andamento de `puuid` na `platform`, como a API manda.
    /// `None` quando ele não está em partida (404).
    pub async fn active_game(
        &self,
        platform: &str,
        puuid: &str,
    ) -> Result<Option<Value>, RiotApiError> {
        let url = format!(
            "{}{ACTIVE_GAME_PREFIX}/{puuid}",
            self.config.base_url(platform)
        );
        let mut request = self.http.get(url);
        if !self.config.key.is_empty() {
            request = request.header("X-Riot-Token", &self.config.key);
        }
        let response = request.send().await?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(None),
            status if status.is_success() => {
                let bytes = response.bytes().await?;
                serde_json::from_slice(&bytes)
                    .map(Some)
                    .map_err(|_| RiotApiError::InvalidBody)
            }
            status => Err(RiotApiError::Status(status.as_u16())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuracao() {
        assert_eq!(RiotApiConfig::from_parts(None, None), None);
        let riot = RiotApiConfig::from_parts(None, Some("RGAPI-x".into())).unwrap();
        assert_eq!(riot.base_url("br1"), "https://br1.api.riotgames.com");
        let proxy = RiotApiConfig::from_parts(Some("https://api.exemplo/{platform}/".into()), None)
            .unwrap();
        assert_eq!(proxy.base_url("euw1"), "https://api.exemplo/euw1");
        assert_eq!(proxy.key, "");
        assert!(
            !format!("{riot:?}").contains("RGAPI"),
            "a chave não vai para o log"
        );
    }

    #[test]
    fn plataformas() {
        assert_eq!(platform_from_region("BR"), Some("br1"));
        assert_eq!(platform_from_region("euw"), Some("euw1"));
        assert_eq!(platform_from_region("KR"), Some("kr"));
        assert_eq!(platform_from_region("PBE"), None);
    }

    /// Servidor HTTP de uma resposta só; devolve o endereço e a requisição
    /// recebida.
    fn one_shot(
        status: &'static str,
        body: &'static str,
    ) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let task = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buf = vec![0; 4096];
            let n = socket.read(&mut buf).unwrap();
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        (format!("http://{addr}/{{platform}}"), task)
    }

    #[tokio::test]
    async fn partida_em_andamento() {
        let (url, request) = one_shot("200 OK", r#"{"gameId": 7}"#);
        let api = RiotApi::new(RiotApiConfig {
            url_template: url,
            key: "RGAPI-teste".into(),
        })
        .unwrap();
        let game = api.active_game("br1", "fake-puuid-0002").await.unwrap();
        assert_eq!(game.unwrap()["gameId"], 7);
        let request = request.join().unwrap().to_ascii_lowercase();
        assert!(request
            .starts_with("get /br1/lol/spectator/v5/active-games/by-summoner/fake-puuid-0002 "));
        assert!(request.contains("x-riot-token: rgapi-teste"));
    }

    #[tokio::test]
    async fn fora_de_partida_e_erro() {
        let (url, _) = one_shot("404 Not Found", r#"{"status": {}}"#);
        let api = RiotApi::new(RiotApiConfig {
            url_template: url,
            key: String::new(),
        })
        .unwrap();
        assert_eq!(api.active_game("br1", "p").await.unwrap(), None);
        let (url, _) = one_shot("403 Forbidden", "{}");
        let api = RiotApi::new(RiotApiConfig {
            url_template: url,
            key: String::new(),
        })
        .unwrap();
        assert!(matches!(
            api.active_game("br1", "p").await,
            Err(RiotApiError::Status(403))
        ));
    }
}
