//! Páginas de runas (#26) pelo `/ws`, com o agente conectado no mock-lcu: a
//! UI recebe as páginas e as árvores, aplica outra página, cria, edita e
//! apaga, e vê cada mudança chegar antes da resposta do comando.

use std::path::{Path, PathBuf};
use std::time::Duration;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::fixtures::FixtureSet;
use easylobby_agent::runes::{RuneCatalog, RunePageInput, RunePages, RuneSlotKind};
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

/// Páginas da gravação 16.22: a em uso no começo e outra.
const CURRENT: u64 = 994_528_921;
const OTHER: u64 = 156_827_123;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mock-lcu-runas-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

async fn start_mock(dir: &Path) -> MockLcu {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures/16.22");
    let mut config = Config::new(FixtureSet::load(&fixtures).unwrap());
    // Sem reprodução: as páginas só mudam pelos comandos.
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

/// Manda o comando e lê até a resposta. Devolve a última lista de páginas
/// que chegou antes dela e o resultado.
async fn run(ui: &mut Ui, id: u32, command: Command) -> (Option<RunePages>, CommandResult) {
    send(ui, id, command).await;
    let mut pages = None;
    loop {
        match recv(ui).await {
            ServerMessage::Response { id: got, result } if got == id => return (pages, result),
            ServerMessage::RunePages { rune_pages } => pages = rune_pages,
            _ => {}
        }
    }
}

/// Espera o agente conectar e ler as árvores e as páginas.
async fn wait_runes(ui: &mut Ui) -> (RuneCatalog, RunePages) {
    let mut catalog = None;
    loop {
        match recv(ui).await {
            ServerMessage::Hello { state, .. } => {
                if let (Some(c), Some(p)) = (&state.rune_catalog, &state.rune_pages) {
                    return (c.clone(), p.clone());
                }
            }
            ServerMessage::RuneCatalog { rune_catalog } => catalog = rune_catalog,
            ServerMessage::RunePages {
                rune_pages: Some(pages),
            } => {
                let catalog = catalog.expect("as árvores chegam antes das páginas");
                return (catalog, pages);
            }
            _ => {}
        }
    }
}

fn is_error(result: &CommandResult, expected: ErrorCode) -> bool {
    matches!(result, CommandResult::Error { code, .. } if *code == expected)
}

/// Página válida nas árvores sintéticas do mock: Dominação com Precisão.
fn page(name: &str) -> RunePageInput {
    RunePageInput {
        name: name.into(),
        primary_style_id: 8100,
        sub_style_id: 8000,
        selected_perk_ids: vec![8112, 8126, 8137, 8135, 9111, 8017, 5008, 5008, 5011],
    }
}

#[tokio::test]
async fn lista_aplica_cria_edita_e_apaga() {
    let dir = temp_dir("comandos");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;

    let (catalog, pages) = wait_runes(&mut ui).await;
    assert_eq!(catalog.styles.len(), 5);
    let precision = catalog.style(8000).unwrap();
    assert_eq!(precision.slots[0].kind, RuneSlotKind::Keystone);
    assert!(catalog.perks.iter().any(|p| p.name == "Pressione o Ataque"));
    assert_eq!(pages.pages.len(), 3);
    assert_eq!(pages.current_page_id, Some(CURRENT));
    assert!(pages.can_create_page);

    // Aplicar outra página.
    let (pages, result) = run(&mut ui, 1, Command::SelectRunePage { page_id: OTHER }).await;
    assert_eq!(result, CommandResult::Ok);
    assert_eq!(pages.unwrap().current_page_id, Some(OTHER));

    // Página fora das regras não chega na LCU.
    let mut invalid = page("Inválida");
    invalid.selected_perk_ids.swap(0, 1);
    let (pages, result) = run(&mut ui, 2, Command::CreateRunePage { page: invalid }).await;
    assert!(is_error(&result, ErrorCode::InvalidCommand), "{result:?}");
    assert!(pages.is_none());

    // Criar: a página nova entra na lista e passa a ser a em uso.
    let (pages, result) = run(&mut ui, 3, Command::CreateRunePage { page: page("Nova") }).await;
    assert_eq!(result, CommandResult::Ok);
    let pages = pages.unwrap();
    assert_eq!(pages.pages.len(), 4);
    let created = pages.pages.iter().find(|p| p.name == "Nova").unwrap();
    assert_eq!(pages.current_page_id, Some(created.id));
    assert_eq!(created.primary_style_id, 8100);
    assert_eq!(created.selected_perk_ids, page("Nova").selected_perk_ids);
    let created = created.id;

    // Editar: troca nome e runas, e continua em uso.
    let mut edited = page("Editada");
    edited.selected_perk_ids[0] = 8128;
    let (pages, result) = run(
        &mut ui,
        4,
        Command::UpdateRunePage {
            page_id: created,
            page: edited,
        },
    )
    .await;
    assert_eq!(result, CommandResult::Ok);
    let pages = pages.unwrap();
    let page_now = pages.page(created).unwrap();
    assert_eq!(page_now.name, "Editada");
    assert_eq!(page_now.selected_perk_ids[0], 8128);
    assert_eq!(pages.current_page_id, Some(created));

    // Editar ou aplicar uma página que não existe: a LCU recusa.
    let (_, result) = run(
        &mut ui,
        5,
        Command::UpdateRunePage {
            page_id: 1,
            page: page("x"),
        },
    )
    .await;
    assert!(is_error(&result, ErrorCode::Lcu), "{result:?}");
    let (_, result) = run(&mut ui, 6, Command::SelectRunePage { page_id: 1 }).await;
    assert!(is_error(&result, ErrorCode::Lcu), "{result:?}");

    // Apagar a em uso.
    let (pages, result) = run(&mut ui, 7, Command::DeleteRunePage { page_id: created }).await;
    assert_eq!(result, CommandResult::Ok);
    let pages = pages.unwrap();
    assert!(pages.page(created).is_none());
    assert_eq!(pages.pages.len(), 3);
    assert_ne!(pages.current_page_id, Some(created));

    mock.shutdown().await;
}

#[tokio::test]
async fn sem_espaco_a_lcu_recusa_criar() {
    let dir = temp_dir("sem-espaco");
    let mock = start_mock(&dir).await;
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    wait_runes(&mut ui).await;

    // O mock dá 5 páginas; a gravação já tem 3. Depois da quinta, o
    // agente avisa que não cabe mais.
    let mut last = None;
    for (id, name) in [(1, "Quarta"), (2, "Quinta")] {
        let (pages, result) = run(&mut ui, id, Command::CreateRunePage { page: page(name) }).await;
        assert_eq!(result, CommandResult::Ok);
        last = pages;
    }
    assert!(!last.unwrap().can_create_page);
    let (_, result) = run(
        &mut ui,
        4,
        Command::CreateRunePage {
            page: page("Sexta"),
        },
    )
    .await;
    assert!(is_error(&result, ErrorCode::Lcu), "{result:?}");

    mock.shutdown().await;
}

#[tokio::test]
async fn sem_lol_responde_not_connected() {
    let dir = temp_dir("sem-lol");
    let url = start_agent(&dir).await;
    let mut ui = tokio_tungstenite::connect_async(&url).await.unwrap().0;
    let ServerMessage::Hello { state, .. } = recv(&mut ui).await else {
        panic!("a primeira mensagem é o hello");
    };
    assert_eq!(state.rune_pages, None);
    assert_eq!(state.rune_catalog, None);
    let (_, result) = run(&mut ui, 1, Command::SelectRunePage { page_id: CURRENT }).await;
    assert!(is_error(&result, ErrorCode::NotConnected), "{result:?}");
    let (_, result) = run(&mut ui, 2, Command::CreateRunePage { page: page("x") }).await;
    assert!(is_error(&result, ErrorCode::NotConnected), "{result:?}");
}
