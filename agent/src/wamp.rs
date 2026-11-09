//! Cliente WebSocket WAMP da LCU (#9).
//!
//! A LCU publica as mudanças de estado (fase do gameflow, lobby, fila, ready
//! check, champ select, runas, chat...) num WebSocket WAMP 1 na mesma porta do
//! REST, com o mesmo Basic Auth `riot:<senha>` e o mesmo certificado. O
//! cliente se inscreve em `OnJsonApiEvent` (`[5, "OnJsonApiEvent"]`) e cada
//! mudança chega como `[8, "OnJsonApiEvent", {"data", "eventType", "uri"}]`.
//!
//! A LCU manda eventos de todos os plugins; [`EventFilter`] guarda só os
//! recursos que o easylobby usa. O filtro olha a `uri` antes de interpretar o
//! `data`, então eventos grandes que não interessam custam pouco.
//!
//! O TLS segue o cliente REST ([`crate::rest`]): TLS nativo do sistema com o
//! `riotgames.pem` como **única** raiz confiável, sem desligar a verificação.
//! Para o mock-lcu, use [`LcuEvents::with_root_certificate`] com a CA dele.
//!
//! Uma [`LcuEvents`] é uma conexão. Quando o LoL fecha, [`LcuEvents::next`]
//! devolve `None`. Para reconectar sozinho quando o LoL reabre, use
//! [`crate::connection::LcuConnection`] (#10).
//!
//! ```no_run
//! # async fn exemplo() -> Result<(), Box<dyn std::error::Error>> {
//! use easylobby_agent::lockfile;
//! use easylobby_agent::wamp::{EventFilter, LcuEvents};
//!
//! let credentials = lockfile::discover()?;
//! let mut events = LcuEvents::connect(&credentials, EventFilter::easylobby()).await?;
//! while let Some(event) = events.next().await {
//!     let event = event?;
//!     println!("{:?} {}", event.event_type, event.uri);
//! }
//! // O LoL fechou.
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::time::Duration;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::Value;
use thiserror::Error;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::{AUTHORIZATION, SEC_WEBSOCKET_PROTOCOL};
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream};

use crate::lockfile::{Credentials, Protocol, LCU_USERNAME};
use crate::rest::RIOT_ROOT_PEM;

/// Tópico WAMP com todos os eventos da API da LCU.
pub const ALL_EVENTS_TOPIC: &str = "OnJsonApiEvent";

/// Tempo máximo para abrir o WebSocket (TCP, TLS e handshake).
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Subprotocolo WebSocket da LCU.
const WAMP_PROTOCOL: &str = "wamp";

/// Tipos de mensagem WAMP 1 usados pela LCU.
const WAMP_SUBSCRIBE: u64 = 5;
const WAMP_UNSUBSCRIBE: u64 = 6;
const WAMP_EVENT: u64 = 8;

/// Prefixos de URI dos recursos que o easylobby acompanha: gameflow (fase e
/// sessão), lobby, fila e ready check, champ select, runas, chat, o
/// invocador logado e os ranks dele (#171), e os pedidos do LoL para
/// mostrar a UI oficial (#188). Os da LCU são os mesmos que o gravador de
/// fixtures (#5) grava.
pub const EASYLOBBY_EVENT_PREFIXES: [&str; 11] = [
    "/lol-gameflow/v1/",
    "/lol-lobby/v2/",
    "/lol-matchmaking/v1/",
    "/lol-champ-select/v1/",
    "/lol-perks/v1/",
    "/lol-chat/v1/",
    "/lol-summoner/v1/current-summoner",
    "/lol-ranked/v1/current-ranked-stats",
    crate::ux::UX_STATE_REQUEST_PATH,
    // Cartas de campeão do ARAM (#201).
    crate::champions::SUBSET_CHAMPIONS_PATH,
    // Lista de personalizadas abertas (#264).
    crate::custom_game_list::CUSTOM_GAMES_PATH,
];

/// URIs dentro dos prefixos que o easylobby não usa e que carregam
/// preferências do jogador (como os nomes dos grupos de amigos).
pub const EASYLOBBY_EVENT_EXCLUDED: [&str; 1] = ["/lol-chat/v1/settings"];

/// O que mudou num recurso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    Create,
    Update,
    Delete,
    /// Um tipo que a LCU não mandava quando este código foi escrito.
    #[serde(other)]
    Unknown,
}

/// Um evento da LCU.
#[derive(Debug, Clone, PartialEq)]
pub struct LcuEvent {
    /// Recurso que mudou, por exemplo `/lol-gameflow/v1/gameflow-phase`.
    pub uri: String,
    pub event_type: EventType,
    /// Novo estado do recurso; `Null` num `Delete`.
    pub data: Value,
}

/// Quais eventos chegam a quem chama: URIs que começam com algum dos
/// prefixos e com nenhum dos excluídos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventFilter {
    prefixes: Vec<String>,
    excluded: Vec<String>,
}

impl EventFilter {
    /// Só as URIs que começam com algum destes prefixos.
    pub fn prefixes<I, S>(prefixes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            prefixes: prefixes.into_iter().map(Into::into).collect(),
            excluded: Vec::new(),
        }
    }

    /// Tudo o que a LCU mandar.
    pub fn all() -> Self {
        Self::prefixes(["/"])
    }

    /// Os recursos que o easylobby usa ([`EASYLOBBY_EVENT_PREFIXES`] menos
    /// [`EASYLOBBY_EVENT_EXCLUDED`]).
    pub fn easylobby() -> Self {
        Self::prefixes(EASYLOBBY_EVENT_PREFIXES).exclude(EASYLOBBY_EVENT_EXCLUDED)
    }

    /// Tira as URIs que começam com estes prefixos.
    pub fn exclude<I, S>(mut self, prefixes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.excluded.extend(prefixes.into_iter().map(Into::into));
        self
    }

    /// Se o evento de `uri` passa.
    pub fn matches(&self, uri: &str) -> bool {
        self.prefixes.iter().any(|p| uri.starts_with(p.as_str()))
            && !self.excluded.iter().any(|p| uri.starts_with(p.as_str()))
    }
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::easylobby()
    }
}

/// Erro ao conectar no WebSocket da LCU ou ao ler dele.
#[derive(Debug, Error)]
pub enum WampError {
    #[error("certificado raiz inválido: {0}")]
    Certificate(#[source] native_tls::Error),
    #[error("não foi possível montar o TLS: {0}")]
    Tls(#[source] native_tls::Error),
    #[error("não foi possível montar a requisição do WebSocket: {0}")]
    Request(#[source] Box<tungstenite::Error>),
    #[error("a LCU recusou a senha do WebSocket (401)")]
    Unauthorized,
    #[error("não foi possível conectar no WebSocket da LCU: {0}")]
    Connect(#[source] Box<tungstenite::Error>),
    #[error("o WebSocket da LCU não abriu em {0:?}")]
    Timeout(Duration),
    #[error("falha ao falar com o WebSocket da LCU: {0}")]
    Socket(#[source] Box<tungstenite::Error>),
}

impl WampError {
    /// Se o erro foi de conexão (LoL fechado, porta errada, certificado que
    /// não bate com o fixado, tempo esgotado). Vale tentar de novo depois.
    pub fn is_connect(&self) -> bool {
        matches!(self, WampError::Connect(_) | WampError::Timeout(_))
    }
}

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Conexão com o WebSocket WAMP da LCU, já inscrita em `OnJsonApiEvent`.
///
/// O `Debug` não mostra a senha.
pub struct LcuEvents {
    socket: Socket,
    filter: EventFilter,
    url: String,
    closed: bool,
}

impl LcuEvents {
    /// Conecta no LoL de verdade, com o `riotgames.pem` fixado.
    pub async fn connect(
        credentials: &Credentials,
        filter: EventFilter,
    ) -> Result<Self, WampError> {
        Self::with_root_certificate(credentials, RIOT_ROOT_PEM, filter).await
    }

    /// Conecta com outra raiz fixada no lugar da Riot, como a CA do mock-lcu.
    ///
    /// `root_pem` é a única raiz aceita; as do sistema não entram. Num
    /// lockfile `http` (o mock sem TLS) o certificado não é usado.
    pub async fn with_root_certificate(
        credentials: &Credentials,
        root_pem: &[u8],
        filter: EventFilter,
    ) -> Result<Self, WampError> {
        Self::build(credentials, root_pem, filter, DEFAULT_CONNECT_TIMEOUT).await
    }

    /// Como [`LcuEvents::with_root_certificate`], com outro tempo máximo para
    /// abrir a conexão.
    pub async fn build(
        credentials: &Credentials,
        root_pem: &[u8],
        filter: EventFilter,
        timeout: Duration,
    ) -> Result<Self, WampError> {
        let url = ws_url(credentials);
        let request = ws_request(&url, &credentials.password)?;
        let connector = match credentials.protocol {
            Protocol::Https => {
                let root =
                    native_tls::Certificate::from_pem(root_pem).map_err(WampError::Certificate)?;
                let tls = native_tls::TlsConnector::builder()
                    .disable_built_in_roots(true)
                    .add_root_certificate(root)
                    .build()
                    .map_err(WampError::Tls)?;
                Connector::NativeTls(tls)
            }
            Protocol::Http => Connector::Plain,
        };

        let connect =
            tokio_tungstenite::connect_async_tls_with_config(request, None, false, Some(connector));
        let (mut socket, _) = tokio::time::timeout(timeout, connect)
            .await
            .map_err(|_| WampError::Timeout(timeout))?
            .map_err(connect_error)?;

        socket
            .send(Message::text(subscribe_message(ALL_EVENTS_TOPIC)))
            .await
            .map_err(|e| WampError::Socket(Box::new(e)))?;

        Ok(Self {
            socket,
            filter,
            url,
            closed: false,
        })
    }

    /// URL do WebSocket, por exemplo `wss://127.0.0.1:54321/`.
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn filter(&self) -> &EventFilter {
        &self.filter
    }

    /// Troca o filtro sem reconectar.
    pub fn set_filter(&mut self, filter: EventFilter) {
        self.filter = filter;
    }

    /// Próximo evento que passa no filtro, na ordem em que a LCU mandou.
    ///
    /// Devolve `None` quando a conexão fecha (o LoL fechou ou reiniciou). Um
    /// erro de leitura vem uma vez como `Some(Err(_))` e depois `None`.
    /// Mensagens que não são eventos (o `WELCOME` do WAMP, pings) e frames
    /// mal formados são ignorados.
    pub async fn next(&mut self) -> Option<Result<LcuEvent, WampError>> {
        while !self.closed {
            let message = match self.socket.next().await {
                Some(Ok(message)) => message,
                Some(Err(e)) => {
                    self.closed = true;
                    return Some(Err(WampError::Socket(Box::new(e))));
                }
                None => break,
            };
            match message {
                Message::Text(text) => {
                    if let Some(event) = parse_event(&text, &self.filter) {
                        return Some(Ok(event));
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        self.closed = true;
        None
    }

    /// Cancela a inscrição e fecha a conexão.
    pub async fn close(mut self) {
        if !self.closed {
            let _ = self
                .socket
                .send(Message::text(unsubscribe_message(ALL_EVENTS_TOPIC)))
                .await;
        }
        let _ = self.socket.close(None).await;
    }
}

impl fmt::Debug for LcuEvents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcuEvents")
            .field("url", &self.url)
            .field("filter", &self.filter)
            .field("closed", &self.closed)
            .finish()
    }
}

/// `wss://127.0.0.1:<porta>/` (ou `ws://` num lockfile `http`).
fn ws_url(credentials: &Credentials) -> String {
    let scheme = match credentials.protocol {
        Protocol::Https => "wss",
        Protocol::Http => "ws",
    };
    format!("{scheme}://127.0.0.1:{}/", credentials.port)
}

fn ws_request(
    url: &str,
    password: &str,
) -> Result<tungstenite::handshake::client::Request, WampError> {
    let request_err = |e| WampError::Request(Box::new(e));
    let mut request = url.into_client_request().map_err(request_err)?;
    let auth =
        base64::engine::general_purpose::STANDARD.encode(format!("{LCU_USERNAME}:{password}"));
    let mut authorization = HeaderValue::from_str(&format!("Basic {auth}"))
        .map_err(|e| request_err(tungstenite::Error::HttpFormat(e.into())))?;
    authorization.set_sensitive(true);
    let headers = request.headers_mut();
    headers.insert(AUTHORIZATION, authorization);
    headers.insert(
        SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_static(WAMP_PROTOCOL),
    );
    Ok(request)
}

fn connect_error(error: tungstenite::Error) -> WampError {
    match &error {
        tungstenite::Error::Http(response) if response.status().as_u16() == 401 => {
            WampError::Unauthorized
        }
        _ => WampError::Connect(Box::new(error)),
    }
}

fn subscribe_message(topic: &str) -> String {
    serde_json::json!([WAMP_SUBSCRIBE, topic]).to_string()
}

fn unsubscribe_message(topic: &str) -> String {
    serde_json::json!([WAMP_UNSUBSCRIBE, topic]).to_string()
}

/// Corpo de um `[8, tópico, {...}]`, com o `data` ainda cru.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEvent<'a> {
    uri: String,
    event_type: EventType,
    #[serde(borrow)]
    data: Option<&'a RawValue>,
}

/// Interpreta um frame de texto. Devolve `None` para o que não é evento,
/// para frame mal formado e para evento que não passa no filtro; nesse caso o
/// `data` nem é interpretado.
fn parse_event(text: &str, filter: &EventFilter) -> Option<LcuEvent> {
    let frame: Vec<&RawValue> = serde_json::from_str(text).ok()?;
    let [kind, _topic, payload] = frame.as_slice() else {
        return None;
    };
    if serde_json::from_str::<u64>(kind.get()).ok()? != WAMP_EVENT {
        return None;
    }
    let raw: RawEvent = serde_json::from_str(payload.get()).ok()?;
    if !filter.matches(&raw.uri) {
        return None;
    }
    let data = match raw.data {
        Some(data) => serde_json::from_str(data.get()).ok()?,
        None => Value::Null,
    };
    Some(LcuEvent {
        uri: raw.uri,
        event_type: raw.event_type,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lockfile::Source;
    use serde_json::json;

    const SENHA_FALSA: &str = "senha-falsa-de-teste";

    fn credentials(protocol: Protocol) -> Credentials {
        Credentials {
            pid: Some(1),
            port: 54321,
            password: SENHA_FALSA.into(),
            protocol,
            source: Source::ProcessArgs { pid: 1 },
        }
    }

    fn frame(uri: &str, event_type: &str, data: Value) -> String {
        json!([8, ALL_EVENTS_TOPIC, { "data": data, "eventType": event_type, "uri": uri }])
            .to_string()
    }

    #[test]
    fn interpreta_evento() {
        let text = frame("/lol-gameflow/v1/gameflow-phase", "Update", json!("Lobby"));
        assert_eq!(
            parse_event(&text, &EventFilter::easylobby()),
            Some(LcuEvent {
                uri: "/lol-gameflow/v1/gameflow-phase".into(),
                event_type: EventType::Update,
                data: json!("Lobby"),
            })
        );
    }

    #[test]
    fn delete_vem_com_data_nulo() {
        let text = frame("/lol-lobby/v2/lobby", "Delete", Value::Null);
        let event = parse_event(&text, &EventFilter::easylobby()).unwrap();
        assert_eq!(event.event_type, EventType::Delete);
        assert_eq!(event.data, Value::Null);

        let sem_data =
            r#"[8, "OnJsonApiEvent", {"eventType": "Delete", "uri": "/lol-lobby/v2/lobby"}]"#;
        assert_eq!(
            parse_event(sem_data, &EventFilter::easylobby())
                .unwrap()
                .data,
            Value::Null
        );
    }

    #[test]
    fn tipo_desconhecido_nao_derruba() {
        let text = frame("/lol-lobby/v2/lobby", "Rename", json!({}));
        assert_eq!(
            parse_event(&text, &EventFilter::easylobby())
                .unwrap()
                .event_type,
            EventType::Unknown
        );
    }

    #[test]
    fn ignora_o_que_nao_e_evento() {
        let filter = EventFilter::all();
        // WELCOME do WAMP 1.
        assert_eq!(
            parse_event(r#"[0, "sessao-falsa", 1, "LCU"]"#, &filter),
            None
        );
        assert_eq!(parse_event(r#"[5, "OnJsonApiEvent"]"#, &filter), None);
        assert_eq!(parse_event("lixo", &filter), None);
        assert_eq!(parse_event("{}", &filter), None);
        assert_eq!(parse_event(r#"[8, "OnJsonApiEvent", {}]"#, &filter), None);
    }

    #[test]
    fn filtra_antes_de_ler_o_data() {
        // `data` inválido num evento filtrado não importa: nem é lido.
        let text = r#"[8, "OnJsonApiEvent", {"uri": "/lol-store/v1/catalog", "eventType": "Update", "data": [1, 2, 3]}]"#;
        assert_eq!(parse_event(text, &EventFilter::easylobby()), None);
        assert!(parse_event(text, &EventFilter::all()).is_some());
    }

    #[test]
    fn filtro_do_easylobby() {
        let filter = EventFilter::easylobby();
        for uri in [
            "/lol-gameflow/v1/gameflow-phase",
            "/lol-gameflow/v1/session",
            "/lol-lobby/v2/lobby",
            "/lol-lobby/v2/lobby/members",
            "/lol-matchmaking/v1/search",
            "/lol-matchmaking/v1/ready-check",
            "/lol-champ-select/v1/session",
            "/lol-perks/v1/currentpage",
            "/lol-chat/v1/conversations/fake-pid-0002/messages",
            "/lol-summoner/v1/current-summoner",
            "/riotclient/ux-state/request",
        ] {
            assert!(filter.matches(uri), "{uri}");
        }
        for uri in [
            "/lol-chat/v1/settings",
            "/lol-store/v1/catalog",
            "/lol-summoner/v1/summoners/1",
            "/riotclient/ux-state",
            "/lol-lobby/v1/parties",
        ] {
            assert!(!filter.matches(uri), "{uri}");
        }
        assert_eq!(EventFilter::default(), filter);
    }

    #[test]
    fn filtro_por_prefixo() {
        let filter = EventFilter::prefixes(["/lol-gameflow/v1/gameflow-phase"]);
        assert!(filter.matches("/lol-gameflow/v1/gameflow-phase"));
        assert!(!filter.matches("/lol-gameflow/v1/session"));
        let filter = EventFilter::all().exclude(["/lol-chat/"]);
        assert!(filter.matches("/lol-store/v1/catalog"));
        assert!(!filter.matches("/lol-chat/v1/me"));
    }

    #[test]
    fn mensagens_de_inscricao() {
        assert_eq!(
            subscribe_message(ALL_EVENTS_TOPIC),
            r#"[5,"OnJsonApiEvent"]"#
        );
        assert_eq!(
            unsubscribe_message(ALL_EVENTS_TOPIC),
            r#"[6,"OnJsonApiEvent"]"#
        );
    }

    #[test]
    fn url_segue_o_protocolo_do_lockfile() {
        assert_eq!(
            ws_url(&credentials(Protocol::Https)),
            "wss://127.0.0.1:54321/"
        );
        assert_eq!(
            ws_url(&credentials(Protocol::Http)),
            "ws://127.0.0.1:54321/"
        );
    }

    #[test]
    fn requisicao_tem_basic_auth_e_subprotocolo() {
        let request = ws_request("wss://127.0.0.1:54321/", SENHA_FALSA).unwrap();
        let expected = base64::engine::general_purpose::STANDARD
            .encode(format!("{LCU_USERNAME}:{SENHA_FALSA}"));
        let auth = request.headers().get(AUTHORIZATION).unwrap();
        assert_eq!(auth.to_str().unwrap(), format!("Basic {expected}"));
        assert!(auth.is_sensitive());
        assert_eq!(
            request.headers().get(SEC_WEBSOCKET_PROTOCOL).unwrap(),
            "wamp"
        );
    }

    #[tokio::test]
    async fn recusa_certificado_invalido() {
        let err = LcuEvents::with_root_certificate(
            &credentials(Protocol::Https),
            b"nao e um pem",
            EventFilter::easylobby(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, WampError::Certificate(_)), "{err}");
    }
}
