//! Lista de amigos com presença (#30) contra o mock-lcu com a gravação
//! 16.22: a UI recebe os 21 amigos da gravação pelo `/ws`, na ordem da tela,
//! e acompanha as mudanças de presença que chegam pelo WAMP.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::friends::{Activity, Availability, Friend, LeagueStatus};
use easylobby_agent::ws::protocol::ServerMessage;
use easylobby_agent::ws::{self, WsConfig};
use futures_util::StreamExt;
use mock_lcu::{Config, MockLcu, ReplayStart};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ui = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Amigos na gravação 16.22.
const FRIEND_COUNT: usize = 21;

fn fixtures() -> FixtureSet {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    FixtureSet::load(&dir).unwrap()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-amigos-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path, replay_start: ReplayStart) -> MockLcu {
    let mut config = Config::new(fixtures());
    config.speed = 500.0;
    config.replay_start = replay_start;
    config.lockfile = Some(dir.join("lockfile"));
    config.ca_file = Some(dir.join("mock-lcu-ca.pem"));
    MockLcu::start(config).await.unwrap()
}

/// Sobe o agente (conexão com o LoL + `/ws`) numa porta livre.
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

async fn connect(url: &str) -> Ui {
    tokio_tungstenite::connect_async(url).await.unwrap().0
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

/// Espera uma lista de amigos que satisfaça `done`, vinda de uma mensagem
/// `friends` ou de um `hello`.
async fn friends_until(ui: &mut Ui, done: impl Fn(&[Friend]) -> bool) -> Vec<Friend> {
    loop {
        let list = match recv(ui).await {
            ServerMessage::Friends { friends } => friends,
            ServerMessage::Hello { state, .. } => state.friends,
            _ => continue,
        };
        if done(&list) {
            return list;
        }
    }
}

fn find<'a>(list: &'a [Friend], id: &str) -> &'a Friend {
    list.iter()
        .find(|f| f.id == id)
        .unwrap_or_else(|| panic!("{id} não está na lista"))
}

#[tokio::test]
async fn mostra_os_amigos_da_gravacao_e_segue_a_presenca() {
    let dir = temp_dir("gravacao");
    let url = start_agent(&dir).await;
    let mut ui = connect(&url).await;
    let mock = start_mock(&dir, ReplayStart::OnFirstSubscribe).await;

    let list = friends_until(&mut ui, |l| l.len() == FRIEND_COUNT).await;

    // Disponíveis primeiro; offline no fim.
    let ranks: Vec<u8> = list.iter().map(|f| f.availability.rank()).collect();
    let mut sorted = ranks.clone();
    sorted.sort();
    assert_eq!(ranks, sorted);
    assert_eq!(
        list.iter()
            .filter(|f| f.availability == Availability::Offline)
            .count(),
        12
    );
    assert!(list.iter().all(|f| !f.game_name.is_empty()));

    // No LoL, fora de partida.
    let fora = find(&list, "fake-pid-0008");
    assert_eq!(fora.availability, Availability::Away);
    assert!(matches!(
        &fora.activity,
        Some(Activity::League(a)) if a.status == LeagueStatus::OutOfGame
    ));
    // Num lobby (`hosting_KIWI`).
    assert!(matches!(
        &find(&list, "fake-pid-0013").activity,
        Some(Activity::League(a)) if a.status == LeagueStatus::InLobby
    ));
    // No celular: sem atividade.
    let celular = find(&list, "fake-pid-0003");
    assert_eq!(celular.availability, Availability::Mobile);
    assert_eq!(celular.activity, None);
    // Em outro jogo. Disponível no começo da gravação; a reprodução pode já
    // ter mudado para ocupado antes de o agente ler a lista.
    let valorant = find(&list, "fake-pid-0016");
    assert!(matches!(
        valorant.availability,
        Availability::Online | Availability::Busy
    ));
    assert_eq!(
        valorant.activity,
        Some(Activity::OtherGame {
            product_name: "VALORANT".into()
        })
    );

    // A gravação tem o amigo do VALORANT ficando ocupado.
    let list = friends_until(&mut ui, |l| {
        l.iter()
            .any(|f| f.id == "fake-pid-0016" && f.availability == Availability::Busy)
    })
    .await;
    assert_eq!(list.len(), FRIEND_COUNT);

    // Uma UI que conecta depois recebe a lista no `hello`.
    let mut late = connect(&url).await;
    match recv(&mut late).await {
        ServerMessage::Hello { state, .. } => assert_eq!(state.friends.len(), FRIEND_COUNT),
        other => panic!("esperava o hello: {other:?}"),
    }

    // O LoL fechou: lista vazia.
    mock.shutdown().await;
    friends_until(&mut ui, |l| l.is_empty()).await;
    let _ = std::fs::remove_dir_all(&dir);
}
