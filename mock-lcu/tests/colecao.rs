//! Coleção de campeões e skins (#369) pelo `/ws`, com o agente conectado no
//! mock-lcu e o conjunto `exemplo` (coleção sintética): a UI abre a aba e
//! recebe os campeões com as contagens; abre um campeão e recebe as skins
//! com os cromas.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::ws::protocol::{
    ClientMessage, Command, CommandResult, ErrorCode, ServerMessage,
};
use easylobby_agent::ws::{self, WsConfig};
use futures_util::{SinkExt, StreamExt};
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-colecao-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/exemplo");
    let mut config = Config::new(FixtureSet::load(&fixtures).unwrap());
    config.replay_start = ReplayStart::Never;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

async fn start_agent(dir: &Path) -> String {
    let lcu = LcuConnection::new(ReconnectConfig {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        ..ReconnectConfig::with_source(CredentialSource::Lockfile(dir.join("lockfile")))
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}{}", listener.local_addr().unwrap(), ws::WS_PATH);
    tokio::spawn(ws::run(lcu, listener, WsConfig::default()));
    url
}

async fn recv(ui: &mut Ui) -> ServerMessage {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(20), ui.next())
            .await
            .expect("o agente não mandou nada a tempo")
            .expect("o agente fechou o /ws")
            .unwrap();
        if let Message::Text(text) = frame {
            return serde_json::from_str(&text).unwrap();
        }
    }
}

async fn send(ui: &mut Ui, id: u32, command: Command) {
    let json = serde_json::to_string(&ClientMessage { id, command }).unwrap();
    ui.send(Message::text(json)).await.unwrap();
}

async fn until_response(ui: &mut Ui, id: u32) -> (Vec<ServerMessage>, CommandResult) {
    let mut before = Vec::new();
    loop {
        match recv(ui).await {
            ServerMessage::Response { id: got, result } if got == id => return (before, result),
            other => before.push(other),
        }
    }
}

async fn wait_summoner(ui: &mut Ui) {
    loop {
        match recv(ui).await {
            ServerMessage::Hello { state, .. } if state.summoner.is_some() => break,
            ServerMessage::Summoner { summoner: Some(_) } => break,
            _ => {}
        }
    }
}

#[tokio::test]
async fn abre_a_colecao_e_as_skins_de_um_campeao() {
    let dir = temp_dir("abre");
    let _mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    send(&mut ui, 1, Command::OpenCollection).await;
    let (before, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    let collection = before
        .into_iter()
        .find_map(|m| match m {
            ServerMessage::Collection {
                collection: Some(c),
            } => Some(c),
            _ => None,
        })
        .expect("a coleção chega antes da resposta");
    assert!(collection.total_champions > 150);
    assert!(collection.owned_champions > 0);
    assert!(collection.owned_champions < collection.total_champions);
    assert_eq!(collection.total_skins, collection.total_champions * 3);
    let aatrox = collection
        .champions
        .iter()
        .find(|c| c.id == 266)
        .expect("Aatrox na coleção");
    assert!(aatrox.owned);
    assert_eq!(aatrox.total_skins, 3);

    send(&mut ui, 2, Command::OpenChampionSkins { champion_id: 266 }).await;
    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    let skins = before
        .into_iter()
        .find_map(|m| match m {
            ServerMessage::ChampionSkins {
                champion_skins: Some(s),
            } => Some(s),
            _ => None,
        })
        .expect("as skins chegam antes da resposta");
    assert_eq!(skins.champion_id, 266);
    assert_eq!(skins.skins.len(), 4);
    assert!(skins.skins[0].is_base && skins.skins[0].owned);
    assert_eq!(skins.skins[2].chromas.len(), 3);

    // Campeão que não existe: a LCU responde 404.
    send(
        &mut ui,
        3,
        Command::OpenChampionSkins {
            champion_id: 999_999,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 3).await;
    assert!(matches!(
        result,
        CommandResult::Error {
            code: ErrorCode::Lcu,
            ..
        }
    ));
}
