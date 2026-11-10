//! Páginas de runas (#26): listar, editar, criar, apagar e aplicar.
//!
//! As páginas ficam em `/lol-perks/v1/`:
//!
//! - `GET /lol-perks/v1/pages` lista as páginas do jogador. A LCU manda um
//!   evento WAMP nesse URI com a lista inteira sempre que alguma muda.
//! - `GET /lol-perks/v1/currentpage` é a página em uso, e `PUT` no mesmo
//!   caminho, com o `id` da página no corpo, aplica outra.
//! - `POST /lol-perks/v1/pages` cria uma página; `PUT` e `DELETE` em
//!   `/lol-perks/v1/pages/<id>` editam e apagam.
//! - `GET /lol-perks/v1/inventory` diz quantas páginas o jogador tem e se
//!   cabe mais uma (`canAddCustomPage`).
//!
//! Para montar o editor, a UI precisa das árvores de runas: que runas cabem
//! em cada linha de cada árvore. O agente lê isso uma vez por conexão dos
//! arquivos de dados do jogo que a própria LCU serve, já no idioma do client:
//! `/lol-game-data/assets/v1/perkstyles.json` (árvores e linhas) e
//! `/lol-game-data/assets/v1/perks.json` (nome e descrição de cada runa).
//! Os ícones saem do `/assets` do agente ([`crate::assets`], tipos `perk` e
//! `perkStyle`).
//!
//! O [`Hub`](crate::ws::Hub) lê as páginas pelo REST a cada conexão e depois
//! segue os eventos; a UI recebe [`RunePages`] e [`RuneCatalog`] pelo `/ws`
//! e pede as mudanças com os comandos `selectRunePage`, `createRunePage`,
//! `updateRunePage` e `deleteRunePage`.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::rest::{LcuClient, LcuError};

/// Lista de páginas (e URI dos eventos WAMP dela).
pub const PAGES_PATH: &str = "/lol-perks/v1/pages";

/// Página em uso (e URI dos eventos WAMP dela).
pub const CURRENT_PAGE_PATH: &str = "/lol-perks/v1/currentpage";

/// Quantas páginas o jogador tem e se cabe mais uma.
pub const INVENTORY_PATH: &str = "/lol-perks/v1/inventory";

/// Árvores de runas, com as linhas de cada uma.
pub const PERK_STYLES_PATH: &str = "/lol-game-data/assets/v1/perkstyles.json";

/// Nome e descrição de cada runa.
pub const PERKS_PATH: &str = "/lol-game-data/assets/v1/perks.json";

/// Runas numa página completa quando o agente não tem as árvores para
/// contar: 4 da árvore primária, 2 da secundária e 3 fragmentos.
pub const DEFAULT_PERK_COUNT: usize = 9;

/// As páginas do jogador e qual está em uso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunePages {
    /// Páginas na ordem da LCU (a mesma do client oficial), incluindo as
    /// temporárias (`isTemporary`), como a recomendada para o campeão na
    /// seleção.
    pub pages: Vec<RunePage>,
    /// `id` da página em uso, ou `null` se a LCU não disse.
    #[ts(type = "number | null")]
    pub current_page_id: Option<u64>,
    /// Se cabe mais uma página. `false` quando todas as páginas compradas
    /// estão ocupadas: a UI deve oferecer apagar uma antes de criar. `true`
    /// também quando a LCU não informou; aí ela decide no `createRunePage`.
    pub can_create_page: bool,
}

/// Uma página de runas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunePage {
    #[ts(type = "number")]
    pub id: u64,
    pub name: String,
    /// Árvore primária (`perkStyle`), por exemplo `8000` (Precisão).
    pub primary_style_id: u32,
    /// Árvore secundária.
    pub sub_style_id: u32,
    /// Runas escolhidas, na ordem do client: a principal (keystone), as 3
    /// da árvore primária, as 2 da secundária e os 3 fragmentos. `0` é uma
    /// linha vazia (página incompleta).
    pub selected_perk_ids: Vec<u32>,
    /// Se pode ser editada (`updateRunePage`).
    pub is_editable: bool,
    /// Se pode ser apagada (`deleteRunePage`).
    pub is_deletable: bool,
    /// Página temporária, como a recomendada para o campeão na seleção.
    /// Some sozinha e não ocupa espaço de página comprada.
    pub is_temporary: bool,
    /// Se a página está completa e vale no jogo.
    pub is_valid: bool,
}

/// As árvores de runas, para o editor. Lidas uma vez por conexão.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuneCatalog {
    /// Árvores (Precisão, Dominação...), na ordem da LCU.
    pub styles: Vec<RuneStyle>,
    /// Nome e descrição das runas que aparecem em [`RuneStyle::slots`].
    pub perks: Vec<Perk>,
}

/// Uma árvore de runas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuneStyle {
    pub id: u32,
    pub name: String,
    /// Árvores que podem ser a secundária quando esta é a primária.
    pub allowed_sub_styles: Vec<u32>,
    /// Linhas da árvore, de cima para baixo: a das runas principais, as 3
    /// linhas comuns e as 3 de fragmentos.
    pub slots: Vec<RuneSlot>,
}

/// Uma linha de uma árvore: o jogador escolhe uma runa dela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuneSlot {
    pub kind: RuneSlotKind,
    /// Rótulo da linha (como "Ofensiva" nos fragmentos). Muitas vezes vazio.
    pub label: String,
    pub perks: Vec<u32>,
}

/// Tipo da linha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RuneSlotKind {
    /// Runa principal. Só vale na árvore primária.
    Keystone,
    /// Linha comum: uma runa na árvore primária, ou até duas linhas
    /// diferentes na secundária.
    Regular,
    /// Fragmentos. Vêm da árvore primária (todas têm os mesmos).
    StatMod,
}

/// Nome e descrição de uma runa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Perk {
    pub id: u32,
    pub name: String,
    /// Descrição curta, em texto puro (sem as marcações da Riot).
    pub short_desc: String,
    /// Descrição completa, para o tooltip da runa (#420): texto puro, com
    /// as quebras de linha da Riot (`<br>`) como `\n`. Vazia quando o
    /// `perks.json` não traz.
    pub long_desc: String,
}

/// Página que a UI manda para criar ou editar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunePageInput {
    pub name: String,
    pub primary_style_id: u32,
    pub sub_style_id: u32,
    /// Na ordem de [`RunePage::selected_perk_ids`].
    pub selected_perk_ids: Vec<u32>,
}

impl Default for RunePages {
    /// Sem páginas e, até a LCU dizer o contrário, com espaço para criar.
    fn default() -> Self {
        RunePages {
            pages: Vec::new(),
            current_page_id: None,
            can_create_page: true,
        }
    }
}

impl RunePages {
    /// Lê as páginas do JSON da LCU (resposta do `GET` ou `data` do evento
    /// de [`PAGES_PATH`]). Páginas que não têm o formato esperado ficam de
    /// fora; `None` quando o valor nem é uma lista.
    pub fn from_json(pages: &Value) -> Option<RunePages> {
        let raw = pages.as_array()?;
        let pages: Vec<RunePage> = raw.iter().filter_map(RunePage::from_json).collect();
        let current_page_id = raw
            .iter()
            .find(|p| p["current"] == true)
            .and_then(|p| p["id"].as_u64());
        Some(RunePages {
            pages,
            current_page_id,
            can_create_page: true,
        })
    }

    /// Troca a lista pela do evento. A página em uso só muda se a lista
    /// marcar uma (`current`).
    pub fn apply_pages(&mut self, pages: &Value) {
        if let Some(new) = RunePages::from_json(pages) {
            self.pages = new.pages;
            if new.current_page_id.is_some() {
                self.current_page_id = new.current_page_id;
            }
        }
    }

    /// Página em uso pelo JSON de [`CURRENT_PAGE_PATH`].
    pub fn apply_current_page(&mut self, page: &Value) {
        if let Some(id) = page["id"].as_u64() {
            self.current_page_id = Some(id);
        }
    }

    /// Espaço para página nova pelo JSON de [`INVENTORY_PATH`].
    pub fn apply_inventory(&mut self, inventory: &Value) {
        if let Some(can) = inventory["canAddCustomPage"].as_bool() {
            self.can_create_page = can;
        }
    }

    pub fn page(&self, id: u64) -> Option<&RunePage> {
        self.pages.iter().find(|p| p.id == id)
    }
}

impl RunePage {
    fn from_json(value: &Value) -> Option<RunePage> {
        let raw = RawPage::deserialize(value).ok()?;
        Some(RunePage {
            id: raw.id,
            name: raw.name,
            primary_style_id: u32::try_from(raw.primary_style_id).unwrap_or(0),
            sub_style_id: u32::try_from(raw.sub_style_id).unwrap_or(0),
            selected_perk_ids: raw
                .selected_perk_ids
                .into_iter()
                .map(|id| u32::try_from(id).unwrap_or(0))
                .collect(),
            is_editable: raw.is_editable,
            is_deletable: raw.is_deletable,
            is_temporary: raw.is_temporary,
            is_valid: raw.is_valid,
        })
    }
}

/// A página como a LCU manda, só com os campos usados.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPage {
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    primary_style_id: i64,
    #[serde(default)]
    sub_style_id: i64,
    #[serde(default)]
    selected_perk_ids: Vec<i64>,
    #[serde(default)]
    is_editable: bool,
    #[serde(default)]
    is_deletable: bool,
    #[serde(default)]
    is_temporary: bool,
    #[serde(default)]
    is_valid: bool,
}

impl RuneCatalog {
    /// Monta o catálogo com o `perkstyles.json` e o `perks.json` da LCU.
    /// `None` se o `perkstyles.json` não tiver o formato esperado; runa sem
    /// entrada no `perks.json` fica sem nome.
    pub fn from_json(styles: &Value, perks: &Value) -> Option<RuneCatalog> {
        let raw = RawStyles::deserialize(styles).ok()?;
        let styles: Vec<RuneStyle> = raw
            .styles
            .into_iter()
            .map(|s| RuneStyle {
                id: s.id,
                name: s.name,
                allowed_sub_styles: s.allowed_sub_styles,
                slots: s
                    .slots
                    .into_iter()
                    .map(|slot| RuneSlot {
                        kind: match slot.slot_type.as_str() {
                            "kKeyStone" => RuneSlotKind::Keystone,
                            "kStatMod" => RuneSlotKind::StatMod,
                            _ => RuneSlotKind::Regular,
                        },
                        label: slot.slot_label,
                        perks: slot.perks,
                    })
                    .collect(),
            })
            .collect();
        if styles.is_empty() {
            return None;
        }

        let raw_perks: Vec<RawPerk> = perks
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|p| RawPerk::deserialize(p).ok())
                    .collect()
            })
            .unwrap_or_default();
        // Só as runas das árvores, uma vez cada; o `perks.json` também traz
        // runas de outros modos e antigas.
        let mut ids: Vec<u32> = styles
            .iter()
            .flat_map(|s| s.slots.iter().flat_map(|slot| slot.perks.iter().copied()))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let perks = ids
            .into_iter()
            .map(|id| {
                let raw = raw_perks.iter().find(|p| p.id == id);
                Perk {
                    id,
                    name: raw.map(|p| p.name.clone()).unwrap_or_default(),
                    short_desc: raw.map(|p| plain_text(&p.short_desc)).unwrap_or_default(),
                    long_desc: raw
                        .map(|p| multiline_text(&p.long_desc))
                        .unwrap_or_default(),
                }
            })
            .collect();
        Some(RuneCatalog { styles, perks })
    }

    pub fn style(&self, id: u32) -> Option<&RuneStyle> {
        self.styles.iter().find(|s| s.id == id)
    }
}

#[derive(Deserialize)]
struct RawStyles {
    styles: Vec<RawStyle>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStyle {
    id: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    allowed_sub_styles: Vec<u32>,
    #[serde(default)]
    slots: Vec<RawSlot>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSlot {
    #[serde(rename = "type", default)]
    slot_type: String,
    #[serde(default)]
    slot_label: String,
    #[serde(default)]
    perks: Vec<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPerk {
    id: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    short_desc: String,
    #[serde(default)]
    long_desc: String,
}

/// Tira as marcações (`<b>`, `<lol-uikit-tooltipped-keyword ...>`) e junta os
/// espaços. `<br>` vira espaço.
fn plain_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut tag: Option<String> = None;
    for c in text.chars() {
        match (&mut tag, c) {
            (None, '<') => tag = Some(String::new()),
            (None, _) => out.push(c),
            (Some(name), '>') => {
                if name
                    .trim_start_matches('/')
                    .to_ascii_lowercase()
                    .starts_with("br")
                {
                    out.push(' ');
                }
                tag = None;
            }
            (Some(name), _) => name.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Como [`plain_text`], mas guarda as quebras de linha: `<br>` e `<hr>`
/// viram `\n`, cada linha tem os espaços juntados e nunca ficam mais de
/// uma linha em branco seguida nem linhas em branco nas pontas.
fn multiline_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut tag: Option<String> = None;
    for c in text.chars() {
        match (&mut tag, c) {
            (None, '<') => tag = Some(String::new()),
            (None, '\n') => out.push(' '),
            (None, _) => out.push(c),
            (Some(name), '>') => {
                let name = name.trim_start_matches('/').to_ascii_lowercase();
                if name.starts_with("br") || name.starts_with("hr") {
                    out.push('\n');
                }
                tag = None;
            }
            (Some(name), _) => name.push(c),
        }
    }
    let mut lines: Vec<String> = Vec::new();
    for line in out.split('\n') {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.is_empty() && lines.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        lines.push(line);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// Por que uma página não vale. Nada foi mandado para a LCU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidRunePage {
    EmptyName,
    SameStyles,
    UnknownStyle(u32),
    SubStyleNotAllowed {
        primary: u32,
        sub: u32,
    },
    WrongPerkCount {
        expected: usize,
        got: usize,
    },
    /// A runa na posição `index` (a partir de 0) não está na linha que a
    /// posição pede.
    PerkOutOfPlace {
        index: usize,
        perk_id: u32,
    },
    /// As duas runas da árvore secundária estão na mesma linha.
    SubPerksSameSlot,
}

impl std::fmt::Display for InvalidRunePage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvalidRunePage::EmptyName => f.write_str("a página precisa de um nome"),
            InvalidRunePage::SameStyles => {
                f.write_str("a árvore primária e a secundária são iguais")
            }
            InvalidRunePage::UnknownStyle(id) => write!(f, "árvore de runas {id} não existe"),
            InvalidRunePage::SubStyleNotAllowed { primary, sub } => {
                write!(f, "a árvore {sub} não pode ser secundária da {primary}")
            }
            InvalidRunePage::WrongPerkCount { expected, got } => {
                write!(f, "a página precisa de {expected} runas, veio com {got}")
            }
            InvalidRunePage::PerkOutOfPlace { index, perk_id } => {
                write!(f, "a runa {perk_id} não vale na posição {index} da página")
            }
            InvalidRunePage::SubPerksSameSlot => {
                f.write_str("as duas runas da árvore secundária estão na mesma linha")
            }
        }
    }
}

impl std::error::Error for InvalidRunePage {}

/// Confere a página com as regras do client oficial. Sem o `catalog`, só
/// confere o nome, as árvores diferentes e as [`DEFAULT_PERK_COUNT`] runas;
/// com ele, confere também cada runa contra a linha da árvore.
///
/// Com as árvores, a ordem esperada é: uma runa de cada linha não-fragmento
/// da primária (de cima para baixo), duas de linhas comuns diferentes da
/// secundária, e uma de cada linha de fragmentos da primária.
pub fn check_page(
    page: &RunePageInput,
    catalog: Option<&RuneCatalog>,
) -> Result<(), InvalidRunePage> {
    if page.name.trim().is_empty() {
        return Err(InvalidRunePage::EmptyName);
    }
    if page.primary_style_id == page.sub_style_id {
        return Err(InvalidRunePage::SameStyles);
    }
    let perks = &page.selected_perk_ids;
    let Some(catalog) = catalog else {
        if perks.len() != DEFAULT_PERK_COUNT {
            return Err(InvalidRunePage::WrongPerkCount {
                expected: DEFAULT_PERK_COUNT,
                got: perks.len(),
            });
        }
        return Ok(());
    };

    let primary = catalog
        .style(page.primary_style_id)
        .ok_or(InvalidRunePage::UnknownStyle(page.primary_style_id))?;
    let sub = catalog
        .style(page.sub_style_id)
        .ok_or(InvalidRunePage::UnknownStyle(page.sub_style_id))?;
    if !primary.allowed_sub_styles.is_empty() && !primary.allowed_sub_styles.contains(&sub.id) {
        return Err(InvalidRunePage::SubStyleNotAllowed {
            primary: primary.id,
            sub: sub.id,
        });
    }

    let primary_slots: Vec<&RuneSlot> = primary
        .slots
        .iter()
        .filter(|s| s.kind != RuneSlotKind::StatMod)
        .collect();
    let stat_slots: Vec<&RuneSlot> = primary
        .slots
        .iter()
        .filter(|s| s.kind == RuneSlotKind::StatMod)
        .collect();
    let sub_slots: Vec<&RuneSlot> = sub
        .slots
        .iter()
        .filter(|s| s.kind == RuneSlotKind::Regular)
        .collect();
    let expected = primary_slots.len() + 2 + stat_slots.len();
    if perks.len() != expected {
        return Err(InvalidRunePage::WrongPerkCount {
            expected,
            got: perks.len(),
        });
    }

    let out_of_place = |index: usize| InvalidRunePage::PerkOutOfPlace {
        index,
        perk_id: perks[index],
    };
    for (index, slot) in primary_slots.iter().enumerate() {
        if !slot.perks.contains(&perks[index]) {
            return Err(out_of_place(index));
        }
    }
    let sub_start = primary_slots.len();
    let mut sub_rows = [0usize; 2];
    for (k, row) in sub_rows.iter_mut().enumerate() {
        let index = sub_start + k;
        *row = sub_slots
            .iter()
            .position(|s| s.perks.contains(&perks[index]))
            .ok_or_else(|| out_of_place(index))?;
    }
    if sub_rows[0] == sub_rows[1] {
        return Err(InvalidRunePage::SubPerksSameSlot);
    }
    let stat_start = sub_start + 2;
    for (k, slot) in stat_slots.iter().enumerate() {
        let index = stat_start + k;
        if !slot.perks.contains(&perks[index]) {
            return Err(out_of_place(index));
        }
    }
    Ok(())
}

/// Erro dos comandos que mandam uma página.
#[derive(Debug, thiserror::Error)]
pub enum RunePageError {
    /// A página não vale; nada foi mandado para a LCU.
    #[error("{0}")]
    Invalid(InvalidRunePage),
    /// A LCU recusou (sem espaço para página nova, página não editável...)
    /// ou não respondeu.
    #[error(transparent)]
    Lcu(#[from] LcuError),
}

/// Páginas, página em uso e espaço livre. Lê os três juntos; só a lista é
/// obrigatória.
pub async fn current_pages(lcu: &LcuClient) -> Result<RunePages, LcuError> {
    let (pages, current, inventory) = tokio::join!(
        lcu.get::<Value>(PAGES_PATH),
        lcu.get::<Value>(CURRENT_PAGE_PATH),
        lcu.get::<Value>(INVENTORY_PATH),
    );
    let mut runes = RunePages::default();
    runes.apply_pages(&pages?);
    if let Ok(current) = current {
        runes.apply_current_page(&current);
    }
    if let Ok(inventory) = inventory {
        runes.apply_inventory(&inventory);
    }
    Ok(runes)
}

/// Árvores e nomes das runas, no idioma do client.
pub async fn catalog(lcu: &LcuClient) -> Result<Option<RuneCatalog>, LcuError> {
    let (styles, perks) = tokio::join!(
        lcu.get::<Value>(PERK_STYLES_PATH),
        lcu.get::<Value>(PERKS_PATH),
    );
    // Sem nomes o editor ainda funciona; sem as árvores, não.
    let perks = perks.unwrap_or(Value::Null);
    Ok(RuneCatalog::from_json(&styles?, &perks))
}

/// Aplica a página `id` (`PUT /lol-perks/v1/currentpage`).
pub async fn select_page(lcu: &LcuClient, id: u64) -> Result<(), LcuError> {
    lcu.put::<_, Option<Value>>(CURRENT_PAGE_PATH, &id)
        .await
        .map(|_| ())
}

/// Cria uma página e já a aplica (`POST /lol-perks/v1/pages`), depois de
/// conferir com [`check_page`].
pub async fn create_page(
    lcu: &LcuClient,
    page: &RunePageInput,
    catalog: Option<&RuneCatalog>,
) -> Result<(), RunePageError> {
    check_page(page, catalog).map_err(RunePageError::Invalid)?;
    let body = json!({
        "name": page.name.trim(),
        "primaryStyleId": page.primary_style_id,
        "subStyleId": page.sub_style_id,
        "selectedPerkIds": page.selected_perk_ids,
        "current": true,
    });
    lcu.post::<_, Option<Value>>(PAGES_PATH, &body).await?;
    Ok(())
}

/// Troca nome, árvores e runas da página `id` (`PUT
/// /lol-perks/v1/pages/<id>`), depois de conferir com [`check_page`]. Lê a
/// página antes e manda de volta com os campos novos, para não perder o
/// resto (como se ela está em uso).
pub async fn update_page(
    lcu: &LcuClient,
    id: u64,
    page: &RunePageInput,
    catalog: Option<&RuneCatalog>,
) -> Result<(), RunePageError> {
    check_page(page, catalog).map_err(RunePageError::Invalid)?;
    let path = page_path(id);
    let mut body: Value = lcu.get(&path).await?;
    if let Some(fields) = body.as_object_mut() {
        fields.insert("name".into(), json!(page.name.trim()));
        fields.insert("primaryStyleId".into(), json!(page.primary_style_id));
        fields.insert("subStyleId".into(), json!(page.sub_style_id));
        fields.insert("selectedPerkIds".into(), json!(page.selected_perk_ids));
    }
    lcu.put::<_, Option<Value>>(&path, &body).await?;
    Ok(())
}

/// Apaga a página `id` (`DELETE /lol-perks/v1/pages/<id>`).
pub async fn delete_page(lcu: &LcuClient, id: u64) -> Result<(), LcuError> {
    lcu.send::<Option<Value>>(Method::DELETE, &page_path(id), None)
        .await
        .map(|_| ())
}

/// `/lol-perks/v1/pages/<id>`.
pub fn page_path(id: u64) -> String {
    format!("{PAGES_PATH}/{id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recorded(path: &str) -> Vec<Value> {
        let file =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/16.22/rest.json");
        let rest: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        rest.into_iter()
            .filter(|r| r["path"] == path && r["status"] == 200)
            .map(|r| r["body"].clone())
            .collect()
    }

    /// Duas árvores enxutas, no formato do `perkstyles.json`.
    fn catalog() -> RuneCatalog {
        let stat_slots = json!([
            {"type": "kStatMod", "slotLabel": "Ofensiva", "perks": [5008, 5005, 5007]},
            {"type": "kStatMod", "slotLabel": "Flex", "perks": [5008, 5010, 5001]},
            {"type": "kStatMod", "slotLabel": "Defesa", "perks": [5011, 5013, 5001]},
        ]);
        let style = |id: u32, base: u32, subs: Value| {
            let mut slots = vec![
                json!({"type": "kKeyStone", "slotLabel": "", "perks": [base + 1, base + 2]}),
                json!({"type": "kMixedRegularSplashable", "slotLabel": "", "perks": [base + 11, base + 12]}),
                json!({"type": "kMixedRegularSplashable", "slotLabel": "", "perks": [base + 21, base + 22]}),
                json!({"type": "kMixedRegularSplashable", "slotLabel": "", "perks": [base + 31, base + 32]}),
            ];
            slots.extend(stat_slots.as_array().unwrap().iter().cloned());
            json!({"id": id, "name": format!("Árvore {id}"), "allowedSubStyles": subs, "slots": slots})
        };
        let styles = json!({"schemaVersion": 2, "styles": [
            style(8000, 100, json!([8100])),
            style(8100, 200, json!([8000])),
        ]});
        let perks = json!([
            {"id": 101, "name": "Pressione o Ataque", "shortDesc": "Três <b>ataques</b> seguidos<br>causam dano <lol-uikit-tooltipped-keyword key='x'>adaptativo</lol-uikit-tooltipped-keyword>.",
             "longDesc": "<br>Causa <truedamage>dano</truedamage>   adicional.<br><br><br>Tempo de Recarga: <scaleLevel>6s</scaleLevel><hr><rules>Só em Campeões.</rules><br>"},
            {"id": 9999, "name": "Runa antiga", "shortDesc": ""},
        ]);
        RuneCatalog::from_json(&styles, &perks).unwrap()
    }

    fn input(primary: u32, sub: u32, perks: &[u32]) -> RunePageInput {
        RunePageInput {
            name: "Teste".into(),
            primary_style_id: primary,
            sub_style_id: sub,
            selected_perk_ids: perks.to_vec(),
        }
    }

    const VALID: [u32; 9] = [101, 111, 121, 131, 211, 231, 5008, 5010, 5011];

    #[test]
    fn le_as_paginas_gravadas() {
        let body = recorded(PAGES_PATH).remove(0);
        let runes = RunePages::from_json(&body).unwrap();
        assert_eq!(runes.pages.len(), 3);
        assert_eq!(runes.current_page_id, Some(994_528_921));
        let page = runes.page(994_528_921).unwrap();
        assert_eq!(page.name, "Nova Página de Runas 1");
        assert_eq!(page.primary_style_id, 8000);
        assert_eq!(page.sub_style_id, 8300);
        assert_eq!(
            page.selected_perk_ids,
            [8005, 9111, 9104, 8017, 8313, 8321, 5005, 5008, 5011]
        );
        assert!(page.is_editable && page.is_deletable && page.is_valid);
        assert!(!page.is_temporary);
        // Página incompleta: a linha vazia vem como 0.
        assert_eq!(runes.page(509_916_045).unwrap().selected_perk_ids[5], 0);

        // Na partida, a página recomendada (temporária) é a em uso.
        let in_game = recorded(PAGES_PATH).pop().unwrap();
        let runes = RunePages::from_json(&in_game).unwrap();
        assert_eq!(runes.pages.len(), 4);
        assert_eq!(runes.current_page_id, Some(395_560_545));
        assert!(runes.page(395_560_545).unwrap().is_temporary);
    }

    #[test]
    fn eventos_mudam_partes_das_paginas() {
        let mut runes = RunePages::from_json(&recorded(PAGES_PATH).remove(0)).unwrap();
        runes.apply_current_page(&json!({"id": 156_827_123u64}));
        assert_eq!(runes.current_page_id, Some(156_827_123));
        // Lista sem nenhuma `current` não apaga a página em uso.
        runes.apply_pages(&json!([{"id": 1, "name": "x", "current": false}]));
        assert_eq!(runes.pages.len(), 1);
        assert_eq!(runes.current_page_id, Some(156_827_123));
        runes.apply_inventory(&json!({"canAddCustomPage": false, "ownedPageCount": 3}));
        assert!(!runes.can_create_page);
        // Lixo não muda nada.
        let before = runes.clone();
        runes.apply_pages(&json!({"nao": "lista"}));
        runes.apply_current_page(&json!(null));
        runes.apply_inventory(&json!({}));
        assert_eq!(runes, before);
    }

    #[test]
    fn monta_o_catalogo() {
        let catalog = catalog();
        assert_eq!(catalog.styles.len(), 2);
        let style = catalog.style(8000).unwrap();
        assert_eq!(style.allowed_sub_styles, [8100]);
        assert_eq!(style.slots.len(), 7);
        assert_eq!(style.slots[0].kind, RuneSlotKind::Keystone);
        assert_eq!(style.slots[1].kind, RuneSlotKind::Regular);
        assert_eq!(style.slots[6].kind, RuneSlotKind::StatMod);
        assert_eq!(style.slots[4].label, "Ofensiva");
        // Só as runas das árvores, sem repetir os fragmentos.
        assert!(catalog.perks.iter().all(|p| p.id != 9999));
        assert_eq!(catalog.perks.iter().filter(|p| p.id == 5008).count(), 1);
        let pta = catalog.perks.iter().find(|p| p.id == 101).unwrap();
        assert_eq!(pta.name, "Pressione o Ataque");
        assert_eq!(
            pta.short_desc,
            "Três ataques seguidos causam dano adaptativo."
        );
        assert_eq!(
            pta.long_desc,
            "Causa dano adicional.\n\nTempo de Recarga: 6s\nSó em Campeões."
        );
        // Sem `longDesc` no perks.json: fica vazia.
        assert_eq!(
            catalog
                .perks
                .iter()
                .find(|p| p.id == 102)
                .unwrap()
                .long_desc,
            ""
        );
        // Sem nome no perks.json: fica vazio.
        assert_eq!(catalog.perks.iter().find(|p| p.id == 102).unwrap().name, "");
        assert_eq!(RuneCatalog::from_json(&json!({}), &json!([])), None);
        assert_eq!(
            RuneCatalog::from_json(&json!({"styles": []}), &json!([])),
            None
        );
    }

    #[test]
    fn confere_a_pagina_com_as_arvores() {
        let catalog = catalog();
        let check = |page: &RunePageInput| check_page(page, Some(&catalog));
        assert_eq!(check(&input(8000, 8100, &VALID)), Ok(()));
        // As duas da secundária em qualquer ordem de linha.
        let mut perks = VALID;
        perks.swap(4, 5);
        assert_eq!(check(&input(8000, 8100, &perks)), Ok(()));

        let mut sem_nome = input(8000, 8100, &VALID);
        sem_nome.name = "   ".into();
        assert_eq!(check(&sem_nome), Err(InvalidRunePage::EmptyName));
        assert_eq!(
            check(&input(8000, 8000, &VALID)),
            Err(InvalidRunePage::SameStyles)
        );
        assert_eq!(
            check(&input(8000, 8300, &VALID)),
            Err(InvalidRunePage::UnknownStyle(8300))
        );
        assert_eq!(
            check(&input(8000, 8100, &VALID[..8])),
            Err(InvalidRunePage::WrongPerkCount {
                expected: 9,
                got: 8
            })
        );
        // Runa principal na linha comum.
        let mut perks = VALID;
        perks.swap(0, 1);
        assert_eq!(
            check(&input(8000, 8100, &perks)),
            Err(InvalidRunePage::PerkOutOfPlace {
                index: 0,
                perk_id: 111
            })
        );
        // Runa principal da secundária não vale.
        let mut perks = VALID;
        perks[4] = 201;
        assert_eq!(
            check(&input(8000, 8100, &perks)),
            Err(InvalidRunePage::PerkOutOfPlace {
                index: 4,
                perk_id: 201
            })
        );
        let mut perks = VALID;
        perks[5] = 212;
        assert_eq!(
            check(&input(8000, 8100, &perks)),
            Err(InvalidRunePage::SubPerksSameSlot)
        );
        // Fragmento de defesa na linha de ataque.
        let mut perks = VALID;
        perks[6] = 5011;
        assert_eq!(
            check(&input(8000, 8100, &perks)),
            Err(InvalidRunePage::PerkOutOfPlace {
                index: 6,
                perk_id: 5011
            })
        );
    }

    #[test]
    fn sem_as_arvores_confere_o_basico() {
        assert_eq!(check_page(&input(8000, 8100, &[1; 9]), None), Ok(()));
        assert_eq!(
            check_page(&input(8000, 8100, &[1; 3]), None),
            Err(InvalidRunePage::WrongPerkCount {
                expected: 9,
                got: 3
            })
        );
        assert_eq!(
            check_page(&input(8200, 8200, &[1; 9]), None),
            Err(InvalidRunePage::SameStyles)
        );
    }

    #[test]
    fn formato_para_a_ui() {
        let runes = RunePages {
            pages: vec![RunePage {
                id: 7,
                name: "Minha".into(),
                primary_style_id: 8000,
                sub_style_id: 8100,
                selected_perk_ids: VALID.to_vec(),
                is_editable: true,
                is_deletable: true,
                is_temporary: false,
                is_valid: true,
            }],
            current_page_id: Some(7),
            can_create_page: false,
        };
        assert_eq!(
            serde_json::to_value(&runes).unwrap(),
            json!({
                "pages": [{
                    "id": 7,
                    "name": "Minha",
                    "primaryStyleId": 8000,
                    "subStyleId": 8100,
                    "selectedPerkIds": VALID,
                    "isEditable": true,
                    "isDeletable": true,
                    "isTemporary": false,
                    "isValid": true,
                }],
                "currentPageId": 7,
                "canCreatePage": false,
            })
        );
        assert_eq!(
            serde_json::to_value(RuneSlotKind::StatMod).unwrap(),
            json!("statMod")
        );
    }
}
