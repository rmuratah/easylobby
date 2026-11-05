//! Canal interno `/ws` entre o agente e a UI (#15).
//!
//! A UI nunca fala com a LCU: ela abre um WebSocket em
//! `ws://127.0.0.1:<porta>/ws` e troca as mensagens de [`protocol`] com o
//! agente. Ao conectar, a UI recebe um `hello` com o estado inteiro
//! ([`protocol::AgentState`]) e depois só as mudanças. Os comandos vão com um
//! `id` e voltam como `response` com o mesmo `id`; as mudanças de estado
//! publicadas enquanto o comando rodava chegam antes da resposta.
//!
//! As peças:
//!
//! - [`Hub`]: o estado atual e o canal que avisa as UIs conectadas.
//!   [`drive`] o mantém atualizado a partir de uma [`LcuConnection`].
//! - [`router`] e [`serve`]: o servidor `axum` com a rota `/ws`.
//! - [`run`]: as duas coisas juntas, como o app desktop vai usar.
//!
//! O servidor só deve escutar em `127.0.0.1`. Como qualquer página aberta
//! no navegador consegue abrir um WebSocket para o localhost, o servidor
//! recusa (403) pedidos com um `Origin` fora de [`WsConfig::allowed_origins`]:
//! por padrão, só a janela do Tauri e o Vite em modo dev. Programas locais,
//! que não mandam `Origin`, passam.
//!
//! ```no_run
//! # async fn exemplo() -> std::io::Result<()> {
//! use easylobby_agent::connection::{LcuConnection, ReconnectConfig};
//! use easylobby_agent::ws::{self, WsConfig};
//!
//! let listener = tokio::net::TcpListener::bind(ws::default_addr()).await?;
//! let lcu = LcuConnection::new(ReconnectConfig::default());
//! ws::run(lcu, listener, WsConfig::default()).await
//! # }
//! ```

pub mod hub;
pub mod protocol;

use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::header::ORIGIN;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

use crate::connection::LcuConnection;
use crate::launcher::{self, LaunchOptions};

pub use hub::{drive, Hub};
use protocol::{ClientMessage, Command, CommandResult, ErrorCode, ServerMessage, PROTOCOL_VERSION};

/// Caminho do WebSocket.
pub const WS_PATH: &str = "/ws";

/// Porta padrão do canal em `127.0.0.1`.
pub const DEFAULT_PORT: u16 = 47_800;

/// Tamanho máximo de uma mensagem da UI. Os comandos são pequenos.
const MAX_MESSAGE_SIZE: usize = 64 * 1024;

/// Origens aceitas por padrão: a janela do Tauri 2 (Windows usa
/// `http(s)://tauri.localhost`, macOS e Linux `tauri://localhost`) e o Vite
/// em modo dev (`pnpm --filter easylobby-ui dev`).
pub const DEFAULT_ALLOWED_ORIGINS: [&str; 5] = [
    "tauri://localhost",
    "http://tauri.localhost",
    "https://tauri.localhost",
    "http://localhost:5173",
    "http://127.0.0.1:5173",
];

/// `127.0.0.1:47800`.
pub fn default_addr() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, DEFAULT_PORT))
}

/// Configuração do servidor `/ws`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsConfig {
    /// Valores do cabeçalho `Origin` aceitos. Pedido sem `Origin` sempre
    /// passa.
    pub allowed_origins: Vec<String>,
    /// Como o comando `launchLeague` abre o LoL.
    pub launch: LaunchOptions,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: DEFAULT_ALLOWED_ORIGINS.map(String::from).to_vec(),
            launch: LaunchOptions::default(),
        }
    }
}

impl WsConfig {
    fn origin_allowed(&self, headers: &HeaderMap) -> bool {
        match headers.get(ORIGIN) {
            None => true,
            Some(origin) => self
                .allowed_origins
                .iter()
                .any(|allowed| origin.as_bytes() == allowed.as_bytes()),
        }
    }
}

#[derive(Clone)]
struct App {
    hub: Hub,
    config: Arc<WsConfig>,
}

/// Rotas do canal: só o [`WS_PATH`].
pub fn router(hub: Hub, config: WsConfig) -> Router {
    Router::new().route(WS_PATH, get(upgrade)).with_state(App {
        hub,
        config: Arc::new(config),
    })
}

/// Serve o canal em `listener` até dar erro.
pub async fn serve(listener: TcpListener, hub: Hub, config: WsConfig) -> io::Result<()> {
    axum::serve(listener, router(hub, config)).await
}

/// Liga a conexão com o LoL ao canal: atualiza um [`Hub`] novo com a
/// `connection` e serve o `/ws` em `listener`.
pub async fn run(
    connection: LcuConnection,
    listener: TcpListener,
    config: WsConfig,
) -> io::Result<()> {
    let hub = Hub::new();
    let driver = tokio::spawn(drive(connection, hub.clone()));
    let result = serve(listener, hub, config).await;
    driver.abort();
    result
}

async fn upgrade(State(app): State<App>, headers: HeaderMap, ws: WebSocketUpgrade) -> Response {
    if !app.config.origin_allowed(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.max_message_size(MAX_MESSAGE_SIZE)
        .on_upgrade(move |socket| session(socket, app.hub, app.config))
}

/// Uma UI conectada: manda o `hello`, repassa as mudanças e responde os
/// comandos, até a UI fechar.
async fn session(mut socket: WebSocket, hub: Hub, config: Arc<WsConfig>) {
    let (state, mut changes) = hub.subscribe();
    if send(&mut socket, &hello(state)).await.is_err() {
        return;
    }
    loop {
        tokio::select! {
            change = changes.recv() => {
                let message = match change {
                    Ok(message) => message,
                    // A UI ficou para trás: em vez das mudanças perdidas, o
                    // estado inteiro de novo.
                    Err(RecvError::Lagged(_)) => {
                        let (state, fresh) = hub.subscribe();
                        changes = fresh;
                        hello(state)
                    }
                    Err(RecvError::Closed) => break,
                };
                if send(&mut socket, &message).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => {
                let text = match incoming {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    // Ping e pong o axum responde sozinho; binário não é usado.
                    Some(Ok(_)) => continue,
                };
                let replies = match serde_json::from_str::<ClientMessage>(text.as_str()) {
                    Ok(ClientMessage { id, command }) => {
                        let mut replies = Vec::new();
                        let result = match command {
                            Command::Ping => CommandResult::Ok,
                            Command::GetState => {
                                // Assina de novo para não entregar, depois do
                                // estado novo, mudanças mais velhas que ele.
                                let (state, fresh) = hub.subscribe();
                                changes = fresh;
                                replies.push(hello(state));
                                CommandResult::Ok
                            }
                            Command::LaunchLeague => launch_league(&config.launch).await,
                            Command::LaunchUx => hub.launch_ux().await,
                            Command::CreateLobby { queue } => hub.create_lobby(queue).await,
                            Command::LeaveLobby => hub.leave_lobby().await,
                        };
                        // As mudanças que o comando publicou (como o lobby
                        // novo) chegam antes da resposta.
                        loop {
                            match changes.try_recv() {
                                Ok(change) => replies.push(change),
                                Err(TryRecvError::Lagged(_)) => {
                                    let (state, fresh) = hub.subscribe();
                                    changes = fresh;
                                    replies.push(hello(state));
                                }
                                Err(TryRecvError::Empty | TryRecvError::Closed) => break,
                            }
                        }
                        replies.push(ServerMessage::Response { id, result });
                        replies
                    }
                    Err(error) => vec![ServerMessage::InvalidMessage {
                        message: error.to_string(),
                    }],
                };
                for reply in &replies {
                    if send(&mut socket, reply).await.is_err() {
                        return;
                    }
                }
            }
        }
    }
}

/// Abre o LoL e traduz o resultado para a UI. Não espera o LoL subir: isso
/// chega pelas mensagens `lcu`.
async fn launch_league(options: &LaunchOptions) -> CommandResult {
    match launcher::launch_league(options).await {
        Ok(_) => CommandResult::Ok,
        Err(e) => CommandResult::error(ErrorCode::LaunchFailed, e.to_string()),
    }
}

fn hello(state: protocol::AgentState) -> ServerMessage {
    ServerMessage::Hello {
        protocol: PROTOCOL_VERSION,
        agent_version: crate::VERSION.to_owned(),
        state,
    }
}

async fn send(socket: &mut WebSocket, message: &ServerMessage) -> Result<(), axum::Error> {
    let json = serde_json::to_string(message).expect("ServerMessage sempre vira JSON");
    socket.send(Message::text(json)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn origens_aceitas() {
        let config = WsConfig::default();
        let with = |origin: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(ORIGIN, HeaderValue::from_str(origin).unwrap());
            config.origin_allowed(&headers)
        };
        assert!(config.origin_allowed(&HeaderMap::new()));
        assert!(with("tauri://localhost"));
        assert!(with("http://tauri.localhost"));
        assert!(with("http://localhost:5173"));
        assert!(!with("http://localhost:3000"));
        assert!(!with("https://exemplo.com"));
        assert!(!with("null"));
    }
}
