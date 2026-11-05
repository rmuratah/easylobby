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
//! é desligada. Ctrl+C para sair.

use std::path::PathBuf;

use easylobby_agent::lockfile::{self, Credentials, Protocol};
use easylobby_agent::wamp::{EventFilter, LcuEvent, LcuEvents};
use serde_json::Value;

/// Arquivo da CA que o mock-lcu grava ao lado do lockfile.
const MOCK_CA_FILE: &str = "mock-lcu-ca.pem";

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
    let mut events = match args.lockfile {
        None => {
            let credentials = lockfile::discover().map_err(|e| e.to_string())?;
            LcuEvents::connect(&credentials, filter).await
        }
        Some(path) => {
            let credentials = Credentials::from_lockfile_path(&path).map_err(|e| e.to_string())?;
            let ca_path = path.with_file_name(MOCK_CA_FILE);
            match (credentials.protocol, std::fs::read(&ca_path)) {
                (Protocol::Https, Ok(ca_pem)) => {
                    println!("CA fixada: {}", ca_path.display());
                    LcuEvents::with_root_certificate(&credentials, &ca_pem, filter).await
                }
                _ => LcuEvents::connect(&credentials, filter).await,
            }
        }
    }
    .map_err(|e| e.to_string())?;
    println!("Inscrito em {} (Ctrl+C para sair)", events.url());

    loop {
        tokio::select! {
            event = events.next() => match event {
                Some(Ok(event)) => print_event(&event),
                Some(Err(e)) => return Err(e.to_string()),
                None => {
                    println!("A LCU fechou a conexão.");
                    return Ok(());
                }
            },
            _ = tokio::signal::ctrl_c() => {
                events.close().await;
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
