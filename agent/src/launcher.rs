//! Abre o LoL pelo Riot Client quando ele não está rodando (#11).
//!
//! O LoL não abre sozinho: quem abre é o Riot Client. [`launch_league`] tenta,
//! nesta ordem:
//!
//! 1. Se já existe `LeagueClient` ou `LeagueClientUx`, não faz nada
//!    ([`LaunchOutcome::AlreadyRunning`]).
//! 2. Se o Riot Client está aberto, pede o LoL pela API local dele:
//!    `POST /product-launcher/v1/products/league_of_legends/patchlines/live`,
//!    com porta e senha do lockfile do Riot Client
//!    (`%LOCALAPPDATA%\Riot Games\Riot Client\Config\lockfile`), Basic Auth e o
//!    `riotgames.pem` fixado, como na LCU
//!    ([`LaunchOutcome::RequestedFromRiotClient`]). Chamar o executável com o
//!    Riot Client já aberto só mostrava a janela dele, sem abrir o LoL.
//! 3. Senão (ou se a API falhar), chama `RiotClientServices.exe
//!    --launch-product=league_of_legends --launch-patchline=live`
//!    ([`LaunchOutcome::Launched`]). Se ninguém fez login, o Riot Client
//!    mostra a tela de login e abre o LoL depois.
//!
//! O executável é procurado nesta ordem:
//!
//! 1. O caminho dado em [`LaunchOptions::riot_client`], se houver.
//! 2. O executável de um processo `RiotClientServices` já aberto.
//! 3. O `RiotClientInstalls.json` em `%ProgramData%\Riot Games\`, que o
//!    instalador da Riot mantém (`rc_default`, `rc_live`, `associated_client`
//!    e `rc_beta`).
//! 4. A pasta padrão, `C:\Riot Games\Riot Client\`.
//!
//! [`launch_league`] não espera o LoL subir: ele só faz o pedido e volta. Quem
//! espera é a [`LcuConnection`](crate::connection::LcuConnection), que já
//! tenta de novo até o client abrir.
//!
//! ```no_run
//! # async fn exemplo() -> Result<(), easylobby_agent::launcher::LaunchError> {
//! use easylobby_agent::launcher::{self, LaunchOptions, LaunchOutcome};
//!
//! match launcher::launch_league(&LaunchOptions::default()).await? {
//!     LaunchOutcome::AlreadyRunning { pid } => println!("o LoL já está aberto ({pid})"),
//!     LaunchOutcome::RequestedFromRiotClient { .. } => println!("pedido ao Riot Client"),
//!     LaunchOutcome::Launched { riot_client, .. } => {
//!         println!("abrindo pelo {}", riot_client.path.display())
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;
use thiserror::Error;

use crate::lockfile::{self, Credentials, LockfileError, ProcessInfo};
use crate::rest::{LcuClient, LcuError};

/// Processo e executável do Riot Client.
pub const RIOT_CLIENT_PROCESS: &str = "RiotClientServices";

/// Produto do LoL no Riot Client.
pub const LEAGUE_PRODUCT: &str = "league_of_legends";

/// Patchline normal (o servidor de produção).
pub const LIVE_PATCHLINE: &str = "live";

/// Arquivo com os caminhos do Riot Client, em `%ProgramData%\Riot Games\`.
pub const RIOT_CLIENT_INSTALLS_FILE: &str = "RiotClientInstalls.json";

/// Onde o instalador da Riot põe o Riot Client por padrão.
pub const DEFAULT_RIOT_CLIENT_PATH: &str = r"C:\Riot Games\Riot Client\RiotClientServices.exe";

/// Caminho do lockfile do Riot Client dentro do `%LOCALAPPDATA%`.
pub const RIOT_CLIENT_LOCKFILE: [&str; 4] = ["Riot Games", "Riot Client", "Config", "lockfile"];

/// O que abrir e como achar o Riot Client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchOptions {
    /// Caminho do `RiotClientServices.exe`. `None` procura sozinho.
    pub riot_client: Option<PathBuf>,
    /// Lockfile do Riot Client. `None` usa o do `%LOCALAPPDATA%`.
    pub riot_client_lockfile: Option<PathBuf>,
    /// Produto do Riot Client; [`LEAGUE_PRODUCT`] por padrão.
    pub product: String,
    /// Patchline; [`LIVE_PATCHLINE`] por padrão (`pbe` para o PBE).
    pub patchline: String,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            riot_client: None,
            riot_client_lockfile: None,
            product: LEAGUE_PRODUCT.into(),
            patchline: LIVE_PATCHLINE.into(),
        }
    }
}

impl LaunchOptions {
    /// Rota da API local do Riot Client que abre o produto.
    pub fn product_launcher_path(&self) -> String {
        format!(
            "/product-launcher/v1/products/{}/patchlines/{}",
            self.product, self.patchline
        )
    }

    /// Argumentos passados ao Riot Client.
    pub fn args(&self) -> Vec<String> {
        vec![
            format!("--launch-product={}", self.product),
            format!("--launch-patchline={}", self.patchline),
        ]
    }
}

/// Conteúdo do `RiotClientInstalls.json`. Só os campos que importam.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct RiotClientInstalls {
    #[serde(default)]
    pub rc_default: Option<String>,
    #[serde(default)]
    pub rc_live: Option<String>,
    #[serde(default)]
    pub rc_beta: Option<String>,
    /// Pasta de cada jogo instalado -> Riot Client que ele usa.
    #[serde(default)]
    pub associated_client: BTreeMap<String, String>,
}

impl RiotClientInstalls {
    /// Interpreta o conteúdo do arquivo.
    pub fn parse(content: &str) -> Result<Self, serde_json::Error> {
        // O arquivo às vezes vem com BOM.
        serde_json::from_str(content.trim_start_matches('\u{feff}'))
    }

    /// Caminhos do Riot Client, sem repetição, na ordem de preferência.
    fn candidates(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = Vec::new();
        let all = [self.rc_default.as_ref(), self.rc_live.as_ref()]
            .into_iter()
            .flatten()
            .chain(self.associated_client.values())
            .chain(self.rc_beta.as_ref());
        for path in all.filter(|p| !p.trim().is_empty()).map(PathBuf::from) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
        paths
    }
}

/// De onde veio o caminho do Riot Client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiotClientSource {
    /// [`LaunchOptions::riot_client`].
    Configured,
    /// Executável do processo `RiotClientServices` aberto com este pid.
    RunningProcess { pid: u32 },
    /// O `RiotClientInstalls.json` neste caminho.
    InstallsFile(PathBuf),
    /// [`DEFAULT_RIOT_CLIENT_PATH`].
    DefaultPath,
}

/// Um Riot Client achado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiotClient {
    /// Caminho do `RiotClientServices.exe`.
    pub path: PathBuf,
    pub source: RiotClientSource,
}

/// Resultado de [`launch_league`].
#[derive(Debug)]
pub enum LaunchOutcome {
    /// O LoL já estava aberto; nada foi feito.
    AlreadyRunning { pid: u32 },
    /// O LoL foi pedido pela API local do Riot Client, que estava aberto.
    RequestedFromRiotClient {
        /// Porta da API do Riot Client.
        port: u16,
    },
    /// O executável do Riot Client foi chamado para abrir o LoL.
    Launched {
        riot_client: RiotClient,
        /// Pid do processo disparado.
        pid: u32,
        /// Por que a API do Riot Client não serviu, quando ele estava aberto.
        api_error: Option<RiotClientApiError>,
    },
}

/// Erro ao pedir o LoL pela API local do Riot Client.
#[derive(Debug, Error)]
pub enum RiotClientApiError {
    #[error("não sei onde fica o lockfile do Riot Client (%LOCALAPPDATA% não definido)")]
    NoLockfilePath,
    #[error(transparent)]
    Lockfile(#[from] LockfileError),
    #[error(transparent)]
    Rest(#[from] LcuError),
}

/// O que [`plan_launch`] decidiu fazer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchPlan {
    /// O LoL já está aberto.
    AlreadyRunning { pid: u32 },
    /// Chamar este Riot Client com estes argumentos.
    Launch {
        riot_client: RiotClient,
        args: Vec<String>,
    },
}

/// Erro ao abrir o LoL.
#[derive(Debug, Error)]
pub enum LaunchError {
    #[error(
        "não achei o Riot Client (RiotClientServices.exe). Procurei em: {}",
        format_paths(searched)
    )]
    RiotClientNotFound { searched: Vec<PathBuf> },
    #[error("não consegui abrir o Riot Client em {}: {source}", path.display())]
    Spawn { path: PathBuf, source: io::Error },
}

fn format_paths(paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        return "nenhum lugar".into();
    }
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// O `RiotClientInstalls.json` lido do disco, para [`plan_launch`].
#[derive(Debug, Clone)]
pub struct InstallsFile {
    pub path: PathBuf,
    pub installs: RiotClientInstalls,
}

/// Abre o LoL pelo Riot Client, se ele ainda não estiver aberto, na ordem
/// descrita no [módulo](self). Não espera o LoL subir.
pub async fn launch_league(options: &LaunchOptions) -> Result<LaunchOutcome, LaunchError> {
    // Listar processos e ler arquivos é bloqueante.
    let (processes, installs) =
        tokio::task::spawn_blocking(|| (lockfile::list_processes(), read_installs_file()))
            .await
            .unwrap_or_default();

    if let Some(client) = processes.iter().find(|p| p.is_client()) {
        return Ok(LaunchOutcome::AlreadyRunning { pid: client.pid });
    }

    let mut api_error = None;
    if processes.iter().any(is_riot_client) || options.riot_client_lockfile.is_some() {
        match request_from_riot_client(options).await {
            Ok(port) => return Ok(LaunchOutcome::RequestedFromRiotClient { port }),
            Err(e) => api_error = Some(e),
        }
    }

    match plan_launch(&processes, options, installs.as_ref(), |p| p.is_file())? {
        LaunchPlan::AlreadyRunning { pid } => Ok(LaunchOutcome::AlreadyRunning { pid }),
        LaunchPlan::Launch { riot_client, args } => {
            let pid = spawn_detached(&riot_client.path, &args)?;
            Ok(LaunchOutcome::Launched {
                riot_client,
                pid,
                api_error,
            })
        }
    }
}

/// Lê o lockfile do Riot Client e pede o LoL pela API dele, com o
/// `riotgames.pem` fixado. Devolve a porta da API.
async fn request_from_riot_client(options: &LaunchOptions) -> Result<u16, RiotClientApiError> {
    let path = match &options.riot_client_lockfile {
        Some(path) => path.clone(),
        None => riot_client_lockfile_path().ok_or(RiotClientApiError::NoLockfilePath)?,
    };
    let credentials = Credentials::from_lockfile_path(&path)?;
    let client = LcuClient::new(&credentials)?;
    request_launch(&client, options).await?;
    Ok(credentials.port)
}

/// Pede ao Riot Client, pela API local dele, que abra o produto de `options`.
///
/// `client` aponta para o Riot Client (porta e senha do lockfile dele), não
/// para a LCU.
pub async fn request_launch(client: &LcuClient, options: &LaunchOptions) -> Result<(), LcuError> {
    client
        .post::<_, serde_json::Value>(&options.product_launcher_path(), &serde_json::Value::Null)
        .await
        .map(drop)
}

/// Lockfile do Riot Client, a partir do `%LOCALAPPDATA%`.
pub fn riot_client_lockfile_path() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(
        RIOT_CLIENT_LOCKFILE
            .iter()
            .fold(PathBuf::from(local), |p, c| p.join(c)),
    )
}

fn is_riot_client(p: &ProcessInfo) -> bool {
    p.base_name().eq_ignore_ascii_case(RIOT_CLIENT_PROCESS)
}

/// Decide o que [`launch_league`] faz, com processos, o
/// `RiotClientInstalls.json` e a checagem de arquivo injetados (para testar
/// sem o LoL).
pub fn plan_launch<F>(
    processes: &[ProcessInfo],
    options: &LaunchOptions,
    installs: Option<&InstallsFile>,
    exists: F,
) -> Result<LaunchPlan, LaunchError>
where
    F: Fn(&Path) -> bool,
{
    if let Some(client) = processes.iter().find(|p| p.is_client()) {
        return Ok(LaunchPlan::AlreadyRunning { pid: client.pid });
    }
    let riot_client = find_riot_client(processes, options, installs, exists)?;
    Ok(LaunchPlan::Launch {
        riot_client,
        args: options.args(),
    })
}

/// Acha o `RiotClientServices.exe`, na ordem descrita no [módulo](self).
pub fn find_riot_client<F>(
    processes: &[ProcessInfo],
    options: &LaunchOptions,
    installs: Option<&InstallsFile>,
    exists: F,
) -> Result<RiotClient, LaunchError>
where
    F: Fn(&Path) -> bool,
{
    // Caminho configurado: usa como está; se não existir, o erro do spawn diz.
    if let Some(path) = &options.riot_client {
        return Ok(RiotClient {
            path: path.clone(),
            source: RiotClientSource::Configured,
        });
    }

    let running = processes
        .iter()
        .filter(|p| is_riot_client(p))
        .filter_map(|p| {
            Some((
                p.exe.clone()?,
                RiotClientSource::RunningProcess { pid: p.pid },
            ))
        });
    let from_file = installs.into_iter().flat_map(|file| {
        file.installs
            .candidates()
            .into_iter()
            .map(|path| (path, RiotClientSource::InstallsFile(file.path.clone())))
    });
    let default = std::iter::once((
        PathBuf::from(DEFAULT_RIOT_CLIENT_PATH),
        RiotClientSource::DefaultPath,
    ));

    let mut searched: Vec<PathBuf> = Vec::new();
    for (path, source) in running.chain(from_file).chain(default) {
        if searched.contains(&path) {
            continue;
        }
        if exists(&path) {
            return Ok(RiotClient { path, source });
        }
        searched.push(path);
    }
    Err(LaunchError::RiotClientNotFound { searched })
}

/// Caminho do `RiotClientInstalls.json`, a partir do `%ProgramData%`.
pub fn installs_file_path() -> Option<PathBuf> {
    let program_data = std::env::var_os("ProgramData")?;
    Some(
        PathBuf::from(program_data)
            .join("Riot Games")
            .join(RIOT_CLIENT_INSTALLS_FILE),
    )
}

/// Lê o `RiotClientInstalls.json`. Arquivo ausente ou inválido vira `None`:
/// ainda sobram o processo aberto e a pasta padrão.
fn read_installs_file() -> Option<InstallsFile> {
    let path = installs_file_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let installs = RiotClientInstalls::parse(&content).ok()?;
    Some(InstallsFile { path, installs })
}

/// Dispara o processo sem esperar por ele, como o PowerShell ou o atalho do LoL
/// chamam.
fn spawn_detached(path: &Path, args: &[String]) -> Result<u32, LaunchError> {
    let mut command = Command::new(path);
    command.args(args).stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Fora do grupo do agente: Ctrl+C no terminal do agente não chega no
        // Riot Client.
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }
    let child = command.spawn().map_err(|source| LaunchError::Spawn {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(child.id())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Diferente da pasta padrão: no Windows, `C:/...` e `C:\...` são o mesmo
    // `Path`.
    const RC: &str = "E:/Riot Games/Riot Client/RiotClientServices.exe";
    const RC_OUTRO: &str = "D:/Jogos/Riot Client/RiotClientServices.exe";

    fn processo(pid: u32, name: &str, exe: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.into(),
            exe: exe.map(PathBuf::from),
            args: vec![],
        }
    }

    fn installs(json: &str) -> InstallsFile {
        InstallsFile {
            path: PathBuf::from("C:/ProgramData/Riot Games/RiotClientInstalls.json"),
            installs: RiotClientInstalls::parse(json).unwrap(),
        }
    }

    fn existe(paths: &[&str]) -> impl Fn(&Path) -> bool {
        let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        move |p| paths.iter().any(|x| x == p)
    }

    #[test]
    fn argumentos_padrao_abrem_o_lol_live() {
        assert_eq!(
            LaunchOptions::default().args(),
            [
                "--launch-product=league_of_legends",
                "--launch-patchline=live"
            ]
        );
    }

    #[test]
    fn nao_abre_de_novo_se_o_lol_ja_esta_aberto() {
        let processes = [
            processo(1, "RiotClientServices.exe", Some(RC)),
            processo(42, "LeagueClient.exe", None),
        ];
        let plan = plan_launch(&processes, &LaunchOptions::default(), None, existe(&[RC]));
        assert_eq!(plan.unwrap(), LaunchPlan::AlreadyRunning { pid: 42 });
    }

    #[test]
    fn depois_do_kill_ux_o_lol_continua_aberto() {
        // Sem LeagueClientUx, mas o LeagueClient segue rodando.
        let processes = [processo(7, "LeagueClient", None)];
        let plan = plan_launch(&processes, &LaunchOptions::default(), None, existe(&[]));
        assert_eq!(plan.unwrap(), LaunchPlan::AlreadyRunning { pid: 7 });
    }

    #[test]
    fn prefere_o_riot_client_aberto() {
        let processes = [processo(9, "RiotClientServices.exe", Some(RC_OUTRO))];
        let file = installs(&format!(r#"{{"rc_default": "{RC}"}}"#));
        let plan = plan_launch(
            &processes,
            &LaunchOptions::default(),
            Some(&file),
            existe(&[RC, RC_OUTRO]),
        )
        .unwrap();
        let LaunchPlan::Launch { riot_client, args } = plan else {
            panic!("esperava Launch: {plan:?}");
        };
        assert_eq!(riot_client.path, PathBuf::from(RC_OUTRO));
        assert_eq!(
            riot_client.source,
            RiotClientSource::RunningProcess { pid: 9 }
        );
        assert_eq!(args, LaunchOptions::default().args());
    }

    #[test]
    fn usa_o_riot_client_installs_json() {
        let file = installs(&format!(
            "\u{feff}{{\"rc_default\": \"{RC_OUTRO}\", \"rc_live\": \"{RC}\", \"patchlines\": {{}}}}"
        ));
        let rc =
            find_riot_client(&[], &LaunchOptions::default(), Some(&file), existe(&[RC])).unwrap();
        // rc_default não existe no disco, então cai no rc_live.
        assert_eq!(rc.path, PathBuf::from(RC));
        assert_eq!(rc.source, RiotClientSource::InstallsFile(file.path.clone()));
    }

    #[test]
    fn usa_o_associated_client() {
        let file = installs(&format!(
            r#"{{"associated_client": {{"D:/Jogos/League of Legends/": "{RC_OUTRO}"}}}}"#
        ));
        let rc = find_riot_client(
            &[],
            &LaunchOptions::default(),
            Some(&file),
            existe(&[RC_OUTRO]),
        )
        .unwrap();
        assert_eq!(rc.path, PathBuf::from(RC_OUTRO));
    }

    #[test]
    fn cai_na_pasta_padrao() {
        let rc = find_riot_client(
            &[],
            &LaunchOptions::default(),
            None,
            existe(&[DEFAULT_RIOT_CLIENT_PATH]),
        )
        .unwrap();
        assert_eq!(rc.source, RiotClientSource::DefaultPath);
    }

    #[test]
    fn caminho_configurado_vence() {
        let options = LaunchOptions {
            riot_client: Some(PathBuf::from("E:/rc/RiotClientServices.exe")),
            ..LaunchOptions::default()
        };
        let processes = [processo(9, "RiotClientServices.exe", Some(RC))];
        let rc = find_riot_client(&processes, &options, None, existe(&[RC])).unwrap();
        assert_eq!(rc.source, RiotClientSource::Configured);
    }

    #[test]
    fn sem_riot_client_lista_onde_procurou() {
        let processes = [processo(9, "RiotClientServices.exe", Some(RC_OUTRO))];
        let file = installs(&format!(r#"{{"rc_default": "{RC}", "rc_live": "{RC}"}}"#));
        let err = find_riot_client(
            &processes,
            &LaunchOptions::default(),
            Some(&file),
            existe(&[]),
        )
        .unwrap_err();
        let LaunchError::RiotClientNotFound { searched } = &err else {
            panic!("erro inesperado: {err}");
        };
        assert_eq!(
            searched,
            &[
                PathBuf::from(RC_OUTRO),
                PathBuf::from(RC),
                PathBuf::from(DEFAULT_RIOT_CLIENT_PATH)
            ]
        );
        assert!(err.to_string().contains("Riot Client"));
    }

    #[test]
    fn processo_parecido_nao_conta_como_riot_client() {
        let processes = [processo(9, "RiotClientUx.exe", Some(RC))];
        let err = find_riot_client(&processes, &LaunchOptions::default(), None, existe(&[RC]))
            .unwrap_err();
        assert!(matches!(err, LaunchError::RiotClientNotFound { .. }));
    }

    #[test]
    fn installs_json_invalido_da_erro() {
        assert!(RiotClientInstalls::parse("não é json").is_err());
        assert_eq!(
            RiotClientInstalls::parse("{}").unwrap(),
            RiotClientInstalls::default()
        );
    }

    #[test]
    fn erro_de_spawn_diz_o_caminho() {
        let path = std::env::temp_dir().join("easylobby-nao-existe/RiotClientServices.exe");
        let err = spawn_detached(&path, &[]).unwrap_err();
        assert!(matches!(err, LaunchError::Spawn { .. }));
        assert!(err.to_string().contains("easylobby-nao-existe"), "{err}");
    }

    /// Um "Riot Client" falso que grava os argumentos num arquivo.
    #[cfg(unix)]
    #[test]
    fn dispara_o_riot_client_com_os_argumentos() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{Duration, Instant};

        let dir = std::env::temp_dir().join(format!("easylobby-launcher-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let saida = dir.join("args.txt");
        let script = dir.join("RiotClientServices");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"$@\" > \"{}.tmp\"\nmv \"{0}.tmp\" \"{0}\"\n",
                saida.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

        let options = LaunchOptions {
            riot_client: Some(script.clone()),
            ..LaunchOptions::default()
        };
        let LaunchPlan::Launch { riot_client, args } =
            plan_launch(&[], &options, None, |_| false).unwrap()
        else {
            panic!("esperava Launch");
        };
        spawn_detached(&riot_client.path, &args).unwrap();

        let prazo = Instant::now() + Duration::from_secs(10);
        let conteudo = loop {
            if let Ok(c) = std::fs::read_to_string(&saida) {
                break c;
            }
            assert!(Instant::now() < prazo, "o script não rodou");
            std::thread::sleep(Duration::from_millis(20));
        };
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            conteudo.trim(),
            "--launch-product=league_of_legends --launch-patchline=live"
        );
    }
}
