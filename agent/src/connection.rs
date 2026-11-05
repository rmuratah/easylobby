//! Conexão com a LCU que sobrevive ao LoL fechar e reabrir (#10).
//!
//! Uma [`LcuEvents`] é uma conexão só: quando o LoL fecha ou reinicia, ela
//! acaba, e o próximo client vem com outra porta, outra senha e outro
//! lockfile. [`LcuConnection`] cuida disso: procura as credenciais, conecta
//! REST e WAMP, repassa os eventos e, quando a conexão cai, volta a procurar o
//! LoL com espera crescente até ele voltar. O agente nunca cai por isso.
//!
//! Cada [`LcuConnection::next`] devolve um [`ConnectionEvent`]:
//!
//! - [`ConnectionEvent::Connected`]: conexão nova, com um [`LcuSession`] que
//!   traz o [`LcuClient`] para os comandos REST. Os eventos de antes se
//!   perderam, então quem chama deve reler pelo REST o estado que acompanha.
//! - [`ConnectionEvent::Event`]: um evento da LCU que passou no filtro.
//! - [`ConnectionEvent::Disconnected`]: a conexão caiu (o LoL fechou ou
//!   reiniciou). O [`LcuClient`] da sessão anterior não serve mais.
//! - [`ConnectionEvent::Retrying`]: uma tentativa falhou (o LoL ainda não
//!   abriu, o lockfile é velho...). A próxima sai depois de `delay`.
//!
//! `next` nunca termina por conta própria: para parar, solte a conexão ou use
//! [`LcuConnection::close`]. Ele pode ser cancelado num `tokio::select!` sem
//! perder evento nem reiniciar a espera.
//!
//! ```no_run
//! # async fn exemplo() {
//! use easylobby_agent::connection::{ConnectionEvent, LcuConnection, ReconnectConfig};
//!
//! let mut lcu = LcuConnection::new(ReconnectConfig::default());
//! loop {
//!     match lcu.next().await {
//!         ConnectionEvent::Connected(session) => {
//!             println!("conectado na porta {}", session.credentials.port);
//!             // Reler o estado com session.client.get(...).
//!         }
//!         ConnectionEvent::Event(event) => println!("{}", event.uri),
//!         ConnectionEvent::Disconnected(reason) => println!("caiu: {reason}"),
//!         ConnectionEvent::Retrying { error, delay, .. } => {
//!             println!("{error}; tentando de novo em {delay:?}")
//!         }
//!     }
//! }
//! # }
//! ```

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use thiserror::Error;
use tokio::time::Instant;

use crate::lockfile::{self, Credentials, DiscoveryError, LockfileError, Protocol};
use crate::rest::{LcuClient, LcuError, RIOT_ROOT_PEM};
use crate::wamp::{EventFilter, LcuEvent, LcuEvents, WampError, DEFAULT_CONNECT_TIMEOUT};

/// Arquivo da CA que o mock-lcu grava ao lado do lockfile.
pub const MOCK_CA_FILE: &str = "mock-lcu-ca.pem";

/// Espera antes da segunda tentativa. Dobra a cada falha.
pub const DEFAULT_RETRY_MIN: Duration = Duration::from_millis(500);

/// Espera máxima entre tentativas: o LoL fechado é procurado a cada 5 s.
pub const DEFAULT_RETRY_MAX: Duration = Duration::from_secs(5);

/// De onde vêm as credenciais a cada tentativa.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CredentialSource {
    /// Acha o LoL aberto ([`lockfile::discover`]) e fixa o `riotgames.pem`.
    #[default]
    Discover,
    /// Lê este lockfile. Se ele diz `https` e há um [`MOCK_CA_FILE`] ao lado,
    /// essa CA é fixada no lugar do `riotgames.pem` (o mock-lcu). O lockfile
    /// e a CA são relidos a cada tentativa, porque mudam quando o servidor
    /// reinicia.
    Lockfile(PathBuf),
}

/// Configuração da reconexão.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    pub source: CredentialSource,
    pub filter: EventFilter,
    /// Espera depois da primeira falha seguida.
    pub retry_min: Duration,
    /// Teto da espera entre tentativas.
    pub retry_max: Duration,
    /// Tempo máximo para abrir o WebSocket em cada tentativa.
    pub connect_timeout: Duration,
}

impl ReconnectConfig {
    /// Configuração padrão com outra fonte de credenciais.
    pub fn with_source(source: CredentialSource) -> Self {
        Self {
            source,
            ..Self::default()
        }
    }
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            source: CredentialSource::Discover,
            filter: EventFilter::easylobby(),
            retry_min: DEFAULT_RETRY_MIN,
            retry_max: DEFAULT_RETRY_MAX,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
        }
    }
}

/// Uma conexão com um client do LoL, do `Connected` até o `Disconnected`.
#[derive(Debug, Clone)]
pub struct LcuSession {
    /// Conta as conexões desde a criação da [`LcuConnection`], a partir de 1.
    pub id: u64,
    /// Credenciais desta conexão. O `Debug` esconde a senha.
    pub credentials: Credentials,
    /// Cliente REST desta conexão. Para de funcionar quando ela cai.
    pub client: LcuClient,
}

/// Por que uma conexão acabou.
#[derive(Debug, Error)]
pub enum DisconnectReason {
    /// A LCU fechou o WebSocket: o LoL fechou ou reiniciou.
    #[error("a LCU fechou a conexão")]
    Closed,
    /// A leitura do WebSocket falhou.
    #[error(transparent)]
    Socket(WampError),
}

/// Por que uma tentativa de conectar falhou. Toda falha é tentada de novo.
#[derive(Debug, Error)]
pub enum ConnectError {
    /// O LoL não está aberto ou não achei as credenciais.
    #[error(transparent)]
    Discovery(#[from] DiscoveryError),
    /// O lockfile configurado não existe ou é inválido.
    #[error(transparent)]
    Lockfile(#[from] LockfileError),
    #[error("não foi possível ler {}: {source}", path.display())]
    Certificate { path: PathBuf, source: io::Error },
    #[error(transparent)]
    Rest(#[from] LcuError),
    /// O WebSocket não abriu: lockfile velho, LoL ainda subindo, senha
    /// recusada...
    #[error(transparent)]
    Wamp(#[from] WampError),
}

impl ConnectError {
    /// Se o LoL simplesmente não está aberto (o estado normal enquanto o
    /// usuário não abre o jogo).
    pub fn is_client_not_running(&self) -> bool {
        matches!(
            self,
            ConnectError::Discovery(DiscoveryError::ClientNotRunning)
        ) || matches!(self, ConnectError::Lockfile(LockfileError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound)
    }
}

/// O que aconteceu com a conexão.
#[derive(Debug)]
pub enum ConnectionEvent {
    /// Conectou num client do LoL (o primeiro ou um que reabriu).
    Connected(LcuSession),
    /// Um evento da LCU que passou no filtro.
    Event(LcuEvent),
    /// A conexão caiu. A próxima tentativa sai em seguida.
    Disconnected(DisconnectReason),
    /// Uma tentativa falhou; a próxima sai depois de `delay`.
    Retrying {
        /// Falhas seguidas até aqui, a partir de 1.
        attempt: u32,
        delay: Duration,
        error: ConnectError,
    },
}

/// Conexão com a LCU que reconecta sozinha. Veja o [módulo](self).
pub struct LcuConnection {
    config: ReconnectConfig,
    events: Option<LcuEvents>,
    session: Option<LcuSession>,
    sessions: u64,
    /// Falhas seguidas desde a última conexão.
    failures: u32,
    /// Quando a próxima tentativa pode sair.
    retry_at: Option<Instant>,
}

impl LcuConnection {
    /// Cria a conexão. Nada acontece até o primeiro [`LcuConnection::next`].
    pub fn new(config: ReconnectConfig) -> Self {
        Self {
            config,
            events: None,
            session: None,
            sessions: 0,
            failures: 0,
            retry_at: None,
        }
    }

    pub fn config(&self) -> &ReconnectConfig {
        &self.config
    }

    /// A conexão atual, se houver.
    pub fn session(&self) -> Option<&LcuSession> {
        self.session.as_ref()
    }

    pub fn is_connected(&self) -> bool {
        self.events.is_some()
    }

    /// Troca o filtro, inclusive da conexão aberta, sem reconectar.
    pub fn set_filter(&mut self, filter: EventFilter) {
        if let Some(events) = &mut self.events {
            events.set_filter(filter.clone());
        }
        self.config.filter = filter;
    }

    /// Próxima mudança: um evento, uma conexão nova, uma queda ou uma
    /// tentativa que falhou. Nunca termina por conta própria.
    pub async fn next(&mut self) -> ConnectionEvent {
        if let Some(events) = &mut self.events {
            let reason = match events.next().await {
                Some(Ok(event)) => return ConnectionEvent::Event(event),
                Some(Err(error)) => DisconnectReason::Socket(error),
                None => DisconnectReason::Closed,
            };
            self.events = None;
            self.session = None;
            return ConnectionEvent::Disconnected(reason);
        }

        if let Some(at) = self.retry_at {
            tokio::time::sleep_until(at).await;
        }
        match self.connect().await {
            Ok((events, session)) => {
                self.events = Some(events);
                self.session = Some(session.clone());
                self.failures = 0;
                self.retry_at = None;
                ConnectionEvent::Connected(session)
            }
            Err(error) => {
                self.failures = self.failures.saturating_add(1);
                let delay =
                    retry_delay(self.failures, self.config.retry_min, self.config.retry_max);
                self.retry_at = Some(Instant::now() + delay);
                ConnectionEvent::Retrying {
                    attempt: self.failures,
                    delay,
                    error,
                }
            }
        }
    }

    /// Fecha a conexão aberta, se houver.
    pub async fn close(mut self) {
        if let Some(events) = self.events.take() {
            events.close().await;
        }
    }

    async fn connect(&mut self) -> Result<(LcuEvents, LcuSession), ConnectError> {
        let (credentials, root_pem) = load_credentials(&self.config.source).await?;
        let client = LcuClient::with_root_certificate(&credentials, &root_pem)?;
        let events = LcuEvents::build(
            &credentials,
            &root_pem,
            self.config.filter.clone(),
            self.config.connect_timeout,
        )
        .await?;
        self.sessions += 1;
        let session = LcuSession {
            id: self.sessions,
            credentials,
            client,
        };
        Ok((events, session))
    }
}

impl fmt::Debug for LcuConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LcuConnection")
            .field("config", &self.config)
            .field("session", &self.session.as_ref().map(|s| s.id))
            .field("failures", &self.failures)
            .finish()
    }
}

/// Credenciais e certificado fixado para uma tentativa.
async fn load_credentials(
    source: &CredentialSource,
) -> Result<(Credentials, Vec<u8>), ConnectError> {
    match source {
        CredentialSource::Discover => {
            // Listar os processos é bloqueante e pode levar dezenas de ms.
            let credentials = tokio::task::spawn_blocking(lockfile::discover)
                .await
                .unwrap_or(Err(DiscoveryError::ClientNotRunning))?;
            Ok((credentials, RIOT_ROOT_PEM.to_vec()))
        }
        CredentialSource::Lockfile(path) => {
            let credentials = Credentials::from_lockfile_path(path)?;
            let root_pem = root_for_lockfile(path, credentials.protocol)?;
            Ok((credentials, root_pem))
        }
    }
}

/// O [`MOCK_CA_FILE`] ao lado do lockfile, se existir; senão o `riotgames.pem`.
fn root_for_lockfile(path: &Path, protocol: Protocol) -> Result<Vec<u8>, ConnectError> {
    if protocol == Protocol::Http {
        return Ok(RIOT_ROOT_PEM.to_vec());
    }
    let ca_path = path.with_file_name(MOCK_CA_FILE);
    match std::fs::read(&ca_path) {
        Ok(pem) => Ok(pem),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(RIOT_ROOT_PEM.to_vec()),
        Err(source) => Err(ConnectError::Certificate {
            path: ca_path,
            source,
        }),
    }
}

/// Espera depois de `failures` falhas seguidas: `min`, `2·min`, `4·min`...
/// até `max`.
fn retry_delay(failures: u32, min: Duration, max: Duration) -> Duration {
    let doublings = failures.saturating_sub(1).min(16);
    min.saturating_mul(1 << doublings).min(max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn espera_dobra_ate_o_teto() {
        let min = Duration::from_millis(500);
        let max = Duration::from_secs(5);
        let delays: Vec<u64> = (1..=7)
            .map(|n| retry_delay(n, min, max).as_millis() as u64)
            .collect();
        assert_eq!(delays, [500, 1000, 2000, 4000, 5000, 5000, 5000]);
        assert_eq!(retry_delay(u32::MAX, min, max), max);
    }

    #[test]
    fn espera_com_teto_menor_que_o_minimo() {
        let d = retry_delay(1, Duration::from_secs(2), Duration::from_secs(1));
        assert_eq!(d, Duration::from_secs(1));
    }

    #[test]
    fn lockfile_sem_ca_ao_lado_usa_o_riotgames_pem() {
        let dir =
            std::env::temp_dir().join(format!("easylobby-conn-sem-ca-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lockfile");
        assert_eq!(
            root_for_lockfile(&path, Protocol::Https).unwrap(),
            RIOT_ROOT_PEM
        );
        std::fs::write(dir.join(MOCK_CA_FILE), "ca do mock").unwrap();
        assert_eq!(
            root_for_lockfile(&path, Protocol::Https).unwrap(),
            b"ca do mock"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn lockfile_que_nao_existe_e_lol_fechado() {
        let path = std::env::temp_dir().join("easylobby-conn-nao-existe/lockfile");
        let mut conn = LcuConnection::new(ReconnectConfig {
            retry_min: Duration::from_millis(1),
            retry_max: Duration::from_millis(2),
            ..ReconnectConfig::with_source(CredentialSource::Lockfile(path))
        });
        for attempt in 1..=3 {
            match conn.next().await {
                ConnectionEvent::Retrying {
                    attempt: n, error, ..
                } => {
                    assert_eq!(n, attempt);
                    assert!(error.is_client_not_running(), "{error}");
                }
                other => panic!("esperava Retrying: {other:?}"),
            }
        }
        assert!(!conn.is_connected());
        assert!(conn.session().is_none());
    }
}
