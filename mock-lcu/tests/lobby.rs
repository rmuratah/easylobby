//! Criar lobby e escolher fila (#17) e cargos (#18) pelo `/ws`, com o agente
//! conectado no mock-lcu: a UI cria o lobby, troca a fila, escolhe os cargos
//! e sai, e vê cada mudança chegar antes da resposta do comando.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::lobby::{Position, Queue};
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
    let dir = std::env::temp_dir().join(format!("mock-lcu-lobby-{name}-{}", std::process::id()));
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

fn lobby_queues(messages: &[ServerMessage]) -> Vec<Option<Queue>> {
    messages
        .iter()
        .filter_map(|m| match m {
            ServerMessage::Lobby { lobby } => Some(lobby.as_ref().and_then(|l| l.queue)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn cria_lobby_troca_a_fila_e_sai() {
    let dir = temp_dir("comandos");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;

    // Espera conectar e o agente reler o estado pelo REST (o invocador
    // chega junto com a releitura do lobby). A gravação começa fora de lobby.
    loop {
        match recv(&mut ui).await {
            ServerMessage::Hello { state, .. } if state.summoner.is_some() => {
                assert_eq!(state.lobby, None);
                break;
            }
            ServerMessage::Summoner { summoner: Some(_) } => break,
            _ => {}
        }
    }
    assert_eq!(mock.phase(), "None");

    // Cria o lobby: a mensagem `lobby` chega antes da resposta.
    send(
        &mut ui,
        2,
        Command::CreateLobby {
            queue: Queue::RankedSolo,
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(lobby_queues(&before), [Some(Queue::RankedSolo)]);
    let lobby = before
        .iter()
        .find_map(|m| match m {
            ServerMessage::Lobby { lobby } => lobby.clone(),
            _ => None,
        })
        .unwrap();
    assert_eq!(lobby.queue_id, 420);
    assert_eq!(lobby.max_lobby_size, 2);
    assert!(lobby.is_leader);
    assert_eq!(lobby.members.len(), 1);

    // Troca a fila.
    send(&mut ui, 3, Command::CreateLobby { queue: Queue::Aram }).await;
    let (before, result) = until_response(&mut ui, 3).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(lobby_queues(&before), [Some(Queue::Aram)]);

    assert_eq!(mock.phase(), "Lobby");

    // Uma UI nova recebe o lobby no hello.
    let mut late = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    match recv(&mut late).await {
        ServerMessage::Hello { state, .. } => {
            assert_eq!(state.lobby.and_then(|l| l.queue), Some(Queue::Aram));
        }
        other => panic!("esperava hello: {other:?}"),
    }

    // Sai do lobby.
    send(&mut ui, 4, Command::LeaveLobby).await;
    let (before, result) = until_response(&mut ui, 4).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(lobby_queues(&before), [None]);
    // A fase volta para `None` pelo WAMP (antes ou depois da resposta), e
    // nenhum lobby reaparece.
    let is_idle = |m: &ServerMessage| matches!(m, ServerMessage::GameflowPhase { phase } if phase.map(|p| p.as_str()) == Some("None"));
    let mut seen = before;
    while !seen.iter().any(is_idle) {
        seen.push(recv(&mut ui).await);
    }
    assert_eq!(lobby_queues(&seen), [None], "{seen:?}");
    assert_eq!(mock.phase(), "None");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
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

fn is_error(result: &CommandResult, expected: ErrorCode) -> bool {
    matches!(result, CommandResult::Error { code, .. } if *code == expected)
}

#[tokio::test]
async fn escolhe_os_cargos() {
    let dir = temp_dir("cargos");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    // Fora de lobby a LCU recusa.
    let meio_selva = Command::SetPositions {
        first: Position::Middle,
        second: Position::Jungle,
    };
    send(&mut ui, 1, meio_selva.clone()).await;
    let (_, result) = until_response(&mut ui, 1).await;
    assert!(is_error(&result, ErrorCode::Lcu), "{result:?}");

    send(
        &mut ui,
        2,
        Command::CreateLobby {
            queue: Queue::RankedSolo,
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    let lobby = before
        .iter()
        .find_map(|m| match m {
            ServerMessage::Lobby { lobby } => lobby.clone(),
            _ => None,
        })
        .unwrap();
    assert!(lobby.show_position_selector);
    assert_eq!(lobby.members[0].first_position, Position::Unselected);

    // Escolhe meio e selva: o lobby com os cargos chega antes da resposta.
    send(&mut ui, 3, meio_selva).await;
    let (before, result) = until_response(&mut ui, 3).await;
    assert_eq!(result, CommandResult::Ok);
    let lobby = before
        .iter()
        .rev()
        .find_map(|m| match m {
            ServerMessage::Lobby { lobby } => lobby.clone(),
            _ => None,
        })
        .expect("o lobby com os cargos chega antes da resposta");
    let me = &lobby.members[0];
    assert_eq!(
        (me.first_position, me.second_position),
        (Position::Middle, Position::Jungle)
    );

    // Par inválido: o agente recusa sem mudar nada.
    send(
        &mut ui,
        4,
        Command::SetPositions {
            first: Position::Fill,
            second: Position::Top,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 4).await;
    assert!(is_error(&result, ErrorCode::InvalidCommand), "{result:?}");

    // Preenchimento sozinho vale.
    send(
        &mut ui,
        5,
        Command::SetPositions {
            first: Position::Fill,
            second: Position::Unselected,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 5).await;
    assert_eq!(result, CommandResult::Ok);
    let me = &mock_lobby(&url).await.members[0];
    assert_eq!(me.first_position, Position::Fill);

    // No ARAM não há cargos: a LCU recusa.
    send(&mut ui, 6, Command::CreateLobby { queue: Queue::Aram }).await;
    let (before, result) = until_response(&mut ui, 6).await;
    assert_eq!(result, CommandResult::Ok);
    assert!(before.iter().any(|m| matches!(
        m,
        ServerMessage::Lobby { lobby: Some(l) } if !l.show_position_selector
    )));
    send(
        &mut ui,
        7,
        Command::SetPositions {
            first: Position::Top,
            second: Position::Utility,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 7).await;
    assert!(is_error(&result, ErrorCode::Lcu), "{result:?}");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn abre_e_fecha_o_lobby() {
    let dir = temp_dir("aberto");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    // Fora de lobby o agente recusa sem falar com a LCU.
    send(&mut ui, 1, Command::SetLobbyOpen { open: true }).await;
    let (_, result) = until_response(&mut ui, 1).await;
    assert!(is_error(&result, ErrorCode::InvalidCommand), "{result:?}");

    send(
        &mut ui,
        2,
        Command::CreateLobby {
            queue: Queue::NormalDraft,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    assert!(!mock_lobby(&url).await.is_open);

    for (id, open) in [(3, true), (4, false)] {
        send(&mut ui, id, Command::SetLobbyOpen { open }).await;
        let (before, result) = until_response(&mut ui, id).await;
        assert_eq!(result, CommandResult::Ok);
        let lobby = before
            .iter()
            .rev()
            .find_map(|m| match m {
                ServerMessage::Lobby { lobby } => lobby.clone(),
                _ => None,
            })
            .expect("o lobby novo chega antes da resposta");
        assert_eq!(lobby.is_open, open);
    }

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn lider_remove_e_passa_a_lideranca() {
    let dir = temp_dir("membros");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    send(
        &mut ui,
        1,
        Command::CreateLobby {
            queue: Queue::NormalDraft,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);

    // Dois amigos entram; o agente precisa ver os dois antes do comando.
    let first = mock.join_lobby_member().expect("o primeiro amigo entra");
    let second = mock.join_lobby_member().expect("o segundo amigo entra");
    loop {
        if let ServerMessage::Lobby { lobby: Some(lobby) } = recv(&mut ui).await {
            if lobby.members.len() == 3 {
                break;
            }
        }
    }

    // Remover: o amigo some da lista antes da resposta.
    send(&mut ui, 2, Command::KickLobbyMember { summoner_id: first }).await;
    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    let lobby = last_lobby(&before);
    assert_eq!(lobby.members.len(), 2);
    assert!(lobby.members.iter().all(|m| m.summoner_id != first));
    assert!(lobby.is_leader);

    // Quem não está mais no lobby é recusado sem falar com a LCU.
    send(
        &mut ui,
        3,
        Command::PromoteLobbyMember { summoner_id: first },
    )
    .await;
    let (_, result) = until_response(&mut ui, 3).await;
    assert!(is_error(&result, ErrorCode::InvalidCommand), "{result:?}");

    // Passar a liderança: o jogador logado vira membro.
    send(
        &mut ui,
        4,
        Command::PromoteLobbyMember {
            summoner_id: second,
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 4).await;
    assert_eq!(result, CommandResult::Ok);
    let lobby = last_lobby(&before);
    assert!(!lobby.is_leader);
    let leader = lobby.members.iter().find(|m| m.is_leader).unwrap();
    assert_eq!(leader.summoner_id, second);

    // Sem ser mais o líder, o agente recusa.
    send(
        &mut ui,
        5,
        Command::KickLobbyMember {
            summoner_id: second,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 5).await;
    assert!(is_error(&result, ErrorCode::InvalidCommand), "{result:?}");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// O último lobby antes da resposta.
fn last_lobby(messages: &[ServerMessage]) -> easylobby_agent::lobby::Lobby {
    messages
        .iter()
        .rev()
        .find_map(|m| match m {
            ServerMessage::Lobby { lobby } => lobby.clone(),
            _ => None,
        })
        .expect("o lobby novo chega antes da resposta")
}

#[tokio::test]
async fn modo_de_treino_com_equipes_e_bots() {
    use easylobby_agent::custom_lobby::{BotDifficulty, CustomTeam};
    let dir = temp_dir("treino");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    // Abre o Modo de Treino: fica no lobby, com o jogador na Equipe 1.
    send(&mut ui, 1, Command::StartPracticeTool).await;
    let (before, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    let custom = last_lobby(&before).custom.expect("personalizada");
    assert_eq!(custom.team_size, 5);
    assert_eq!(custom.team1.len(), 1);
    assert!(custom.team2.is_empty());
    assert_eq!(mock.phase(), "Lobby");

    // Adiciona um bot na Equipe 2.
    send(
        &mut ui,
        2,
        Command::AddBot {
            team: CustomTeam::Team2,
            champion_id: 22,
            difficulty: BotDifficulty::Beginner,
            position: Position::Bottom,
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 2).await;
    assert_eq!(result, CommandResult::Ok);
    let custom = last_lobby(&before).custom.unwrap();
    let bot = custom.team2[0].bot.clone().unwrap();
    assert_eq!(
        (bot.champion_id, bot.position, bot.difficulty),
        (22, Position::Bottom, BotDifficulty::Beginner)
    );

    // Muda o bot.
    send(
        &mut ui,
        3,
        Command::UpdateBot {
            bot_uuid: bot.uuid.clone(),
            champion_id: 1,
            difficulty: BotDifficulty::Intermediate,
            position: Position::Middle,
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 3).await;
    assert_eq!(result, CommandResult::Ok);
    let changed = last_lobby(&before).custom.unwrap().team2[0]
        .bot
        .clone()
        .unwrap();
    assert_eq!(
        (changed.champion_id, changed.position),
        (1, Position::Middle)
    );

    // "Juntar-se" à Equipe 2.
    send(
        &mut ui,
        4,
        Command::SwitchTeam {
            team: CustomTeam::Team2,
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 4).await;
    assert_eq!(result, CommandResult::Ok);
    let custom = last_lobby(&before).custom.unwrap();
    assert!(custom.team1.is_empty());
    assert_eq!(custom.team2.len(), 2);

    // Tira o bot.
    send(
        &mut ui,
        5,
        Command::RemoveBot {
            bot_uuid: bot.uuid.clone(),
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 5).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(last_lobby(&before).custom.unwrap().team2.len(), 1);

    // Inicia a partida.
    send(&mut ui, 6, Command::StartCustomGame).await;
    let (_, result) = until_response(&mut ui, 6).await;
    assert_eq!(result, CommandResult::Ok);

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

/// O lobby que uma UI nova recebe no `hello`.
async fn mock_lobby(url: &str) -> easylobby_agent::lobby::Lobby {
    let mut ui = tokio_tungstenite::connect_async(url).await.unwrap().0;
    match recv(&mut ui).await {
        ServerMessage::Hello { state, .. } => state.lobby.expect("sem lobby no hello"),
        other => panic!("esperava hello: {other:?}"),
    }
}

#[tokio::test]
async fn sem_lol_responde_not_connected() {
    let dir = temp_dir("sem-lol");
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    send(
        &mut ui,
        1,
        Command::CreateLobby {
            queue: Queue::NormalDraft,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 1).await;
    assert!(matches!(
        result,
        CommandResult::Error {
            code: ErrorCode::NotConnected,
            ..
        }
    ));
}

#[tokio::test]
async fn troca_de_vaga_no_lobby_da_arena() {
    use easylobby_agent::lobby::{self, SubteamLayout, SubteamSlot};
    use easylobby_agent::lockfile::Credentials;
    use easylobby_agent::rest::LcuClient;
    let dir = temp_dir("arena");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    // A Arena 3x6 vem fechada na gravação, então o agente não cria o lobby
    // nela; o teste cria direto na LCU falsa, como o client oficial faria.
    let creds = Credentials::from_lockfile_path(&dir.join("lockfile")).unwrap();
    let ca_pem = std::fs::read_to_string(dir.join("mock-lcu-ca.pem")).unwrap();
    let lcu = LcuClient::with_root_certificate(&creds, ca_pem.as_bytes()).unwrap();
    lobby::create_lobby_by_id(&lcu, 1750).await.unwrap();
    let arena = loop {
        if let ServerMessage::Lobby { lobby: Some(lobby) } = recv(&mut ui).await {
            break lobby;
        }
    };
    assert_eq!(arena.subteams, Some(SubteamLayout { count: 6, size: 3 }));
    assert_eq!(arena.max_lobby_size, 18);
    assert_eq!(
        arena.members[0].subteam,
        Some(SubteamSlot {
            team: 1,
            position: 1
        })
    );

    send(
        &mut ui,
        1,
        Command::SetSubteamSlot {
            team: 4,
            position: 2,
        },
    )
    .await;
    let (before, result) = until_response(&mut ui, 1).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(
        last_lobby(&before).members[0].subteam,
        Some(SubteamSlot {
            team: 4,
            position: 2
        })
    );

    // Uma vaga que não existe: a LCU recusa.
    send(
        &mut ui,
        2,
        Command::SetSubteamSlot {
            team: 7,
            position: 1,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 2).await;
    assert!(is_error(&result, ErrorCode::Lcu), "{result:?}");

    // Numa fila sem equipes o agente recusa sem falar com a LCU.
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
    let normal = last_lobby(&before);
    assert_eq!(normal.subteams, None);
    assert!(normal.members.iter().all(|m| m.subteam.is_none()));
    send(
        &mut ui,
        4,
        Command::SetSubteamSlot {
            team: 1,
            position: 1,
        },
    )
    .await;
    let (_, result) = until_response(&mut ui, 4).await;
    assert!(is_error(&result, ErrorCode::InvalidCommand), "{result:?}");

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn gera_o_link_do_lobby_e_ele_some_ao_sair() {
    let dir = temp_dir("link");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_summoner(&mut ui).await;

    // Fora de lobby o agente recusa sem falar com a LCU.
    send(&mut ui, 1, Command::CreateLobbyLink).await;
    let (_, result) = until_response(&mut ui, 1).await;
    assert!(is_error(&result, ErrorCode::InvalidCommand), "{result:?}");

    send(
        &mut ui,
        2,
        Command::CreateLobby {
            queue: Queue::NormalDraft,
        },
    )
    .await;
    assert_eq!(until_response(&mut ui, 2).await.1, CommandResult::Ok);

    // O link chega antes da resposta.
    send(&mut ui, 3, Command::CreateLobbyLink).await;
    let (before, result) = until_response(&mut ui, 3).await;
    assert_eq!(result, CommandResult::Ok);
    let link = before
        .iter()
        .find_map(|m| match m {
            ServerMessage::LobbyLink { lobby_link } => lobby_link.clone(),
            _ => None,
        })
        .expect("o link chega antes da resposta");
    assert!(
        link.url
            .starts_with("https://gg.riotgames.com/LOL?joinCode="),
        "{link:?}"
    );
    assert!(link.url.ends_with(&link.join_code));
    assert!(link.expires_at.is_some());

    // Sair do lobby leva o link junto.
    send(&mut ui, 4, Command::LeaveLobby).await;
    let (mut seen, result) = until_response(&mut ui, 4).await;
    assert_eq!(result, CommandResult::Ok);
    let cleared = |m: &ServerMessage| matches!(m, ServerMessage::LobbyLink { lobby_link: None });
    while !seen.iter().any(cleared) {
        seen.push(recv(&mut ui).await);
    }

    mock.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
