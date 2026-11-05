//! Mostra os eventos da LCU em tempo real (#9), no LoL real ou no mock-lcu.
//!
//! No LoL real, aberto e logado:
//!
//! ```bash
//! cargo run -p easylobby-agent --example eventos
//! ```
//!
//! No mock-lcu, com `cargo run -p mock-lcu` rodando em outro terminal:
//!
//! ```bash
//! cargo run -p easylobby-agent --example eventos -- --mock
//! cargo run -p easylobby-agent --example eventos -- --lockfile <caminho do lockfile>
//! ```
//!
//! Com `--todos`, mostra também os eventos que o easylobby não usa. Com
//! `--lockfile`, se o lockfile diz `https` e há um `mock-lcu-ca.pem` ao lado
//! dele, essa CA é fixada no lugar do `riotgames.pem`. A verificação TLS nunca
//! é desligada.
//!
//! O example reconecta sozinho (#10): pode abrir antes do LoL, e fechar e
//! reabrir o LoL (ou o mock-lcu) não o derruba. Ctrl+C para sair.

use std::path::PathBuf;

use easylobby_agent::connection::{
    ConnectionEvent, CredentialSource, LcuConnection, ReconnectConfig,
};
use easylobby_agent::lockfile;
use easylobby_agent::wamp::{EventFilter, LcuEvent};
use serde_json::Value;

/// Tamanho máximo do `data` impresso em cada linha.
const MAX_DATA_CHARS: usize = 100;

const USAGE: &str = "\
uso: eventos [--mock | --lockfile <arquivo>] [--todos]

  (sem opções)          acha o LoL aberto e usa o riotgames.pem
  --mock                usa o lockfile padrão do mock-lcu
                        (<temp>/easylobby-mock-lcu/lockfile)
  --lockfile <arquivo>  usa este lockfile; fixa o mock-lcu-ca.pem ao lado
                        dele, se existir
  --todos               mostra todos os eventos, não só os que o easylobby usa
";

struct Args {
    lockfile: Option<PathBuf>,
    all: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args = parse_args()?;
    let filter = if args.all {
        EventFilter::all()
    } else {
        EventFilter::easylobby()
    };
    let source = match args.lockfile {
        None => CredentialSource::Discover,
        Some(path) => CredentialSource::Lockfile(path),
    };
    let mut lcu = LcuConnection::new(ReconnectConfig {
        filter,
        ..ReconnectConfig::with_source(source)
    });
    println!("Procurando o LoL (Ctrl+C para sair)");

    // Repete o motivo da falha só quando ele muda.
    let mut last_error = String::new();
    loop {
        tokio::select! {
            event = lcu.next() => match event {
                ConnectionEvent::Connected(session) => {
                    last_error.clear();
                    let phase = session
                        .client
                        .get::<String>("/lol-gameflow/v1/gameflow-phase")
                        .await
                        .unwrap_or_else(|e| format!("? ({e})"));
                    println!(
                        "Conectado (#{}) na porta {}, fase {phase}",
                        session.id, session.credentials.port
                    );
                }
                ConnectionEvent::Event(event) => print_event(&event),
                ConnectionEvent::Disconnected(reason) => {
                    println!("Desconectado: {reason}. Procurando o LoL de novo.")
                }
                ConnectionEvent::Retrying { error, .. } => {
                    let error = error.to_string();
                    if error != last_error {
                        println!("{error}");
                        last_error = error;
                    }
                }
            },
            _ = tokio::signal::ctrl_c() => {
                lcu.close().await;
                return Ok(());
            }
        }
    }
}

fn print_event(event: &LcuEvent) {
    let data = match &event.data {
        Value::Null => String::new(),
        data => {
            let text = data.to_string();
            match text.char_indices().nth(MAX_DATA_CHARS) {
                Some((cut, _)) => format!(" {}...", &text[..cut]),
                None => format!(" {text}"),
            }
        }
    };
    println!("{:?} {}{data}", event.event_type, event.uri);
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        lockfile: None,
        all: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--mock" => {
                args.lockfile = Some(
                    std::env::temp_dir()
                        .join("easylobby-mock-lcu")
                        .join(lockfile::LOCKFILE_NAME),
                )
            }
            "--lockfile" => {
                args.lockfile = Some(PathBuf::from(
                    it.next().ok_or("--lockfile precisa de um caminho")?,
                ))
            }
            "--todos" => args.all = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("opção desconhecida: {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}
