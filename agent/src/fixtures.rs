//! Fixtures da LCU gravadas de sessões reais (#5).
//!
//! Cada conjunto fica em `agent/tests/fixtures/<patch>/` com três arquivos:
//!
//! - `manifest.json`: patch, data da gravação e se o conjunto é sintético
//!   ([`Manifest`]).
//! - `rest.json`: respostas de `GET` da LCU ([`RestFixture`]), cada uma marcada
//!   com a fase do gameflow em que foi tirada.
//! - `events.jsonl`: eventos WAMP de `OnJsonApiEvent`, um por linha, na ordem
//!   em que chegaram ([`EventFixture`]).
//!
//! O mock-lcu (#6) lê esses arquivos com [`FixtureSet::load`].
//!
//! Antes de ir para o disco, tudo passa pelo [`Sanitizer`], que troca dados da
//! conta (puuid, ids, nomes, mensagens de chat) por valores falsos e apaga
//! tokens e senhas. Fixtures nunca devem ter dados reais de ninguém.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Nome do arquivo com os metadados do conjunto.
pub const MANIFEST_FILE: &str = "manifest.json";
/// Nome do arquivo com as respostas REST.
pub const REST_FILE: &str = "rest.json";
/// Nome do arquivo com os eventos WAMP (JSON Lines).
pub const EVENTS_FILE: &str = "events.jsonl";

/// Versão do formato dos arquivos. Mude quando o formato quebrar.
pub const FORMAT_VERSION: u32 = 1;

/// Prefixo de todo valor falso gerado pelo [`Sanitizer`].
pub const FAKE_PREFIX: &str = "fake-";

/// Valor que substitui tokens, senhas e afins.
pub const REDACTED: &str = "REDACTED";

/// Texto que substitui o conteúdo das mensagens de chat.
pub const FAKE_CHAT_BODY: &str = "mensagem de exemplo";

/// Metadados de um conjunto de fixtures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Versão do formato ([`FORMAT_VERSION`]).
    pub format_version: u32,
    /// Patch do LoL, por exemplo `14.22`.
    pub patch: String,
    /// Versão completa do jogo devolvida por `/lol-patch/v1/game-version`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_version: Option<String>,
    /// Data da gravação em RFC 3339.
    pub recorded_at: String,
    /// `true` quando o conjunto foi escrito à mão, não gravado do LoL.
    #[serde(default)]
    pub synthetic: bool,
    /// O que a sessão gravada cobre (lobby, fila, champ select...).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

/// Uma resposta REST gravada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestFixture {
    /// Método HTTP (por enquanto só `GET`).
    pub method: String,
    /// Caminho com query, por exemplo `/lol-lobby/v2/lobby`.
    pub path: String,
    /// Fase do gameflow quando a resposta foi tirada (`None`, `Lobby`,
    /// `Matchmaking`, `ReadyCheck`, `ChampSelect`...).
    pub phase: String,
    /// Status HTTP.
    pub status: u16,
    /// Corpo em JSON. `null` quando a resposta veio vazia.
    pub body: Value,
}

/// Um evento WAMP gravado (`[8, "OnJsonApiEvent", {...}]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventFixture {
    /// Milissegundos desde o início da gravação.
    pub t_ms: u64,
    /// Recurso que mudou, por exemplo `/lol-gameflow/v1/gameflow-phase`.
    pub uri: String,
    /// `Create`, `Update` ou `Delete`.
    pub event_type: String,
    /// Novo estado do recurso.
    pub data: Value,
}

/// Um conjunto de fixtures carregado do disco.
#[derive(Debug, Clone, PartialEq)]
pub struct FixtureSet {
    pub manifest: Manifest,
    pub rest: Vec<RestFixture>,
    pub events: Vec<EventFixture>,
}

/// Erro ao ler ou gravar fixtures.
#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("não foi possível acessar {}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("{} inválido: {source}", path.display())]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("{} linha {line} inválida: {source}", path.display())]
    JsonLine {
        path: PathBuf,
        line: usize,
        source: serde_json::Error,
    },
}

impl FixtureSet {
    /// Carrega o conjunto da pasta `dir`.
    pub fn load(dir: &Path) -> Result<Self, FixtureError> {
        let manifest = read_json(&dir.join(MANIFEST_FILE))?;
        let rest = read_json(&dir.join(REST_FILE))?;

        let events_path = dir.join(EVENTS_FILE);
        let content = read_to_string(&events_path)?;
        let events = content
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(i, line)| {
                serde_json::from_str(line).map_err(|source| FixtureError::JsonLine {
                    path: events_path.clone(),
                    line: i + 1,
                    source,
                })
            })
            .collect::<Result<_, _>>()?;

        Ok(Self {
            manifest,
            rest,
            events,
        })
    }

    /// Grava o conjunto na pasta `dir`, criando a pasta se precisar.
    ///
    /// Não sanitiza: passe tudo pelo [`Sanitizer`] antes.
    pub fn save(&self, dir: &Path) -> Result<(), FixtureError> {
        fs::create_dir_all(dir).map_err(|source| FixtureError::Io {
            path: dir.to_path_buf(),
            source,
        })?;

        write(&dir.join(MANIFEST_FILE), &to_pretty(&self.manifest)?)?;
        write(&dir.join(REST_FILE), &to_pretty(&self.rest)?)?;

        let mut lines = String::new();
        for event in &self.events {
            let line = serde_json::to_string(event).map_err(|source| FixtureError::Json {
                path: dir.join(EVENTS_FILE),
                source,
            })?;
            lines.push_str(&line);
            lines.push('\n');
        }
        write(&dir.join(EVENTS_FILE), &lines)
    }
}

fn read_to_string(path: &Path) -> Result<String, FixtureError> {
    fs::read_to_string(path).map_err(|source| FixtureError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, FixtureError> {
    serde_json::from_str(&read_to_string(path)?).map_err(|source| FixtureError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn to_pretty<T: Serialize>(value: &T) -> Result<String, FixtureError> {
    let mut text = serde_json::to_string_pretty(value).map_err(|source| FixtureError::Json {
        path: PathBuf::new(),
        source,
    })?;
    text.push('\n');
    Ok(text)
}

fn write(path: &Path, content: &str) -> Result<(), FixtureError> {
    fs::write(path, content).map_err(|source| FixtureError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Chaves com identificadores da conta, trocados por texto falso.
const ID_STRING_KEYS: [&str; 9] = [
    "puuid",
    "obfuscatedPuuid",
    "pid",
    "accountId",
    // No histórico de partidas (#172), ao lado do `accountId`.
    "currentAccountId",
    "summonerId",
    "obfuscatedSummonerId",
    "playerId",
    // Com o id da partida e a região dá para achar os participantes no
    // histórico, inclusive a partida em que um amigo está agora.
    "gameId",
];

/// Chaves com nomes de jogadores.
const NAME_KEYS: [&str; 6] = [
    "gameName",
    "displayName",
    "internalName",
    "summonerName",
    "playerName",
    "gameTag",
];

/// Recursos cujo corpo é só uma lista de ids de jogadores.
const ID_LIST_RESOURCES: [&str; 1] = ["/lol-chat/v1/obfuscated-puuids"];

/// Chaves com nomes de grupos de amigos, que o jogador escolhe.
const GROUP_KEYS: [&str; 2] = ["groupName", "displayGroupName"];

/// Chaves com texto livre escrito por jogadores.
const FREE_TEXT_KEYS: [&str; 3] = ["statusMessage", "note", "lastSeenMessage"];

/// Trechos que marcam uma chave como segredo (comparados em minúsculas).
const SECRET_KEY_PARTS: [&str; 9] = [
    "token",
    "password",
    "jwt",
    "ticket",
    "secret",
    "authorization",
    "cookie",
    "signature",
    "credential",
];

/// Ids numéricos abaixo disto ficam como estão: os falsos começam em 1001 e
/// ids reais da Riot são bem maiores. Assim sanitizar de novo não muda nada.
const FAKE_NUMBER_LIMIT: u64 = 100_000;

/// Valores reais mais curtos que isto não são procurados dentro de outros
/// textos, para não trocar números pequenos por engano.
const MIN_EMBEDDED_LEN: usize = 6;

/// Troca dados da conta por valores falsos e apaga segredos.
///
/// O mesmo valor real vira sempre o mesmo valor falso dentro de uma gravação,
/// então as relações continuam valendo (o `summonerId` do lobby bate com o do
/// champ select, o `pid` do chat bate com o `puuid`). Valores já falsos
/// (começando com [`FAKE_PREFIX`]) ficam como estão, então sanitizar uma
/// fixture já sanitizada não muda nada.
#[derive(Debug, Default)]
pub struct Sanitizer {
    /// Valor real -> valor falso, para textos.
    strings: HashMap<String, String>,
    /// Valor real -> valor falso, para ids numéricos.
    numbers: HashMap<u64, u64>,
    /// Contador por tipo de valor, para gerar `fake-puuid-0001` etc.
    counters: HashMap<&'static str, usize>,
    /// Segredos conhecidos (senha do lockfile) que nunca podem aparecer.
    secrets: Vec<String>,
}

impl Sanitizer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra um segredo (por exemplo a senha do lockfile) para ser apagado
    /// onde quer que apareça.
    pub fn add_secret(&mut self, secret: impl Into<String>) {
        let secret = secret.into();
        if !secret.is_empty() {
            self.secrets.push(secret);
        }
    }

    /// Sanitiza um valor JSON qualquer (corpo REST ou `data` de evento).
    pub fn sanitize(&mut self, value: &Value) -> Value {
        // Primeiro passa trocando pelas chaves e aprendendo os valores reais;
        // depois troca os valores reais que aparecem dentro de outros textos
        // (como `<puuid>@br1.pvp.net`).
        let replaced = self.replace_by_key(value);
        self.replace_embedded(&replaced)
    }

    /// Sanitiza o corpo de um recurso da LCU, sabendo de qual caminho ele veio.
    ///
    /// Alguns recursos são uma lista solta de ids, sem chave que diga o que
    /// são, como `/lol-chat/v1/obfuscated-puuids` (os jogadores do champ
    /// select). Esses viram ids falsos inteiros; o resto segue [`Self::sanitize`].
    pub fn sanitize_resource(&mut self, path: &str, value: &Value) -> Value {
        let resource = path.split('?').next().unwrap_or(path);
        if ID_LIST_RESOURCES.contains(&resource) {
            let replaced = self.fake_id("puuid", value);
            return self.replace_embedded(&replaced);
        }
        self.sanitize(value)
    }

    /// Sanitiza um texto solto, como o caminho de uma requisição ou a URI de
    /// um evento (`/lol-chat/v1/conversations/<pid>/messages`).
    pub fn sanitize_str(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for secret in &self.secrets {
            text = text.replace(secret.as_str(), REDACTED);
        }
        // Os mais longos primeiro, para um valor que contém outro trocar inteiro.
        let mut known: Vec<_> = self
            .strings
            .iter()
            .filter(|(real, _)| real.len() >= MIN_EMBEDDED_LEN)
            .collect();
        known.sort_by_key(|(real, _)| std::cmp::Reverse(real.len()));
        for (real, fake) in known {
            text = text.replace(real.as_str(), fake);
        }
        let mut numbers: Vec<_> = self
            .numbers
            .iter()
            .map(|(real, fake)| (real.to_string(), fake.to_string()))
            .filter(|(real, _)| real.len() >= MIN_EMBEDDED_LEN)
            .collect();
        numbers.sort_by_key(|(real, _)| std::cmp::Reverse(real.len()));
        for (real, fake) in numbers {
            text = text.replace(&real, &fake);
        }
        text
    }

    fn replace_by_key(&mut self, value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                // `name` é ambíguo (campeão, fila, runa). Só é nome de jogador
                // quando o objeto também identifica um jogador.
                let is_player = map.contains_key("puuid") || map.contains_key("pid");
                // Grupo de amigos (`/lol-chat/v1/friend-groups`).
                let is_group = map.contains_key("isMetaGroup");
                let mut out = serde_json::Map::new();
                for (key, child) in map {
                    let new = if is_secret_key(key) {
                        redact(child)
                    } else if ID_STRING_KEYS.contains(&key.as_str()) {
                        self.fake_id(key_kind(key), child)
                    } else if NAME_KEYS.contains(&key.as_str()) || (is_player && key == "name") {
                        self.fake_text("name", child)
                    } else if GROUP_KEYS.contains(&key.as_str()) || (is_group && key == "name") {
                        self.fake_group(child)
                    } else if key == "tagLine" {
                        self.fake_text("tag", child)
                    } else if key == "body" {
                        replace_text(child, FAKE_CHAT_BODY)
                    } else if FREE_TEXT_KEYS.contains(&key.as_str()) {
                        replace_text(child, "")
                    } else {
                        self.replace_by_key(child)
                    };
                    out.insert(key.clone(), new);
                }
                Value::Object(out)
            }
            Value::Array(items) => {
                Value::Array(items.iter().map(|v| self.replace_by_key(v)).collect())
            }
            other => other.clone(),
        }
    }

    fn replace_embedded(&self, value: &Value) -> Value {
        match value {
            Value::String(s) => Value::String(self.sanitize_str(s)),
            Value::Array(items) => {
                Value::Array(items.iter().map(|v| self.replace_embedded(v)).collect())
            }
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| (self.sanitize_str(k), self.replace_embedded(v)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    /// Troca um id (texto ou número) por um falso. Ids `0` e textos vazios
    /// ficam como estão: na LCU eles querem dizer "ninguém" (bot, vaga vazia).
    /// Ids pequenos ([`FAKE_NUMBER_LIMIT`]) já são falsos.
    fn fake_id(&mut self, kind: &'static str, value: &Value) -> Value {
        match value {
            Value::Number(n) => match n.as_u64() {
                Some(real) if real < FAKE_NUMBER_LIMIT => value.clone(),
                None => value.clone(),
                Some(real) => Value::from(self.fake_number(real)),
            },
            // A LCU às vezes manda ids numéricos como texto (`"lol": {"gameId": "123"}`).
            Value::String(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => {
                match s.parse::<u64>() {
                    Ok(real) if real >= FAKE_NUMBER_LIMIT => {
                        Value::String(self.fake_number(real).to_string())
                    }
                    _ => value.clone(),
                }
            }
            Value::String(_) => self.fake_text(kind, value),
            Value::Array(items) => {
                Value::Array(items.iter().map(|v| self.fake_id(kind, v)).collect())
            }
            other => other.clone(),
        }
    }

    fn fake_number(&mut self, real: u64) -> u64 {
        let next = 1000 + self.numbers.len() as u64 + 1;
        *self.numbers.entry(real).or_insert(next)
    }

    /// Nomes de grupo começando com `**` são os do sistema (`**Default`).
    fn fake_group(&mut self, value: &Value) -> Value {
        match value {
            Value::String(s) if s.starts_with("**") => value.clone(),
            _ => self.fake_text("group", value),
        }
    }

    fn fake_text(&mut self, kind: &'static str, value: &Value) -> Value {
        let Value::String(real) = value else {
            return self.replace_by_key(value);
        };
        if real.is_empty() || real.starts_with(FAKE_PREFIX) {
            return value.clone();
        }
        if let Some(fake) = self.strings.get(real) {
            return Value::String(fake.clone());
        }
        let counter = self.counters.entry(kind).or_insert(0);
        *counter += 1;
        let fake = format!("{FAKE_PREFIX}{kind}-{counter:04}");
        self.strings.insert(real.clone(), fake.clone());
        Value::String(fake)
    }
}

fn key_kind(key: &str) -> &'static str {
    match key {
        "puuid" | "obfuscatedPuuid" => "puuid",
        "pid" => "pid",
        "accountId" | "currentAccountId" => "account",
        "summonerId" | "obfuscatedSummonerId" => "summoner",
        _ => "player",
    }
}

fn is_secret_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    SECRET_KEY_PARTS.iter().any(|part| key.contains(part))
}

fn redact(value: &Value) -> Value {
    match value {
        Value::Null => Value::Null,
        Value::String(s) if s.is_empty() => value.clone(),
        _ => Value::String(REDACTED.to_owned()),
    }
}

fn replace_text(value: &Value, with: &str) -> Value {
    match value {
        Value::String(s) if !s.is_empty() && s != with => Value::String(with.to_owned()),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn troca_ids_e_mantem_relacoes() {
        let mut s = Sanitizer::new();
        let lobby = s.sanitize(&json!({
            "members": [{ "puuid": "aaaa-1111-real", "summonerId": 123456789, "gameName": "Jogador Real" }]
        }));
        let chat = s.sanitize(
            &json!({ "pid": "aaaa-1111-real@br1.pvp.net", "id": "aaaa-1111-real@br1.pvp.net" }),
        );

        let member = &lobby["members"][0];
        assert_eq!(member["puuid"], "fake-puuid-0001");
        assert_eq!(member["summonerId"], 1001);
        assert_eq!(member["gameName"], "fake-name-0001");
        // O `id` do chat repete o `pid`, então vira o mesmo valor falso.
        assert_eq!(chat["pid"], "fake-pid-0001");
        assert_eq!(chat["id"], "fake-pid-0001");
    }

    #[test]
    fn name_so_muda_em_objeto_de_jogador() {
        let mut s = Sanitizer::new();
        let out = s.sanitize(&json!({
            "champion": { "name": "Ahri" },
            "friend": { "puuid": "bbbb-2222-real", "name": "Amigo Real" }
        }));
        assert_eq!(out["champion"]["name"], "Ahri");
        assert_eq!(out["friend"]["name"], "fake-name-0001");
    }

    #[test]
    fn apaga_segredos_e_texto_livre() {
        let mut s = Sanitizer::new();
        s.add_secret("senha-do-lockfile");
        let out = s.sanitize(&json!({
            "accessToken": "abc",
            "idToken": { "x": 1 },
            "body": "oi, tudo bem?",
            "statusMessage": "jogando",
            "url": "https://riot:senha-do-lockfile@127.0.0.1"
        }));
        assert_eq!(out["accessToken"], REDACTED);
        assert_eq!(out["idToken"], REDACTED);
        assert_eq!(out["body"], FAKE_CHAT_BODY);
        assert_eq!(out["statusMessage"], "");
        assert_eq!(out["url"], "https://riot:REDACTED@127.0.0.1");
    }

    #[test]
    fn troca_id_da_partida_e_grupos_de_amigos() {
        let mut s = Sanitizer::new();
        let out = s.sanitize(&json!({
            "friend": { "puuid": "eeee-5555-real", "groupName": "amigos da facul", "displayGroupName": "**Default",
                        "lol": { "gameId": "3300811915" } },
            "session": { "gameData": { "gameId": 3300811915_u64 } },
            "group": { "isMetaGroup": false, "name": "amigos da facul" }
        }));
        assert_eq!(out["friend"]["lol"]["gameId"], "1001");
        assert_eq!(out["session"]["gameData"]["gameId"], 1001);
        assert_eq!(out["friend"]["groupName"], "fake-group-0001");
        assert_eq!(out["friend"]["displayGroupName"], "**Default");
        assert_eq!(out["group"]["name"], "fake-group-0001");
    }

    #[test]
    fn historico_de_partidas_sem_conta_real() {
        let mut s = Sanitizer::new();
        let out = s.sanitize(&json!({
            "gameId": 3300811915_u64,
            "participantIdentities": [{"participantId": 1, "player": {
                "accountId": 2876543210_u64, "currentAccountId": 2876543210_u64,
                "puuid": "aaaa-real", "gameName": "Jogador", "tagLine": "BR1",
                "summonerName": "Jogador", "summonerId": 98765432_u64,
                "matchHistoryUri": "/v1/stats/player_history/BR1/2876543210"
            }}]
        }));
        let player = &out["participantIdentities"][0]["player"];
        assert_eq!(out["gameId"], 1001);
        assert_eq!(player["accountId"], player["currentAccountId"]);
        assert_ne!(player["currentAccountId"], 2876543210_u64);
        assert_eq!(player["tagLine"], "fake-tag-0001");
        assert!(!player["matchHistoryUri"]
            .as_str()
            .unwrap()
            .contains("2876543210"));
        assert_eq!(
            s.sanitize_str("/lol-match-history/v1/games/3300811915"),
            "/lol-match-history/v1/games/1001"
        );
    }

    #[test]
    fn troca_lista_solta_de_puuids() {
        let mut s = Sanitizer::new();
        let out = s.sanitize_resource(
            "/lol-chat/v1/obfuscated-puuids",
            &json!(["ffff-6666-real", "gggg-7777-real"]),
        );
        assert_eq!(out, json!(["fake-puuid-0001", "fake-puuid-0002"]));
        // Em outro recurso, uma lista de textos fica como está.
        let out = s.sanitize_resource("/lol-chat/v1/friend-counts", &json!(["Ahri"]));
        assert_eq!(out, json!(["Ahri"]));
    }

    #[test]
    fn zero_e_vazio_ficam_como_estao() {
        let mut s = Sanitizer::new();
        let out = s.sanitize(&json!({ "summonerId": 0, "puuid": "" }));
        assert_eq!(out, json!({ "summonerId": 0, "puuid": "" }));
    }

    #[test]
    fn sanitizar_duas_vezes_nao_muda_nada() {
        let mut s = Sanitizer::new();
        let once = s.sanitize(&json!({
            "puuid": "cccc-3333-real",
            "summonerId": 987654321,
            "accessToken": "abc",
            "body": "oi"
        }));
        let twice = Sanitizer::new().sanitize(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn sanitiza_caminhos() {
        let mut s = Sanitizer::new();
        s.sanitize(&json!({ "pid": "dddd-4444-real@br1.pvp.net" }));
        assert_eq!(
            s.sanitize_str("/lol-chat/v1/conversations/dddd-4444-real@br1.pvp.net/messages"),
            "/lol-chat/v1/conversations/fake-pid-0001/messages"
        );
    }

    #[test]
    fn salva_e_carrega() {
        let dir = std::env::temp_dir().join(format!("easylobby-fixtures-{}", std::process::id()));
        let set = FixtureSet {
            manifest: Manifest {
                format_version: FORMAT_VERSION,
                patch: "0.0".into(),
                game_version: None,
                recorded_at: "2026-01-01T00:00:00Z".into(),
                synthetic: true,
                notes: String::new(),
            },
            rest: vec![RestFixture {
                method: "GET".into(),
                path: "/lol-gameflow/v1/gameflow-phase".into(),
                phase: "None".into(),
                status: 200,
                body: json!("None"),
            }],
            events: vec![EventFixture {
                t_ms: 10,
                uri: "/lol-gameflow/v1/gameflow-phase".into(),
                event_type: "Update".into(),
                data: json!("Lobby"),
            }],
        };
        set.save(&dir).unwrap();
        let loaded = FixtureSet::load(&dir).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(loaded, set);
    }
}
