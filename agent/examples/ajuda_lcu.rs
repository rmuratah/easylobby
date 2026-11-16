//! Mostra a documentação que a própria LCU dá das suas funções (`/help`),
//! para descobrir o formato de um endpoint que as gravações não mostram
//! (o corpo de um `POST`, por exemplo).
//!
//! Com o LoL aberto e logado:
//!
//! ```bash
//! cargo run -p easylobby-agent --example ajuda_lcu -- honor
//! cargo run -p easylobby-agent --example ajuda_lcu -- honor pre-end-of-game --saida target/ajuda-lcu.json
//! ```
//!
//! Cada termo filtra (sem diferenciar maiúsculas, e ignorando `-` e `/`) os
//! nomes das funções, eventos e tipos da LCU, como
//! `PostLolHonorV2V1HonorPlayer` ou `LolHonorV2ApiHonorPlayerServerRequest`.
//! Para cada nome achado, pede `/help?target=<nome>&format=Full` e junta
//! tudo num JSON (por padrão em `target/ajuda-lcu.json`). Só lê; nada é
//! mandado para o LoL além dos `GET /help`.

use std::path::PathBuf;

use easylobby_agent::lockfile;
use easylobby_agent::rest::LcuClient;
use reqwest::Method;
use serde_json::{Map, Value};

const USAGE: &str = "uso: ajuda_lcu <termo>... [--saida arquivo.json]";

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

async fn run() -> Result<(), String> {
    let mut terms = Vec::new();
    let mut out = PathBuf::from("target/ajuda-lcu.json");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--saida" => out = PathBuf::from(args.next().ok_or(USAGE)?),
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            _ => terms.push(normalize(&arg)),
        }
    }
    if terms.is_empty() {
        return Err(USAGE.into());
    }

    let credentials = lockfile::discover().map_err(|e| e.to_string())?;
    let lcu = LcuClient::new(&credentials).map_err(|e| e.to_string())?;
    let help = lcu
        .request(Method::GET, "/help", None)
        .await
        .map_err(|e| e.to_string())?;
    if !help.is_success() {
        return Err(format!("a LCU respondeu {} ao /help", help.status));
    }

    let mut names: Vec<String> = ["functions", "events", "types"]
        .iter()
        .filter_map(|k| help.body[*k].as_object())
        .flat_map(|m| m.keys().cloned())
        .filter(|name| {
            let name = normalize(name);
            terms.iter().any(|t| name.contains(t.as_str()))
        })
        .collect();
    names.sort();
    names.dedup();
    println!("{} nomes achados", names.len());

    let mut result = Map::new();
    for name in &names {
        let path = format!("/help?target={name}&format=Full");
        let body = match lcu.request(Method::GET, &path, None).await {
            Ok(response) => response.body,
            Err(e) => Value::String(e.to_string()),
        };
        println!("  {name}");
        result.insert(name.clone(), body);
    }

    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&Value::Object(result)).map_err(|e| e.to_string())?;
    std::fs::write(&out, text).map_err(|e| e.to_string())?;
    println!("salvo em {}", out.display());
    Ok(())
}
