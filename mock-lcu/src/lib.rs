//! mock-lcu: servidor falso da LCU para desenvolver sem abrir o LoL (#6).
//!
//! Responde aos `GET` com as respostas gravadas em
//! `agent/tests/fixtures/<patch>/` (#5) e reproduz os eventos WAMP no
//! WebSocket, no mesmo ritmo da gravação (ou mais rápido). Como a LCU real:
//!
//! - escuta em `127.0.0.1`, numa porta aleatória por padrão;
//! - REST e WebSocket na mesma porta, com Basic Auth `riot:<senha>`;
//! - escreve um `lockfile` no formato `nome:pid:porta:senha:protocolo`, que o
//!   agente lê com [`Credentials::from_lockfile_path`];
//! - HTTPS por padrão, com um certificado gerado na hora (veja [`tls`]).
//!
//! Módulos:
//!
//! - [`state`]: que resposta cada `GET` recebe em cada momento.
//! - [`lobby`]: criar lobby, trocar a fila e sair (#17).
//! - [`matchmaking`]: entrar e sair da fila, com o tempo na fila (#19).
//! - [`wamp`]: mensagens do WebSocket WAMP.
//! - [`tls`]: CA e certificado gerados a cada execução.
//!
//! [`Credentials::from_lockfile_path`]: easylobby_agent::lockfile::Credentials::from_lockfile_path

pub mod lobby;
pub mod matchmaking;
pub mod state;
pub mod tls;
pub mod wamp;

use std::collections::HashSet;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use axum::body::Body;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response as HttpResponse};
use axum::Router;
use base64::Engine;
use easylobby_agent::fixtures::{EventFixture, FixtureSet};
use easylobby_agent::lockfile::LCU_USERNAME;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::{broadcast, watch, Notify};
use tokio::task::JoinHandle;

use crate::state::{LcuState, Response, GAMEFLOW_PHASE};

/// Nome do processo escrito no lockfile, o mesmo do LoL.
pub const LOCKFILE_PROCESS_NAME: &str = "LeagueClient";

/// Quando a reprodução dos eventos começa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplayStart {
    /// Na primeira inscrição WAMP, para o cliente não perder nada.
    #[default]
    OnFirstSubscribe,
    /// Assim que o servidor sobe.
    Immediately,
    /// Nunca: o estado fica parado no começo da gravação.
    Never,
}

/// Configuração do servidor.
#[derive(Debug, Clone)]
pub struct Config {
    pub fixtures: FixtureSet,
    /// Porta em `127.0.0.1`. `0` escolhe uma livre.
    pub port: u16,
    /// Senha do Basic Auth. Gere com [`random_password`].
    pub password: String,
    /// HTTPS com certificado gerado (padrão) ou HTTP puro.
    pub tls: bool,
    /// Onde escrever o lockfile. Apagado quando o servidor para.
    pub lockfile: Option<PathBuf>,
    /// Onde escrever o PEM da CA do mock (só com `tls`). Apagado quando o
    /// servidor para.
    pub ca_file: Option<PathBuf>,
    /// Multiplica a velocidade da reprodução (`2.0` = duas vezes mais rápido).
    pub speed: f64,
    pub replay_start: ReplayStart,
    /// Recomeça a gravação quando ela acaba.
    pub repeat: bool,
    /// Escreve cada requisição no stdout.
    pub verbose: bool,
}

impl Config {
    /// Configuração padrão para um conjunto: porta livre, senha aleatória,
    /// HTTPS, velocidade normal, sem arquivos.
    pub fn new(fixtures: FixtureSet) -> Self {
        Self {
            fixtures,
            port: 0,
            password: random_password(),
            tls: true,
            lockfile: None,
            ca_file: None,
            speed: 1.0,
            replay_start: ReplayStart::default(),
            repeat: false,
            verbose: false,
        }
    }
}

/// Erro ao subir o mock.
#[derive(Debug, Error)]
pub enum MockError {
    #[error("velocidade {0} inválida: use um número maior que zero")]
    InvalidSpeed(f64),
    #[error("a senha não pode ser vazia nem ter ':'")]
    InvalidPassword,
    #[error("não foi possível abrir a porta {port}: {source}")]
    Bind { port: u16, source: io::Error },
    #[error("não foi possível escrever {}: {source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("falha ao gerar o certificado: {0}")]
    Tls(String),
}

/// Senha aleatória como a do LoL: 16 bytes em base64url, 22 caracteres.
pub fn random_password() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("o sistema não forneceu bytes aleatórios");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Conteúdo do lockfile: `LeagueClient:<pid>:<porta>:<senha>:<protocolo>`.
pub fn lockfile_content(port: u16, password: &str, tls: bool) -> String {
    let protocol = if tls { "https" } else { "http" };
    format!(
        "{LOCKFILE_PROCESS_NAME}:{}:{port}:{password}:{protocol}",
        std::process::id()
    )
}

struct Shared {
    state: Mutex<LcuState>,
    events: broadcast::Sender<EventFixture>,
    start: Notify,
    /// Vira `true` quando o servidor para; derruba os WebSockets abertos,
    /// como o LoL fechando.
    stopped: watch::Sender<bool>,
    /// Valor esperado do cabeçalho `Authorization`.
    authorization: String,
    verbose: bool,
    /// Velocidade da reprodução; também acelera o relógio da fila.
    speed: f64,
    /// Soma um segundo na fila a cada segundo, enquanto procura (#19).
    queue_timer: Mutex<Option<JoinHandle<()>>>,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, LcuState> {
        // Um pânico com o lock na mão não deixa o estado pela metade: cada
        // operação troca valores inteiros.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Um mock-lcu rodando. Para quando é derrubado ou com [`MockLcu::shutdown`],
/// e apaga o lockfile e o PEM da CA.
pub struct MockLcu {
    addr: SocketAddr,
    password: String,
    ca_pem: Option<String>,
    tls: bool,
    shared: Arc<Shared>,
    handle: axum_server::Handle,
    server: Option<JoinHandle<io::Result<()>>>,
    replay: JoinHandle<()>,
    files: Vec<PathBuf>,
}

impl MockLcu {
    /// Sobe o servidor e começa a aceitar conexões.
    pub async fn start(config: Config) -> Result<Self, MockError> {
        if !(config.speed.is_finite() && config.speed > 0.0) {
            return Err(MockError::InvalidSpeed(config.speed));
        }
        if config.password.is_empty() || config.password.contains(':') {
            return Err(MockError::InvalidPassword);
        }

        let listener =
            std::net::TcpListener::bind(("127.0.0.1", config.port)).map_err(|source| {
                MockError::Bind {
                    port: config.port,
                    source,
                }
            })?;
        let bind_err = |source| MockError::Bind {
            port: config.port,
            source,
        };
        listener.set_nonblocking(true).map_err(bind_err)?;
        let addr = listener.local_addr().map_err(bind_err)?;

        let generated = if config.tls {
            Some(tls::generate().map_err(MockError::Tls)?)
        } else {
            None
        };

        let mut files = Vec::new();
        if let Some(path) = &config.lockfile {
            write_file(
                path,
                &lockfile_content(addr.port(), &config.password, config.tls),
            )?;
            files.push(path.clone());
        }
        if let (Some(path), Some(generated)) = (&config.ca_file, &generated) {
            write_file(path, &generated.ca_pem)?;
            files.push(path.clone());
        }

        let credentials = format!("{LCU_USERNAME}:{}", config.password);
        let (events, _) = broadcast::channel(1024);
        let shared = Arc::new(Shared {
            state: Mutex::new(LcuState::new(&config.fixtures)),
            events,
            start: Notify::new(),
            stopped: watch::Sender::new(false),
            authorization: format!(
                "Basic {}",
                base64::engine::general_purpose::STANDARD.encode(credentials)
            ),
            verbose: config.verbose,
            speed: config.speed,
            queue_timer: Mutex::new(None),
        });

        let app = Router::new()
            .fallback(handle_request)
            .layer(middleware::from_fn_with_state(shared.clone(), require_auth))
            .with_state(shared.clone());

        let handle = axum_server::Handle::new();
        let ca_pem = generated.as_ref().map(|g| g.ca_pem.clone());
        let server = match generated {
            Some(generated) => {
                let rustls =
                    axum_server::tls_rustls::RustlsConfig::from_config(generated.server_config);
                let server = axum_server::from_tcp_rustls(listener, rustls)
                    .handle(handle.clone())
                    .serve(app.into_make_service());
                tokio::spawn(server)
            }
            None => {
                let server = axum_server::from_tcp(listener)
                    .handle(handle.clone())
                    .serve(app.into_make_service());
                tokio::spawn(server)
            }
        };

        let replay = tokio::spawn(replay(
            shared.clone(),
            config.fixtures.events,
            config.speed,
            config.replay_start,
            config.repeat,
        ));

        Ok(Self {
            addr,
            password: config.password,
            ca_pem,
            tls: config.tls,
            shared,
            handle,
            server: Some(server),
            replay,
            files,
        })
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    /// PEM da CA que assina o certificado do servidor (`None` sem TLS).
    pub fn ca_pem(&self) -> Option<&str> {
        self.ca_pem.as_deref()
    }

    /// URL base, por exemplo `https://127.0.0.1:54321`.
    pub fn base_url(&self) -> String {
        let scheme = if self.tls { "https" } else { "http" };
        format!("{scheme}://{}", self.addr)
    }

    /// Fase atual do gameflow.
    pub fn phase(&self) -> String {
        self.shared.state().phase().to_owned()
    }

    /// Se a UI oficial "está aberta" (`kill-ux` fecha, `launch-ux` abre).
    pub fn ux_open(&self) -> bool {
        self.shared.state().ux_open()
    }

    /// Pedidos ao `/product-launcher` do Riot Client recebidos até agora, como
    /// `league_of_legends/live`.
    pub fn launches(&self) -> Vec<String> {
        self.shared.state().launches().to_vec()
    }

    /// Começa a reprodução agora, sem esperar uma inscrição WAMP.
    pub fn start_replay(&self) {
        self.shared.start.notify_one();
    }

    /// Para o servidor, esperando até um segundo pelas conexões abertas.
    pub async fn shutdown(mut self) {
        self.shared.stopped.send_replace(true);
        self.handle.graceful_shutdown(Some(Duration::from_secs(1)));
        if let Some(server) = self.server.take() {
            let _ = server.await;
        }
    }
}

impl Drop for MockLcu {
    fn drop(&mut self) {
        self.shared.stopped.send_replace(true);
        self.handle.shutdown();
        self.replay.abort();
        stop_queue_timer(&self.shared);
        for path in &self.files {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn write_file(path: &Path, content: &str) -> Result<(), MockError> {
    let err = |source| MockError::Write {
        path: path.to_path_buf(),
        source,
    };
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(err)?;
    }
    std::fs::write(path, content).map_err(err)
}

async fn replay(
    shared: Arc<Shared>,
    events: Vec<EventFixture>,
    speed: f64,
    start: ReplayStart,
    repeat: bool,
) {
    match start {
        ReplayStart::Never => return,
        ReplayStart::OnFirstSubscribe => shared.start.notified().await,
        ReplayStart::Immediately => {}
    }
    if events.is_empty() {
        return;
    }
    loop {
        println!("reproduzindo {} eventos", events.len());
        let began = tokio::time::Instant::now();
        for event in &events {
            let at = began + Duration::from_secs_f64(event.t_ms as f64 / 1000.0 / speed);
            tokio::time::sleep_until(at).await;
            emit(&shared, event.clone());
        }
        println!("fim da gravação");
        if !repeat {
            return;
        }
        tokio::time::sleep(Duration::from_secs_f64(1.0 / speed)).await;
        // Recomeça avisando o cliente da volta para a fase inicial.
        let initial = {
            let mut state = shared.state();
            state.reset();
            state.initial_phase().to_owned()
        };
        emit(
            &shared,
            EventFixture {
                t_ms: 0,
                uri: GAMEFLOW_PHASE.to_owned(),
                event_type: "Update".to_owned(),
                data: Value::String(initial),
            },
        );
    }
}

fn emit(shared: &Shared, event: EventFixture) {
    shared.state().apply(&event);
    if shared.verbose {
        println!("evento {} {}", event.event_type, event.uri);
    }
    // Sem ninguém inscrito o envio falha, e tudo bem.
    let _ = shared.events.send(event);
}

/// Começa a somar um segundo na fila a cada segundo. Para sozinho quando a
/// busca deixa de procurar.
fn start_queue_timer(shared: &Arc<Shared>) {
    let period = Duration::from_secs_f64(1.0 / shared.speed);
    let weak = Arc::downgrade(shared);
    let timer = tokio::spawn(async move {
        loop {
            tokio::time::sleep(period).await;
            let Some(shared) = weak.upgrade() else { return };
            let Some(event) = matchmaking::tick(&shared.state()) else {
                return;
            };
            emit(&shared, event);
        }
    });
    let old = shared
        .queue_timer
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .replace(timer);
    if let Some(old) = old {
        old.abort();
    }
}

fn stop_queue_timer(shared: &Shared) {
    let timer = shared
        .queue_timer
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    if let Some(timer) = timer {
        timer.abort();
    }
}

async fn require_auth(
    State(shared): State<Arc<Shared>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> HttpResponse {
    let ok = headers
        .get(header::AUTHORIZATION)
        .is_some_and(|v| v.as_bytes() == shared.authorization.as_bytes());
    if ok {
        return next.run(request).await;
    }
    if shared.verbose {
        println!("{} {} -> 401", request.method(), request.uri());
    }
    let mut response = json_response(&Response::error(401, "Unauthorized"));
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        header::HeaderValue::from_static("Basic realm=\"LCU\""),
    );
    response
}

async fn handle_request(
    State(shared): State<Arc<Shared>>,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
    request: Request,
) -> HttpResponse {
    let method = request.method().clone();
    let path = request
        .uri()
        .path_and_query()
        .map_or("/", |p| p.as_str())
        .to_owned();

    if let (true, Ok(ws)) = (request.uri().path() == "/", ws) {
        if shared.verbose {
            println!("WebSocket aberto");
        }
        return ws
            .protocols(["wamp"])
            .on_upgrade(move |socket| serve_wamp(shared, socket));
    }

    let response = match (&method, path.as_str()) {
        // Lobby (#17): mudam o estado e avisam pelo WAMP, como a LCU.
        (&Method::POST, lobby::LOBBY) => {
            let body = axum::body::to_bytes(request.into_body(), 64 * 1024)
                .await
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .unwrap_or(Value::Null);
            let (response, events) = lobby::create(&shared.state(), &body);
            for event in events {
                emit(&shared, event);
            }
            response
        }
        (&Method::DELETE, lobby::LOBBY) => {
            let (response, events) = lobby::leave(&shared.state());
            for event in events {
                emit(&shared, event);
            }
            response
        }
        // Fila (#19): o relógio da fila anda enquanto procura.
        (&Method::POST, matchmaking::LOBBY_SEARCH) => {
            let (response, events) = matchmaking::start(&shared.state());
            if !events.is_empty() {
                for event in events {
                    emit(&shared, event);
                }
                start_queue_timer(&shared);
            }
            response
        }
        (&Method::DELETE, matchmaking::LOBBY_SEARCH) => {
            stop_queue_timer(&shared);
            let (response, events) = matchmaking::stop(&shared.state());
            for event in events {
                emit(&shared, event);
            }
            response
        }
        (&Method::POST, "/riotclient/kill-ux") => {
            println!("kill-ux: UI oficial fechada (de mentira)");
            shared.state().set_ux_open(false);
            Response {
                status: 204,
                body: Value::Null,
            }
        }
        (&Method::POST, "/riotclient/launch-ux") => {
            println!("launch-ux: UI oficial aberta (de mentira)");
            shared.state().set_ux_open(true);
            Response {
                status: 204,
                body: Value::Null,
            }
        }
        // O Riot Client tem uma API local igual à da LCU; o agente usa esta
        // rota para abrir o LoL quando o Riot Client já está aberto (#11).
        (&Method::POST, p) if p.starts_with(PRODUCT_LAUNCHER_PREFIX) => {
            match parse_launch(&p[PRODUCT_LAUNCHER_PREFIX.len()..]) {
                Some(launch) => {
                    println!("product-launcher: abrindo {launch} (de mentira)");
                    shared.state().record_launch(launch);
                    Response {
                        status: 204,
                        body: Value::Null,
                    }
                }
                None => Response::error(404, format!("mock-lcu: {path} não existe")),
            }
        }
        (&Method::GET, _) => shared.state().get(&path),
        _ => Response::error(
            501,
            format!("mock-lcu: {method} {path} não está implementado; só GET é reproduzido"),
        ),
    };
    if shared.verbose {
        println!("{method} {path} -> {}", response.status);
    }
    json_response(&response)
}

/// Prefixo da rota do Riot Client que abre um produto.
const PRODUCT_LAUNCHER_PREFIX: &str = "/product-launcher/v1/products/";

/// `league_of_legends/patchlines/live` -> `league_of_legends/live`.
fn parse_launch(rest: &str) -> Option<String> {
    let (product, patchline) = rest.split_once("/patchlines/")?;
    let valid = |s: &str| !s.is_empty() && !s.contains(['/', '?']);
    (valid(product) && valid(patchline)).then(|| format!("{product}/{patchline}"))
}

fn json_response(response: &Response) -> HttpResponse {
    let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::OK);
    if response.body.is_null() {
        return status.into_response();
    }
    HttpResponse::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(response.body.to_string()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn serve_wamp(shared: Arc<Shared>, socket: WebSocket) {
    // Inscreve no canal antes de ler qualquer mensagem: a reprodução só
    // começa depois da primeira inscrição, então nada se perde.
    let mut events = shared.events.subscribe();
    let mut stopped = shared.stopped.subscribe();
    let (mut sink, mut stream) = socket.split();
    let mut topics: HashSet<String> = HashSet::new();

    loop {
        tokio::select! {
            // O servidor parou: fecha sem handshake, como o LoL morrendo.
            _ = async { stopped.wait_for(|stopped| *stopped).await.map(|_| ()) } => break,
            incoming = stream.next() => {
                let Some(Ok(message)) = incoming else { break };
                match message {
                    Message::Text(text) => match wamp::parse(&text) {
                        Some(wamp::ClientMessage::Subscribe(topic)) => {
                            if shared.verbose {
                                println!("WAMP inscrito em {topic}");
                            }
                            topics.insert(topic);
                            shared.start.notify_one();
                        }
                        Some(wamp::ClientMessage::Unsubscribe(topic)) => {
                            topics.remove(&topic);
                        }
                        None => {}
                    },
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            event = events.recv() => {
                let event = match event {
                    Ok(event) => event,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        eprintln!("cliente WAMP lento: {n} eventos perdidos");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };
                for topic in topics.iter().filter(|t| wamp::matches(t, &event.uri)) {
                    let text = wamp::event_message(topic, &event);
                    if sink.send(Message::text(text)).await.is_err() {
                        return;
                    }
                }
            }
        }
    }
    if shared.verbose {
        println!("WebSocket fechado");
    }
}
