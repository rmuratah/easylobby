//! Linha de comando do mock-lcu (#6).
//!
//! ```bash
//! cargo run -p mock-lcu
//! cargo run -p mock-lcu -- --fixtures exemplo --velocidade 10 --repetir
//! cargo run -p mock-lcu -- --http --porta 2999 --verbose
//! ```
//!
//! Veja `mock-lcu/README.md` para todas as opções.

use std::path::{Path, PathBuf};

use easylobby_agent::fixtures::FixtureSet;
use mock_lcu::{Config, MockLcu, ReplayStart};

const USAGE: &str = "\
uso: mock-lcu [opções]

  --fixtures <patch|pasta>  conjunto em agent/tests/fixtures/ ou caminho de uma
                            pasta (padrão: a gravação real mais nova)
  --porta <n>               porta em 127.0.0.1 (padrão: uma livre)
  --senha <texto>           senha do Basic Auth (padrão: aleatória)
  --lockfile <arquivo>      onde escrever o lockfile
                            (padrão: <temp>/easylobby-mock-lcu/lockfile)
  --http                    HTTP puro em vez de HTTPS
  --velocidade <x>          reproduz os eventos x vezes mais rápido (padrão: 1)
  --iniciar-ja              reproduz sem esperar a primeira inscrição WAMP
  --parado                  não reproduz os eventos
  --repetir                 recomeça a gravação quando ela acaba
  --verbose                 mostra cada requisição e evento
";

struct Args {
    fixtures: Option<String>,
    port: u16,
    password: Option<String>,
    lockfile: Option<PathBuf>,
    http: bool,
    speed: f64,
    replay_start: ReplayStart,
    repeat: bool,
    verbose: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        fixtures: None,
        port: 0,
        password: None,
        lockfile: None,
        http: false,
        speed: 1.0,
        replay_start: ReplayStart::OnFirstSubscribe,
        repeat: false,
        verbose: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} precisa de um valor"));
        match arg.as_str() {
            "--fixtures" => args.fixtures = Some(value()?),
            "--porta" => args.port = value()?.parse().map_err(|_| "--porta inválida")?,
            "--senha" => args.password = Some(value()?),
            "--lockfile" => args.lockfile = Some(PathBuf::from(value()?)),
            "--http" => args.http = true,
            "--velocidade" => args.speed = value()?.parse().map_err(|_| "--velocidade inválida")?,
            "--iniciar-ja" => args.replay_start = ReplayStart::Immediately,
            "--parado" => args.replay_start = ReplayStart::Never,
            "--repetir" => args.repeat = true,
            "--verbose" => args.verbose = true,
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            _ => return Err(format!("argumento desconhecido: {arg}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

/// `agent/tests/fixtures/`.
fn fixtures_root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../agent/tests/fixtures");
    root.canonicalize().unwrap_or(root)
}

/// Um nome sem barra é um conjunto em `agent/tests/fixtures/`; o resto é
/// caminho. Sem nada, escolhe a gravação real de patch mais novo.
fn resolve_fixtures(arg: Option<&str>) -> Result<PathBuf, String> {
    if let Some(arg) = arg {
        let path = Path::new(arg);
        if path.is_dir() {
            return Ok(path.to_path_buf());
        }
        let in_root = fixtures_root().join(arg);
        if in_root.is_dir() {
            return Ok(in_root);
        }
        return Err(format!("não achei o conjunto de fixtures {arg}"));
    }
    let entries = std::fs::read_dir(fixtures_root()).map_err(|e| e.to_string())?;
    let mut sets: Vec<(bool, Vec<u32>, PathBuf)> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter_map(|dir| {
            let set = FixtureSet::load(&dir).ok()?;
            let version = set
                .manifest
                .patch
                .split('.')
                .map(|p| p.parse().unwrap_or(0))
                .collect();
            Some((!set.manifest.synthetic, version, dir))
        })
        .collect();
    sets.sort();
    sets.pop()
        .map(|(_, _, dir)| dir)
        .ok_or_else(|| "nenhum conjunto de fixtures em agent/tests/fixtures".to_owned())
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
    let dir = resolve_fixtures(args.fixtures.as_deref())?;
    let fixtures = FixtureSet::load(&dir).map_err(|e| e.to_string())?;

    let lockfile = args.lockfile.unwrap_or_else(|| {
        std::env::temp_dir()
            .join("easylobby-mock-lcu")
            .join("lockfile")
    });
    let ca_file = lockfile.with_file_name("mock-lcu-ca.pem");

    let mut config = Config::new(fixtures);
    config.port = args.port;
    if let Some(password) = args.password {
        config.password = password;
    }
    config.tls = !args.http;
    config.lockfile = Some(lockfile.clone());
    config.ca_file = (!args.http).then(|| ca_file.clone());
    config.speed = args.speed;
    config.replay_start = args.replay_start;
    config.repeat = args.repeat;
    config.verbose = args.verbose;

    let manifest = config.fixtures.manifest.clone();
    let (rest, events) = (config.fixtures.rest.len(), config.fixtures.events.len());
    let mock = MockLcu::start(config).await.map_err(|e| e.to_string())?;

    println!(
        "mock-lcu {} em {}",
        env!("CARGO_PKG_VERSION"),
        mock.base_url()
    );
    println!(
        "fixtures: {} (patch {}{}), {rest} respostas REST, {events} eventos",
        dir.display(),
        manifest.patch,
        if manifest.synthetic {
            ", sintético"
        } else {
            ""
        },
    );
    println!("lockfile: {}", lockfile.display());
    if mock.ca_pem().is_some() {
        println!("CA do mock (fixe no cliente): {}", ca_file.display());
    }
    println!("fase inicial: {}. Ctrl+C para parar.", mock.phase());

    tokio::signal::ctrl_c().await.map_err(|e| e.to_string())?;
    println!("parando; lockfile apagado");
    mock.shutdown().await;
    Ok(())
}
