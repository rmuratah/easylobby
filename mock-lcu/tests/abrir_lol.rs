//! O pedido do agente para o Riot Client abrir o LoL (#11), contra o mock-lcu
//! fazendo o papel da API local do Riot Client.

use std::path::{Path, PathBuf};

use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::launcher::{self, LaunchOptions};
use easylobby_agent::lockfile::Credentials;
use easylobby_agent::rest::LcuClient;
use mock_lcu::{Config, MockLcu, ReplayStart};

/// Senha de teste, obviamente falsa.
const PASSWORD: &str = "senha-falsa-do-riot-client";

fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("mock-lcu-abrir-lol-{name}-{}", std::process::id()))
}

async fn start(name: &str) -> (MockLcu, Credentials, String, PathBuf) {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let dir = temp_dir(name);
    let mut config = Config::new(FixtureSet::load(&fixtures).unwrap());
    config.password = PASSWORD.into();
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    let mock = MockLcu::start(config).await.unwrap();
    let creds = Credentials::from_lockfile_path(&dir.join("lockfile")).unwrap();
    let ca_pem = std::fs::read_to_string(dir.join("mock-lcu-ca.pem")).unwrap();
    (mock, creds, ca_pem, dir)
}

#[tokio::test]
async fn pede_o_lol_ao_riot_client() {
    let (mock, creds, ca_pem, dir) = start("live").await;
    let client = LcuClient::with_root_certificate(&creds, ca_pem.as_bytes()).unwrap();

    launcher::request_launch(&client, &LaunchOptions::default())
        .await
        .unwrap();
    let pbe = LaunchOptions {
        patchline: "pbe".into(),
        ..LaunchOptions::default()
    };
    launcher::request_launch(&client, &pbe).await.unwrap();

    assert_eq!(
        mock.launches(),
        ["league_of_legends/live", "league_of_legends/pbe"]
    );
    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn senha_errada_nao_abre_nada() {
    let (mock, mut creds, ca_pem, dir) = start("401").await;
    creds.password = "outra-senha-falsa".into();
    let client = LcuClient::with_root_certificate(&creds, ca_pem.as_bytes()).unwrap();

    let err = launcher::request_launch(&client, &LaunchOptions::default())
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(401), "{err}");
    assert!(mock.launches().is_empty());
    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(dir);
}
