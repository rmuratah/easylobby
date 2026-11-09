//! Filas disponíveis para criar lobby (#146).
//!
//! As cinco filas fixas de [`Queue`] não cobrem os modos rotativos e de
//! evento (ARAM: Desordem, Jade...), que entram e saem a cada patch. Em vez
//! de manter uma lista no código, o agente lê a lista de filas da própria
//! LCU, `GET /lol-game-queues/v1/queues`, uma vez por conexão (e de novo a
//! cada evento do WAMP no mesmo URI), e passa para a UI só as que dá para
//! criar um lobby agora:
//!
//! - `queueAvailability` `Available`, `isEnabled` e `isVisible` (o Clash e
//!   o tutorial vêm invisíveis);
//! - `gameSelectCategory` `kPvP` ou `kVersusAI` (fica de fora o que é
//!   personalizada, `CreateCustom`, e treino, `kTraining`);
//! - nem TFT (`TFT`, outro jogo).
//!
//! A Arena (`CHERRY`) entra como as outras filas (#154): a Arena 3x6 (1750)
//! tem lugar fixo na tela de escolha de modo (#209) e as outras Arenas
//! abertas (Arena da Bravura, 1740) vão para o mesmo card na UI.
//!
//! As filas fixas da tela de escolha de modo ([`MODE_SCREEN_QUEUES`], #209)
//! vêm mesmo fechadas, com `available: false`, para a UI acinzentar o modo
//! em vez de escondê-lo. As outras só vêm abertas.
//!
//! Na gravação do patch 16.22 sobram as cinco filas de [`Queue`], a ARAM:
//! Desordem (2400, modo `KIWI`), a Jade (4310, e 4320 contra IA), o
//! cooperativo contra IA (870, 880 e 890), a Arena 3x6 (1750), fechada, e a
//! Arena da Bravura (1740), aberta.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::lobby::Queue;
use crate::rest::{LcuClient, LcuError};

/// Endpoint (e URI dos eventos WAMP) da lista de filas.
pub const GAME_QUEUES_PATH: &str = "/lol-game-queues/v1/queues";

/// `gameSelectModeGroup` das filas no Abismo Uivante (ARAM e variações,
/// como a ARAM: Desordem).
pub const ARAM_MODE_GROUP: &str = "kARAM";

/// Modos de jogo que não entram na lista mesmo disponíveis (menos as filas
/// de [`MODE_SCREEN_QUEUES`]).
const EXCLUDED_GAME_MODES: [&str; 1] = ["TFT"];

/// `queueId` da Arena 3x6 (`CHERRY`), a Arena com lugar fixo na tela de
/// escolha de modo (#209).
pub const ARENA_3X6_QUEUE_ID: u32 = 1750;

/// Filas com lugar fixo na tela de escolha de modo (#209, #210): vêm na
/// lista mesmo fechadas, com `available: false`. Summoner's Rift (480, 400,
/// 440, 420), ARAM (450, 2400), Arena 3x6 (1750) e o cooperativo contra IA
/// (870, 880, 890).
pub const MODE_SCREEN_QUEUES: [u32; 10] = [
    480,
    400,
    440,
    420,
    450,
    2400,
    ARENA_3X6_QUEUE_ID,
    870,
    880,
    890,
];

/// Uma fila da lista do agente: aberta agora, ou uma das filas fixas de
/// [`MODE_SCREEN_QUEUES`], aberta ou fechada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GameQueue {
    /// `queueId` da Riot; é o que vai no comando `createLobbyById`.
    pub queue_id: u32,
    /// A fila do easylobby com esse id, ou `null` nas que não são uma das
    /// [`Queue`] fixas (Desordem, Jade, contra IA...).
    pub queue: Option<Queue>,
    /// Nome no idioma do client (`ARAM: Desordem`, `Iniciante`). Quando a
    /// LCU manda o nome vazio (Jade PvP no 16.22), vem a descrição.
    pub name: String,
    /// Descrição curta no idioma do client (`Escolha alternada`,
    /// `5v5 Clássico`...).
    pub description: String,
    /// Modo de jogo da LCU (`CLASSIC`, `ARAM`, `KIWI`, `JADE`,
    /// `SWIFTPLAY`...). É o que aparece em `gameModes` de cada feitiço de
    /// invocador.
    pub game_mode: String,
    /// Grupo da tela de escolha de modo do client oficial
    /// (`kSummonersRift`, `kARAM`, `kJade`...), para a UI agrupar as filas.
    pub mode_group: String,
    pub map_id: u32,
    /// Contra bots (`kVersusAI`) em vez de contra jogadores.
    pub versus_ai: bool,
    pub ranked: bool,
    /// Se o lobby nesta fila tem escolha de cargos.
    pub show_position_selector: bool,
    /// Fila de tempo limitado (evento), segundo a LCU.
    pub limited_time: bool,
    /// Se a fila está aberta agora (`queueAvailability` `Available`,
    /// `isEnabled` e `isVisible`). Só as filas fixas de
    /// [`MODE_SCREEN_QUEUES`] vêm com `false`; `createLobbyById` recusa
    /// essas (#209).
    pub available: bool,
}

/// A fila como a LCU manda, só com os campos usados.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawQueue {
    id: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    game_mode: String,
    #[serde(default)]
    game_select_category: String,
    #[serde(default)]
    game_select_mode_group: String,
    #[serde(default)]
    map_id: u32,
    #[serde(default)]
    queue_availability: String,
    #[serde(default)]
    is_enabled: bool,
    #[serde(default)]
    is_visible: bool,
    #[serde(default)]
    is_custom: bool,
    #[serde(default)]
    is_ranked: bool,
    #[serde(default)]
    is_limited_time_queue: bool,
    #[serde(default)]
    show_position_selector: bool,
}

impl GameQueue {
    fn from_raw(raw: RawQueue) -> Option<Self> {
        let versus_ai = match raw.game_select_category.as_str() {
            "kPvP" => false,
            "kVersusAI" => true,
            _ => return None,
        };
        if raw.is_custom || raw.id == 0 {
            return None;
        }
        let fixed = MODE_SCREEN_QUEUES.contains(&raw.id);
        if !fixed && EXCLUDED_GAME_MODES.contains(&raw.game_mode.as_str()) {
            return None;
        }
        let open = raw.queue_availability == "Available" && raw.is_enabled && raw.is_visible;
        if !open && !fixed {
            return None;
        }
        let name = if raw.name.trim().is_empty() {
            raw.description.clone()
        } else {
            raw.name
        };
        Some(Self {
            queue_id: raw.id,
            queue: Queue::from_id(raw.id),
            name,
            description: raw.description,
            game_mode: raw.game_mode,
            mode_group: raw.game_select_mode_group,
            map_id: raw.map_id,
            versus_ai,
            ranked: raw.is_ranked,
            show_position_selector: raw.show_position_selector,
            limited_time: raw.is_limited_time_queue,
            available: open,
        })
    }

    /// Se a fila é no Abismo Uivante, com as regras do ARAM.
    pub fn is_aram_like(&self) -> bool {
        self.mode_group == ARAM_MODE_GROUP
    }
}

/// Lê as filas do JSON da LCU (resposta do `GET` ou `data` de
/// um evento), na ordem do `queueId`. `None` se não for uma lista.
pub fn from_json(value: &Value) -> Option<Vec<GameQueue>> {
    let mut queues: Vec<GameQueue> = value
        .as_array()?
        .iter()
        .filter_map(|raw| RawQueue::deserialize(raw).ok())
        .filter_map(GameQueue::from_raw)
        .collect();
    queues.sort_by_key(|q| q.queue_id);
    queues.dedup_by_key(|q| q.queue_id);
    Some(queues)
}

/// Busca as filas disponíveis agora.
pub async fn available(lcu: &LcuClient) -> Result<Option<Vec<GameQueue>>, LcuError> {
    let body = lcu.get::<Value>(GAME_QUEUES_PATH).await?;
    Ok(from_json(&body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A lista de filas da gravação do patch 16.22.
    fn recorded() -> Value {
        let rest: Value = serde_json::from_str(include_str!("../tests/fixtures/16.22/rest.json"))
            .expect("rest.json");
        rest.as_array()
            .unwrap()
            .iter()
            .find(|e| e["path"] == GAME_QUEUES_PATH)
            .map(|e| e["body"].clone())
            .expect("a gravação tem a lista de filas")
    }

    #[test]
    fn gravacao_16_22_tem_as_filas_fixas_e_as_novas() {
        let queues = from_json(&recorded()).unwrap();
        let ids: Vec<u32> = queues.iter().map(|q| q.queue_id).collect();
        assert_eq!(
            ids,
            [400, 420, 440, 450, 480, 870, 880, 890, 1740, 1750, 2400, 4310, 4320]
        );
        for queue in Queue::ALL {
            let found = queues.iter().find(|q| q.queue_id == queue.id()).unwrap();
            assert_eq!(found.queue, Some(queue));
        }
        let closed: Vec<u32> = queues
            .iter()
            .filter(|q| !q.available)
            .map(|q| q.queue_id)
            .collect();
        assert_eq!(closed, [ARENA_3X6_QUEUE_ID]);
    }

    #[test]
    fn arena_3x6_fechada_vem_acinzentada_e_a_arena_da_bravura_aberta_entra() {
        let queues = from_json(&recorded()).unwrap();
        let arena = queues
            .iter()
            .find(|q| q.queue_id == ARENA_3X6_QUEUE_ID)
            .unwrap();
        assert_eq!(arena.name, "Arena 3x6");
        assert_eq!(arena.game_mode, "CHERRY");
        assert!(!arena.available);
        // A Arena da Bravura (1740) estava aberta e entra (#154); as Arenas
        // fechadas que não são fixas ficam de fora.
        let bravura = queues.iter().find(|q| q.queue_id == 1740).unwrap();
        assert_eq!(bravura.name, "Arena da Bravura");
        assert_eq!(bravura.game_mode, "CHERRY");
        assert!(bravura.available);
        assert!(!queues
            .iter()
            .any(|q| [1700, 1704, 1710].contains(&q.queue_id)));
    }

    #[test]
    fn fila_fixa_fechada_vem_e_aberta_fica_disponivel() {
        let raw = |id: u32, availability: &str, game_mode: &str| {
            json!({
                "id": id, "name": "Fila", "gameMode": game_mode, "gameSelectCategory": "kPvP",
                "queueAvailability": availability, "isEnabled": availability == "Available",
                "isVisible": true
            })
        };
        let list = json!([
            raw(420, "PlatformDisabled", "CLASSIC"),
            raw(ARENA_3X6_QUEUE_ID, "Available", "CHERRY"),
            raw(4310, "PlatformDisabled", "JADE"),
        ]);
        let queues = from_json(&list).unwrap();
        let summary: Vec<(u32, bool)> = queues.iter().map(|q| (q.queue_id, q.available)).collect();
        assert_eq!(summary, [(420, false), (ARENA_3X6_QUEUE_ID, true)]);
    }

    #[test]
    fn desordem_e_aram_com_modo_proprio() {
        let queues = from_json(&recorded()).unwrap();
        let mayhem = queues.iter().find(|q| q.queue_id == 2400).unwrap();
        assert_eq!(mayhem.queue, None);
        assert_eq!(mayhem.name, "ARAM: Desordem");
        assert_eq!(mayhem.game_mode, "KIWI");
        assert_eq!(mayhem.map_id, 12);
        assert!(mayhem.is_aram_like());
        assert!(!mayhem.versus_ai);
        assert!(!mayhem.show_position_selector);
    }

    #[test]
    fn jade_sem_nome_usa_a_descricao() {
        let queues = from_json(&recorded()).unwrap();
        let jade = queues.iter().find(|q| q.queue_id == 4310).unwrap();
        assert_eq!(jade.name, "5v5 Clássico");
        assert_eq!(jade.game_mode, "JADE");
        assert_eq!(jade.mode_group, "kJade");
        assert!(jade.ranked);
        assert!(jade.show_position_selector);
        let bots = queues.iter().find(|q| q.queue_id == 4320).unwrap();
        assert_eq!(bots.name, "Jade");
        assert!(bots.versus_ai);
    }

    #[test]
    fn contra_ia_marcado() {
        let queues = from_json(&recorded()).unwrap();
        for id in [870, 880, 890] {
            let queue = queues.iter().find(|q| q.queue_id == id).unwrap();
            assert!(queue.versus_ai, "{id}");
            assert_eq!(queue.game_mode, "SWIFTPLAY");
        }
        assert_eq!(
            queues.iter().find(|q| q.queue_id == 880).unwrap().name,
            "Iniciante"
        );
    }

    #[test]
    fn deixa_de_fora_o_que_nao_da_para_criar() {
        let base = json!({
            "id": 1, "name": "Fila", "gameMode": "CLASSIC", "gameSelectCategory": "kPvP",
            "gameSelectModeGroup": "kSummonersRift", "mapId": 11,
            "queueAvailability": "Available", "isEnabled": true, "isVisible": true,
            "isCustom": false
        });
        let with = |key: &str, value: Value| {
            let mut q = base.clone();
            q[key] = value;
            q
        };
        let list = json!([
            base,
            with("queueAvailability", json!("PlatformDisabled")),
            with("isEnabled", json!(false)),
            with("isVisible", json!(false)),
            with("isCustom", json!(true)),
            with("gameSelectCategory", json!("CreateCustom")),
            with("gameSelectCategory", json!("kTraining")),
            with("gameMode", json!("TFT")),
            "lixo",
            {"sem": "id"},
        ]);
        let queues = from_json(&list).unwrap();
        assert_eq!(queues.len(), 1);
        assert_eq!(queues[0].queue_id, 1);
        assert!(queues[0].available);
        assert!(from_json(&json!({"não": "é lista"})).is_none());
    }

    #[test]
    fn ids_repetidos_contam_uma_vez() {
        let q = json!({
            "id": 450, "name": "ARAM", "gameMode": "ARAM", "gameSelectCategory": "kPvP",
            "queueAvailability": "Available", "isEnabled": true, "isVisible": true
        });
        let queues = from_json(&json!([q, q])).unwrap();
        assert_eq!(queues.len(), 1);
        assert_eq!(queues[0].queue, Some(Queue::Aram));
    }
}
