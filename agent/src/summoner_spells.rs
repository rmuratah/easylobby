//! Feitiços de invocador na seleção de campeões (#24).
//!
//! - Os feitiços do jogador logado já chegam em `spell1Id` e `spell2Id` de
//!   [`ChampSelectPlayer`](crate::champ_select::ChampSelectPlayer) (#22).
//! - Para trocar, o agente manda
//!   `PATCH /lol-champ-select/v1/session/my-selection` com
//!   `{"spell1Id": N, "spell2Id": M}`, como o client oficial. A LCU responde
//!   sem corpo e a sessão nova chega pelo WAMP.
//! - Para a tela montar a escolha, o agente lê uma vez por conexão o arquivo
//!   de dados do jogo que a própria LCU serve, já no idioma do client:
//!   `/lol-game-data/assets/v1/summoner-spells.json` (nome, descrição, nível
//!   mínimo e modos de jogo de cada feitiço). Os ícones saem do `/assets` do
//!   agente ([`crate::assets`], tipo `summonerSpell`).
//!
//! [`check_spells`] confere o pedido antes de falar com a LCU: seleção
//! aberta, dois feitiços diferentes, que existem, valem na fila e estão
//! liberados no nível do jogador.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::champ_select::{ChampSelect, ChampSelectPhase};
use crate::lobby::Queue;
use crate::queues::GameQueue;
use crate::rest::{LcuClient, LcuError};

/// Escolhas do jogador logado na seleção (feitiços, skin...).
pub const MY_SELECTION_PATH: &str = "/lol-champ-select/v1/session/my-selection";

/// Nome, descrição, nível mínimo e modos de jogo de cada feitiço.
pub const SUMMONER_SPELLS_PATH: &str = "/lol-game-data/assets/v1/summoner-spells.json";

/// Os feitiços de invocador do jogo, para a tela de escolha. Lidos uma vez
/// por conexão.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SummonerSpellCatalog {
    /// Feitiços na ordem da LCU. Só os que valem em algum modo de jogo.
    pub spells: Vec<SummonerSpell>,
}

/// Um feitiço de invocador (Flash, Incendiar...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SummonerSpell {
    /// O mesmo de `spell1Id`/`spell2Id` e do ícone em
    /// `/assets/summonerSpell/{id}`.
    pub id: u32,
    /// Nome no idioma do client.
    pub name: String,
    /// Descrição no idioma do client, com as tags da Riot (`<br>`...).
    pub description: String,
    /// Nível de invocador a partir do qual o feitiço está liberado.
    pub summoner_level: u32,
    /// Modos de jogo da LCU em que o feitiço vale (`CLASSIC`, `ARAM`,
    /// `SWIFTPLAY`...).
    pub game_modes: Vec<String>,
    /// As filas do easylobby em que o feitiço vale, calculadas de
    /// `gameModes`. A tela mostra os que têm `champSelect.queue` aqui (e
    /// todos quando `champSelect.queue` é `null`, como numa personalizada).
    pub queues: Vec<Queue>,
}

impl SummonerSpellCatalog {
    /// Monta o catálogo a partir de `summoner-spells.json`. `None` se o
    /// arquivo não for uma lista.
    pub fn from_json(spells: &Value) -> Option<Self> {
        let list = spells.as_array()?;
        let spells = list
            .iter()
            .filter_map(|raw| serde_json::from_value::<RawSpell>(raw.clone()).ok())
            .filter_map(SummonerSpell::from_raw)
            .collect();
        Some(Self { spells })
    }

    pub fn get(&self, id: u32) -> Option<&SummonerSpell> {
        self.spells.iter().find(|s| s.id == id)
    }

    /// O modo de jogo de `gameModes` que vale na fila `queue` (#146).
    ///
    /// Normalmente é o próprio modo da fila. Mas um modo novo pode chegar
    /// antes de o arquivo de feitiços listá-lo (a ARAM: Desordem é `KIWI`):
    /// se nenhum feitiço tem o modo da fila, uma fila do Abismo Uivante usa
    /// as regras do `ARAM`, e as outras ficam sem filtro (`None`, a LCU
    /// decide).
    pub fn spell_game_mode<'a>(&self, queue: &'a GameQueue) -> Option<&'a str> {
        let mode = queue.game_mode.as_str();
        let listed = |mode: &str| {
            self.spells
                .iter()
                .any(|s| s.game_modes.iter().any(|m| m == mode))
        };
        if listed(mode) {
            Some(mode)
        } else if queue.is_aram_like() {
            Some(game_mode(Queue::Aram))
        } else {
            None
        }
    }
}

impl SummonerSpell {
    fn from_raw(raw: RawSpell) -> Option<Self> {
        let id = u32::try_from(raw.id).ok().filter(|&id| id > 0)?;
        // Sem modo de jogo é um feitiço interno (placeholders, testes).
        if raw.game_modes.is_empty() {
            return None;
        }
        let queues = Queue::ALL
            .into_iter()
            .filter(|&q| raw.game_modes.iter().any(|m| m == game_mode(q)))
            .collect();
        Some(Self {
            id,
            name: raw.name,
            description: raw.description,
            summoner_level: raw.summoner_level,
            game_modes: raw.game_modes,
            queues,
        })
    }

    /// Se o feitiço vale na fila `queue`.
    pub fn allowed_in(&self, queue: Queue) -> bool {
        self.queues.contains(&queue)
    }
}

/// Modo de jogo da LCU de cada fila, como em `/lol-game-queues/v1/queues`
/// (conferido na gravação do patch 16.22).
pub const fn game_mode(queue: Queue) -> &'static str {
    match queue {
        Queue::NormalDraft | Queue::RankedSolo | Queue::RankedFlex => "CLASSIC",
        Queue::Swiftplay => "SWIFTPLAY",
        Queue::Aram => "ARAM",
    }
}

/// Um feitiço como a LCU manda, só com os campos usados.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSpell {
    id: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    summoner_level: u32,
    #[serde(default)]
    game_modes: Vec<String>,
}

/// Lê os feitiços do jogo, no idioma do client.
pub async fn catalog(lcu: &LcuClient) -> Result<Option<SummonerSpellCatalog>, LcuError> {
    let spells = lcu.get::<Value>(SUMMONER_SPELLS_PATH).await?;
    Ok(SummonerSpellCatalog::from_json(&spells))
}

/// Por que um pedido de troca de feitiços não vale. Nada foi mandado para a
/// LCU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidSpells {
    /// Não há seleção de campeões aberta (ou o jogador só assiste).
    NoChampSelect,
    /// A seleção já acabou e a partida está abrindo.
    SelectionOver,
    /// Um dos feitiços veio vazio (`0`).
    NoSpell,
    /// Os dois feitiços são o mesmo.
    SameSpell,
    /// Feitiço que não existe no jogo.
    UnknownSpell(u32),
    /// Feitiço que não vale nesta fila (como Marcar fora do ARAM).
    NotInQueue(u32),
    /// Feitiço ainda bloqueado no nível do jogador.
    LevelTooLow(u32),
}

impl fmt::Display for InvalidSpells {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvalidSpells::NoChampSelect => f.write_str("não há seleção de campeões aberta"),
            InvalidSpells::SelectionOver => f.write_str("a seleção de campeões já acabou"),
            InvalidSpells::NoSpell => f.write_str("escolha os dois feitiços"),
            InvalidSpells::SameSpell => f.write_str("os dois feitiços precisam ser diferentes"),
            InvalidSpells::UnknownSpell(id) => write!(f, "o feitiço {id} não existe"),
            InvalidSpells::NotInQueue(id) => write!(f, "o feitiço {id} não vale nesta fila"),
            InvalidSpells::LevelTooLow(id) => {
                write!(f, "o feitiço {id} ainda está bloqueado no seu nível")
            }
        }
    }
}

impl std::error::Error for InvalidSpells {}

/// Confere um pedido de troca de feitiços contra a seleção atual, com as
/// regras do client oficial. Sem o catálogo (a LCU não mandou), só confere
/// a seleção e os ids; sem o nível do invocador, não confere o nível.
pub fn check_spells(
    champ_select: Option<&ChampSelect>,
    catalog: Option<&SummonerSpellCatalog>,
    summoner_level: Option<u32>,
    spell1_id: u32,
    spell2_id: u32,
) -> Result<(), InvalidSpells> {
    check_spells_in(
        champ_select,
        None,
        catalog,
        summoner_level,
        spell1_id,
        spell2_id,
    )
}

/// Como [`check_spells`], com a fila da seleção na lista de filas
/// disponíveis (#146). Numa fila que não é uma das [`Queue`] fixas (ARAM:
/// Desordem, Jade...), confere o feitiço pelo modo de jogo dela; veja
/// [`SummonerSpellCatalog::spell_game_mode`].
pub fn check_spells_in(
    champ_select: Option<&ChampSelect>,
    game_queue: Option<&GameQueue>,
    catalog: Option<&SummonerSpellCatalog>,
    summoner_level: Option<u32>,
    spell1_id: u32,
    spell2_id: u32,
) -> Result<(), InvalidSpells> {
    let cs = champ_select.ok_or(InvalidSpells::NoChampSelect)?;
    if cs.local_player().is_none() {
        return Err(InvalidSpells::NoChampSelect);
    }
    if cs.timer.phase == ChampSelectPhase::GameStarting {
        return Err(InvalidSpells::SelectionOver);
    }
    if spell1_id == 0 || spell2_id == 0 {
        return Err(InvalidSpells::NoSpell);
    }
    if spell1_id == spell2_id {
        return Err(InvalidSpells::SameSpell);
    }
    let Some(catalog) = catalog else {
        return Ok(());
    };
    for id in [spell1_id, spell2_id] {
        let spell = catalog.get(id).ok_or(InvalidSpells::UnknownSpell(id))?;
        let allowed = match (cs.queue, game_queue) {
            (Some(queue), _) => spell.allowed_in(queue),
            (None, Some(game_queue)) => catalog
                .spell_game_mode(game_queue)
                .is_none_or(|mode| spell.game_modes.iter().any(|m| m == mode)),
            // Fila desconhecida (personalizada...): a LCU decide.
            (None, None) => true,
        };
        if !allowed {
            return Err(InvalidSpells::NotInQueue(id));
        }
        if summoner_level.is_some_and(|level| level < spell.summoner_level) {
            return Err(InvalidSpells::LevelTooLow(id));
        }
    }
    Ok(())
}

/// Troca os dois feitiços do jogador logado. A LCU responde sem corpo; a
/// sessão nova chega pelo WAMP (ou por
/// [`current_session`](crate::champ_select::current_session)).
pub async fn set_spells(lcu: &LcuClient, spell1_id: u32, spell2_id: u32) -> Result<(), LcuError> {
    lcu.patch::<_, Option<Value>>(
        MY_SELECTION_PATH,
        &json!({ "spell1Id": spell1_id, "spell2Id": spell2_id }),
    )
    .await
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_catalog() -> Value {
        json!([
            {"id": 4, "name": "Flash", "description": "Teleporta.", "summonerLevel": 7,
             "gameModes": ["CLASSIC", "ARAM", "SWIFTPLAY", "URF"],
             "iconPath": "/lol-game-data/assets/DATA/Spells/Icons2D/Summoner_flash.png"},
            {"id": 11, "name": "Golpear", "description": "Dano em monstros.", "summonerLevel": 9,
             "gameModes": ["CLASSIC", "SWIFTPLAY"]},
            {"id": 14, "name": "Incendiar", "description": "Queima.", "summonerLevel": 9,
             "gameModes": ["CLASSIC", "ARAM", "SWIFTPLAY"]},
            {"id": 32, "name": "Marcar", "description": "Bola de neve.", "summonerLevel": 7,
             "gameModes": ["ARAM"]},
            {"id": 54, "name": "Placeholder", "description": "", "summonerLevel": 1,
             "gameModes": []},
            {"id": 0, "name": "Nenhum", "gameModes": ["CLASSIC"]},
            "lixo",
        ])
    }

    fn session(queue_id: u32, phase: &str) -> ChampSelect {
        let raw = json!({
            "id": "s",
            "queueId": queue_id,
            "localPlayerCellId": 0,
            "timer": {"phase": phase},
            "myTeam": [{"cellId": 0, "spell1Id": 4, "spell2Id": 14}],
        });
        ChampSelect::from_json(&raw).unwrap()
    }

    #[test]
    fn le_o_catalogo() {
        let catalog = SummonerSpellCatalog::from_json(&raw_catalog()).unwrap();
        let ids: Vec<u32> = catalog.spells.iter().map(|s| s.id).collect();
        assert_eq!(ids, [4, 11, 14, 32]);
        let flash = catalog.get(4).unwrap();
        assert_eq!(flash.name, "Flash");
        assert_eq!(flash.summoner_level, 7);
        assert_eq!(flash.queues, Queue::ALL);
        let smite = catalog.get(11).unwrap();
        assert_eq!(
            smite.queues,
            [
                Queue::NormalDraft,
                Queue::Swiftplay,
                Queue::RankedSolo,
                Queue::RankedFlex
            ]
        );
        assert_eq!(catalog.get(32).unwrap().queues, [Queue::Aram]);
        assert!(SummonerSpellCatalog::from_json(&json!({})).is_none());
    }

    #[test]
    fn modos_de_jogo_das_filas() {
        assert_eq!(game_mode(Queue::RankedSolo), "CLASSIC");
        assert_eq!(game_mode(Queue::Swiftplay), "SWIFTPLAY");
        assert_eq!(game_mode(Queue::Aram), "ARAM");
    }

    #[test]
    fn confere_os_feiticos() {
        let catalog = SummonerSpellCatalog::from_json(&raw_catalog()).unwrap();
        let ranked = session(420, "BAN_PICK");
        let check =
            |cs: &ChampSelect, level, s1, s2| check_spells(Some(cs), Some(&catalog), level, s1, s2);

        assert_eq!(check(&ranked, Some(30), 4, 11), Ok(()));
        assert_eq!(check(&ranked, Some(30), 11, 4), Ok(()));
        assert_eq!(
            check_spells(None, Some(&catalog), Some(30), 4, 11),
            Err(InvalidSpells::NoChampSelect)
        );
        assert_eq!(check(&ranked, Some(30), 4, 0), Err(InvalidSpells::NoSpell));
        assert_eq!(
            check(&ranked, Some(30), 4, 4),
            Err(InvalidSpells::SameSpell)
        );
        assert_eq!(
            check(&ranked, Some(30), 4, 999),
            Err(InvalidSpells::UnknownSpell(999))
        );
        assert_eq!(
            check(&ranked, Some(30), 4, 54),
            Err(InvalidSpells::UnknownSpell(54))
        );
        assert_eq!(
            check(&ranked, Some(30), 32, 4),
            Err(InvalidSpells::NotInQueue(32))
        );
        assert_eq!(
            check(&ranked, Some(8), 4, 14),
            Err(InvalidSpells::LevelTooLow(14))
        );
        // Sem o nível, não confere o nível.
        assert_eq!(check(&ranked, None, 4, 14), Ok(()));

        // ARAM: Marcar vale, Golpear não.
        let aram = session(450, "BAN_PICK");
        assert_eq!(check(&aram, Some(30), 4, 32), Ok(()));
        assert_eq!(
            check(&aram, Some(30), 4, 11),
            Err(InvalidSpells::NotInQueue(11))
        );

        // Fila fora do easylobby (personalizada): não confere o modo.
        let custom = session(0, "BAN_PICK");
        assert_eq!(check(&custom, Some(30), 4, 32), Ok(()));

        // Na finalização ainda troca; com a partida abrindo, não.
        let finalization = session(420, "FINALIZATION");
        assert_eq!(check(&finalization, Some(30), 4, 11), Ok(()));
        let starting = session(420, "GAME_STARTING");
        assert_eq!(
            check(&starting, Some(30), 4, 11),
            Err(InvalidSpells::SelectionOver)
        );

        // Sem o catálogo, só a seleção e os ids.
        assert_eq!(check_spells(Some(&ranked), None, Some(1), 4, 32), Ok(()));
        assert_eq!(
            check_spells(Some(&ranked), None, Some(1), 4, 4),
            Err(InvalidSpells::SameSpell)
        );
    }

    /// Uma fila da lista de filas disponíveis (#146).
    fn game_queue(queue_id: u32, game_mode: &str, mode_group: &str) -> GameQueue {
        let raw = json!([{
            "id": queue_id, "name": "Fila", "gameMode": game_mode,
            "gameSelectCategory": "kPvP", "gameSelectModeGroup": mode_group,
            "queueAvailability": "Available", "isEnabled": true, "isVisible": true,
        }]);
        crate::queues::from_json(&raw).unwrap().remove(0)
    }

    #[test]
    fn desordem_usa_as_regras_do_aram_sem_modo_proprio_no_catalogo() {
        let catalog = SummonerSpellCatalog::from_json(&raw_catalog()).unwrap();
        let mayhem = game_queue(2400, "KIWI", "kARAM");
        assert_eq!(catalog.spell_game_mode(&mayhem), Some("ARAM"));
        let cs = session(2400, "BAN_PICK");
        assert_eq!(cs.queue, None);
        let check =
            |s1, s2| check_spells_in(Some(&cs), Some(&mayhem), Some(&catalog), Some(30), s1, s2);
        // Marcar vale, Golpear não.
        assert_eq!(check(4, 32), Ok(()));
        assert_eq!(check(4, 11), Err(InvalidSpells::NotInQueue(11)));
    }

    #[test]
    fn modo_listado_no_catalogo_vale_por_si() {
        let mut raw = raw_catalog();
        // Um patch em que o arquivo de feitiços já conhece o `KIWI`, só com
        // o Flash e o Incendiar.
        for spell in raw.as_array_mut().unwrap() {
            if spell["id"] == 4 || spell["id"] == 14 {
                spell["gameModes"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("KIWI"));
            }
        }
        let catalog = SummonerSpellCatalog::from_json(&raw).unwrap();
        let mayhem = game_queue(2400, "KIWI", "kARAM");
        assert_eq!(catalog.spell_game_mode(&mayhem), Some("KIWI"));
        let cs = session(2400, "BAN_PICK");
        let check =
            |s1, s2| check_spells_in(Some(&cs), Some(&mayhem), Some(&catalog), Some(30), s1, s2);
        assert_eq!(check(4, 14), Ok(()));
        assert_eq!(check(4, 32), Err(InvalidSpells::NotInQueue(32)));
    }

    #[test]
    fn modo_desconhecido_fora_do_aram_fica_sem_filtro() {
        let catalog = SummonerSpellCatalog::from_json(&raw_catalog()).unwrap();
        let jade = game_queue(4310, "JADE", "kJade");
        assert_eq!(catalog.spell_game_mode(&jade), None);
        let cs = session(4310, "BAN_PICK");
        assert_eq!(
            check_spells_in(Some(&cs), Some(&jade), Some(&catalog), Some(30), 4, 32),
            Ok(())
        );
        // Contra IA no Rift usa o modo `SWIFTPLAY`, que o catálogo conhece.
        let bots = game_queue(880, "SWIFTPLAY", "kSummonersRift");
        let cs = session(880, "BAN_PICK");
        assert_eq!(
            check_spells_in(Some(&cs), Some(&bots), Some(&catalog), Some(30), 4, 32),
            Err(InvalidSpells::NotInQueue(32))
        );
    }

    #[test]
    fn sem_o_jogador_no_time_nao_troca() {
        let raw = json!({
            "localPlayerCellId": 7,
            "timer": {"phase": "BAN_PICK"},
            "myTeam": [{"cellId": 0}],
        });
        let cs = ChampSelect::from_json(&raw).unwrap();
        assert_eq!(
            check_spells(Some(&cs), None, None, 4, 14),
            Err(InvalidSpells::NoChampSelect)
        );
    }
}
