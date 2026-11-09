//! Descoberta das credenciais da LCU API (#4).
//!
//! O LoL roda um servidor local protegido por senha. Porta e senha ficam no
//! `lockfile` da pasta de instalação, no formato
//! `nome:pid:porta:senha:protocolo`, por exemplo
//! `LeagueClient:12345:54321:abcdef:https`. O arquivo só existe enquanto o
//! client está aberto.
//!
//! A descoberta segue esta ordem:
//!
//! 1. Acha os processos `LeagueClientUx` e `LeagueClient`.
//! 2. Procura o `lockfile` na pasta do `--install-directory=` dos argumentos e
//!    na pasta do executável de cada processo (ou, se nenhuma for conhecida,
//!    em [`DEFAULT_LEAGUE_DIR`]).
//! 3. Se nenhum lockfile servir, usa `--app-port=` e `--remoting-auth-token=`
//!    dos argumentos do `LeagueClientUx`. Esse fallback some depois do
//!    `kill-ux`, por isso o lockfile vem primeiro.
//!
//! No Windows os argumentos nunca são lidos: lê-los exige abrir o processo
//! com `PROCESS_VM_READ` e ler a memória dele, o que o Vanguard pune com o
//! erro VAN7003 (veja [`list_processes`]). Lá valem só o lockfile na pasta do
//! executável e a pasta padrão.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Usuário do Basic Auth da LCU. É fixo; só a senha muda a cada sessão.
pub const LCU_USERNAME: &str = "riot";

/// Nome do arquivo de credenciais na pasta de instalação do LoL.
pub const LOCKFILE_NAME: &str = "lockfile";

/// Onde o instalador da Riot põe o LoL por padrão.
pub const DEFAULT_LEAGUE_DIR: &str = r"C:\Riot Games\League of Legends";

/// Processos do LoL que podem apontar para o lockfile.
const CLIENT_PROCESSES: [&str; 2] = ["LeagueClientUx", "LeagueClient"];

/// Protocolo do servidor da LCU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Http,
    Https,
}

impl Protocol {
    /// Esquema usado na URL (`http` ou `https`).
    pub fn scheme(self) -> &'static str {
        match self {
            Protocol::Http => "http",
            Protocol::Https => "https",
        }
    }
}

/// De onde as credenciais vieram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Lidas do lockfile neste caminho.
    Lockfile(PathBuf),
    /// Lidas dos argumentos de linha de comando do processo com este pid.
    ProcessArgs { pid: u32 },
}

/// Credenciais para falar com a LCU API.
///
/// O `Debug` esconde a senha para ela não vazar em logs.
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    /// Pid do processo do client, quando conhecido.
    pub pid: Option<u32>,
    pub port: u16,
    pub password: String,
    pub protocol: Protocol,
    pub source: Source,
}

impl Credentials {
    /// URL base da LCU, por exemplo `https://127.0.0.1:54321`.
    pub fn base_url(&self) -> String {
        format!("{}://127.0.0.1:{}", self.protocol.scheme(), self.port)
    }

    /// Lê e interpreta um lockfile num caminho conhecido.
    ///
    /// Útil para apontar o agente para o mock-lcu ou para uma instalação fora
    /// do padrão.
    pub fn from_lockfile_path(path: &Path) -> Result<Self, LockfileError> {
        let content = std::fs::read_to_string(path).map_err(|source| LockfileError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        parse_lockfile(&content, path)
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("pid", &self.pid)
            .field("port", &self.port)
            .field("password", &"<oculta>")
            .field("protocol", &self.protocol)
            .field("source", &self.source)
            .finish()
    }
}

/// Erro ao ler ou interpretar um lockfile.
#[derive(Debug, Error)]
pub enum LockfileError {
    #[error("não foi possível ler {}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("lockfile {} inválido: {reason}", path.display())]
    Invalid { path: PathBuf, reason: String },
}

/// Erro na descoberta das credenciais.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("o LoL não está aberto (nenhum processo LeagueClient ou LeagueClientUx)")]
    ClientNotRunning,
    #[error(
        "o LoL está aberto, mas não achei o lockfile nem as credenciais nos argumentos do processo{}",
        last_error.as_ref().map(|e| format!(" (último erro: {e})")).unwrap_or_default()
    )]
    CredentialsNotFound { last_error: Option<LockfileError> },
}

/// Interpreta o conteúdo de um lockfile (`nome:pid:porta:senha:protocolo`).
///
/// `path` só entra nas mensagens de erro e no [`Source`].
pub fn parse_lockfile(content: &str, path: &Path) -> Result<Credentials, LockfileError> {
    let invalid = |reason: String| LockfileError::Invalid {
        path: path.to_path_buf(),
        reason,
    };

    let fields: Vec<&str> = content.trim().split(':').collect();
    let [_name, pid, port, password, protocol] = fields[..] else {
        return Err(invalid(format!(
            "esperava 5 campos separados por ':', achei {}",
            fields.len()
        )));
    };

    let pid = pid
        .parse::<u32>()
        .map_err(|_| invalid(format!("pid {pid:?} não é um número")))?;
    let port = parse_port(port).ok_or_else(|| invalid(format!("porta {port:?} inválida")))?;
    if password.is_empty() {
        return Err(invalid("senha vazia".into()));
    }
    let protocol = match protocol {
        "https" => Protocol::Https,
        "http" => Protocol::Http,
        other => return Err(invalid(format!("protocolo {other:?} desconhecido"))),
    };

    Ok(Credentials {
        pid: Some(pid),
        port,
        password: password.to_owned(),
        protocol,
        source: Source::Lockfile(path.to_path_buf()),
    })
}

/// Tira as credenciais de `--app-port=` e `--remoting-auth-token=`.
///
/// Devolve `None` se faltar algum dos dois ou se a porta for inválida.
pub fn parse_process_args<S: AsRef<str>>(pid: u32, args: &[S]) -> Option<Credentials> {
    let port = arg_value(args, "--app-port").and_then(parse_port)?;
    let password = arg_value(args, "--remoting-auth-token").filter(|p| !p.is_empty())?;
    Some(Credentials {
        pid: Some(pid),
        port,
        password: password.to_owned(),
        protocol: Protocol::Https,
        source: Source::ProcessArgs { pid },
    })
}

/// Um processo como a descoberta precisa enxergar.
#[derive(Debug, Clone, Default)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub exe: Option<PathBuf>,
    pub args: Vec<String>,
}

impl ProcessInfo {
    /// Nome sem `.exe`, para comparar igual no Windows e fora dele.
    pub(crate) fn base_name(&self) -> &str {
        base_name(&self.name)
    }

    /// Se é um processo do client do LoL (`LeagueClient` ou `LeagueClientUx`).
    pub(crate) fn is_client(&self) -> bool {
        CLIENT_PROCESSES
            .iter()
            .any(|c| self.base_name().eq_ignore_ascii_case(c))
    }
}

/// Acha as credenciais da LCU olhando os processos do sistema.
pub fn discover() -> Result<Credentials, DiscoveryError> {
    discover_from(&list_processes(), |path| std::fs::read_to_string(path))
}

/// Mesma lógica de [`discover`], com a lista de processos e a leitura de
/// arquivos injetadas (para testar sem o LoL).
pub fn discover_from<F>(processes: &[ProcessInfo], read: F) -> Result<Credentials, DiscoveryError>
where
    F: Fn(&Path) -> io::Result<String>,
{
    let mut clients: Vec<&ProcessInfo> = processes.iter().filter(|p| p.is_client()).collect();
    if clients.is_empty() {
        return Err(DiscoveryError::ClientNotRunning);
    }
    // LeagueClientUx primeiro: é ele que costuma ter --install-directory.
    clients.sort_by_key(|p| {
        CLIENT_PROCESSES
            .iter()
            .position(|c| p.base_name().eq_ignore_ascii_case(c))
    });

    let mut last_error = None;
    for path in lockfile_candidates(&clients) {
        let result = read(&path)
            .map_err(|source| LockfileError::Io {
                path: path.clone(),
                source,
            })
            .and_then(|content| parse_lockfile(&content, &path));
        match result {
            Ok(credentials) => return Ok(credentials),
            Err(e) => last_error = Some(e),
        }
    }

    clients
        .iter()
        .find_map(|p| parse_process_args(p.pid, &p.args))
        .ok_or(DiscoveryError::CredentialsNotFound { last_error })
}

/// Caminhos possíveis do lockfile, sem repetição, na ordem de preferência.
fn lockfile_candidates(clients: &[&ProcessInfo]) -> Vec<PathBuf> {
    let install_dirs = clients
        .iter()
        .filter_map(|p| arg_value(&p.args, "--install-directory"))
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from);
    let exe_dirs = clients
        .iter()
        .filter_map(|p| p.exe.as_deref()?.parent().map(Path::to_path_buf));

    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in install_dirs.chain(exe_dirs) {
        let path = dir.join(LOCKFILE_NAME);
        if !candidates.contains(&path) {
            candidates.push(path);
        }
    }
    // O Vanguard pode negar até o acesso mínimo ao LeagueClient; aí não sei a
    // pasta e tento a de instalação padrão.
    if candidates.is_empty() {
        candidates.push(PathBuf::from(DEFAULT_LEAGUE_DIR).join(LOCKFILE_NAME));
    }
    candidates
}

/// Valor de um argumento `--chave=valor`, sem aspas em volta.
fn arg_value<'a, S: AsRef<str>>(args: &'a [S], key: &str) -> Option<&'a str> {
    args.iter().find_map(|arg| {
        let arg = arg.as_ref().trim().trim_matches('"');
        let value = arg.strip_prefix(key)?.strip_prefix('=')?;
        Some(value.trim_matches('"'))
    })
}

fn parse_port(s: &str) -> Option<u16> {
    s.trim().parse::<u16>().ok().filter(|&p| p != 0)
}

/// Processos cujo executável o agente precisa saber. Só esses são abertos, e
/// só com `PROCESS_QUERY_LIMITED_INFORMATION` (o mesmo acesso do Gerenciador
/// de Tarefas).
const PROCESSES_WITH_EXE: [&str; 3] = ["LeagueClientUx", "LeagueClient", "RiotClientServices"];

/// Nome do processo sem `.exe`.
fn base_name(name: &str) -> &str {
    name.len()
        .checked_sub(4)
        .filter(|&i| name.is_char_boundary(i) && name[i..].eq_ignore_ascii_case(".exe"))
        .map_or(name, |i| &name[..i])
}

/// Se o agente precisa do executável deste processo.
fn wants_exe(name: &str) -> bool {
    PROCESSES_WITH_EXE
        .iter()
        .any(|p| base_name(name).eq_ignore_ascii_case(p))
}

/// Lista os processos do sistema com nome e, para os do LoL e do Riot Client,
/// o executável.
///
/// No Windows, a lista vem da Toolhelp, que não abre processo nenhum. Só os
/// processos de [`PROCESSES_WITH_EXE`] são abertos, com
/// `PROCESS_QUERY_LIMITED_INFORMATION`, para ler o caminho do executável. O
/// agente **nunca** abre processo com `PROCESS_VM_READ` nem lê a memória de
/// outro processo (os argumentos de linha de comando ficam vazios): o Vanguard
/// trata isso como ameaça e fecha o LoL com o erro VAN7003. O `sysinfo`, usado
/// antes, abria todos os processos do sistema assim a cada tentativa de
/// conexão, inclusive o jogo e os do Vanguard.
///
/// É bloqueante: em código async, chame dentro de `spawn_blocking`.
pub(crate) fn list_processes() -> Vec<ProcessInfo> {
    #[cfg(windows)]
    {
        windows::list_processes()
    }
    #[cfg(not(windows))]
    {
        list_processes_sysinfo()
    }
}

/// Pid e nome de cada processo, pela Toolhelp, sem abrir nenhum (#286).
#[cfg(windows)]
pub(crate) fn process_names() -> Vec<(u32, String)> {
    windows::snapshot()
}

/// Fora do Windows (o mock-lcu no Linux ou no macOS) não há Vanguard; o
/// `sysinfo` lê o `/proc` e serve.
#[cfg(not(windows))]
fn list_processes_sysinfo() -> Vec<ProcessInfo> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always),
    );

    system
        .processes()
        .values()
        .map(|p| {
            let name = p.name().to_string_lossy().into_owned();
            let exe = if wants_exe(&name) {
                p.exe().map(Path::to_path_buf)
            } else {
                None
            };
            ProcessInfo {
                pid: p.pid().as_u32(),
                name,
                exe,
                args: p
                    .cmd()
                    .iter()
                    .map(|a| a.to_string_lossy().into_owned())
                    .collect(),
            }
        })
        .collect()
}

/// Lista de processos no Windows sem tocar na memória de ninguém.
///
/// Com o `ux_window::windows`, é o único `unsafe` do agente: as chamadas da
/// API do Windows. Cada handle é
/// fechado no mesmo bloco em que foi aberto.
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::path::PathBuf;

    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    use super::{wants_exe, ProcessInfo};

    pub(super) fn list_processes() -> Vec<ProcessInfo> {
        snapshot()
            .into_iter()
            .map(|(pid, name)| {
                let exe = if wants_exe(&name) {
                    exe_path(pid)
                } else {
                    None
                };
                ProcessInfo {
                    pid,
                    name,
                    exe,
                    args: Vec::new(),
                }
            })
            .collect()
    }

    /// Pid e nome de cada processo, pela Toolhelp. Não abre os processos.
    pub(super) fn snapshot() -> Vec<(u32, String)> {
        let mut processes = Vec::new();
        // SAFETY: a Toolhelp devolve um handle próprio ou INVALID_HANDLE_VALUE;
        // `entry` tem o `dwSize` que a API exige e vive até o fim do bloco; o
        // handle é fechado antes de sair.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
                return processes;
            }
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut ok = Process32FirstW(snapshot, &mut entry) != 0;
            while ok {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                processes.push((entry.th32ProcessID, name));
                ok = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        processes
    }

    /// Caminho do executável, com o acesso mínimo
    /// (`PROCESS_QUERY_LIMITED_INFORMATION`, sem `PROCESS_VM_READ`).
    fn exe_path(pid: u32) -> Option<PathBuf> {
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        // SAFETY: `buf` e `len` vivem até o fim do bloco e `len` diz o tamanho
        // de `buf` em u16; o handle é fechado antes de sair.
        let ok = unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return None;
            }
            let ok =
                QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
            CloseHandle(handle);
            ok != 0
        };
        let len = usize::try_from(len)
            .ok()
            .filter(|&l| ok && l <= buf.len())?;
        Some(PathBuf::from(OsString::from_wide(&buf[..len])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Valores falsos, só para os testes.
    const SENHA_FALSA: &str = "valor-falso-do-lockfile";
    const TOKEN_FALSO: &str = "valor-falso-dos-argumentos";
    const LOCKFILE: &str = "LeagueClient:12345:54321:valor-falso-do-lockfile:https";

    fn args_validos() -> Vec<String> {
        vec![
            "--app-port=50000".into(),
            format!("--remoting-auth-token={TOKEN_FALSO}"),
        ]
    }

    fn ux<S: AsRef<str>>(args: &[S]) -> ProcessInfo {
        ProcessInfo {
            pid: 200,
            name: "LeagueClientUx.exe".into(),
            exe: Some(PathBuf::from("/riot/lol/LeagueClientUx.exe")),
            args: args.iter().map(|s| s.as_ref().to_owned()).collect(),
        }
    }

    fn client() -> ProcessInfo {
        ProcessInfo {
            pid: 12345,
            name: "LeagueClient.exe".into(),
            exe: Some(PathBuf::from("/riot/lol/LeagueClient.exe")),
            args: vec![],
        }
    }

    fn other() -> ProcessInfo {
        ProcessInfo {
            pid: 1,
            name: "explorer.exe".into(),
            exe: Some(PathBuf::from("/windows/explorer.exe")),
            args: vec!["--app-port=1".into(), "--remoting-auth-token=nao".into()],
        }
    }

    fn fs(files: &[(&str, &str)]) -> impl Fn(&Path) -> io::Result<String> {
        let files: HashMap<PathBuf, String> = files
            .iter()
            .map(|(p, c)| (PathBuf::from(p), c.to_string()))
            .collect();
        move |path| {
            files
                .get(path)
                .cloned()
                .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }
    }

    #[test]
    fn interpreta_lockfile_valido() {
        let c = parse_lockfile(LOCKFILE, Path::new("lockfile")).unwrap();
        assert_eq!(c.pid, Some(12345));
        assert_eq!(c.port, 54321);
        assert_eq!(c.password, SENHA_FALSA);
        assert_eq!(c.protocol, Protocol::Https);
        assert_eq!(c.base_url(), "https://127.0.0.1:54321");
    }

    #[test]
    fn aceita_quebra_de_linha_no_fim() {
        let c = parse_lockfile(&format!("{LOCKFILE}\r\n"), Path::new("lockfile")).unwrap();
        assert_eq!(c.protocol, Protocol::Https);
    }

    #[test]
    fn rejeita_lockfile_invalido() {
        for content in [
            "",
            "LeagueClient:1:2:senha",
            "LeagueClient:1:2:senha:https:extra",
            "LeagueClient:x:54321:senha:https",
            "LeagueClient:1:99999:senha:https",
            "LeagueClient:1:0:senha:https",
            "LeagueClient:1:54321::https",
            "LeagueClient:1:54321:senha:ftp",
        ] {
            let err = parse_lockfile(content, Path::new("lockfile")).unwrap_err();
            assert!(
                matches!(err, LockfileError::Invalid { .. }),
                "{content:?}: {err}"
            );
        }
    }

    #[test]
    fn debug_nao_mostra_a_senha() {
        let c = parse_lockfile(LOCKFILE, Path::new("lockfile")).unwrap();
        let debug = format!("{c:?}");
        assert!(!debug.contains(SENHA_FALSA), "{debug}");
        assert!(debug.contains("54321"));
    }

    #[test]
    fn interpreta_argumentos_do_processo() {
        let c = parse_process_args(
            200,
            &[
                "\"--app-port=50000\"",
                &format!("--remoting-auth-token={TOKEN_FALSO}"),
                "--foo",
            ],
        )
        .unwrap();
        assert_eq!(c.port, 50000);
        assert_eq!(c.password, TOKEN_FALSO);
        assert_eq!(c.source, Source::ProcessArgs { pid: 200 });
    }

    #[test]
    fn argumentos_incompletos_nao_servem() {
        assert!(parse_process_args(1, &["--app-port=50000"]).is_none());
        assert!(parse_process_args(1, &["--remoting-auth-token=t"]).is_none());
        assert!(parse_process_args(1, &["--app-port=abc", "--remoting-auth-token=t"]).is_none());
        // Prefixo parecido não conta.
        assert!(parse_process_args(1, &["--app-portx=1", "--remoting-auth-token=t"]).is_none());
    }

    #[test]
    fn sem_processo_do_lol() {
        let err = discover_from(&[other()], fs(&[])).unwrap_err();
        assert!(matches!(err, DiscoveryError::ClientNotRunning));
    }

    #[test]
    fn acha_lockfile_pelo_install_directory() {
        let processes = [other(), ux(&["--install-directory=/jogos/lol"])];
        let c = discover_from(&processes, fs(&[("/jogos/lol/lockfile", LOCKFILE)])).unwrap();
        assert_eq!(c.port, 54321);
        assert_eq!(c.source, Source::Lockfile("/jogos/lol/lockfile".into()));
    }

    #[test]
    fn acha_lockfile_pela_pasta_do_executavel_depois_do_kill_ux() {
        // Depois do kill-ux só sobra o LeagueClient, sem argumentos úteis.
        let c = discover_from(&[client()], fs(&[("/riot/lol/lockfile", LOCKFILE)])).unwrap();
        assert_eq!(c.source, Source::Lockfile("/riot/lol/lockfile".into()));
    }

    #[test]
    fn lockfile_tem_prioridade_sobre_argumentos() {
        let processes = [ux(&args_validos())];
        let c = discover_from(&processes, fs(&[("/riot/lol/lockfile", LOCKFILE)])).unwrap();
        assert_eq!(c.port, 54321);
    }

    #[test]
    fn usa_argumentos_quando_nao_ha_lockfile() {
        let processes = [client(), ux(&args_validos())];
        let c = discover_from(&processes, fs(&[])).unwrap();
        assert_eq!(c.port, 50000);
        assert_eq!(c.source, Source::ProcessArgs { pid: 200 });
    }

    #[test]
    fn lockfile_invalido_cai_no_fallback() {
        let processes = [ux(&args_validos())];
        let c = discover_from(&processes, fs(&[("/riot/lol/lockfile", "lixo")])).unwrap();
        assert_eq!(c.port, 50000);
    }

    #[test]
    fn sem_lockfile_nem_argumentos_informa_o_ultimo_erro() {
        let err = discover_from(&[client()], fs(&[("/riot/lol/lockfile", "lixo")])).unwrap_err();
        let DiscoveryError::CredentialsNotFound { last_error } = &err else {
            panic!("erro inesperado: {err}");
        };
        assert!(matches!(last_error, Some(LockfileError::Invalid { .. })));
        assert!(err.to_string().contains("inválido"));
    }

    #[test]
    fn nome_do_processo_sem_exe_e_sem_diferenciar_maiusculas() {
        let mut p = client();
        p.name = "leagueclient".into();
        assert!(p.is_client());
        p.name = "LeagueClientUxRender.exe".into();
        assert!(!p.is_client());
    }

    #[test]
    fn sem_pasta_conhecida_tenta_a_pasta_padrao() {
        // O Vanguard negou o acesso ao LeagueClient: sem exe nem argumentos.
        let mut p = client();
        p.exe = None;
        let padrao = PathBuf::from(DEFAULT_LEAGUE_DIR).join(LOCKFILE_NAME);
        let c = discover_from(&[p], fs(&[(padrao.to_str().unwrap(), LOCKFILE)])).unwrap();
        assert_eq!(c.source, Source::Lockfile(padrao));
    }

    #[test]
    fn so_abre_os_processos_do_lol_e_do_riot_client() {
        for nome in [
            "LeagueClient.exe",
            "leagueclientux.exe",
            "RiotClientServices.exe",
        ] {
            assert!(wants_exe(nome), "{nome}");
        }
        // O jogo, o Vanguard e o resto do sistema nunca são abertos.
        for nome in [
            "League of Legends.exe",
            "vgc.exe",
            "vgtray.exe",
            "LeagueClientUxRender.exe",
            "Riot Client.exe",
            "explorer.exe",
        ] {
            assert!(!wants_exe(nome), "{nome}");
        }
    }

    #[test]
    fn le_lockfile_do_disco() {
        let dir = std::env::temp_dir().join(format!("easylobby-lockfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(LOCKFILE_NAME);
        std::fs::write(&path, LOCKFILE).unwrap();
        let c = Credentials::from_lockfile_path(&path).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(c.port, 54321);
        assert!(matches!(
            Credentials::from_lockfile_path(&path),
            Err(LockfileError::Io { .. })
        ));
    }
}
