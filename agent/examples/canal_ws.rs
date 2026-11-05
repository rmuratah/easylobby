//! Sobe o canal `/ws` do agente (#15), no LoL real ou no mock-lcu, para
//! desenvolver a UI ou olhar as mensagens.
//!
//! No mock-lcu, com `cargo run -p mock-lcu` rodando em outro terminal:
//!
//! ```bash
//! cargo run -p easylobby-agent --example canal_ws -- --mock
//! ```
//!
//! No LoL real:
//!
//! ```bash
//! cargo run -p easylobby-agent --example canal_ws
//! ```
//!
//! O canal fica em `ws://127.0.0.1:47800/ws` (`--porta` muda). A UI em modo
//! dev (`http://localhost:5173`) e a janela do Tauri podem conectar; outras
//! origens recebem 403. Cada mensagem mandada para as UIs também sai no
//! terminal. O agente reconecta sozinho quando o LoL fecha e reabre. Ctrl+C
//! para sair.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use easylobby_agent::connection::{CredentialSource, LcuConnection, ReconnectConfig};
use easylobby_agent::lockfile;
use easylobby_agent::ws::{self, Hub, WsConfig};
use tokio::net::TcpListener;

const USAGE: &str = "\
uso: canal_ws [--mock | --lockfile <arquivo>] [--porta <n>]

  (sem opções)          acha o LoL aberto e usa o riotgames.pem
  --mock                usa o lockfile padrão do mock-lcu
                        (<temp>/easylobby-mock-lcu/lockfile)
  --lockfile <arquivo>  usa este lockfile; fixa o mock-lcu-ca.pem ao lado
                        dele, se existir
  --porta <n>           porta do /ws em 127.0.0.1 (padrão: 47800)
";

struct Args {
    lockfile: Option<PathBuf>,
    port: u16,
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
    let source = match args.lockfile {
        None => CredentialSource::Discover,
        Some(path) => CredentialSource::Lockfile(path),
    };
    let lcu = LcuConnection::new(ReconnectConfig::with_source(source));

    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, args.port));
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| format!("não foi possível abrir {addr}: {e}"))?;
    println!("Canal em ws://{addr}{} (Ctrl+C para sair)", ws::WS_PATH);

    let hub = Hub::new();
    tokio::spawn(ws::drive(lcu, hub.clone()));
    tokio::spawn(print_messages(hub.clone()));
    tokio::select! {
        result = ws::serve(listener, hub, WsConfig::default()) => {
            result.map_err(|e| e.to_string())
        }
        _ = tokio::signal::ctrl_c() => Ok(()),
    }
}

/// Mostra no terminal o que as UIs conectadas recebem.
async fn print_messages(hub: Hub) {
    let (state, mut changes) = hub.subscribe();
    println!(
        "estado: {}",
        serde_json::to_string(&state).unwrap_or_default()
    );
    while let Ok(message) = changes.recv().await {
        println!("{}", serde_json::to_string(&message).unwrap_or_default());
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        lockfile: None,
        port: ws::DEFAULT_PORT,
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
            "--porta" => {
                args.port = it
                    .next()
                    .ok_or("--porta precisa de um número")?
                    .parse()
                    .map_err(|_| "--porta precisa de um número de 1 a 65535")?
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("opção desconhecida: {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}
