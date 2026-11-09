//! Grava as filas do client e compara com as do app (#283).
//!
//! Com o LoL aberto e logado:
//!
//! ```bash
//! cargo run -p easylobby-agent --example comparar_filas
//! cargo run -p easylobby-agent --example comparar_filas -- --saida C:\filas
//! ```
//!
//! Lê `GET /lol-game-queues/v1/queues`, grava a lista crua em
//! `filas-<patch>.json` e escreve `comparacao-filas-<patch>.md`, que diz, para
//! cada fila do client:
//!
//! - as filas com lugar fixo na tela de escolha de modo do app
//!   (`ui/src/agent/gameModes.ts`), com o nome do app ao lado do nome da LCU;
//! - as que o agente repassa e o app mostra no card de eventos;
//! - as abertas no client que o agente deixa de fora, com o motivo;
//! - as filas fixas do app que a LCU não listou ou estão fechadas.
//!
//! Sem abrir o LoL, a comparação roda sobre uma lista já gravada (a lista crua
//! ou o `rest.json` de uma fixture):
//!
//! ```bash
//! cargo run -p easylobby-agent --example comparar_filas -- --arquivo agent/tests/fixtures/16.22/rest.json
//! ```
//!
//! Também aceita `--mock` e `--lockfile <arquivo>`, como o example
//! `invocador`. A lista de filas não tem dado pessoal: não passa pelo
//! sanitizer.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use easylobby_agent::lockfile::{self, Credentials, Protocol};
use easylobby_agent::queues::{self, GameQueue, GAME_QUEUES_PATH};
use easylobby_agent::rest::LcuClient;
use serde_json::Value;

/// Arquivo da CA que o mock-lcu grava ao lado do lockfile.
const MOCK_CA_FILE: &str = "mock-lcu-ca.pem";

const USAGE: &str = "\
uso: comparar_filas [--mock | --lockfile <arquivo> | --arquivo <json>]
                    [--saida <pasta>] [--ui <gameModes.ts>]

  (sem opções)          acha o LoL aberto e lê as filas dele
  --mock                usa o lockfile padrão do mock-lcu
  --lockfile <arquivo>  usa este lockfile (fixa o mock-lcu-ca.pem ao lado)
  --arquivo <json>      não abre o LoL: compara uma lista já gravada (a lista
                        crua ou o rest.json de uma fixture)
  --saida <pasta>       onde gravar (padrão: filas-lol/ na pasta atual)
  --ui <arquivo>        o gameModes.ts da UI (padrão: o do repositório)
";

enum Source {
    Discover,
    Lockfile(PathBuf),
    File(PathBuf),
}

struct Args {
    source: Source,
    out: PathBuf,
    ui: PathBuf,
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
    let (list, patch, from_lcu) = match &args.source {
        Source::File(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|e| format!("não consegui ler {}: {e}", path.display()))?;
            let value: Value = serde_json::from_str(&text)
                .map_err(|e| format!("{} não é JSON: {e}", path.display()))?;
            let list = queue_list(value)
                .ok_or_else(|| format!("{} não tem a lista de filas", path.display()))?;
            (list, patch_from_path(path), false)
        }
        source => {
            let lcu = match source {
                Source::Lockfile(path) => client_from_lockfile(path)?,
                _ => {
                    let credentials = lockfile::discover().map_err(|e| e.to_string())?;
                    LcuClient::new(&credentials).map_err(|e| e.to_string())?
                }
            };
            println!("LCU em {}", lcu.base_url());
            let list: Value = lcu
                .get(GAME_QUEUES_PATH)
                .await
                .map_err(|e| format!("falha ao ler {GAME_QUEUES_PATH}: {e}"))?;
            let version: Option<String> = lcu.get("/lol-patch/v1/game-version").await.ok();
            let patch = version
                .as_deref()
                .and_then(short_patch)
                .unwrap_or_else(|| "sem-patch".into());
            (list, patch, true)
        }
    };

    let app = std::fs::read_to_string(&args.ui)
        .map(|text| parse_app_modes(&text))
        .map_err(|e| format!("não consegui ler {}: {e}", args.ui.display()))?;
    if app.is_empty() {
        return Err(format!(
            "não achei nenhuma fila fixa em {}; o formato do arquivo mudou?",
            args.ui.display()
        ));
    }

    let report = build_report(&list, &app, &patch);

    std::fs::create_dir_all(&args.out)
        .map_err(|e| format!("não consegui criar {}: {e}", args.out.display()))?;
    if from_lcu {
        let json_path = args.out.join(format!("filas-{patch}.json"));
        let json = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
        std::fs::write(&json_path, json + "\n")
            .map_err(|e| format!("não consegui gravar {}: {e}", json_path.display()))?;
        println!("lista crua: {}", json_path.display());
    }
    let md_path = args.out.join(format!("comparacao-filas-{patch}.md"));
    std::fs::write(&md_path, &report.markdown)
        .map_err(|e| format!("não consegui gravar {}: {e}", md_path.display()))?;
    println!("comparação: {}", md_path.display());
    println!();
    println!("{}", report.summary);
    Ok(())
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        source: Source::Discover,
        out: PathBuf::from("filas-lol"),
        ui: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("ui")
            .join("src")
            .join("agent")
            .join("gameModes.ts"),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} precisa de um valor"));
        match arg.as_str() {
            "--mock" => {
                args.source = Source::Lockfile(
                    std::env::temp_dir()
                        .join("easylobby-mock-lcu")
                        .join(lockfile::LOCKFILE_NAME),
                )
            }
            "--lockfile" => args.source = Source::Lockfile(PathBuf::from(value()?)),
            "--arquivo" => args.source = Source::File(PathBuf::from(value()?)),
            "--saida" => args.out = PathBuf::from(value()?),
            "--ui" => args.ui = PathBuf::from(value()?),
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("opção desconhecida: {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

fn client_from_lockfile(path: &Path) -> Result<LcuClient, String> {
    let credentials = Credentials::from_lockfile_path(path).map_err(|e| e.to_string())?;
    let ca_path = path.with_file_name(MOCK_CA_FILE);
    match (credentials.protocol, std::fs::read(&ca_path)) {
        (Protocol::Https, Ok(ca_pem)) => LcuClient::with_root_certificate(&credentials, &ca_pem),
        _ => LcuClient::new(&credentials),
    }
    .map_err(|e| e.to_string())
}

/// A lista crua, ou o `body` dela num `rest.json` de fixture.
fn queue_list(value: Value) -> Option<Value> {
    let Value::Array(items) = value else {
        return None;
    };
    let fixture = items
        .iter()
        .find(|e| e.get("path").and_then(Value::as_str) == Some(GAME_QUEUES_PATH));
    match fixture {
        Some(entry) => entry.get("body").cloned().filter(Value::is_array),
        None => Some(Value::Array(items)),
    }
}

/// `16.22` de `.../fixtures/16.22/rest.json`, ou o nome do arquivo.
fn patch_from_path(path: &Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem == "rest" {
        if let Some(dir) = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|s| s.to_str())
        {
            return dir.to_owned();
        }
    }
    stem.strip_prefix("filas-").unwrap_or(stem).to_owned()
}

fn short_patch(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    (!major.is_empty() && !minor.is_empty()).then(|| format!("{major}.{minor}"))
}

/// Uma fila com lugar fixo na tela de escolha de modo do app.
struct AppQueue {
    queue_id: u32,
    tab: &'static str,
    card: String,
    name: String,
}

/// Lê as filas fixas de `gameModes.ts`: as abas pelas listas `pvpModes` e
/// `coopModes`, o card pelo `id`/`name` do modo e cada submodo pela linha
/// com `queueId`.
fn parse_app_modes(text: &str) -> Vec<AppQueue> {
    let mut tab = None;
    let mut card = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        if line.contains("const pvpModes") {
            tab = Some("PvP");
        } else if line.contains("const coopModes") {
            tab = Some("Co-op versus IA");
        } else if line.starts_with("const ") || line.starts_with("function ") {
            tab = None;
        }
        let Some(tab) = tab else { continue };
        if let Some(id) = number_field(line, "queueId") {
            for (queue_id, name) in submodes_in_line(line, id) {
                out.push(AppQueue {
                    queue_id,
                    tab,
                    card: card.clone(),
                    name,
                });
            }
        } else if string_field(line, "id").is_some() {
            if let Some(name) = string_field(line, "name") {
                card = name;
            }
        }
    }
    out
}

/// Um card pode ter o submodo na mesma linha (`submodes: [{ queueId: 1750,
/// name: ... }]`); cada `queueId` da linha leva o `name` que vem depois dele.
fn submodes_in_line(line: &str, first: u32) -> Vec<(u32, String)> {
    let parts: Vec<&str> = line.split("queueId:").skip(1).collect();
    if parts.is_empty() {
        return vec![(first, String::new())];
    }
    parts
        .iter()
        .filter_map(|part| {
            let rest = format!("queueId:{part}");
            let id = number_field(&rest, "queueId")?;
            Some((id, string_field(&rest, "name").unwrap_or_default()))
        })
        .collect()
}

fn field_start<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(pos) = line[from..].find(key) {
        let at = from + pos;
        let before = line[..at].chars().next_back();
        let rest = &line[at + key.len()..];
        let is_word = before.is_some_and(|c| c.is_alphanumeric() || c == '_');
        if !is_word {
            if let Some(value) = rest.trim_start().strip_prefix(':') {
                return Some(value.trim_start());
            }
        }
        from = at + key.len();
    }
    None
}

fn number_field(line: &str, key: &str) -> Option<u32> {
    let value = field_start(line, key)?;
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn string_field(line: &str, key: &str) -> Option<String> {
    let value = field_start(line, key)?;
    let quote = value
        .chars()
        .next()
        .filter(|c| matches!(c, '\'' | '"' | '`'))?;
    let body = &value[1..];
    let end = body.find(quote)?;
    Some(body[..end].to_owned())
}

/// Os campos da fila crua que o relatório mostra.
struct LcuQueue {
    id: u32,
    name: String,
    description: String,
    game_mode: String,
    category: String,
    mode_group: String,
    map_id: u32,
    availability: String,
    enabled: bool,
    visible: bool,
    custom: bool,
}

impl LcuQueue {
    fn from_json(raw: &Value) -> Option<Self> {
        let s = |key: &str| {
            raw.get(key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        let b = |key: &str| raw.get(key).and_then(Value::as_bool).unwrap_or(false);
        Some(Self {
            id: u32::try_from(raw.get("id")?.as_u64()?).ok()?,
            name: s("name"),
            description: s("description"),
            game_mode: s("gameMode"),
            category: s("gameSelectCategory"),
            mode_group: s("gameSelectModeGroup"),
            map_id: raw
                .get("mapId")
                .and_then(Value::as_u64)
                .and_then(|m| u32::try_from(m).ok())
                .unwrap_or(0),
            availability: s("queueAvailability"),
            enabled: b("isEnabled"),
            visible: b("isVisible"),
            custom: b("isCustom"),
        })
    }

    fn open(&self) -> bool {
        self.availability == "Available" && self.enabled && self.visible
    }

    /// O nome que o client mostra (a Jade PvP vem sem nome, só descrição).
    fn display_name(&self) -> &str {
        if self.name.trim().is_empty() {
            &self.description
        } else {
            &self.name
        }
    }

    fn state(&self) -> String {
        if self.open() {
            "aberta".into()
        } else if self.availability == "Available" && !self.visible {
            "invisível".into()
        } else {
            let mut why = vec![];
            if self.availability != "Available" {
                why.push(self.availability.as_str());
            }
            if !self.enabled {
                why.push("desligada");
            }
            if !self.visible {
                why.push("invisível");
            }
            format!("fechada ({})", why.join(", "))
        }
    }

    /// Por que o agente deixa a fila de fora (para as abertas).
    fn exclusion_reason(&self) -> &'static str {
        if self.category == "kTraining" {
            "treino (aba Treinamento do app)"
        } else if self.custom || self.category == "CreateCustom" {
            "personalizada (aba Personalizada do app)"
        } else if self.game_mode == "TFT" {
            "TFT (outro jogo)"
        } else if self.game_mode == "CHERRY" {
            "Arena fora da 3x6 (sem tela no app)"
        } else if self.category.is_empty() {
            "sem categoria na LCU"
        } else {
            "categoria fora de PvP e contra IA"
        }
    }
}

fn map_name(map_id: u32) -> String {
    let name = match map_id {
        11 => "Summoner's Rift",
        12 => "Abismo Uivante",
        21 => "Blitz do Nexus",
        22 => "Convergência (TFT)",
        30 => "Arena",
        33 => "Enxame",
        35 => "Treta",
        453 => "Summoner's Rift (Jade)",
        0 => return "—".into(),
        _ => return format!("mapa {map_id}"),
    };
    format!("{name} ({map_id})")
}

/// O mapa que cada card fixo do app espera.
fn expected_map(card: &str) -> Option<u32> {
    match card {
        "Summoner's Rift" => Some(11),
        "ARAM" => Some(12),
        "Arena" => Some(30),
        _ => None,
    }
}

/// Compara nomes sem diferença de caixa, acento de pontuação ou espaços.
fn same_name(a: &str, b: &str) -> bool {
    let norm = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect()
    };
    norm(a) == norm(b)
}

struct Report {
    markdown: String,
    summary: String,
}

fn cell(text: &str) -> String {
    if text.is_empty() {
        "—".into()
    } else {
        text.replace('|', "\\|")
    }
}

fn build_report(list: &Value, app: &[AppQueue], patch: &str) -> Report {
    let mut lcu: Vec<LcuQueue> = list
        .as_array()
        .map(|items| items.iter().filter_map(LcuQueue::from_json).collect())
        .unwrap_or_default();
    lcu.sort_by_key(|q| q.id);
    lcu.dedup_by_key(|q| q.id);
    let agent: Vec<GameQueue> = queues::from_json(list).unwrap_or_default();
    let app_ids: BTreeSet<u32> = app.iter().map(|q| q.queue_id).collect();
    let find = |id: u32| lcu.iter().find(|q| q.id == id);

    let mut md = String::new();
    let _ = writeln!(md, "# Filas do client x filas do app (patch {patch})\n");
    let _ = writeln!(
        md,
        "{} filas na LCU, {} abertas. Gerado pelo example `comparar_filas` (#283).\n",
        lcu.len(),
        lcu.iter().filter(|q| q.open()).count()
    );

    // 1. Filas fixas do app.
    let mut fixed_issues = 0;
    let _ = writeln!(md, "## Filas fixas da tela de escolha de modo\n");
    let _ = writeln!(
        md,
        "| queueId | Aba | Card | Nome no app | Nome no client | Descrição no client | Mapa | Situação | Observação |"
    );
    let _ = writeln!(
        md,
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    );
    for q in app {
        let mut notes = vec![];
        let (client_name, description, map, state) = match find(q.queue_id) {
            Some(l) => {
                if !same_name(&q.name, l.display_name()) {
                    notes.push("nome diferente".to_owned());
                }
                if let Some(expected) = expected_map(&q.card) {
                    if l.map_id != expected {
                        notes.push(format!("mapa esperado {expected}"));
                    }
                }
                if !l.open() {
                    notes.push("o app mostra acinzentada".to_owned());
                }
                (
                    l.display_name().to_owned(),
                    l.description.clone(),
                    map_name(l.map_id),
                    l.state(),
                )
            }
            None => {
                notes.push("a LCU não listou esta fila".to_owned());
                (String::new(), String::new(), "—".into(), "ausente".into())
            }
        };
        if notes.iter().any(|n| n != "o app mostra acinzentada") {
            fixed_issues += 1;
        }
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            q.queue_id,
            q.tab,
            cell(&q.card),
            cell(&q.name),
            cell(&client_name),
            cell(&description),
            map,
            state,
            cell(&notes.join("; "))
        );
    }

    // 2. Card de eventos.
    let events: Vec<&GameQueue> = agent
        .iter()
        .filter(|q| q.available && !app_ids.contains(&q.queue_id))
        .collect();
    let _ = writeln!(
        md,
        "\n## Filas abertas que o app mostra no card de eventos\n"
    );
    if events.is_empty() {
        let _ = writeln!(md, "Nenhuma.");
    } else {
        let _ = writeln!(
            md,
            "| queueId | Aba | Nome no app | Descrição no client | Modo | Grupo | Mapa |"
        );
        let _ = writeln!(md, "| --- | --- | --- | --- | --- | --- | --- |");
        for q in &events {
            let tab = if q.versus_ai {
                "Co-op versus IA"
            } else {
                "PvP"
            };
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} | {} | {} | {} |",
                q.queue_id,
                tab,
                cell(&q.name),
                cell(&q.description),
                cell(&q.game_mode),
                cell(&q.mode_group),
                map_name(q.map_id)
            );
        }
    }

    // 3. Abertas no client, fora do app.
    let agent_ids: BTreeSet<u32> = agent.iter().map(|q| q.queue_id).collect();
    let left_out: Vec<&LcuQueue> = lcu
        .iter()
        .filter(|q| q.open() && !agent_ids.contains(&q.id) && !app_ids.contains(&q.id))
        .collect();
    let _ = writeln!(md, "\n## Filas abertas no client que o app não mostra\n");
    if left_out.is_empty() {
        let _ = writeln!(md, "Nenhuma.");
    } else {
        let _ = writeln!(
            md,
            "| queueId | Nome no client | Descrição | Modo | Categoria | Mapa | Motivo |"
        );
        let _ = writeln!(md, "| --- | --- | --- | --- | --- | --- | --- |");
        for q in &left_out {
            let _ = writeln!(
                md,
                "| {} | {} | {} | {} | {} | {} | {} |",
                q.id,
                cell(q.display_name()),
                cell(&q.description),
                cell(&q.game_mode),
                cell(&q.category),
                map_name(q.map_id),
                q.exclusion_reason()
            );
        }
    }

    // 4. Todas as filas da LCU.
    let _ = writeln!(md, "\n## Todas as filas da LCU\n");
    let _ = writeln!(
        md,
        "| queueId | Nome | Descrição | Modo | Categoria | Grupo | Mapa | Situação | No app |"
    );
    let _ = writeln!(
        md,
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    );
    for q in &lcu {
        let in_app = if app_ids.contains(&q.id) {
            "fixa"
        } else if events.iter().any(|e| e.queue_id == q.id) {
            "eventos"
        } else {
            "não"
        };
        let _ = writeln!(
            md,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            q.id,
            cell(&q.name),
            cell(&q.description),
            cell(&q.game_mode),
            cell(&q.category),
            cell(&q.mode_group),
            map_name(q.map_id),
            q.state(),
            in_app
        );
    }

    let summary = format!(
        "{} filas na LCU ({} abertas)\n\
         {} filas fixas no app, {} com nome, mapa ou ausência para conferir\n\
         {} no card de eventos\n\
         {} abertas no client que o app não mostra",
        lcu.len(),
        lcu.iter().filter(|q| q.open()).count(),
        app.len(),
        fixed_issues,
        events.len(),
        left_out.len()
    );
    Report {
        markdown: md,
        summary,
    }
}
