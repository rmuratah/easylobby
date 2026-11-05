//! O cliente REST do agente (#8) contra o mock-lcu: lockfile, HTTPS com o
//! certificado fixado e Basic Auth, sem desligar a verificação TLS.

use std::path::{Path, PathBuf};

use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::lockfile::{Credentials, Protocol};
use easylobby_agent::rest::{LcuClient, LcuError};
use mock_lcu::{Config, MockLcu, ReplayStart};
use reqwest::Method;
use serde_json::{json, Value};

/// Senha de teste, obviamente falsa.
const PASSWORD: &str = "senha-falsa-do-cliente-rest";

fn config(tls: bool) -> Config {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut config = Config::new(FixtureSet::load(&dir).unwrap());
    config.password = PASSWORD.into();
    config.tls = tls;
    config.replay_start = ReplayStart::Never;
    config
}

fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("mock-lcu-rest-{name}-{}", std::process::id()))
}

/// Sobe o mock com lockfile e CA em disco e monta o cliente como o agente
/// vai montar: credenciais do lockfile e a CA do mock fixada.
async fn start_https(name: &str) -> (MockLcu, Credentials, String, PathBuf) {
    let dir = temp_dir(name);
    let mut config = config(true);
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    let mock = MockLcu::start(config).await.unwrap();
    let creds = Credentials::from_lockfile_path(&dir.join("lockfile")).unwrap();
    let ca_pem = std::fs::read_to_string(dir.join("mock-lcu-ca.pem")).unwrap();
    (mock, creds, ca_pem, dir)
}

#[tokio::test]
async fn faz_get_com_o_certificado_do_mock_fixado() {
    let (mock, creds, ca_pem, dir) = start_https("get").await;
    assert_eq!(creds.protocol, Protocol::Https);
    let lcu = LcuClient::with_root_certificate(&creds, ca_pem.as_bytes()).unwrap();
    assert_eq!(lcu.base_url(), mock.base_url());

    let phase: String = lcu.get("/lol-gameflow/v1/gameflow-phase").await.unwrap();
    assert_eq!(phase, "None");

    let summoner: Value = lcu.get("/lol-summoner/v1/current-summoner").await.unwrap();
    assert!(summoner["gameName"].as_str().unwrap().starts_with("fake-"));

    // Tipo errado vira erro de formato, não pânico.
    let err = lcu
        .get::<u64>("/lol-gameflow/v1/gameflow-phase")
        .await
        .unwrap_err();
    assert!(matches!(err, LcuError::Decode { .. }), "{err}");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn status_de_erro_vira_lcu_error_com_a_mensagem() {
    let (mock, creds, ca_pem, dir) = start_https("status").await;
    let lcu = LcuClient::with_root_certificate(&creds, ca_pem.as_bytes()).unwrap();

    // Fora de um lobby a LCU responde 404 no formato RPC_ERROR.
    let err = lcu.get::<Value>("/lol-lobby/v2/lobby").await.unwrap_err();
    assert_eq!(err.status(), Some(404), "{err}");
    let LcuError::Status { message, body, .. } = &err else {
        panic!("erro inesperado: {err}");
    };
    assert!(message.is_some(), "{body}");
    assert_eq!(body["httpStatus"], 404);

    // `request` devolve o status sem transformar em erro.
    let response = lcu
        .request(Method::GET, "/lol-lobby/v2/lobby", None)
        .await
        .unwrap();
    assert_eq!(response.status, 404);
    assert!(!response.is_success());

    // O mock só reproduz GET; outros métodos chegam lá com corpo JSON.
    let err = lcu
        .post::<_, Value>("/lol-lobby/v2/lobby", &json!({ "queueId": 430 }))
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(501), "{err}");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn post_sem_corpo_de_resposta() {
    let (mock, creds, ca_pem, dir) = start_https("post").await;
    let lcu = LcuClient::with_root_certificate(&creds, ca_pem.as_bytes()).unwrap();

    lcu.post::<_, ()>("/riotclient/kill-ux", &Value::Null)
        .await
        .unwrap();
    assert!(!mock.ux_open());
    lcu.post::<_, ()>("/riotclient/launch-ux", &Value::Null)
        .await
        .unwrap();
    assert!(mock.ux_open());

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn senha_errada_recebe_401() {
    let (mock, mut creds, ca_pem, dir) = start_https("auth").await;
    creds.password = "outra-senha-falsa".into();
    let lcu = LcuClient::with_root_certificate(&creds, ca_pem.as_bytes()).unwrap();

    let err = lcu
        .get::<Value>("/lol-gameflow/v1/gameflow-phase")
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(401), "{err}");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn certificado_que_nao_bate_e_recusado() {
    let (mock, creds, _ca_pem, dir) = start_https("pin").await;

    // O mock não tem a chave da Riot: com o `riotgames.pem` fixado, o
    // handshake falha. Prova que a verificação TLS está ligada.
    let lcu = LcuClient::new(&creds).unwrap();
    let err = lcu
        .get::<Value>("/lol-gameflow/v1/gameflow-phase")
        .await
        .unwrap_err();
    assert!(err.is_connect(), "{err}");

    // A CA de outra execução do mock também não serve.
    let other = MockLcu::start(config(true)).await.unwrap();
    let lcu = LcuClient::with_root_certificate(&creds, other.ca_pem().unwrap().as_bytes()).unwrap();
    let err = lcu
        .get::<Value>("/lol-gameflow/v1/gameflow-phase")
        .await
        .unwrap_err();
    assert!(err.is_connect(), "{err}");

    other.shutdown().await;
    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn funciona_com_lockfile_http() {
    let dir = temp_dir("http");
    let mut config = config(false);
    config.lockfile = Some(dir.join("lockfile"));
    let mock = MockLcu::start(config).await.unwrap();
    let creds = Credentials::from_lockfile_path(&dir.join("lockfile")).unwrap();
    assert_eq!(creds.protocol, Protocol::Http);

    // Sem TLS o certificado fixado não entra em jogo.
    let lcu = LcuClient::new(&creds).unwrap();
    let phase: String = lcu.get("/lol-gameflow/v1/gameflow-phase").await.unwrap();
    assert_eq!(phase, "None");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
