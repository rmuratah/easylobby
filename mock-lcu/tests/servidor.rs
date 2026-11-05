//! Sobe o mock-lcu e conversa com ele como o agente vai conversar (#6):
//! lockfile, HTTPS com o certificado fixado, Basic Auth e WAMP.

use std::path::Path;
use std::time::Duration;

use base64::Engine;
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::lockfile::{Credentials, Protocol, LCU_USERNAME};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::{Config, MockLcu, ReplayStart};
use serde_json::Value;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::Connector;

/// Senha de teste, obviamente falsa.
const PASSWORD: &str = "senha-falsa-de-teste";

fn fixtures(name: &str) -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../agent/tests/fixtures")
        .join(name);
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mock-lcu-{name}-{}", std::process::id()))
}

fn config(name: &str) -> Config {
    let mut config = Config::new(fixtures(name));
    config.password = PASSWORD.into();
    // Rápido o bastante para a gravação do exemplo (125 s) caber em ~0,1 s.
    config.speed = 1000.0;
    config
}

fn https_client(ca_pem: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .use_native_tls()
        .tls_built_in_root_certs(false)
        .add_root_certificate(reqwest::Certificate::from_pem(ca_pem.as_bytes()).unwrap())
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
}

async fn get(client: &reqwest::Client, creds: &Credentials, path: &str) -> (u16, Value) {
    let response = client
        .get(format!("{}{path}", creds.base_url()))
        .basic_auth(LCU_USERNAME, Some(&creds.password))
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    let body = if text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap()
    };
    (status, body)
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect_wamp(creds: &Credentials, ca_pem: &str) -> Ws {
    let mut request = format!("wss://127.0.0.1:{}/", creds.port)
        .into_client_request()
        .unwrap();
    let auth = base64::engine::general_purpose::STANDARD
        .encode(format!("{LCU_USERNAME}:{}", creds.password));
    let headers = request.headers_mut();
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Basic {auth}")).unwrap(),
    );
    headers.insert("Sec-WebSocket-Protocol", HeaderValue::from_static("wamp"));
    let tls = native_tls::TlsConnector::builder()
        .disable_built_in_roots(true)
        .add_root_certificate(native_tls::Certificate::from_pem(ca_pem.as_bytes()).unwrap())
        .build()
        .unwrap();
    let (ws, response) = tokio_tungstenite::connect_async_tls_with_config(
        request,
        None,
        false,
        Some(Connector::NativeTls(tls)),
    )
    .await
    .unwrap();
    assert_eq!(
        response.headers().get("sec-websocket-protocol").unwrap(),
        "wamp"
    );
    ws
}

/// Lê eventos `[8, tópico, payload]` até achar a fase pedida.
async fn wait_phase(ws: &mut Ws, phase: &str) -> Vec<Value> {
    let mut seen = Vec::new();
    let found = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(msg) = ws.next().await {
            let Ok(Message::Text(text)) = msg else {
                continue;
            };
            let frame: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(frame[0], 8);
            let payload = frame[2].clone();
            seen.push(frame);
            if payload["uri"] == "/lol-gameflow/v1/gameflow-phase" && payload["data"] == phase {
                return true;
            }
        }
        false
    })
    .await;
    assert_eq!(found, Ok(true), "a fase {phase} não chegou");
    seen
}

#[tokio::test]
async fn agente_conecta_pelo_lockfile_e_recebe_eventos() {
    let dir = temp_dir("https");
    let lockfile = dir.join("lockfile");
    let mut config = config("exemplo");
    config.lockfile = Some(lockfile.clone());
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    let mock = MockLcu::start(config).await.unwrap();

    // O lockfile tem o formato do LoL e o leitor do agente (#4) entende.
    let creds = Credentials::from_lockfile_path(&lockfile).unwrap();
    assert_eq!(creds.port, mock.port());
    assert_eq!(creds.password, PASSWORD);
    assert_eq!(creds.protocol, Protocol::Https);
    let ca_pem = std::fs::read_to_string(dir.join("mock-lcu-ca.pem")).unwrap();
    assert_eq!(Some(ca_pem.as_str()), mock.ca_pem());

    let client = https_client(&ca_pem);

    // Sem senha: 401, como a LCU.
    let response = client
        .get(format!(
            "{}/lol-gameflow/v1/gameflow-phase",
            creds.base_url()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 401);

    // Antes da inscrição WAMP a gravação não anda.
    assert_eq!(
        get(&client, &creds, "/lol-gameflow/v1/gameflow-phase").await,
        (200, Value::from("None"))
    );
    let (status, summoner) = get(&client, &creds, "/lol-summoner/v1/current-summoner").await;
    assert_eq!(status, 200);
    assert!(summoner["gameName"].as_str().unwrap().starts_with("fake-"));
    assert_eq!(get(&client, &creds, "/lol-lobby/v2/lobby").await.0, 404);

    // kill-ux e launch-ux respondem 204.
    for (path, open) in [
        ("/riotclient/kill-ux", false),
        ("/riotclient/launch-ux", true),
    ] {
        let response = client
            .post(format!("{}{path}", creds.base_url()))
            .basic_auth(LCU_USERNAME, Some(PASSWORD))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 204);
        assert_eq!(mock.ux_open(), open);
    }

    let mut ws = connect_wamp(&creds, &ca_pem).await;
    ws.send(Message::text(r#"[5, "OnJsonApiEvent"]"#))
        .await
        .unwrap();
    let seen = wait_phase(&mut ws, "ChampSelect").await;
    assert!(seen.iter().all(|f| f[1] == "OnJsonApiEvent"));
    assert!(seen
        .iter()
        .any(|f| f[2]["uri"] == "/lol-lobby/v2/lobby" && f[2]["eventType"] == "Create"));

    // O REST acompanha os eventos.
    assert_eq!(
        get(&client, &creds, "/lol-gameflow/v1/gameflow-phase").await,
        (200, Value::from("ChampSelect"))
    );
    let (status, session) = get(&client, &creds, "/lol-champ-select/v1/session").await;
    assert_eq!(status, 200);
    assert!(session["actions"].is_array());

    drop(ws);
    mock.shutdown().await;
    // Como o LoL, o lockfile some quando o servidor para.
    assert!(!lockfile.exists());
    assert!(!dir.join("mock-lcu-ca.pem").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn http_puro_e_inscricao_por_recurso() {
    let dir = temp_dir("http");
    let lockfile = dir.join("lockfile");
    let mut config = config("exemplo");
    config.tls = false;
    config.lockfile = Some(lockfile.clone());
    let mock = MockLcu::start(config).await.unwrap();
    assert!(mock.ca_pem().is_none());

    let creds = Credentials::from_lockfile_path(&lockfile).unwrap();
    assert_eq!(creds.protocol, Protocol::Http);
    let client = reqwest::Client::new();
    assert_eq!(get(&client, &creds, "/lol-chat/v1/me").await.0, 200);

    let mut request = format!("ws://127.0.0.1:{}/", creds.port)
        .into_client_request()
        .unwrap();
    let auth = base64::engine::general_purpose::STANDARD.encode(format!("riot:{PASSWORD}"));
    request.headers_mut().insert(
        "Authorization",
        HeaderValue::from_str(&format!("Basic {auth}")).unwrap(),
    );
    let (mut ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let topic = "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase";
    ws.send(Message::text(format!("[5, \"{topic}\"]")))
        .await
        .unwrap();
    let seen = wait_phase(&mut ws, "ChampSelect").await;
    // Só os eventos do recurso inscrito.
    assert!(seen
        .iter()
        .all(|f| f[1] == topic && f[2]["uri"] == "/lol-gameflow/v1/gameflow-phase"));
    assert_eq!(
        seen.iter()
            .map(|f| f[2]["data"].clone())
            .collect::<Vec<_>>(),
        ["Lobby", "Matchmaking", "ReadyCheck", "ChampSelect"]
    );

    drop(ws);
    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn gravacao_real_do_inicio_ao_fim() {
    let mut config = config("16.22");
    config.tls = false;
    config.replay_start = ReplayStart::Immediately;
    config.speed = 10_000.0;
    let mock = MockLcu::start(config).await.unwrap();
    // 238 s de gravação a 10.000x: uns 24 ms.
    tokio::time::timeout(Duration::from_secs(10), async {
        while mock.phase() != "InProgress" {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a gravação não chegou em InProgress");
    mock.shutdown().await;
}

#[tokio::test]
async fn recusa_configuracao_invalida() {
    let mut bad_speed = config("exemplo");
    bad_speed.speed = 0.0;
    assert!(MockLcu::start(bad_speed).await.is_err());

    let mut bad_password = config("exemplo");
    bad_password.password = "com:dois-pontos".into();
    assert!(MockLcu::start(bad_password).await.is_err());
}
