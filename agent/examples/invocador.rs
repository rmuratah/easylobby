//! Mostra o invocador logado (#7), no LoL real ou no mock-lcu.
//!
//! No LoL real, aberto e logado:
//!
//! ```bash
//! cargo run -p easylobby-agent --example invocador
//! ```
//!
//! No mock-lcu, com `cargo run -p mock-lcu` rodando em outro terminal:
//!
//! ```bash
//! cargo run -p easylobby-agent --example invocador -- --mock
//! cargo run -p easylobby-agent --example invocador -- --lockfile <caminho do lockfile>
//! ```
//!
//! Com `--lockfile`, se o lockfile diz `https` e há um `mock-lcu-ca.pem` ao
//! lado dele, essa CA é fixada no lugar do `riotgames.pem`. A verificação TLS
//! nunca é desligada.

use std::path::{Path, PathBuf};

use easylobby_agent::lockfile::{self, Credentials, Protocol};
use easylobby_agent::rest::LcuClient;
use easylobby_agent::summoner;

/// Arquivo da CA que o mock-lcu grava ao lado do lockfile.
const MOCK_CA_FILE: &str = "mock-lcu-ca.pem";

const USAGE: &str = "\
uso: invocador [--mock | --lockfile <arquivo>]

  (sem opções)          acha o LoL aberto e usa o riotgames.pem
  --mock                usa o lockfile padrão do mock-lcu
                        (<temp>/easylobby-mock-lcu/lockfile)
  --lockfile <arquivo>  usa este lockfile; fixa o mock-lcu-ca.pem ao lado
                        dele, se existir
";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let lockfile = parse_args()?;
    let (credentials, lcu) = match lockfile {
        None => {
            let credentials = lockfile::discover().map_err(|e| e.to_string())?;
            let lcu = LcuClient::new(&credentials).map_err(|e| e.to_string())?;
            (credentials, lcu)
        }
        Some(path) => client_from_lockfile(&path)?,
    };
    println!("LCU em {}", credentials.base_url());

    let summoner = summoner::current_summoner(&lcu)
        .await
        .map_err(|e| match e.status() {
            Some(404) => format!("{e}\nO client está aberto, mas ninguém fez login ainda?"),
            _ => e.to_string(),
        })?;
    println!("Invocador: {summoner}");
    Ok(())
}

fn parse_args() -> Result<Option<PathBuf>, String> {
    let mut it = std::env::args().skip(1);
    let lockfile = match it.next().as_deref() {
        None => None,
        Some("--mock") => Some(
            std::env::temp_dir()
                .join("easylobby-mock-lcu")
                .join(lockfile::LOCKFILE_NAME),
        ),
        Some("--lockfile") => Some(PathBuf::from(
            it.next().ok_or("--lockfile precisa de um caminho")?,
        )),
        Some("-h" | "--help") => {
            print!("{USAGE}");
            std::process::exit(0);
        }
        Some(other) => return Err(format!("opção desconhecida: {other}\n\n{USAGE}")),
    };
    if let Some(extra) = it.next() {
        return Err(format!("opção a mais: {extra}\n\n{USAGE}"));
    }
    Ok(lockfile)
}

fn client_from_lockfile(path: &Path) -> Result<(Credentials, LcuClient), String> {
    let credentials = Credentials::from_lockfile_path(path).map_err(|e| e.to_string())?;
    let ca_path = path.with_file_name(MOCK_CA_FILE);
    let lcu = match (credentials.protocol, std::fs::read(&ca_path)) {
        (Protocol::Https, Ok(ca_pem)) => {
            println!("CA fixada: {}", ca_path.display());
            LcuClient::with_root_certificate(&credentials, &ca_pem)
        }
        _ => LcuClient::new(&credentials),
    }
    .map_err(|e| e.to_string())?;
    Ok((credentials, lcu))
}
