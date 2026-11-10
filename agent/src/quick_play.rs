//! Posições e campeões do Jogo Dinâmico (Swiftplay, fila 480), #218.
//!
//! No Jogo Dinâmico não há seleção de campeões depois da fila: o jogador
//! escolhe ainda no lobby duas posições e um campeão para cada uma, com
//! feitiços e runas. A LCU chama cada par de "slot":
//!
//! - o lobby diz se a fila pede os slots em
//!   `gameConfig.showQuickPlaySlotSelection`, e quais posições faltam na
//!   fila (as prioritárias) em `scarcePositions`;
//! - cada membro traz os seus em `playerSlots`: `championId`, `skinId`,
//!   `positionPreference` (`TOP`, `JUNGLE`...), `spell1`, `spell2` e
//!   `perks`, um texto com o JSON da página de runas
//!   (`{"perkIds":[...],"perkStyle":8000,"perkSubStyle":8100}`);
//! - o jogador troca os dele com
//!   `PUT /lol-lobby/v1/lobby/members/localMember/player-slots`, mandando a
//!   lista inteira no mesmo formato. A LCU responde sem corpo e manda o
//!   lobby novo pelo WAMP.
//!
//! Quando o jogador não escolhe feitiços ou runas, o agente usa os que a
//! LCU recomenda para o campeão na posição
//! (`/lol-perks/v1/recommended-pages/champion/{id}/position/{posição}/map/11`),
//! como o client oficial faz.
//!
//! Os campeões que o jogador pode pôr num slot (os dele e os da rotação
//! gratuita) saem de `/lol-champions/v1/owned-champions-minimal`.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::lobby::Position;
use crate::rest::{LcuClient, LcuError};
use crate::runes::RunePages;

/// Endpoint dos slots do jogador logado.
pub const PLAYER_SLOTS_PATH: &str = "/lol-lobby/v1/lobby/members/localMember/player-slots";

/// Campeões do jogador, com os da rotação gratuita.
pub const OWNED_CHAMPIONS_PATH: &str = "/lol-champions/v1/owned-champions-minimal";

/// Quantos slots o Jogo Dinâmico pede.
pub const SLOT_COUNT: usize = 2;

/// Posições que valem num slot: sem preenchimento.
pub const SLOT_POSITIONS: [Position; 5] = [
    Position::Top,
    Position::Jungle,
    Position::Middle,
    Position::Bottom,
    Position::Utility,
];

/// Mapa do Summoner's Rift, para as runas recomendadas.
const SUMMONERS_RIFT: u32 = 11;

/// Um slot (posição e campeão) de um jogador no lobby.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSlot {
    /// `0` enquanto o jogador não escolheu.
    pub champion_id: u32,
    pub skin_id: u32,
    pub position: Position,
    /// Feitiço da tecla D, `0` sem feitiço.
    pub spell1_id: u32,
    /// Feitiço da tecla F, `0` sem feitiço.
    pub spell2_id: u32,
    /// Árvore primária das runas, `0` sem runas.
    pub rune_style_id: u32,
    /// Árvore secundária das runas, `0` sem runas.
    pub rune_sub_style_id: u32,
    /// Runa principal (a primeira da página), `0` sem runas.
    pub keystone_id: u32,
    /// Todas as runas do slot, na ordem da página (principal, 3 da
    /// primária, 2 da secundária e 3 fragmentos), para editar (#331).
    /// Vazia sem runas.
    pub perk_ids: Vec<u32>,
    /// `perks` como a LCU mandou, para devolver igual quando o slot não muda.
    #[serde(skip)]
    #[ts(skip)]
    pub perks: String,
}

/// O que a UI manda para cada slot (comando `setPlayerSlots`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSlotInput {
    pub champion_id: u32,
    pub position: Position,
    /// Feitiços escolhidos, os dois juntos. `null` nos dois mantém os do
    /// slot quando o campeão e a posição não mudam, e senão usa os
    /// recomendados.
    pub spell1_id: Option<u32>,
    pub spell2_id: Option<u32>,
    /// Página de runas do jogador (`runePages`) para o slot. `null` mantém as
    /// runas do slot quando o campeão e a posição não mudam, e senão usa as
    /// recomendadas.
    #[ts(type = "number | null")]
    pub rune_page_id: Option<u64>,
    /// Runas editadas na hora para o slot (#331), sem página salva. Vale
    /// mais que `runePageId`. `null` segue as regras de `runePageId`.
    #[serde(default)]
    pub runes: Option<SlotRunesInput>,
    /// Skin escolhida para o slot (#403). Precisa ser do campeão do slot
    /// (`skinId / 1000 == championId`). `null` mantém a skin do slot quando
    /// o campeão não muda, e senão usa a padrão.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub skin_id: Option<u32>,
}

/// Runas de um slot escolhidas no editor (#331), na ordem de
/// [`PlayerSlot::perk_ids`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SlotRunesInput {
    pub perk_ids: Vec<u32>,
    pub style_id: u32,
    pub sub_style_id: u32,
}

/// Quantas runas uma página completa tem: principal, 3 da primária, 2 da
/// secundária e 3 fragmentos.
pub const SLOT_PERK_COUNT: usize = 9;

/// Por que uma lista de slots não vale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidSlots {
    /// Não são [`SLOT_COUNT`] slots.
    Count,
    /// Uma posição fora de [`SLOT_POSITIONS`].
    Position,
    /// A mesma posição nos dois slots.
    RepeatedPosition,
    /// Slot sem campeão.
    NoChampion,
    /// O mesmo campeão nos dois slots.
    RepeatedChampion,
    /// Só um dos feitiços, um feitiço `0` ou os dois iguais.
    Spells,
    /// Runas editadas incompletas: sem as 9 runas, sem árvore ou com as duas
    /// árvores iguais.
    Runes,
    /// Skin de outro campeão (#403).
    Skin,
}

impl std::fmt::Display for InvalidSlots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            InvalidSlots::Count => "o Jogo Dinâmico pede duas posições",
            InvalidSlots::Position => "posição que não vale no Jogo Dinâmico",
            InvalidSlots::RepeatedPosition => "as duas posições são iguais",
            InvalidSlots::NoChampion => "escolha um campeão para cada posição",
            InvalidSlots::RepeatedChampion => "os dois campeões são iguais",
            InvalidSlots::Spells => "escolha dois feitiços diferentes",
            InvalidSlots::Runes => "escolha uma runa em cada linha das duas árvores",
            InvalidSlots::Skin => "essa skin não é do campeão escolhido",
        })
    }
}

impl std::error::Error for InvalidSlots {}

/// Confere os slots com as regras do client oficial.
pub fn check_slots(slots: &[PlayerSlotInput]) -> Result<(), InvalidSlots> {
    let [a, b] = slots else {
        return Err(InvalidSlots::Count);
    };
    for slot in slots {
        if !SLOT_POSITIONS.contains(&slot.position) {
            return Err(InvalidSlots::Position);
        }
        if slot.champion_id == 0 {
            return Err(InvalidSlots::NoChampion);
        }
        match (slot.spell1_id, slot.spell2_id) {
            (None, None) => {}
            (Some(d), Some(f)) if d != 0 && f != 0 && d != f => {}
            _ => return Err(InvalidSlots::Spells),
        }
        if slot
            .skin_id
            .is_some_and(|skin| !skin_of(skin, slot.champion_id))
        {
            return Err(InvalidSlots::Skin);
        }
        if let Some(runes) = &slot.runes {
            if runes.perk_ids.len() != SLOT_PERK_COUNT
                || runes.perk_ids.contains(&0)
                || runes.style_id == 0
                || runes.sub_style_id == 0
                || runes.style_id == runes.sub_style_id
            {
                return Err(InvalidSlots::Runes);
            }
        }
    }
    if a.position == b.position {
        return Err(InvalidSlots::RepeatedPosition);
    }
    if a.champion_id == b.champion_id {
        return Err(InvalidSlots::RepeatedChampion);
    }
    Ok(())
}

/// Se `skin_id` é uma skin de `champion_id`: a LCU numera as skins de cada
/// campeão a partir de `championId * 1000` (a padrão).
pub const fn skin_of(skin_id: u32, champion_id: u32) -> bool {
    skin_id / 1000 == champion_id
}

/// Uma página de runas no formato dos slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotRunes {
    pub perk_ids: Vec<u32>,
    pub style_id: u32,
    pub sub_style_id: u32,
}

impl SlotRunes {
    /// O texto de `perks` que a LCU espera.
    pub fn to_perks(&self) -> String {
        json!({
            "perkIds": self.perk_ids,
            "perkStyle": self.style_id,
            "perkSubStyle": self.sub_style_id,
        })
        .to_string()
    }

    /// Lê o texto de `perks` de um slot. `None` vazio ou fora do formato.
    pub fn from_perks(perks: &str) -> Option<SlotRunes> {
        let value: Value = serde_json::from_str(perks).ok()?;
        let perk_ids = ids(&value["perkIds"]);
        let style_id = as_u32(&value["perkStyle"]);
        (!perk_ids.is_empty() || style_id != 0).then(|| SlotRunes {
            perk_ids,
            style_id,
            sub_style_id: as_u32(&value["perkSubStyle"]),
        })
    }
}

/// Runas e feitiços que a LCU recomenda para um campeão numa posição.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Recommendation {
    pub runes: Option<SlotRunes>,
    pub spells: Option<(u32, u32)>,
}

impl Recommendation {
    /// Lê a primeira página da resposta de [`recommended_pages_path`].
    pub fn from_json(pages: &Value) -> Option<Recommendation> {
        let page = pages.as_array()?.first()?;
        let perk_ids: Vec<u32> = page["perks"]
            .as_array()
            .map(|perks| {
                perks
                    .iter()
                    .map(|p| as_u32(&p["id"]))
                    .filter(|&id| id > 0)
                    .collect()
            })
            .unwrap_or_default();
        let style_id = as_u32(&page["primaryPerkStyleId"]);
        let runes = (!perk_ids.is_empty() && style_id != 0).then(|| SlotRunes {
            perk_ids,
            style_id,
            sub_style_id: as_u32(&page["secondaryPerkStyleId"]),
        });
        let spells = match ids(&page["summonerSpellIds"])[..] {
            [d, f, ..] if d != f => Some((d, f)),
            _ => None,
        };
        Some(Recommendation { runes, spells })
    }
}

/// Endpoint das runas recomendadas. A LCU usa as posições em minúsculas,
/// como em `assignedPosition` da seleção.
pub fn recommended_pages_path(champion_id: u32, position: Position) -> String {
    format!(
        "/lol-perks/v1/recommended-pages/champion/{champion_id}/position/{}/map/{SUMMONERS_RIFT}",
        position.lcu_name().to_lowercase()
    )
}

/// Feitiços de reserva quando a LCU não recomenda nenhum: Flash e o
/// feitiço mais comum da posição.
pub const fn default_spells(position: Position) -> (u32, u32) {
    const FLASH: u32 = 4;
    match position {
        Position::Jungle => (FLASH, 11), // Golpear
        Position::Top => (FLASH, 12),    // Teleporte
        Position::Bottom => (FLASH, 7),  // Curar
        _ => (FLASH, 14),                // Incendiar
    }
}

impl PlayerSlot {
    /// Lê um item de `playerSlots`. `None` se não for um objeto.
    pub fn from_json(value: &Value) -> Option<PlayerSlot> {
        value.as_object()?;
        let perks = value["perks"].as_str().unwrap_or_default().to_owned();
        let runes = SlotRunes::from_perks(&perks);
        Some(PlayerSlot {
            champion_id: as_u32(&value["championId"]),
            skin_id: as_u32(&value["skinId"]),
            position: Position::from_lcu(value["positionPreference"].as_str()),
            spell1_id: as_u32(&value["spell1"]),
            spell2_id: as_u32(&value["spell2"]),
            rune_style_id: runes.as_ref().map_or(0, |r| r.style_id),
            rune_sub_style_id: runes.as_ref().map_or(0, |r| r.sub_style_id),
            keystone_id: runes
                .as_ref()
                .and_then(|r| r.perk_ids.first().copied())
                .unwrap_or(0),
            perk_ids: runes.map(|r| r.perk_ids).unwrap_or_default(),
            perks,
        })
    }

    /// Lê a lista `playerSlots` de um membro.
    pub fn list_from_json(value: &Value) -> Vec<PlayerSlot> {
        value
            .as_array()
            .map(|slots| slots.iter().filter_map(PlayerSlot::from_json).collect())
            .unwrap_or_default()
    }
}

/// Se o slot novo precisa das recomendações da LCU: campeão ou posição
/// mudou e falta escolher feitiços ou runas.
pub fn needs_recommendation(input: &PlayerSlotInput, current: Option<&PlayerSlot>) -> bool {
    let kept =
        current.is_some_and(|c| c.champion_id == input.champion_id && c.position == input.position);
    !kept && (input.spell1_id.is_none() || (input.rune_page_id.is_none() && input.runes.is_none()))
}

/// Monta o slot no formato da LCU.
///
/// - Feitiços: os escolhidos; senão os do slot atual, se o campeão e a
///   posição não mudaram; senão os recomendados; senão
///   [`default_spells`].
/// - Runas: as editadas (#331); senão as da página escolhida; senão as do
///   slot atual, se o campeão e
///   a posição não mudaram; senão as recomendadas; senão a página em uso.
/// - Skin: a escolhida (#403); senão a do slot atual com o mesmo campeão;
///   senão a padrão.
pub fn build_slot(
    input: &PlayerSlotInput,
    current: Option<&PlayerSlot>,
    pages: Option<&RunePages>,
    recommended: Option<&Recommendation>,
) -> Value {
    let kept =
        current.filter(|c| c.champion_id == input.champion_id && c.position == input.position);
    let spells = match (input.spell1_id, input.spell2_id) {
        (Some(d), Some(f)) => (d, f),
        _ => kept
            .filter(|c| c.spell1_id != 0 && c.spell2_id != 0)
            .map(|c| (c.spell1_id, c.spell2_id))
            .or_else(|| recommended.and_then(|r| r.spells))
            .unwrap_or_else(|| default_spells(input.position)),
    };
    let page_runes = |id: u64| {
        pages.and_then(|p| p.page(id)).map(|page| SlotRunes {
            perk_ids: page.selected_perk_ids.clone(),
            style_id: page.primary_style_id,
            sub_style_id: page.sub_style_id,
        })
    };
    let edited = input.runes.as_ref().map(|r| SlotRunes {
        perk_ids: r.perk_ids.clone(),
        style_id: r.style_id,
        sub_style_id: r.sub_style_id,
    });
    let perks = edited
        .or_else(|| input.rune_page_id.and_then(page_runes))
        .map(|r| r.to_perks())
        .or_else(|| {
            kept.filter(|c| !c.perks.is_empty())
                .map(|c| c.perks.clone())
        })
        .or_else(|| {
            recommended
                .and_then(|r| r.runes.as_ref())
                .map(SlotRunes::to_perks)
        })
        .or_else(|| {
            pages
                .and_then(|p| p.current_page_id)
                .and_then(page_runes)
                .map(|r| r.to_perks())
        })
        .unwrap_or_default();
    let skin_id = input
        .skin_id
        .filter(|&skin| skin_of(skin, input.champion_id))
        .or_else(|| {
            current
                .filter(|c| c.champion_id == input.champion_id && c.skin_id != 0)
                .map(|c| c.skin_id)
        })
        .unwrap_or(input.champion_id * 1000);
    json!({
        "championId": input.champion_id,
        "positionPreference": input.position.lcu_name(),
        "skinId": skin_id,
        "spell1": spells.0,
        "spell2": spells.1,
        "perks": perks,
    })
}

/// Busca as runas e os feitiços recomendados. Qualquer erro vira `None`:
/// o slot sai com as reservas de [`build_slot`].
pub async fn recommendation(
    lcu: &LcuClient,
    champion_id: u32,
    position: Position,
) -> Option<Recommendation> {
    let body: Value = lcu
        .get(&recommended_pages_path(champion_id, position))
        .await
        .ok()?;
    Recommendation::from_json(&body)
}

/// O slot atual que corresponde a `input`: o que tem o mesmo campeão (o
/// slot pode ter mudado de lugar ao inverter, #405), senão o do mesmo
/// índice. Assim feitiços, runas e skin acompanham o campeão.
pub fn current_slot<'a>(
    current: &'a [PlayerSlot],
    index: usize,
    input: &PlayerSlotInput,
) -> Option<&'a PlayerSlot> {
    current
        .iter()
        .find(|c| c.champion_id == input.champion_id)
        .or_else(|| current.get(index))
}

/// Troca os slots do jogador logado, depois de conferir com
/// [`check_slots`]. `current` são os slots que ele tem no lobby, e `pages`
/// as páginas de runas, para `runePageId`. A LCU responde sem corpo; o
/// lobby novo chega pelo WAMP.
pub async fn set_player_slots(
    lcu: &LcuClient,
    slots: &[PlayerSlotInput],
    current: &[PlayerSlot],
    pages: Option<&RunePages>,
) -> Result<(), SetSlotsError> {
    check_slots(slots).map_err(SetSlotsError::Invalid)?;
    let mut body = Vec::with_capacity(slots.len());
    for (index, input) in slots.iter().enumerate() {
        let current = current_slot(current, index, input);
        let recommended = if needs_recommendation(input, current) {
            recommendation(lcu, input.champion_id, input.position).await
        } else {
            None
        };
        body.push(build_slot(input, current, pages, recommended.as_ref()));
    }
    lcu.put::<_, Option<Value>>(PLAYER_SLOTS_PATH, &body)
        .await
        .map_err(SetSlotsError::Lcu)?;
    Ok(())
}

/// Erro de [`set_player_slots`].
#[derive(Debug, thiserror::Error)]
pub enum SetSlotsError {
    /// Os slots não valem; nada foi mandado para a LCU.
    #[error("{0}")]
    Invalid(InvalidSlots),
    /// A LCU recusou (fora de lobby, fila sem slots...) ou não respondeu.
    #[error(transparent)]
    Lcu(#[from] LcuError),
}

/// Ids dos campeões que o jogador pode pôr num slot: os dele, os
/// alugados e os da rotação gratuita, em ordem. `None` se a resposta não
/// for uma lista.
pub fn playable_from_json(champions: &Value) -> Option<Vec<u32>> {
    let mut ids: Vec<u32> = champions
        .as_array()?
        .iter()
        .filter(|c| {
            c["ownership"]["owned"] == true
                || c["ownership"]["rental"]["rented"] == true
                || c["freeToPlay"] == true
        })
        .map(|c| as_u32(&c["id"]))
        .filter(|&id| id > 0)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    Some(ids)
}

/// Lê os campeões que o jogador pode pôr num slot.
pub async fn playable_champions(lcu: &LcuClient) -> Result<Option<Vec<u32>>, LcuError> {
    let body: Value = lcu.get(OWNED_CHAMPIONS_PATH).await?;
    Ok(playable_from_json(&body))
}

fn as_u32(value: &Value) -> u32 {
    value
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .unwrap_or(0)
}

fn ids(value: &Value) -> Vec<u32> {
    value
        .as_array()
        .map(|items| items.iter().map(as_u32).filter(|&id| id > 0).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runes::RunePage;

    fn input(champion_id: u32, position: Position) -> PlayerSlotInput {
        PlayerSlotInput {
            champion_id,
            position,
            spell1_id: None,
            spell2_id: None,
            rune_page_id: None,
            runes: None,
            skin_id: None,
        }
    }

    #[test]
    fn runas_editadas_valem_mais_que_a_pagina_e_precisam_estar_completas() {
        let runes = SlotRunesInput {
            perk_ids: vec![8010, 9111, 9104, 8299, 8444, 8451, 5008, 5008, 5011],
            style_id: 8000,
            sub_style_id: 8400,
        };
        let edited = PlayerSlotInput {
            runes: Some(runes.clone()),
            rune_page_id: Some(1),
            ..input(1, Position::Top)
        };
        let slot = build_slot(&edited, None, None, None);
        let perks = SlotRunes::from_perks(slot["perks"].as_str().unwrap()).unwrap();
        assert_eq!(perks.perk_ids, runes.perk_ids);
        assert_eq!((perks.style_id, perks.sub_style_id), (8000, 8400));
        // Com as runas editadas, só os feitiços pedem a recomendação.
        assert!(needs_recommendation(&edited, None));
        let with_spells = PlayerSlotInput {
            spell1_id: Some(4),
            spell2_id: Some(14),
            ..edited.clone()
        };
        assert!(!needs_recommendation(&with_spells, None));

        let other = input(2, Position::Jungle);
        assert_eq!(check_slots(&[edited.clone(), other.clone()]), Ok(()));
        let incomplete = PlayerSlotInput {
            runes: Some(SlotRunesInput {
                perk_ids: vec![8010],
                ..runes.clone()
            }),
            ..edited.clone()
        };
        assert_eq!(
            check_slots(&[incomplete, other.clone()]),
            Err(InvalidSlots::Runes)
        );
        let same_tree = PlayerSlotInput {
            runes: Some(SlotRunesInput {
                sub_style_id: 8000,
                ..runes
            }),
            ..edited
        };
        assert_eq!(check_slots(&[same_tree, other]), Err(InvalidSlots::Runes));
    }

    fn slot_json(champion_id: u32, position: &str, perks: &str) -> Value {
        json!({"championId": champion_id, "skinId": champion_id * 1000 + 3, "positionPreference": position,
               "spell1": 4, "spell2": 14, "perks": perks})
    }

    #[test]
    fn regras_dos_slots() {
        use Position::*;
        assert_eq!(check_slots(&[input(22, Top), input(266, Jungle)]), Ok(()));
        assert_eq!(check_slots(&[input(22, Top)]), Err(InvalidSlots::Count));
        assert_eq!(
            check_slots(&[input(22, Fill), input(266, Top)]),
            Err(InvalidSlots::Position)
        );
        assert_eq!(
            check_slots(&[input(22, Top), input(266, Top)]),
            Err(InvalidSlots::RepeatedPosition)
        );
        assert_eq!(
            check_slots(&[input(0, Top), input(266, Middle)]),
            Err(InvalidSlots::NoChampion)
        );
        assert_eq!(
            check_slots(&[input(22, Top), input(22, Middle)]),
            Err(InvalidSlots::RepeatedChampion)
        );
        let mut half = input(22, Top);
        half.spell1_id = Some(4);
        assert_eq!(
            check_slots(&[half.clone(), input(266, Middle)]),
            Err(InvalidSlots::Spells)
        );
        half.spell2_id = Some(4);
        assert_eq!(
            check_slots(&[half.clone(), input(266, Middle)]),
            Err(InvalidSlots::Spells)
        );
        half.spell2_id = Some(14);
        assert_eq!(check_slots(&[half, input(266, Middle)]), Ok(()));
    }

    #[test]
    fn le_os_slots_do_lobby() {
        let perks = r#"{"perkIds":[8010,9111,9104,8299,8345,8347,5005,5008,5001],"perkStyle":8000,"perkSubStyle":8300}"#;
        let slots = PlayerSlot::list_from_json(&json!([
            slot_json(22, "BOTTOM", perks),
            {"championId": 0, "positionPreference": "UNSELECTED"},
            "lixo",
        ]));
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].champion_id, 22);
        assert_eq!(slots[0].skin_id, 22003);
        assert_eq!(slots[0].position, Position::Bottom);
        assert_eq!((slots[0].spell1_id, slots[0].spell2_id), (4, 14));
        assert_eq!(slots[0].rune_style_id, 8000);
        assert_eq!(slots[0].rune_sub_style_id, 8300);
        assert_eq!(slots[0].keystone_id, 8010);
        assert_eq!(slots[1].champion_id, 0);
        assert_eq!(slots[1].keystone_id, 0);
        assert!(slots[1].perk_ids.is_empty());
        assert!(PlayerSlot::list_from_json(&json!(null)).is_empty());

        // `perks` não vai para a UI.
        let ui = serde_json::to_value(&slots[0]).unwrap();
        assert_eq!(
            ui,
            json!({"championId": 22, "skinId": 22003, "position": "bottom", "spell1Id": 4, "spell2Id": 14,
                   "runeStyleId": 8000, "runeSubStyleId": 8300, "keystoneId": 8010,
                   "perkIds": [8010, 9111, 9104, 8299, 8345, 8347, 5005, 5008, 5001]})
        );
    }

    #[test]
    fn le_a_recomendacao() {
        let rec = Recommendation::from_json(&json!([
            {"perks": [{"id": 8112}, {"id": 8126}], "primaryPerkStyleId": 8100, "secondaryPerkStyleId": 8200,
             "summonerSpellIds": [4, 14], "isDefault": false},
            {"perks": [{"id": 8005}], "primaryPerkStyleId": 8000},
        ]))
        .unwrap();
        assert_eq!(
            rec.runes,
            Some(SlotRunes {
                perk_ids: vec![8112, 8126],
                style_id: 8100,
                sub_style_id: 8200
            })
        );
        assert_eq!(rec.spells, Some((4, 14)));
        assert_eq!(Recommendation::from_json(&json!([])), None);
        assert_eq!(
            Recommendation::from_json(&json!([{}])),
            Some(Recommendation::default())
        );
        assert_eq!(
            recommended_pages_path(22, Position::Bottom),
            "/lol-perks/v1/recommended-pages/champion/22/position/bottom/map/11"
        );
    }

    fn pages() -> RunePages {
        RunePages {
            pages: vec![RunePage {
                id: 7,
                name: "Minha".into(),
                primary_style_id: 8200,
                sub_style_id: 8300,
                selected_perk_ids: vec![8214, 8226],
                is_editable: true,
                is_deletable: true,
                is_temporary: false,
                is_valid: true,
            }],
            current_page_id: Some(7),
            can_create_page: true,
        }
    }

    #[test]
    fn monta_o_slot() {
        let perks = r#"{"perkIds":[8010],"perkStyle":8000,"perkSubStyle":8300}"#;
        let current = PlayerSlot::from_json(&slot_json(22, "BOTTOM", perks)).unwrap();
        let rec = Recommendation {
            runes: Some(SlotRunes {
                perk_ids: vec![8112],
                style_id: 8100,
                sub_style_id: 8200,
            }),
            spells: Some((4, 7)),
        };

        // Mesmo campeão e posição: mantém feitiços, runas e skin.
        let same = input(22, Position::Bottom);
        assert!(!needs_recommendation(&same, Some(&current)));
        assert_eq!(
            build_slot(&same, Some(&current), Some(&pages()), None),
            json!({"championId": 22, "positionPreference": "BOTTOM", "skinId": 22003,
                   "spell1": 4, "spell2": 14, "perks": perks})
        );

        // Mesmo campeão em outra posição: recomendados, mas a skin fica.
        let moved = input(22, Position::Utility);
        assert!(needs_recommendation(&moved, Some(&current)));
        let built = build_slot(&moved, Some(&current), Some(&pages()), Some(&rec));
        assert_eq!(built["skinId"], 22003);
        assert_eq!(
            (built["spell1"].clone(), built["spell2"].clone()),
            (json!(4), json!(7))
        );
        assert_eq!(
            built["perks"],
            r#"{"perkIds":[8112],"perkStyle":8100,"perkSubStyle":8200}"#
        );

        // Campeão novo com feitiços e página escolhidos.
        let mut chosen = input(266, Position::Top);
        chosen.spell1_id = Some(12);
        chosen.spell2_id = Some(4);
        chosen.rune_page_id = Some(7);
        assert!(!needs_recommendation(&chosen, Some(&current)));
        let built = build_slot(&chosen, Some(&current), Some(&pages()), None);
        assert_eq!(built["skinId"], 266000);
        assert_eq!(
            (built["spell1"].clone(), built["spell2"].clone()),
            (json!(12), json!(4))
        );
        assert_eq!(
            built["perks"],
            r#"{"perkIds":[8214,8226],"perkStyle":8200,"perkSubStyle":8300}"#
        );

        // Skin escolhida (#403): vale mais que a do slot; de outro campeão, não vale.
        let skin = PlayerSlotInput {
            skin_id: Some(22010),
            ..input(22, Position::Bottom)
        };
        assert_eq!(
            build_slot(&skin, Some(&current), Some(&pages()), None)["skinId"],
            22010
        );
        let base = PlayerSlotInput {
            skin_id: Some(22000),
            ..input(22, Position::Bottom)
        };
        assert_eq!(
            build_slot(&base, Some(&current), Some(&pages()), None)["skinId"],
            22000
        );
        let wrong = PlayerSlotInput {
            skin_id: Some(266001),
            ..input(22, Position::Bottom)
        };
        assert_eq!(
            check_slots(&[wrong, input(266, Position::Top)]),
            Err(InvalidSlots::Skin)
        );

        // Sem recomendação: feitiços da posição e a página em uso.
        let built = build_slot(&input(266, Position::Jungle), None, Some(&pages()), None);
        assert_eq!(
            (built["spell1"].clone(), built["spell2"].clone()),
            (json!(4), json!(11))
        );
        assert_eq!(
            built["perks"],
            r#"{"perkIds":[8214,8226],"perkStyle":8200,"perkSubStyle":8300}"#
        );
        assert_eq!(
            build_slot(&input(266, Position::Jungle), None, None, None)["perks"],
            ""
        );
    }

    #[test]
    fn slot_acompanha_o_campeao_ao_inverter() {
        let perks = r#"{"perkIds":[8010],"perkStyle":8000,"perkSubStyle":8300}"#;
        let current = [
            PlayerSlot::from_json(&slot_json(22, "BOTTOM", perks)).unwrap(),
            PlayerSlot::from_json(&slot_json(266, "TOP", "")).unwrap(),
        ];
        // Invertidos: o Aatrox vai para o primeiro slot e leva o que tinha.
        let swapped = input(266, Position::Top);
        assert_eq!(
            current_slot(&current, 0, &swapped).map(|c| c.champion_id),
            Some(266)
        );
        let ashe = input(22, Position::Bottom);
        let found = current_slot(&current, 1, &ashe).unwrap();
        assert!(!needs_recommendation(&ashe, Some(found)));
        assert_eq!(build_slot(&ashe, Some(found), None, None)["perks"], perks);
        // Campeão novo: o slot do mesmo índice.
        assert_eq!(
            current_slot(&current, 1, &input(1, Position::Top)).map(|c| c.champion_id),
            Some(266)
        );
    }

    #[test]
    fn campeoes_que_da_para_jogar() {
        let ids = playable_from_json(&json!([
            {"id": 266, "ownership": {"owned": true, "rental": {"rented": false}}, "freeToPlay": false},
            {"id": 22, "ownership": {"owned": false, "rental": {"rented": false}}, "freeToPlay": true},
            {"id": 1, "ownership": {"owned": false, "rental": {"rented": true}}},
            {"id": 99, "ownership": {"owned": false, "rental": {"rented": false}}, "freeToPlay": false},
            {"id": -1, "ownership": {"owned": true}},
        ]))
        .unwrap();
        assert_eq!(ids, [1, 22, 266]);
        assert_eq!(playable_from_json(&json!({})), None);
    }
}
