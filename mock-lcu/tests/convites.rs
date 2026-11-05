//! Convites de lobby (#32) pelo `/ws`, com o agente conectado no mock-lcu:
//! um amigo convida, a UI recusa e aceita e entra no lobby dele; depois, no
//! próprio lobby, a UI convida um amigo.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::invitations::Invitee;
use easylobby_agent::lobby::{InvitationState, Lobby, Queue};
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
    let dir = std::env::temp_dir().join(format!("mock-lcu-convites-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut config = Config::new(FixtureSet::load(&fixtures).unwrap());
    // Sem reprodução: o lobby só muda pelos comandos.
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

/// Lê mensagens até a resposta do comando `id`, devolvendo as de antes e o
/// resultado.
async fn until_response(ui: &mut Ui, id: u32) -> (Vec<ServerMessage>, CommandResult) {
    let mut before = Vec::new();
    loop {
        match recv(ui).await {
            ServerMessage::Response { id: got, result } if got == id => return (before, result),
            other => before.push(other),
        }
    }
}

/// Espera o agente conectar e reler o estado pelo REST.
async fn wait_summoner(ui: &mut Ui) {
    loop {
        match recv(ui).await {
            ServerMessage::Hello { state, .. } if state.summoner.is_some() => break,
            ServerMessage::Summoner { summoner: Some(_) } => break,
            _ => {}
        }
    }
}

/// Lê mensagens até a lista de convites recebidos ter `n` convites.
async fn until_invitations(ui: &mut Ui, n: usize) -> Vec<String> {
    loop {
        if let ServerMessage::ReceivedInvitations { invitations } = recv(ui).await {
            if invitations.len() == n {
                return invitations.into_iter().map(|i| i.invitation_id).collect();
            }
        }
    }
}

fn last_lobby(messages: &[ServerMessage]) -> Option<Lobby> {
    messages.iter().rev().find_map(|m| match m {
        ServerMessage::Lobby { lobby } => lobby.clone(),
        _ => None,
    })
}

fn is_error(result: &CommandResult, expected: ErrorCode) -> bool {
    matches!(result, CommandResult::Error { code, .. } if *code == expected)
}

#[tokio::test]
async fn recusa_e_aceita_convites() {
    let dir = temp_dir("recebidos");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    // Dois convites chegam pelo WAMP.
    let primeiro = mock.receive_invitation();
    assert_eq!(
        until_invitations(&mut ui, 1).await,
        std::slice::from_ref(&primeiro)
    );
    let segundo = mock.receive_invitation();
    assert_eq!(
        until_invitations(&mut ui, 2).await,
        [primeiro.clone(), segundo.clone()]
    );

    // Uma UI nova recebe os convites no hello.
    let mut late = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    match recv(&mut late).await {
        ServerMessage::Hello { state, .. } => {
            assert_eq!(state.received_invitations.len(), 2);
            let invitation = &state.received_invitations[0];
            assert_eq!(invitation.queue, Some(Queue::RankedSolo));
            assert!(invitation.can_accept);
            assert_eq!(invitation.from_summoner_id, 1003);
        }
        other => panic!("esperava hello: {other:?}"),
    }

    // Recusa o primeiro: a lista sem ele chega antes da resposta.
    send(
        &mut ui,
        1,
        Command::DeclineInvitation {
            invitation_id: primeiro.clone(),
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    assert!(
        before.iter().any(|m| matches!(
            m,
            ServerMessage::ReceivedInvitations { invitations }
                if invitations.len() == 1 && invitations[0].invitation_id == segundo
        )),
        "{before:?}"
    );

    // Recusar de novo: a LCU não acha o convite.
    send(
        &mut ui,
        2,
        Command::DeclineInvitation {
            invitation_id: primeiro,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 2).await;
    assert!(is_error(&result, ErrorCode::Lcu), "{result:?}");

    // Id que mudaria o caminho da URL: nem chega na LCU.
    send(
        &mut ui,
        3,
        Command::AcceptInvitation {
            invitation_id: "../../lobby".into(),
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 3).await;
    assert!(is_error(&result, ErrorCode::InvalidCommand), "{result:?}");

    // Aceita o segundo: entra no lobby do amigo, que é o líder.
    send(
        &mut ui,
        4,
        Command::AcceptInvitation {
            invitation_id: segundo,
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 4).await;
    assert_eq!(result, CommandResult::Ok);
    assert!(
        before.iter().any(|m| matches!(
            m,
            ServerMessage::ReceivedInvitations { invitations } if invitations.is_empty()
        )),
        "{before:?}"
    );
    let lobby = last_lobby(&before).expect("o lobby do amigo chega antes da resposta");
    assert_eq!(lobby.queue, Some(Queue::RankedSolo));
    assert!(!lobby.is_leader);
    assert!(!lobby.can_invite);
    assert_eq!(lobby.members.len(), 2);
    assert!(lobby.members[0].is_leader);
    assert_eq!(mock.phase(), "Lobby");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn convida_um_amigo() {
    let dir = temp_dir("enviados");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;
    let amigo = Invitee {
        summoner_id: 1003,
        puuid: "fake-puuid-0002".into(),
    };

    // Fora de lobby a LCU recusa.
    send(
        &mut ui,
        1,
        Command::InviteToLobby {
            invitees: vec![amigo.clone()],
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 1).await;
    assert!(is_error(&result, ErrorCode::Lcu), "{result:?}");

    // Lista vazia: nem chega na LCU.
    send(&mut ui, 2, Command::InviteToLobby { invitees: vec![] }).await;
    let (_, result) = until_response(&mut ui, 2).await;
    assert!(is_error(&result, ErrorCode::InvalidCommand), "{result:?}");

    send(
        &mut ui,
        3,
        Command::CreateLobby {
            queue: Queue::NormalDraft,
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 3).await;
    assert_eq!(result, CommandResult::Ok);
    let lobby = last_lobby(&before).unwrap();
    assert!(lobby.can_invite);
    assert!(lobby.invitations.is_empty());

    // Convida: o lobby com o convite pendente chega antes da resposta.
    send(
        &mut ui,
        4,
        Command::InviteToLobby {
            invitees: vec![amigo],
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 4).await;
    assert_eq!(result, CommandResult::Ok);
    let lobby = last_lobby(&before).expect("o lobby com o convite chega antes da resposta");
    assert_eq!(lobby.invitations.len(), 1);
    assert_eq!(lobby.invitations[0].puuid, "fake-puuid-0002");
    assert_eq!(lobby.invitations[0].summoner_id, 1003);
    assert_eq!(lobby.invitations[0].state, InvitationState::Pending);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
