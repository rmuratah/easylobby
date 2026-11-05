//! Cache de ícones e imagens (#29): o agente com o mock-lcu e um Data Dragon
//! e CommunityDragon falsos. A UI lê o invocador pelo `/ws` e pede o ícone
//! dele em `/assets`; o agente baixa uma vez, guarda no disco, troca de pasta
//! quando sai um patch e continua servindo sem internet.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::State;
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use easylobby_agent::assets::AssetConfig;
use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::ws::protocol::ServerMessage;
use easylobby_agent::ws::{self, WsConfig};
use futures_util::StreamExt;
use mock_lcu::{Config, MockLcu};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;

const GAME_DATA: &str = "plugins/rcp-be-lol-game-data/global/default";

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-imagens-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Data Dragon e CommunityDragon falsos no mesmo servidor. Anota cada
/// caminho pedido. A imagem devolvida diz a versão e o caminho, para o teste
/// saber de onde ela veio.
#[derive(Clone)]
struct Cdn {
    /// Versão mais nova no Data Dragon (`16.22.1`).
    latest: Arc<Mutex<String>>,
    /// A pasta da versão ainda não existe: só a `latest` (dia de patch).
    only_latest: bool,
    hits: Arc<Mutex<Vec<String>>>,
}

impl Cdn {
    fn new(latest: &str) -> Self {
        Self {
            latest: Arc::new(Mutex::new(latest.into())),
            only_latest: false,
            hits: Arc::default(),
        }
    }

    fn hits(&self, needle: &str) -> usize {
        self.hits
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.contains(needle))
            .count()
    }

    async fn start(self) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().fallback(cdn).with_state(self);
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}"), server)
    }
}

async fn cdn(State(cdn): State<Cdn>, uri: Uri) -> Response {
    let path = uri.path().to_owned();
    cdn.hits.lock().unwrap().push(path.clone());
    if path == "/api/versions.json" {
        let latest = cdn.latest.lock().unwrap().clone();
        return json(format!(r#"["{latest}","16.1.1","lolpatch_7.20"]"#));
    }
    let Some((version, rest)) = path
        .trim_start_matches('/')
        .split_once(&format!("/{GAME_DATA}/"))
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if cdn.only_latest && version != "latest" {
        return StatusCode::NOT_FOUND.into_response();
    }
    let index = |items: &str| json(items.to_owned());
    if rest.starts_with("v1/") && rest.ends_with(".json") {
        return match rest {
            "v1/champion-summary.json" => index(
                r#"[{"id":-1,"name":"None","squarePortraitPath":"/lol-game-data/assets/v1/champion-icons/-1.png"},
                    {"id":266,"name":"Aatrox","squarePortraitPath":"/lol-game-data/assets/v1/champion-icons/266.png"},
                    {"id":103,"name":"Ahri","squarePortraitPath":"/lol-game-data/assets/v1/champion-icons/103.png"}]"#,
            ),
            "v1/profile-icons.json" => {
                // Todos os ícones de invocador de 0 a 10000, como o índice real.
                let items: Vec<String> = (0..=10_000)
                    .map(|id| {
                        format!(r#"{{"id":{id},"iconPath":"/lol-game-data/assets/v1/profile-icons/{id}.jpg"}}"#)
                    })
                    .collect();
                index(&format!("[{}]", items.join(",")))
            }
            "v1/summoner-spells.json" => index(
                r#"[{"id":4,"name":"Flash","iconPath":"/lol-game-data/assets/DATA/Spells/Icons2D/Summoner_flash.png"}]"#,
            ),
            "v1/perks.json" => index(
                r#"[{"id":8005,"iconPath":"/lol-game-data/assets/v1/perk-images/Styles/Precision/PressTheAttack/PressTheAttack.png"}]"#,
            ),
            "v1/perkstyles.json" => index(
                r#"{"schemaVersion":2,"styles":[{"id":8000,"iconPath":"/lol-game-data/assets/v1/perk-images/Styles/7201_Precision.png"}]}"#,
            ),
            _ => StatusCode::NOT_FOUND.into_response(),
        };
    }
    // O CommunityDragon guarda tudo em minúsculas.
    if rest != rest.to_ascii_lowercase() {
        return StatusCode::NOT_FOUND.into_response();
    }
    format!("imagem {version} {rest}").into_response()
}

fn json(body: String) -> Response {
    ([("content-type", "application/json")], body).into_response()
}

/// Sobe o agente (conexão com o LoL + `/ws` + `/assets`) numa porta livre e
/// devolve o endereço base.
async fn start_agent(lcu_dir: Option<&Path>, assets: AssetConfig) -> SocketAddr {
    let lockfile = lcu_dir.map_or_else(
        || PathBuf::from("nao-existe/lockfile"),
        |dir| dir.join("lockfile"),
    );
    let lcu = LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(lockfile))
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let config = WsConfig {
        assets,
        ..WsConfig::default()
    };
    tokio::spawn(ws::run(lcu, listener, config));
    addr
}

fn assets_config(cache_dir: &Path, cdn: &str) -> AssetConfig {
    AssetConfig {
        cache_dir: cache_dir.to_owned(),
        ddragon_url: cdn.to_owned(),
        cdragon_url: cdn.to_owned(),
    }
}

/// `(status, content-type, corpo)` de um `GET`.
async fn get(url: &str, origin: Option<&str>) -> (u16, String, String) {
    let mut request = reqwest::Client::new().get(url);
    if let Some(origin) = origin {
        request = request.header("origin", origin);
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_owned())
        .unwrap_or_default();
    (status, content_type, response.text().await.unwrap())
}

/// Ícone do invocador que a UI recebe pelo `/ws` quando o mock conecta.
async fn summoner_icon(addr: SocketAddr) -> u32 {
    let url = format!("ws://{addr}{}", ws::WS_PATH);
    let (mut ui, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(20), ui.next())
            .await
            .expect("o agente não mandou o invocador a tempo")
            .unwrap()
            .unwrap();
        let Message::Text(text) = frame else { continue };
        let summoner = match serde_json::from_str(&text).unwrap() {
            ServerMessage::Hello { state, .. } => state.summoner,
            ServerMessage::Summoner { summoner } => summoner,
            _ => None,
        };
        if let Some(summoner) = summoner {
            return summoner.profile_icon_id;
        }
    }
}

#[tokio::test]
async fn icone_do_invocador_baixa_uma_vez_e_depois_sai_do_disco() {
    let dir = temp_dir("invocador");
    let cache = dir.join("cache");
    let cdn = Cdn::new("16.22.1");
    let (cdn_url, cdn_server) = cdn.clone().start().await;

    let mut config = Config::new(fixtures());
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    let mock = MockLcu::start(config).await.unwrap();
    let addr = start_agent(Some(&dir), assets_config(&cache, &cdn_url)).await;

    let icon = summoner_icon(addr).await;
    let url = format!("http://{addr}/assets/profileIcon/{icon}");
    let expected = format!("imagem 16.22 v1/profile-icons/{icon}.jpg");
    assert_eq!(
        get(&url, None).await,
        (200, "image/jpeg".to_owned(), expected.clone())
    );
    // A janela do Tauri pode pedir com `Origin`.
    assert_eq!(get(&url, Some("http://tauri.localhost")).await.2, expected);
    assert_eq!(cdn.hits("/api/versions.json"), 1);
    assert_eq!(cdn.hits("profile-icons.json"), 1);
    assert_eq!(cdn.hits(&format!("profile-icons/{icon}.jpg")), 1);
    assert!(cache
        .join("16.22/profileIcon")
        .join(format!("{icon}.jpg"))
        .is_file());
    assert!(cache.join("16.22/index/profile-icons.json").is_file());

    // Os outros tipos, com o caminho em minúsculas no CommunityDragon.
    let base = format!("http://{addr}/assets");
    for (path, content_type, body) in [
        (
            "champion/266",
            "image/png",
            "imagem 16.22 v1/champion-icons/266.png",
        ),
        (
            "champion/-1",
            "image/png",
            "imagem 16.22 v1/champion-icons/-1.png",
        ),
        (
            "summonerSpell/4",
            "image/png",
            "imagem 16.22 data/spells/icons2d/summoner_flash.png",
        ),
        (
            "perk/8005",
            "image/png",
            "imagem 16.22 v1/perk-images/styles/precision/presstheattack/presstheattack.png",
        ),
        (
            "perkStyle/8000",
            "image/png",
            "imagem 16.22 v1/perk-images/styles/7201_precision.png",
        ),
    ] {
        assert_eq!(
            get(&format!("{base}/{path}"), None).await,
            (200, content_type.to_owned(), body.to_owned()),
            "{path}"
        );
    }

    // Fora do índice, tipo desconhecido ou id que não é número: 404 sem
    // baixar nada.
    let before = cdn.hits.lock().unwrap().len();
    for path in [
        "champion/99999",
        "skin/1",
        "champion/abc",
        "champion/..%2F..%2Fx",
    ] {
        assert_eq!(get(&format!("{base}/{path}"), None).await.0, 404, "{path}");
    }
    assert_eq!(cdn.hits.lock().unwrap().len(), before);

    // Outra página no navegador não lê as imagens com `fetch`.
    assert_eq!(get(&url, Some("https://exemplo.com")).await.0, 403);

    // Com o CommunityDragon fora do ar, o que já está no disco continua
    // saindo.
    cdn_server.abort();
    let _ = cdn_server.await;
    assert_eq!(get(&url, None).await.2, expected);
    assert_eq!(get(&format!("{base}/champion/103"), None).await.0, 502);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn patch_novo_troca_a_pasta_e_sem_internet_usa_o_disco() {
    let dir = temp_dir("patch");
    let cache = dir.join("cache");
    std::fs::create_dir_all(cache.join("outra-coisa")).unwrap();

    // Patch 16.21.
    let cdn = Cdn::new("16.21.1");
    let (cdn_url, cdn_server) = cdn.clone().start().await;
    let addr = start_agent(None, assets_config(&cache, &cdn_url)).await;
    let aatrox = format!("http://{addr}/assets/champion/266");
    assert_eq!(
        get(&aatrox, None).await.2,
        "imagem 16.21 v1/champion-icons/266.png"
    );
    assert!(cache.join("16.21/champion/266.png").is_file());

    // Sai o 16.22: o agente que abre depois usa a pasta nova e apaga a velha.
    *cdn.latest.lock().unwrap() = "16.22.1".into();
    let addr = start_agent(None, assets_config(&cache, &cdn_url)).await;
    let aatrox = format!("http://{addr}/assets/champion/266");
    assert_eq!(
        get(&aatrox, None).await.2,
        "imagem 16.22 v1/champion-icons/266.png"
    );
    assert!(!cache.join("16.21").exists());
    assert!(cache.join("outra-coisa").is_dir());

    // Sem internet: o agente que abre agora acha a 16.22 no disco.
    cdn_server.abort();
    let _ = cdn_server.await;
    let addr = start_agent(None, assets_config(&cache, &cdn_url)).await;
    assert_eq!(
        get(&format!("http://{addr}/assets/champion/266"), None)
            .await
            .2,
        "imagem 16.22 v1/champion-icons/266.png"
    );
    assert_eq!(
        get(&format!("http://{addr}/assets/champion/103"), None)
            .await
            .0,
        502
    );

    // Sem internet e sem cache: 502.
    let empty = dir.join("vazio");
    let addr = start_agent(None, assets_config(&empty, &cdn_url)).await;
    assert_eq!(
        get(&format!("http://{addr}/assets/champion/266"), None)
            .await
            .0,
        502
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn sem_a_pasta_do_patch_no_cdragon_usa_o_latest() {
    let dir = temp_dir("latest");
    let cdn = Cdn {
        only_latest: true,
        ..Cdn::new("16.22.1")
    };
    let (cdn_url, cdn_server) = cdn.clone().start().await;
    let addr = start_agent(None, assets_config(&dir, &cdn_url)).await;

    assert_eq!(
        get(&format!("http://{addr}/assets/summonerSpell/4"), None)
            .await
            .2,
        "imagem latest data/spells/icons2d/summoner_flash.png"
    );
    assert_eq!(
        cdn.hits("/16.22/plugins/rcp-be-lol-game-data/global/default/v1/summoner-spells.json"),
        1
    );
    assert_eq!(
        cdn.hits("/latest/plugins/rcp-be-lol-game-data/global/default/v1/summoner-spells.json"),
        1
    );
    assert!(dir.join("16.22/index/summoner-spells.json").is_file());

    cdn_server.abort();
    let _ = std::fs::remove_dir_all(&dir);
}
