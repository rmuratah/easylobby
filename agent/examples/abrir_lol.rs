//! Abre o LoL pelo Riot Client (#11) e espera ele conectar.
//!
//! ```bash
//! cargo run -p easylobby-agent --example abrir_lol
//! cargo run -p easylobby-agent --example abrir_lol -- --riot-client "D:\Riot Games\Riot Client\RiotClientServices.exe"
//! cargo run -p easylobby-agent --example abrir_lol -- --patchline pbe
//! ```
//!
//! Se o LoL já está aberto, só conecta. Senão, chama o Riot Client e espera o
//! LoL subir (e o login, se o Riot Client pedir) até mostrar o invocador
//! logado. Ctrl+C para sair.

use std::path::PathBuf;
use std::time::Duration;

use easylobby_agent::connection::{ConnectionEvent, LcuConnection, ReconnectConfig};
use easylobby_agent::launcher::{self, LaunchOptions, LaunchOutcome};
use easylobby_agent::summoner;

const USAGE: &str = "\
uso: abrir_lol [--riot-client <RiotClientServices.exe>] [--patchline <patchline>]

  --riot-client <arquivo>  usa este Riot Client em vez de procurar
  --patchline <nome>       patchline do LoL (padrão: live)
";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let options = parse_args()?;
    let outcome = tokio::task::spawn_blocking(move || launcher::launch_league(&options))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    match outcome {
        LaunchOutcome::AlreadyRunning { pid } => println!("O LoL já está aberto (pid {pid})"),
        LaunchOutcome::Launched { riot_client, pid } => println!(
            "Riot Client chamado (pid {pid}): {} ({:?})",
            riot_client.path.display(),
            riot_client.source
        ),
    }

    println!("Esperando o LoL conectar (Ctrl+C para sair)");
    let mut lcu = LcuConnection::new(ReconnectConfig::default());
    let wait = async {
        loop {
            if let ConnectionEvent::Connected(session) = lcu.next().await {
                println!("Conectado na porta {}", session.credentials.port);
                // Logo depois de abrir, o invocador ainda não carregou (404).
                loop {
                    match summoner::current_summoner(&session.client).await {
                        Ok(s) => return println!("Invocador: {s}"),
                        Err(e) if e.status() == Some(404) => {
                            tokio::time::sleep(Duration::from_secs(1)).await
                        }
                        Err(e) => {
                            println!("{e}; esperando reconectar");
                            break;
                        }
                    }
                }
            }
        }
    };
    tokio::select! {
        _ = wait => {}
        _ = tokio::signal::ctrl_c() => {}
    }
    lcu.close().await;
    Ok(())
}

fn parse_args() -> Result<LaunchOptions, String> {
    let mut options = LaunchOptions::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--riot-client" => {
                options.riot_client = Some(PathBuf::from(
                    it.next().ok_or("--riot-client precisa de um caminho")?,
                ))
            }
            "--patchline" => {
                options.patchline = it.next().ok_or("--patchline precisa de um nome")?
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("opção desconhecida: {other}\n\n{USAGE}")),
        }
    }
    Ok(options)
}
