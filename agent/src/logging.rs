//! Log de erros em arquivo (#534).
//!
//! Em release o app não tem console, então um erro que acontece no LoL real
//! some sem deixar rastro. Este módulo grava os eventos do `tracing` num
//! arquivo de texto por dia, na pasta que o app desktop escolher (a pasta de
//! logs do app, `%LOCALAPPDATA%\io.github.rmuratah.easylobby\logs`):
//!
//! - um arquivo por dia, `easylobby-AAAA-MM-DD.log`, com a data local;
//! - só os [`RETENTION_DAYS`] arquivos mais novos ficam; os outros são
//!   apagados quando um arquivo novo abre;
//! - cada arquivo para de crescer em [`MAX_FILE_BYTES`], com uma linha
//!   avisando que o limite chegou.
//!
//! Com o envio de erros ligado (#550), os erros também vão para a base dos
//! devs; veja [`crate::error_report`].
//!
//! O que entra: avisos e erros de todo o app e as mensagens `info` do
//! easylobby (conexão com o LoL, por exemplo). Nunca entram a senha do
//! lockfile, o cabeçalho `Authorization`, a chave da API da Riot nem o texto
//! das mensagens do chat: quem chama o `tracing` cuida disso, e os erros do
//! agente já não carregam esses dados.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{Local, NaiveDate};
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt::{self, format::Writer, time::FormatTime};
use tracing_subscriber::prelude::*;

use crate::error_report::ReportLayer;

/// Quantos arquivos (dias) ficam na pasta.
pub const RETENTION_DAYS: usize = 7;

/// Tamanho máximo de um arquivo do dia.
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;

/// Começo do nome dos arquivos.
const PREFIX: &str = "easylobby-";
const SUFFIX: &str = ".log";

/// Liga o log em arquivo na pasta `dir` (e no console, para o `tauri dev`).
///
/// Também registra os pânicos, que de outro jeito só iriam para o console.
/// Com `report`, os erros também vão para a base remota (#550).
///
/// Chame uma vez, no começo do app. Se já houver um subscriber global, não
/// faz nada.
pub fn init(dir: &Path, report: Option<ReportLayer>) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let file = Mutex::new(DailyFile::new(dir.to_path_buf()));
    let filter = Targets::new()
        .with_target("easylobby_agent", Level::INFO)
        .with_target("easylobby_desktop_lib", Level::INFO)
        .with_target("easylobby_desktop", Level::INFO)
        .with_default(Level::WARN);
    let subscriber = tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(file)
                .with_ansi(false)
                .with_timer(LocalTime)
                .with_filter(filter.clone()),
        )
        .with(
            fmt::layer()
                .with_writer(io::stderr)
                .with_timer(LocalTime)
                .with_filter(filter),
        )
        .with(report);
    if tracing::subscriber::set_global_default(subscriber).is_ok() {
        log_panics();
    }
    Ok(())
}

/// Manda os pânicos para o log, mantendo o comportamento de antes.
fn log_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!(" em {}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "sem mensagem".to_owned());
        tracing::error!(target: "easylobby_agent::panic", "pânico{location}: {payload}");
        previous(info);
    }));
}

/// Hora local com milissegundos.
struct LocalTime;

impl FormatTime for LocalTime {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%Y-%m-%d %H:%M:%S%.3f"))
    }
}

/// Nome do arquivo de um dia.
pub fn file_name(date: NaiveDate) -> String {
    format!("{PREFIX}{}{SUFFIX}", date.format("%Y-%m-%d"))
}

/// Se `name` é um arquivo de log do easylobby (`easylobby-AAAA-MM-DD.log`).
fn is_log_file(name: &str) -> bool {
    name.strip_prefix(PREFIX)
        .and_then(|rest| rest.strip_suffix(SUFFIX))
        .is_some_and(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok())
}

/// Apaga os arquivos de log mais velhos, deixando os `keep` mais novos.
/// Outros arquivos da pasta ficam intactos.
pub fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut logs: Vec<String> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| is_log_file(name))
        .collect();
    // AAAA-MM-DD ordena como texto.
    logs.sort();
    let excess = logs.len().saturating_sub(keep);
    for name in &logs[..excess] {
        let _ = fs::remove_file(dir.join(name));
    }
}

/// Arquivo do dia, trocado quando a data local muda.
///
/// Uma falha ao abrir ou gravar o arquivo nunca derruba o app: a linha só
/// não é gravada.
pub struct DailyFile {
    dir: PathBuf,
    today: Box<dyn Fn() -> NaiveDate + Send>,
    max_bytes: u64,
    current: Option<Current>,
}

struct Current {
    date: NaiveDate,
    file: File,
    written: u64,
    full: bool,
}

impl DailyFile {
    pub fn new(dir: PathBuf) -> Self {
        Self::with_clock(dir, MAX_FILE_BYTES, || Local::now().date_naive())
    }

    /// Com outro relógio e outro limite, para os testes.
    pub fn with_clock(
        dir: PathBuf,
        max_bytes: u64,
        today: impl Fn() -> NaiveDate + Send + 'static,
    ) -> Self {
        Self {
            dir,
            today: Box::new(today),
            max_bytes,
            current: None,
        }
    }

    fn current(&mut self) -> Option<&mut Current> {
        let date = (self.today)();
        if self.current.as_ref().is_none_or(|c| c.date != date) {
            self.current = None;
            let _ = fs::create_dir_all(&self.dir);
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.dir.join(file_name(date)))
                .ok()?;
            let written = file.metadata().map(|m| m.len()).unwrap_or(0);
            prune(&self.dir, RETENTION_DAYS);
            self.current = Some(Current {
                date,
                file,
                written,
                full: written >= self.max_bytes,
            });
        }
        self.current.as_mut()
    }
}

impl Write for DailyFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let max_bytes = self.max_bytes;
        let Some(current) = self.current() else {
            return Ok(buf.len());
        };
        if current.full {
            return Ok(buf.len());
        }
        if current.written + buf.len() as u64 > max_bytes {
            current.full = true;
            let _ = current.file.write_all(
                "--- limite do arquivo atingido; o resto do dia não foi gravado ---\n".as_bytes(),
            );
            return Ok(buf.len());
        }
        if current.file.write_all(buf).is_ok() {
            current.written += buf.len() as u64;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(current) = self.current.as_mut() {
            let _ = current.file.flush();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Arc;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("easylobby-logs-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn day(n: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap() + chrono::Days::new(n as u64)
    }

    #[test]
    fn um_arquivo_por_dia() {
        let dir = temp_dir("dia");
        let clock = Arc::new(AtomicI64::new(0));
        let c = clock.clone();
        let mut log =
            DailyFile::with_clock(dir.clone(), 1024, move || day(c.load(Ordering::SeqCst)));
        log.write_all(b"primeiro\n").unwrap();
        clock.store(1, Ordering::SeqCst);
        log.write_all(b"segundo\n").unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("easylobby-2026-11-01.log")).unwrap(),
            "primeiro\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("easylobby-2026-11-02.log")).unwrap(),
            "segundo\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn para_de_crescer_no_limite() {
        let dir = temp_dir("limite");
        let mut log = DailyFile::with_clock(dir.clone(), 10, || day(0));
        log.write_all(b"12345\n").unwrap();
        log.write_all(b"6789012\n").unwrap();
        log.write_all(b"mais\n").unwrap();
        let text = fs::read_to_string(dir.join("easylobby-2026-11-01.log")).unwrap();
        assert!(text.starts_with("12345\n--- limite"), "{text}");
        assert!(!text.contains("mais"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn guarda_so_os_dias_mais_novos() {
        let dir = temp_dir("prune");
        for n in 0..10 {
            fs::write(dir.join(file_name(day(n))), b"x").unwrap();
        }
        fs::write(dir.join("outro.txt"), b"x").unwrap();
        prune(&dir, 7);
        let mut names: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names.len(), 8);
        assert_eq!(names[0], "easylobby-2026-11-04.log");
        assert!(names.contains(&"outro.txt".to_owned()));
        let _ = fs::remove_dir_all(&dir);
    }
}
