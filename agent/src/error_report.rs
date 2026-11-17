//! Envio dos erros do app para uma base remota durante o alfa fechado (#550).
//!
//! O log em arquivo (#534) só ajuda quando o jogador manda o arquivo. Aqui os
//! eventos de nível **erro** do `tracing` (erros do agente, do shell, da UI e
//! os pânicos) também vão para uma tabela do Supabase, onde os devs leem.
//!
//! - Vai só o que já entra no log local, com as mesmas regras: nunca a senha
//!   do lockfile, o cabeçalho `Authorization`, a chave da Riot nem o texto do
//!   chat. A pasta do usuário do Windows vira `~` antes de sair do PC.
//! - Junto vão a versão do app, o sistema e um id aleatório da instalação,
//!   para contar quantos jogadores tiveram o mesmo erro. Nada de nick, e-mail
//!   ou puuid.
//! - A chave embutida é a publicável do Supabase. A tabela tem RLS e essa
//!   chave **só insere**: não lê, não muda e não apaga nada
//!   (`docs/supabase/error_reports.sql`).
//! - Os erros saem em lote, numa thread própria, a cada [`BATCH_WINDOW`].
//!   A mesma mensagem não sai duas vezes na mesma sessão, e uma sessão manda
//!   no máximo [`MAX_PER_SESSION`] erros. Uma falha de rede só perde o lote:
//!   nunca trava nem derruba o app.
//! - O jogador desliga em Configurações; desligado, nada sai a partir dali.

use std::cell::Cell;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// Endereço do projeto "easylobby" do Supabase que recebe os erros. Vazio
/// desliga o envio.
pub const DEFAULT_URL: &str = "https://mmmzvopxfknjncjxmzzg.supabase.co";
/// Chave publicável do projeto (`sb_publishable_...`). Pela RLS, ela só
/// consegue inserir na tabela; não é segredo.
pub const DEFAULT_KEY: &str = "sb_publishable_VovbSBvwMFPr60AABBxhOw_O_B2HPpE";
/// Variáveis que trocam o endereço e a chave (para testar com outro projeto).
pub const URL_ENV: &str = "EASYLOBBY_ERROR_REPORT_URL";
pub const KEY_ENV: &str = "EASYLOBBY_ERROR_REPORT_KEY";

/// Caminho da tabela na API REST do Supabase.
pub const TABLE_PATH: &str = "/rest/v1/error_reports";

/// Quanto a thread espera juntando erros antes de mandar o lote.
pub const BATCH_WINDOW: Duration = Duration::from_secs(10);
/// Erros por lote.
pub const MAX_BATCH: usize = 20;
/// Erros por sessão do app; o resto só fica no log local.
pub const MAX_PER_SESSION: usize = 100;
/// Tamanho máximo da mensagem enviada.
pub const MAX_MESSAGE_CHARS: usize = 4_000;
/// Erros esperando na fila; acima disso são descartados.
const QUEUE: usize = 256;
/// Tempo máximo de um envio.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Onde e como mandar os erros.
#[derive(Clone, PartialEq, Eq)]
pub struct ReportConfig {
    /// Endereço do projeto, como `https://xxxx.supabase.co`.
    pub url: String,
    /// Chave publicável.
    pub key: String,
    /// Versão do app.
    pub app_version: String,
    /// Sistema, como `windows`.
    pub os: String,
    /// Id aleatório desta instalação.
    pub install_id: String,
}

// A chave não precisa aparecer em lugar nenhum.
impl std::fmt::Debug for ReportConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReportConfig")
            .field("url", &self.url)
            .field("app_version", &self.app_version)
            .field("os", &self.os)
            .field("install_id", &self.install_id)
            .finish_non_exhaustive()
    }
}

impl ReportConfig {
    /// Endereço e chave das variáveis ou das constantes. `None` quando não há
    /// projeto configurado: o envio fica desligado.
    pub fn load(install_id: String) -> Option<Self> {
        let env = |name: &str| {
            std::env::var(name)
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        };
        let url = env(URL_ENV).unwrap_or_else(|| DEFAULT_URL.to_owned());
        let key = env(KEY_ENV).unwrap_or_else(|| DEFAULT_KEY.to_owned());
        if url.is_empty() || key.is_empty() {
            return None;
        }
        Some(Self {
            url: url.trim_end_matches('/').to_owned(),
            key,
            app_version: crate::VERSION.to_owned(),
            os: std::env::consts::OS.to_owned(),
            install_id,
        })
    }
}

/// Um erro como vai para a tabela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub install_id: String,
    pub app_version: String,
    pub os: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

/// Liga e desliga o envio (opção das Configurações).
#[derive(Debug, Clone)]
pub struct ReportSwitch(Arc<AtomicBool>);

impl ReportSwitch {
    pub fn new(enabled: bool) -> Self {
        Self(Arc::new(AtomicBool::new(enabled)))
    }

    pub fn is_enabled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub fn set(&self, enabled: bool) {
        self.0.store(enabled, Ordering::SeqCst);
    }
}

thread_local! {
    /// Marca a thread que envia: o que o `reqwest` registrar nela não volta
    /// para a fila.
    static SENDING: Cell<bool> = const { Cell::new(false) };
}

/// Camada do `tracing` que põe os erros na fila de envio.
pub struct ReportLayer {
    config: ReportConfig,
    switch: ReportSwitch,
    queue: SyncSender<Report>,
    home: Option<String>,
}

/// Cria a camada e a thread que manda os erros.
pub fn layer(config: ReportConfig, switch: ReportSwitch) -> ReportLayer {
    let (queue, rx) = mpsc::sync_channel(QUEUE);
    {
        let (config, switch) = (config.clone(), switch.clone());
        let spawned = std::thread::Builder::new()
            .name("easylobby-error-report".into())
            .spawn(move || run_sender(config, switch, rx, BATCH_WINDOW));
        if let Err(error) = spawned {
            eprintln!("easylobby: não deu para iniciar o envio de erros: {error}");
        }
    }
    ReportLayer {
        config,
        switch,
        queue,
        home: home_dir(),
    }
}

fn home_dir() -> Option<String> {
    ["USERPROFILE", "HOME"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|v| v.len() > 3)
}

impl<S: Subscriber> Layer<S> for ReportLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        if *meta.level() != Level::ERROR
            || meta.target().starts_with(module_path!())
            || SENDING.with(Cell::get)
            || !self.switch.is_enabled()
        {
            return;
        }
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let message = clean_message(&visitor.finish(), self.home.as_deref());
        let report = Report {
            install_id: self.config.install_id.clone(),
            app_version: self.config.app_version.clone(),
            os: self.config.os.clone(),
            level: meta.level().to_string(),
            target: meta.target().to_owned(),
            message,
        };
        // Fila cheia ou thread parada: o erro fica só no log local.
        let _ = self.queue.try_send(report);
    }
}

/// Junta a mensagem e os outros campos do evento (`erro=...`).
#[derive(Default)]
struct MessageVisitor {
    message: String,
    fields: String,
}

impl MessageVisitor {
    fn finish(self) -> String {
        match (self.message.is_empty(), self.fields.is_empty()) {
            (_, true) => self.message,
            (true, false) => self.fields,
            (false, false) => format!("{} {}", self.message, self.fields),
        }
    }
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.message, "{value:?}");
        } else {
            if !self.fields.is_empty() {
                self.fields.push(' ');
            }
            let _ = write!(self.fields, "{}={value:?}", field.name());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else {
            self.record_debug(field, &value);
        }
    }
}

/// Troca a pasta do usuário por `~` e corta a mensagem em
/// [`MAX_MESSAGE_CHARS`].
pub fn clean_message(message: &str, home: Option<&str>) -> String {
    let mut text = match home {
        Some(home) if !home.is_empty() => {
            let text = message.replace(home, "~");
            // O Windows aceita as duas barras.
            text.replace(&home.replace('\\', "/"), "~")
        }
        _ => message.to_owned(),
    };
    if text.chars().count() > MAX_MESSAGE_CHARS {
        text = text.chars().take(MAX_MESSAGE_CHARS).collect();
        text.push('…');
    }
    text
}

/// Decide o que sai: no máximo [`MAX_PER_SESSION`] erros, sem repetir.
#[derive(Debug, Default)]
pub struct SessionLimit {
    seen: HashSet<(String, String)>,
    sent: usize,
}

impl SessionLimit {
    /// `true` quando o erro deve ser enviado.
    pub fn accept(&mut self, report: &Report) -> bool {
        if self.sent >= MAX_PER_SESSION {
            return false;
        }
        if !self
            .seen
            .insert((report.target.clone(), report.message.clone()))
        {
            return false;
        }
        self.sent += 1;
        true
    }
}

/// Laço da thread de envio: junta um lote e manda.
fn run_sender(config: ReportConfig, switch: ReportSwitch, rx: Receiver<Report>, window: Duration) {
    SENDING.with(|s| s.set(true));
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("easylobby: envio de erros sem runtime: {error}");
            return;
        }
    };
    let client = match reqwest::Client::builder().timeout(TIMEOUT).build() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("easylobby: envio de erros sem cliente HTTP: {error}");
            return;
        }
    };
    let endpoint = format!("{}{TABLE_PATH}", config.url);
    let mut limit = SessionLimit::default();
    // Um erro abre o lote; os que chegam na janela vão junto.
    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        let deadline = Instant::now() + window;
        while batch.len() < MAX_BATCH {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(report) => batch.push(report),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        if !switch.is_enabled() {
            continue;
        }
        batch.retain(|report| limit.accept(report));
        if batch.is_empty() {
            continue;
        }
        if let Err(error) = runtime.block_on(send(&client, &endpoint, &config.key, &batch)) {
            eprintln!(
                "easylobby: não deu para enviar {} erro(s): {error}",
                batch.len()
            );
        }
    }
}

/// Insere o lote na tabela. `return=minimal`: a chave não pode ler a linha
/// de volta.
async fn send(
    client: &reqwest::Client,
    endpoint: &str,
    key: &str,
    batch: &[Report],
) -> Result<(), String> {
    let body = serde_json::to_vec(batch).map_err(|error| error.to_string())?;
    let response = client
        .post(endpoint)
        .header("apikey", key)
        .header("Content-Type", "application/json")
        .header("Prefer", "return=minimal")
        .body(body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("o servidor respondeu {}", response.status()))
    }
}

/// Id aleatório novo para uma instalação, no formato de um UUID v4.
pub fn new_install_id() -> String {
    use std::hash::{BuildHasher, Hasher};
    // O `RandomState` nasce com sementes aleatórias do sistema.
    let mut bytes = [0u8; 16];
    for (i, chunk) in bytes.chunks_mut(8).enumerate() {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_usize(i);
        hasher.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default(),
        );
        chunk.copy_from_slice(&hasher.finish().to_le_bytes());
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tracing_subscriber::prelude::*;

    fn report(target: &str, message: &str) -> Report {
        Report {
            install_id: "id".into(),
            app_version: "0.1.0".into(),
            os: "windows".into(),
            level: "ERROR".into(),
            target: target.into(),
            message: message.into(),
        }
    }

    #[test]
    fn nao_repete_a_mesma_mensagem() {
        let mut limit = SessionLimit::default();
        assert!(limit.accept(&report("a", "erro 1")));
        assert!(!limit.accept(&report("a", "erro 1")));
        assert!(limit.accept(&report("b", "erro 1")));
    }

    #[test]
    fn para_no_limite_da_sessao() {
        let mut limit = SessionLimit::default();
        for n in 0..MAX_PER_SESSION {
            assert!(limit.accept(&report("a", &n.to_string())));
        }
        assert!(!limit.accept(&report("a", "mais um")));
    }

    #[test]
    fn esconde_a_pasta_do_usuario_e_corta() {
        let text = clean_message(
            r"falhou em C:\Users\Fulano\AppData e C:/Users/Fulano/x",
            Some(r"C:\Users\Fulano"),
        );
        assert_eq!(text, r"falhou em ~\AppData e ~/x");
        let long = "a".repeat(MAX_MESSAGE_CHARS + 10);
        assert_eq!(
            clean_message(&long, None).chars().count(),
            MAX_MESSAGE_CHARS + 1
        );
    }

    #[test]
    fn id_da_instalacao_e_um_uuid_v4() {
        let id = new_install_id();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        assert_ne!(id, new_install_id());
    }

    #[test]
    fn config_padrao_aponta_para_o_projeto() {
        if std::env::var(URL_ENV).is_err() && std::env::var(KEY_ENV).is_err() {
            let config = ReportConfig::load("id".into()).unwrap();
            assert_eq!(config.url, DEFAULT_URL);
            assert!(config.key.starts_with("sb_publishable_"));
            assert!(!format!("{config:?}").contains(&config.key));
        }
    }

    /// Servidor HTTP mínimo que guarda o corpo e os cabeçalhos de cada POST.
    fn fake_server() -> (String, Arc<Mutex<Vec<String>>>) {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut head = String::new();
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                    head.push_str(&line);
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                seen.lock()
                    .unwrap()
                    .push(format!("{head}\n{}", String::from_utf8_lossy(&body)));
                let _ = stream.write_all(
                    b"HTTP/1.1 201 Created\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        });
        (format!("http://{addr}"), requests)
    }

    fn config(url: String) -> ReportConfig {
        ReportConfig {
            url,
            key: "sb_publishable_teste".into(),
            app_version: "0.1.0".into(),
            os: "windows".into(),
            install_id: "id-teste".into(),
        }
    }

    fn wait_for(requests: &Arc<Mutex<Vec<String>>>, count: usize) -> Vec<String> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let current = requests.lock().unwrap().clone();
            if current.len() >= count {
                return current;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        requests.lock().unwrap().clone()
    }

    fn reporting_layer(url: String, switch: ReportSwitch) -> ReportLayer {
        let (queue, rx) = mpsc::sync_channel(QUEUE);
        let cfg = config(url);
        {
            let (cfg, switch) = (cfg.clone(), switch.clone());
            std::thread::spawn(move || {
                run_sender(cfg, switch, rx, Duration::from_millis(100));
            });
        }
        ReportLayer {
            config: cfg,
            switch,
            queue,
            home: None,
        }
    }

    #[test]
    fn manda_so_os_erros_em_lote() {
        let (url, requests) = fake_server();
        let switch = ReportSwitch::new(true);
        let subscriber = tracing_subscriber::registry().with(reporting_layer(url, switch));
        tracing::subscriber::with_default(subscriber, || {
            tracing::warn!("só um aviso");
            tracing::error!(target: "easylobby_ui", "[exceção] quebrou");
            tracing::error!(target: "easylobby_agent::lobby", erro = 42, "falhou");
            tracing::error!(target: "easylobby_ui", "[exceção] quebrou");
        });
        let got = wait_for(&requests, 1);
        assert_eq!(got.len(), 1, "{got:?}");
        let request = &got[0];
        assert!(
            request.starts_with("POST /rest/v1/error_reports"),
            "{request}"
        );
        assert!(
            request.contains("apikey: sb_publishable_teste"),
            "{request}"
        );
        assert!(request.contains("prefer: return=minimal"), "{request}");
        let body = request.split("\r\n\n").last().unwrap_or(request);
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(body.rsplit('\n').next().unwrap()).unwrap();
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert_eq!(rows[0]["target"], "easylobby_ui");
        assert_eq!(rows[0]["message"], "[exceção] quebrou");
        assert_eq!(rows[0]["install_id"], "id-teste");
        assert_eq!(rows[0]["level"], "ERROR");
        assert_eq!(rows[1]["message"], "falhou erro=42");
    }

    #[test]
    fn desligado_nao_manda_nada() {
        let (url, requests) = fake_server();
        let switch = ReportSwitch::new(false);
        let subscriber = tracing_subscriber::registry().with(reporting_layer(url, switch.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!("não sai");
        });
        std::thread::sleep(Duration::from_millis(400));
        assert!(requests.lock().unwrap().is_empty());
    }
}
