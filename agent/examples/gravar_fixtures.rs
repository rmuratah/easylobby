//! Grava fixtures da LCU de uma sessão real (#5).
//!
//! Com o LoL aberto e logado:
//!
//! ```bash
//! cargo run -p easylobby-agent --example gravar_fixtures
//! cargo run -p easylobby-agent --example gravar_fixtures -- --segundos 600
//! cargo run -p easylobby-agent --example gravar_fixtures -- --patch 14.22 --saida /tmp/fixtures
//! ```
//!
//! Tira uma foto dos endpoints REST de lobby, fila, ready check, champ select,
//! runas e chat, grava os eventos WAMP desses recursos e tira uma nova foto a
//! cada mudança de fase do gameflow. Pare com Ctrl+C (ou `--segundos`). Tudo
//! passa pelo `Sanitizer` antes de ir para `agent/tests/fixtures/<patch>/`.
//!
//! Para passar uma gravação antiga pelo sanitizer atual, sem abrir o LoL:
//!
//! ```bash
//! cargo run -p easylobby-agent --example gravar_fixtures -- --ressanitizar agent/tests/fixtures/16.22
//! ```
//!
//! Este example não usa `kill-ux`: deixe a UI oficial aberta e jogue a sessão
//! por ela. Confira os arquivos antes de commitar.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use easylobby_agent::fixtures::{
    EventFixture, FixtureSet, Manifest, RestFixture, Sanitizer, FORMAT_VERSION,
};
use easylobby_agent::lockfile::{self, Credentials, LCU_USERNAME};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::Connector;

/// Certificado raiz da Riot que assina o HTTPS da LCU. É um certificado X.509
/// v1, por isso o TLS é o nativo do sistema (o `webpki` do rustls recusa v1).
const RIOT_ROOT_PEM: &[u8] = include_bytes!("../certs/riotgames.pem");

/// Endpoints fotografados no início e a cada mudança de fase.
const SNAPSHOT_PATHS: [&str; 14] = [
    "/lol-patch/v1/game-version",
    "/lol-summoner/v1/current-summoner",
    "/lol-gameflow/v1/gameflow-phase",
    "/lol-gameflow/v1/session",
    "/lol-lobby/v2/lobby",
    "/lol-matchmaking/v1/search",
    "/lol-matchmaking/v1/ready-check",
    "/lol-champ-select/v1/session",
    "/lol-champ-select/v1/pickable-champion-ids",
    "/lol-perks/v1/pages",
    "/lol-perks/v1/currentpage",
    "/lol-chat/v1/me",
    "/lol-chat/v1/friends",
    "/lol-chat/v1/conversations",
];

/// Endpoints grandes e que quase não mudam: só no início.
const STARTUP_ONLY_PATHS: [&str; 1] = ["/lol-game-queues/v1/queues"];

/// Prefixos de URI dos eventos WAMP que interessam ao easylobby.
const EVENT_PREFIXES: [&str; 7] = [
    "/lol-gameflow/v1/",
    "/lol-lobby/v2/",
    "/lol-matchmaking/v1/",
    "/lol-champ-select/v1/",
    "/lol-perks/v1/",
    "/lol-chat/v1/",
    "/lol-summoner/v1/current-summoner",
];

/// URIs dentro dos prefixos que não interessam e carregam preferências do
/// jogador (como os nomes dos grupos de amigos).
const EVENT_EXCLUDED: [&str; 1] = ["/lol-chat/v1/settings"];

fn wanted_event(uri: &str) -> bool {
    EVENT_PREFIXES.iter().any(|p| uri.starts_with(p))
        && !EVENT_EXCLUDED.iter().any(|p| uri.starts_with(p))
}

/// Tipos de mensagem WAMP 1 usados pela LCU.
const WAMP_SUBSCRIBE: u8 = 5;
const WAMP_EVENT: u8 = 8;

struct Args {
    patch: Option<String>,
    out: Option<PathBuf>,
    seconds: Option<u64>,
    resanitize: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        patch: None,
        out: None,
        seconds: None,
        resanitize: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} precisa de um valor"));
        match arg.as_str() {
            "--patch" => args.patch = Some(value()?),
            "--saida" => args.out = Some(PathBuf::from(value()?)),
            "--segundos" => {
                args.seconds = Some(value()?.parse().map_err(|_| "--segundos inválido")?)
            }
            "--ressanitizar" => args.resanitize = Some(PathBuf::from(value()?)),
            "-h" | "--help" => {
                println!("uso: gravar_fixtures [--patch 14.22] [--saida pasta] [--segundos N]");
                println!("     gravar_fixtures --ressanitizar pasta");
                std::process::exit(0);
            }
            _ => return Err(format!("argumento desconhecido: {arg}")),
        }
    }
    Ok(args)
}

struct Recorder {
    http: reqwest::Client,
    credentials: Credentials,
    sanitizer: Sanitizer,
    rest: Vec<RestFixture>,
    seen: HashSet<String>,
    events: Vec<EventFixture>,
    phase: String,
    started: Instant,
}

impl Recorder {
    /// Faz `GET path` e guarda a resposta sanitizada, sem repetir respostas
    /// iguais na mesma fase.
    async fn snapshot(&mut self, path: &str) {
        let url = format!("{}{}", self.credentials.base_url(), path);
        let response = match self
            .http
            .get(&url)
            .basic_auth(LCU_USERNAME, Some(&self.credentials.password))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("GET {path} falhou: {e}");
                return;
            }
        };
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        let body = if text.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };

        let fixture = RestFixture {
            method: "GET".into(),
            path: self.sanitizer.sanitize_str(path),
            phase: self.phase.clone(),
            status,
            body: self.sanitizer.sanitize_resource(path, &body),
        };
        let key = serde_json::to_string(&fixture).unwrap_or_default();
        if self.seen.insert(key) {
            self.rest.push(fixture);
        }
    }

    async fn snapshot_all(&mut self, startup: bool) {
        for path in SNAPSHOT_PATHS {
            self.snapshot(path).await;
        }
        if startup {
            for path in STARTUP_ONLY_PATHS {
                self.snapshot(path).await;
            }
        }
        println!("[{}] foto REST na fase {}", self.elapsed_ms(), self.phase);
    }

    fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// Trata `[8, "OnJsonApiEvent", {data, eventType, uri}]`. Devolve a nova
    /// fase quando o evento é uma mudança de fase do gameflow.
    fn on_event(&mut self, payload: &Value) -> Option<String> {
        let uri = payload.get("uri")?.as_str()?;
        if !wanted_event(uri) {
            return None;
        }
        let event_type = payload
            .get("eventType")
            .and_then(Value::as_str)
            .unwrap_or("Update")
            .to_owned();
        let data = payload.get("data").cloned().unwrap_or(Value::Null);
        let event = EventFixture {
            t_ms: self.elapsed_ms(),
            event_type,
            // Os dados primeiro: é aí que o sanitizer aprende os ids que
            // podem aparecer na URI (como o pid de uma conversa).
            data: self.sanitizer.sanitize_resource(uri, &data),
            uri: String::new(),
        };
        let event = EventFixture {
            uri: self.sanitizer.sanitize_str(uri),
            ..event
        };
        println!("[{}] {} {}", event.t_ms, event.event_type, event.uri);
        self.events.push(event);

        if uri == "/lol-gameflow/v1/gameflow-phase" {
            data.as_str().map(str::to_owned)
        } else {
            None
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args = parse_args()?;
    if let Some(dir) = &args.resanitize {
        return resanitize(dir);
    }
    let credentials = lockfile::discover().map_err(|e| e.to_string())?;
    println!("LCU em {}", credentials.base_url());

    let root = reqwest::Certificate::from_pem(RIOT_ROOT_PEM).map_err(|e| e.to_string())?;
    let http = reqwest::Client::builder()
        .use_native_tls()
        .tls_built_in_root_certs(false)
        .add_root_certificate(root)
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let mut sanitizer = Sanitizer::new();
    sanitizer.add_secret(credentials.password.clone());

    let mut rec = Recorder {
        http,
        credentials,
        sanitizer,
        rest: Vec::new(),
        seen: HashSet::new(),
        events: Vec::new(),
        phase: "None".into(),
        started: Instant::now(),
    };

    // A fase atual vem antes da primeira foto, para marcar as respostas.
    rec.snapshot("/lol-gameflow/v1/gameflow-phase").await;
    if let Some(Value::String(phase)) = rec.rest.last().map(|f| f.body.clone()) {
        rec.phase = phase;
    }
    rec.rest.clear();
    rec.seen.clear();
    rec.snapshot_all(true).await;

    let game_version = rec
        .rest
        .iter()
        .find(|f| f.path == "/lol-patch/v1/game-version")
        .and_then(|f| f.body.as_str())
        .map(str::to_owned);
    let patch = args
        .patch
        .or_else(|| game_version.as_deref().and_then(short_patch))
        .ok_or("não consegui descobrir o patch; passe --patch 14.22")?;

    let mut ws = connect_ws(&rec.credentials).await?;
    let subscribe = format!("[{WAMP_SUBSCRIBE}, \"OnJsonApiEvent\"]");
    ws.send(Message::text(subscribe))
        .await
        .map_err(|e| format!("falha ao inscrever no WAMP: {e}"))?;
    println!("gravando eventos; jogue a sessão pela UI oficial e aperte Ctrl+C para parar");

    let deadline = args
        .seconds
        .map(|s| tokio::time::Instant::now() + Duration::from_secs(s));
    loop {
        let sleep = async {
            match deadline {
                Some(d) => tokio::time::sleep_until(d).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            _ = sleep => break,
            msg = ws.next() => {
                let Some(msg) = msg else {
                    eprintln!("a LCU fechou o WebSocket");
                    break;
                };
                let Ok(Message::Text(text)) = msg else { continue };
                let Ok(Value::Array(frame)) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if frame.first().and_then(Value::as_u64) != Some(u64::from(WAMP_EVENT)) {
                    continue;
                }
                let Some(payload) = frame.get(2) else { continue };
                if let Some(phase) = rec.on_event(payload) {
                    if phase != rec.phase {
                        rec.phase = phase;
                        rec.snapshot_all(false).await;
                    }
                }
            }
        }
    }

    let out = args.out.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(&patch)
    });
    let set = FixtureSet {
        manifest: Manifest {
            format_version: FORMAT_VERSION,
            patch,
            game_version,
            recorded_at: rfc3339_now(),
            synthetic: false,
            notes: String::new(),
        },
        rest: rec.rest,
        events: rec.events,
    };
    set.save(&out).map_err(|e| e.to_string())?;
    println!(
        "{} respostas REST e {} eventos em {}",
        set.rest.len(),
        set.events.len(),
        out.display()
    );
    println!("confira os arquivos antes de commitar: nada de nomes, ids ou mensagens reais");
    Ok(())
}

/// Passa um conjunto já gravado pelo sanitizer atual e grava por cima.
fn resanitize(dir: &std::path::Path) -> Result<(), String> {
    let mut set = FixtureSet::load(dir).map_err(|e| e.to_string())?;
    set.events.retain(|e| wanted_event(&e.uri));
    let mut sanitizer = Sanitizer::new();
    // Os dados primeiro, para os ids que aparecem nos caminhos já serem conhecidos.
    for r in &mut set.rest {
        r.body = sanitizer.sanitize_resource(&r.path, &r.body);
    }
    for e in &mut set.events {
        e.data = sanitizer.sanitize_resource(&e.uri, &e.data);
    }
    for r in &mut set.rest {
        r.path = sanitizer.sanitize_str(&r.path);
    }
    for e in &mut set.events {
        e.uri = sanitizer.sanitize_str(&e.uri);
    }
    set.save(dir).map_err(|e| e.to_string())?;
    println!("{} sanitizado de novo", dir.display());
    Ok(())
}

async fn connect_ws(
    credentials: &Credentials,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
> {
    let url = format!("wss://127.0.0.1:{}/", credentials.port);
    let mut request = url.into_client_request().map_err(|e| e.to_string())?;
    let auth = base64::engine::general_purpose::STANDARD
        .encode(format!("{LCU_USERNAME}:{}", credentials.password));
    let headers = request.headers_mut();
    headers.insert(
        "Authorization",
        HeaderValue::from_str(&format!("Basic {auth}")).map_err(|e| e.to_string())?,
    );
    headers.insert("Sec-WebSocket-Protocol", HeaderValue::from_static("wamp"));

    let root = native_tls::Certificate::from_pem(RIOT_ROOT_PEM).map_err(|e| e.to_string())?;
    let tls = native_tls::TlsConnector::builder()
        .disable_built_in_roots(true)
        .add_root_certificate(root)
        .build()
        .map_err(|e| e.to_string())?;

    let (ws, _) = tokio_tungstenite::connect_async_tls_with_config(
        request,
        None,
        false,
        Some(Connector::NativeTls(tls)),
    )
    .await
    .map_err(|e| format!("falha ao conectar no WebSocket da LCU: {e}"))?;
    Ok(ws)
}

/// `14.22.629.1234` -> `14.22`.
fn short_patch(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    (!major.is_empty() && !minor.is_empty()).then(|| format!("{major}.{minor}"))
}

/// Data e hora atuais em UTC no formato RFC 3339, sem dependência extra.
fn rfc3339_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Algoritmo "civil_from_days" de Howard Hinnant.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}
