//! Cache local de ícones e imagens do jogo (#29).
//!
//! A UI mostra ícones de campeão, de invocador, de feitiço, de runa e de
//! item. Em vez de cada tela baixar da internet, o agente baixa do
//! CommunityDragon uma vez, guarda no disco e serve pela mesma porta do
//! `/ws`:
//!
//! ```text
//! GET http://127.0.0.1:47800/assets/champion/266        -> PNG do Aatrox
//! GET http://127.0.0.1:47800/assets/profileIcon/29      -> JPG do ícone 29
//! GET http://127.0.0.1:47800/assets/summonerSpell/4     -> PNG do Flash
//! GET http://127.0.0.1:47800/assets/perk/8005           -> PNG do Pressione o Ataque
//! GET http://127.0.0.1:47800/assets/perkStyle/8000      -> PNG da árvore Precisão
//! GET http://127.0.0.1:47800/assets/item/3071           -> PNG da Cutelo Negro
//! ```
//!
//! O `id` é o mesmo que a LCU manda (`championId`, `profileIconId`,
//! `spell1Id`, `perkIds`...). Os tipos de imagem estão em [`AssetKind`], que
//! vira o tipo TypeScript `AssetKind`.
//!
//! Como funciona:
//!
//! 1. A versão do jogo sai do Data Dragon (`/api/versions.json`, a primeira
//!    da lista, cortada em `major.minor`, como `16.22`). Cada versão tem a sua
//!    pasta no cache; quando sai um patch, a pasta do patch anterior é apagada,
//!    e um ícone retrabalhado aparece sozinho. Sem internet, o agente usa a
//!    pasta mais nova que já estiver no disco.
//! 2. Para cada tipo, o agente baixa uma vez o índice do CommunityDragon
//!    (`champion-summary.json`, `summoner-spells.json`...) e guarda no disco.
//!    O índice diz o caminho da imagem de cada `id`. Um `id` fora do índice
//!    responde 404 sem baixar nada.
//! 3. A imagem é baixada na primeira vez que alguém pede e gravada em
//!    `<versão>/<tipo>/<id>.<ext>`. Das próximas vezes sai do disco.
//!
//! Os downloads usam HTTPS com as raízes do sistema e a verificação TLS
//! ligada. Só saem pedidos para os dois endereços de [`AssetConfig`], com
//! caminhos montados a partir do índice, nunca a partir do pedido da UI.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use tokio::sync::Mutex;
use ts_rs::TS;

/// Caminho base das imagens no servidor do `/ws`: `/assets/<tipo>/<id>`.
pub const ASSETS_PATH: &str = "/assets";

/// Data Dragon, de onde sai a versão do jogo.
pub const DEFAULT_DDRAGON_URL: &str = "https://ddragon.leagueoflegends.com";

/// CommunityDragon, de onde saem os índices e as imagens.
pub const DEFAULT_CDRAGON_URL: &str = "https://raw.communitydragon.org";

/// Pasta do CommunityDragon com os dados que o client usa.
const GAME_DATA: &str = "plugins/rcp-be-lol-game-data/global/default";

/// Prefixo dos caminhos de imagem nos índices do CommunityDragon.
const GAME_DATA_PREFIX: &str = "/lol-game-data/assets/";

/// Maior imagem aceita. Os ícones têm dezenas de KB.
const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

/// Maior índice aceito. O de ícones de invocador tem perto de 1 MB.
const MAX_INDEX_BYTES: usize = 32 * 1024 * 1024;

/// Sem internet, quanto tempo esperar para tentar descobrir a versão de
/// novo.
const OFFLINE_RETRY: Duration = Duration::from_secs(5 * 60);

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Tipo de imagem, o primeiro pedaço do caminho depois de `/assets/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum AssetKind {
    /// Ícone quadrado do campeão, pelo `championId`.
    Champion,
    /// Ícone de invocador, pelo `profileIconId`.
    ProfileIcon,
    /// Feitiço de invocador (Flash, Incendiar...), pelo `spell1Id`/`spell2Id`.
    SummonerSpell,
    /// Runa, pelo id da runa (`perkIds`).
    Perk,
    /// Árvore de runas (Precisão, Dominação...), pelo `primaryStyleId`/
    /// `subStyleId`.
    PerkStyle,
    /// Item, pelo id de `items` do histórico de partidas (#172).
    Item,
}

impl AssetKind {
    pub const ALL: [AssetKind; 6] = [
        AssetKind::Champion,
        AssetKind::ProfileIcon,
        AssetKind::SummonerSpell,
        AssetKind::Perk,
        AssetKind::PerkStyle,
        AssetKind::Item,
    ];

    /// Nome no caminho da URL e da pasta do cache.
    pub fn as_str(self) -> &'static str {
        match self {
            AssetKind::Champion => "champion",
            AssetKind::ProfileIcon => "profileIcon",
            AssetKind::SummonerSpell => "summonerSpell",
            AssetKind::Perk => "perk",
            AssetKind::PerkStyle => "perkStyle",
            AssetKind::Item => "item",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == name)
    }

    /// Índice no CommunityDragon, em `<GAME_DATA>/v1/`.
    fn index_file(self) -> &'static str {
        match self {
            AssetKind::Champion => "champion-summary.json",
            AssetKind::ProfileIcon => "profile-icons.json",
            AssetKind::SummonerSpell => "summoner-spells.json",
            AssetKind::Perk => "perks.json",
            AssetKind::PerkStyle => "perkstyles.json",
            AssetKind::Item => "items.json",
        }
    }

    /// Campo com o caminho da imagem em cada item do índice.
    fn path_field(self) -> &'static str {
        match self {
            AssetKind::Champion => "squarePortraitPath",
            _ => "iconPath",
        }
    }
}

/// De onde baixar e onde guardar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetConfig {
    /// Pasta do cache. Cada versão do jogo fica numa subpasta.
    pub cache_dir: PathBuf,
    /// Endereço do Data Dragon, sem `/` no fim.
    pub ddragon_url: String,
    /// Endereço do CommunityDragon, sem `/` no fim.
    pub cdragon_url: String,
}

impl Default for AssetConfig {
    fn default() -> Self {
        Self {
            cache_dir: default_cache_dir(),
            ddragon_url: DEFAULT_DDRAGON_URL.to_owned(),
            cdragon_url: DEFAULT_CDRAGON_URL.to_owned(),
        }
    }
}

/// Pasta padrão do cache: `%LOCALAPPDATA%\easylobby\cache\assets` no
/// Windows, `$XDG_CACHE_HOME/easylobby/assets` (ou `~/.cache/...`) no resto.
/// Sem nenhuma dessas variáveis, a pasta temporária do sistema.
pub fn default_cache_dir() -> PathBuf {
    let env_dir = |name: &str| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        if let Some(dir) = env_dir("LOCALAPPDATA") {
            return dir.join("easylobby").join("cache").join("assets");
        }
    } else if let Some(dir) = env_dir("XDG_CACHE_HOME") {
        return dir.join("easylobby").join("assets");
    } else if let Some(home) = env_dir("HOME") {
        return home.join(".cache").join("easylobby").join("assets");
    }
    std::env::temp_dir().join("easylobby").join("assets")
}

/// Uma imagem pronta para mandar à UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

#[derive(Debug, Error)]
pub enum AssetError {
    /// O `id` não está no índice do tipo, ou o CommunityDragon respondeu 404.
    #[error("imagem não encontrada")]
    NotFound,
    /// Sem internet e sem nada no cache.
    #[error("não deu para baixar: {0}")]
    Download(String),
    #[error("resposta inesperada de {url}: {message}")]
    BadResponse { url: String, message: String },
    #[error("erro no cache em disco: {0}")]
    Io(#[from] io::Error),
}

/// Cache de imagens. Clonar é barato: os clones dividem o mesmo cache.
#[derive(Clone)]
pub struct AssetCache {
    inner: Arc<Inner>,
}

struct Inner {
    config: AssetConfig,
    http: reqwest::Client,
    /// Versão em uso e quando ela foi decidida.
    version: Mutex<Option<ResolvedVersion>>,
    /// Índices já lidos, por versão e tipo: `id -> caminho no CommunityDragon`.
    indexes: Mutex<HashMap<(String, AssetKind), Arc<Index>>>,
}

#[derive(Debug, Clone)]
struct ResolvedVersion {
    version: String,
    /// `false` quando veio do disco por falta de internet.
    online: bool,
    at: Instant,
}

/// `id -> caminho relativo dentro de <GAME_DATA>/`, já em minúsculas.
type Index = HashMap<i64, String>;

impl AssetCache {
    pub fn new(config: AssetConfig) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("easylobby/", env!("CARGO_PKG_VERSION")))
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("cliente HTTP com as opções padrão");
        Self {
            inner: Arc::new(Inner {
                config,
                http,
                version: Mutex::new(None),
                indexes: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub fn config(&self) -> &AssetConfig {
        &self.inner.config
    }

    /// A imagem `id` do tipo `kind`, do disco ou baixada na hora.
    pub async fn get(&self, kind: AssetKind, id: i64) -> Result<Asset, AssetError> {
        let version = self.version().await?;
        let dir = self.inner.config.cache_dir.join(&version);
        let index = self.index(&version, kind).await?;
        let relative = index.get(&id).ok_or(AssetError::NotFound)?;
        let ext = extension(relative);
        let file = dir.join(kind.as_str()).join(format!("{id}.{ext}"));
        let content_type = content_type(ext);

        if let Ok(bytes) = tokio::fs::read(&file).await {
            return Ok(Asset {
                bytes,
                content_type,
            });
        }
        let bytes = self
            .download_game_data(&version, relative, MAX_IMAGE_BYTES)
            .await?;
        write_atomic(&file, &bytes).await?;
        Ok(Asset {
            bytes,
            content_type,
        })
    }

    /// Versão em uso (`16.22`). Pergunta ao Data Dragon uma vez por
    /// execução; sem internet, usa a pasta mais nova do disco e tenta de novo
    /// depois de [`OFFLINE_RETRY`].
    pub async fn version(&self) -> Result<String, AssetError> {
        let mut current = self.inner.version.lock().await;
        if let Some(resolved) = current.as_ref() {
            if resolved.online || resolved.at.elapsed() < OFFLINE_RETRY {
                return Ok(resolved.version.clone());
            }
        }
        let resolved = match self.latest_version().await {
            Ok(version) => {
                prune_other_versions(&self.inner.config.cache_dir, &version).await;
                ResolvedVersion {
                    version,
                    online: true,
                    at: Instant::now(),
                }
            }
            Err(error) => match newest_cached_version(&self.inner.config.cache_dir).await {
                Some(version) => ResolvedVersion {
                    version,
                    online: false,
                    at: Instant::now(),
                },
                None => return Err(error),
            },
        };
        let version = resolved.version.clone();
        *current = Some(resolved);
        Ok(version)
    }

    async fn latest_version(&self) -> Result<String, AssetError> {
        let url = format!("{}/api/versions.json", self.inner.config.ddragon_url);
        let bytes = self.download(&url, MAX_INDEX_BYTES).await?;
        let versions: Vec<String> =
            serde_json::from_slice(&bytes).map_err(|e| AssetError::BadResponse {
                url: url.clone(),
                message: e.to_string(),
            })?;
        versions
            .first()
            .and_then(|v| patch_from_ddragon(v))
            .ok_or_else(|| AssetError::BadResponse {
                url,
                message: "nenhuma versão no formato major.minor.patch".into(),
            })
    }

    async fn index(&self, version: &str, kind: AssetKind) -> Result<Arc<Index>, AssetError> {
        // Uma trava só: dois pedidos ao mesmo tempo não baixam o mesmo índice
        // duas vezes. Depois da primeira vez é só uma consulta ao mapa.
        let mut indexes = self.inner.indexes.lock().await;
        let key = (version.to_owned(), kind);
        if let Some(index) = indexes.get(&key) {
            return Ok(index.clone());
        }
        let file = self
            .inner
            .config
            .cache_dir
            .join(version)
            .join("index")
            .join(kind.index_file());
        let from_disk = match tokio::fs::read(&file).await {
            Ok(bytes) => parse_index(kind, &bytes).ok(),
            Err(_) => None,
        };
        let index = match from_disk {
            Some(index) => index,
            None => {
                let bytes = self.download_index(version, kind).await?;
                let index =
                    parse_index(kind, &bytes).map_err(|message| AssetError::BadResponse {
                        url: kind.index_file().to_owned(),
                        message,
                    })?;
                write_atomic(&file, &bytes).await?;
                index
            }
        };
        let index = Arc::new(index);
        indexes.insert(key, index.clone());
        Ok(index)
    }

    async fn download_index(&self, version: &str, kind: AssetKind) -> Result<Vec<u8>, AssetError> {
        let relative = format!("v1/{}", kind.index_file());
        self.download_game_data(version, &relative, MAX_INDEX_BYTES)
            .await
    }

    /// Um arquivo de `<GAME_DATA>/` na pasta da versão. No dia do patch o
    /// CommunityDragon às vezes ainda não tem a pasta da versão nova: aí vale
    /// a `latest`.
    async fn download_game_data(
        &self,
        version: &str,
        relative: &str,
        max_bytes: usize,
    ) -> Result<Vec<u8>, AssetError> {
        let base = &self.inner.config.cdragon_url;
        let url = format!("{base}/{version}/{GAME_DATA}/{relative}");
        match self.download(&url, max_bytes).await {
            Err(AssetError::NotFound) => {
                let latest = format!("{base}/latest/{GAME_DATA}/{relative}");
                self.download(&latest, max_bytes).await
            }
            other => other,
        }
    }

    async fn download(&self, url: &str, max_bytes: usize) -> Result<Vec<u8>, AssetError> {
        let mut response = self
            .inner
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| AssetError::Download(e.to_string()))?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(AssetError::NotFound);
        }
        if !status.is_success() {
            return Err(AssetError::BadResponse {
                url: url.to_owned(),
                message: format!("status {status}"),
            });
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| AssetError::Download(e.to_string()))?
        {
            if bytes.len() + chunk.len() > max_bytes {
                return Err(AssetError::BadResponse {
                    url: url.to_owned(),
                    message: format!("maior que {max_bytes} bytes"),
                });
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

/// `"16.22.1"` -> `"16.22"`, o nome da pasta no CommunityDragon. Recusa o
/// que não for número (as versões antigas como `lolpatch_7.20`).
pub fn patch_from_ddragon(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    let numeric = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (numeric(major) && numeric(minor)).then(|| format!("{major}.{minor}"))
}

/// Ordem das versões `major.minor` como números (`16.9` < `16.10`).
fn version_key(version: &str) -> Option<(u32, u32)> {
    let (major, minor) = version.split_once('.')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// Lê um índice do CommunityDragon: uma lista de objetos com `id` e o campo
/// de caminho do tipo (o de árvores de runas vem dentro de `styles`).
/// Caminhos fora de `/lol-game-data/assets/` ou com `..` ficam de fora.
fn parse_index(kind: AssetKind, bytes: &[u8]) -> Result<Index, String> {
    let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let items = match &value {
        Value::Array(items) => items,
        Value::Object(object) => match object.get("styles") {
            Some(Value::Array(items)) => items,
            _ => return Err("índice sem lista de itens".into()),
        },
        _ => return Err("índice sem lista de itens".into()),
    };
    let field = kind.path_field();
    let mut index = Index::new();
    for item in items {
        let (Some(id), Some(path)) = (
            item.get("id").and_then(Value::as_i64),
            item.get(field).and_then(Value::as_str),
        ) else {
            continue;
        };
        if let Some(relative) = game_data_path(path) {
            index.insert(id, relative);
        }
    }
    Ok(index)
}

/// `/lol-game-data/assets/DATA/Spells/Icons2D/Summoner_flash.png` ->
/// `data/spells/icons2d/summoner_flash.png`. O CommunityDragon guarda tudo
/// em minúsculas.
fn game_data_path(path: &str) -> Option<String> {
    let prefix = path.get(..GAME_DATA_PREFIX.len())?;
    if !prefix.eq_ignore_ascii_case(GAME_DATA_PREFIX) {
        return None;
    }
    let relative = path[GAME_DATA_PREFIX.len()..].to_ascii_lowercase();
    let safe = !relative.is_empty()
        && relative.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        });
    safe.then_some(relative)
}

fn extension(relative: &str) -> &'static str {
    match relative.rsplit_once('.').map(|(_, ext)| ext) {
        Some("jpg" | "jpeg") => "jpg",
        Some("webp") => "webp",
        Some("svg") => "svg",
        _ => "png",
    }
}

fn content_type(ext: &str) -> &'static str {
    match ext {
        "jpg" => "image/jpeg",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    }
}

/// Grava num arquivo temporário e renomeia: quem lê nunca vê um arquivo
/// pela metade, e dois downloads da mesma imagem não se atrapalham.
async fn write_atomic(file: &Path, bytes: &[u8]) -> io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.{n}.tmp", std::process::id()));
    let tmp = file.with_file_name(name);
    tokio::fs::write(&tmp, bytes).await?;
    if let Err(error) = tokio::fs::rename(&tmp, file).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        // Outro download chegou antes; o arquivo dele serve.
        if tokio::fs::metadata(file).await.is_err() {
            return Err(error);
        }
    }
    Ok(())
}

/// Pastas de versão (`16.22`) que já estão no cache.
async fn cached_versions(cache_dir: &Path) -> Vec<String> {
    let mut versions = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(cache_dir).await else {
        return versions;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
        if let (true, Some(name)) = (is_dir, entry.file_name().to_str()) {
            if version_key(name).is_some() {
                versions.push(name.to_owned());
            }
        }
    }
    versions
}

async fn newest_cached_version(cache_dir: &Path) -> Option<String> {
    cached_versions(cache_dir)
        .await
        .into_iter()
        .max_by_key(|v| version_key(v))
}

/// Apaga as pastas das outras versões. Só toca em pastas com nome de versão,
/// nunca em outra coisa que esteja na pasta do cache.
async fn prune_other_versions(cache_dir: &Path, keep: &str) {
    for version in cached_versions(cache_dir).await {
        if version != keep {
            let _ = tokio::fs::remove_dir_all(cache_dir.join(version)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versao_do_ddragon_vira_pasta_do_cdragon() {
        assert_eq!(patch_from_ddragon("16.22.1").as_deref(), Some("16.22"));
        assert_eq!(patch_from_ddragon("16.3.1").as_deref(), Some("16.3"));
        assert_eq!(patch_from_ddragon("lolpatch_7.20"), None);
        assert_eq!(patch_from_ddragon("16"), None);
        assert_eq!(patch_from_ddragon(""), None);
    }

    #[test]
    fn versoes_em_ordem_numerica() {
        assert!(version_key("16.9") < version_key("16.10"));
        assert!(version_key("15.24") < version_key("16.1"));
        assert_eq!(version_key("latest"), None);
        assert_eq!(version_key("index"), None);
    }

    #[test]
    fn nomes_dos_tipos() {
        for kind in AssetKind::ALL {
            assert_eq!(AssetKind::parse(kind.as_str()), Some(kind));
            assert_eq!(
                serde_json::to_value(kind).unwrap(),
                Value::String(kind.as_str().into())
            );
        }
        assert_eq!(AssetKind::parse("Champion"), None);
        assert_eq!(AssetKind::parse("../etc"), None);
    }

    #[test]
    fn caminho_do_game_data() {
        assert_eq!(
            game_data_path("/lol-game-data/assets/DATA/Spells/Icons2D/Summoner_flash.png")
                .as_deref(),
            Some("data/spells/icons2d/summoner_flash.png")
        );
        assert_eq!(
            game_data_path("/lol-game-data/assets/v1/champion-icons/266.png").as_deref(),
            Some("v1/champion-icons/266.png")
        );
        assert_eq!(game_data_path("/lol-game-data/assets/../../segredo"), None);
        assert_eq!(game_data_path("/lol-game-data/assets/v1//x.png"), None);
        assert_eq!(game_data_path("/lol-game-data/assets/v1/a b.png"), None);
        assert_eq!(game_data_path("https://exemplo.com/x.png"), None);
        assert_eq!(game_data_path("/lol-game-data/assets/"), None);
        assert_eq!(game_data_path(""), None);
    }

    #[test]
    fn le_indices_do_cdragon() {
        let champions = br#"[
            {"id":-1,"name":"None","alias":"None","squarePortraitPath":"/lol-game-data/assets/v1/champion-icons/-1.png"},
            {"id":266,"name":"Aatrox","alias":"Aatrox","squarePortraitPath":"/lol-game-data/assets/v1/champion-icons/266.png"},
            {"id":1,"name":"Sem caminho"}
        ]"#;
        let index = parse_index(AssetKind::Champion, champions).unwrap();
        assert_eq!(index.len(), 2);
        assert_eq!(index[&266], "v1/champion-icons/266.png");
        assert_eq!(index[&-1], "v1/champion-icons/-1.png");

        let styles = br#"{"schemaVersion":2,"styles":[
            {"id":8000,"name":"Precision","iconPath":"/lol-game-data/assets/v1/perk-images/Styles/7201_Precision.png"}
        ]}"#;
        let index = parse_index(AssetKind::PerkStyle, styles).unwrap();
        assert_eq!(index[&8000], "v1/perk-images/styles/7201_precision.png");

        let items = br#"[
            {"id":3071,"name":"Black Cleaver","iconPath":"/lol-game-data/assets/ASSETS/Items/Icons2D/3071_Fighter_T3_BlackCleaver.png"}
        ]"#;
        let index = parse_index(AssetKind::Item, items).unwrap();
        assert_eq!(
            index[&3071],
            "assets/items/icons2d/3071_fighter_t3_blackcleaver.png"
        );

        assert!(parse_index(AssetKind::Perk, b"{}").is_err());
        assert!(parse_index(AssetKind::Perk, b"nao e json").is_err());
    }

    #[test]
    fn extensao_e_tipo_do_conteudo() {
        assert_eq!(extension("v1/profile-icons/29.jpg"), "jpg");
        assert_eq!(extension("v1/champion-icons/266.png"), "png");
        assert_eq!(extension("sem-extensao"), "png");
        assert_eq!(content_type("jpg"), "image/jpeg");
        assert_eq!(content_type("png"), "image/png");
    }

    #[tokio::test]
    async fn so_apaga_pastas_de_versao() {
        let dir =
            std::env::temp_dir().join(format!("easylobby-assets-prune-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["16.21", "16.22", "16.9", "outra-coisa"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        std::fs::write(dir.join("16.20"), b"arquivo, nao pasta").unwrap();

        assert_eq!(newest_cached_version(&dir).await.as_deref(), Some("16.22"));
        prune_other_versions(&dir, "16.22").await;
        assert!(dir.join("16.22").is_dir());
        assert!(!dir.join("16.21").exists());
        assert!(!dir.join("16.9").exists());
        assert!(dir.join("outra-coisa").is_dir());
        assert!(dir.join("16.20").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
