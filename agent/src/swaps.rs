//! Trocas na seleção de campeões (#25): de campeão, de ordem de pick e de
//! cargo, entre o jogador logado e um aliado.
//!
//! A sessão da LCU (`GET /lol-champ-select/v1/session`) traz três listas,
//! uma por tipo de troca, com uma entrada para cada aliado com quem o
//! jogador pode trocar:
//!
//! - `trades`: troca de campeão, depois que os dois escolheram;
//! - `pickOrderSwaps`: troca de vez no draft, antes dos picks;
//! - `positionSwaps`: troca de cargo.
//!
//! Cada entrada tem um `id` (o da troca, que muda quando a LCU refaz a
//! lista), o `cellId` do aliado e o estado da troca do ponto de vista do
//! jogador logado ([`SwapState`]): `AVAILABLE` (dá para pedir), `SENT` (o
//! jogador pediu e espera), `RECEIVED` (o aliado pediu e espera a resposta),
//! `BUSY`/`INVALID` (não dá agora, por exemplo enquanto outra troca está
//! aberta). O agente manda as três listas para a UI em
//! [`ChampSelect`](crate::champ_select::ChampSelect) e a UI decide os botões
//! por elas.
//!
//! Os pedidos são `POST` sem corpo em
//! `/lol-champ-select/v1/session/{tipo}/{id}/{request|accept|decline|cancel}`,
//! com `tipo` `champion-swaps`, `pick-order-swaps` ou `position-swaps`
//! ([`swap_path`]). [`check_swap`] confere o pedido contra a sessão atual
//! antes de falar com a LCU.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::champ_select::{ChampSelect, CHAMP_SELECT_SESSION_PATH};
use crate::rest::{LcuClient, LcuError};

/// Uma troca possível entre o jogador logado e um aliado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ChampSelectSwap {
    /// Id da troca na LCU, o `swapId` dos comandos `requestSwap`,
    /// `acceptSwap`, `declineSwap` e `cancelSwap`. Muda quando a LCU refaz
    /// a lista (por exemplo depois de uma troca feita).
    // Cabe num `number` do JavaScript.
    #[ts(type = "number")]
    pub id: u64,
    /// `cellId` do aliado, para achar ele em `myTeam`.
    pub cell_id: i32,
    /// Estado da troca do ponto de vista do jogador logado.
    pub state: SwapState,
}

/// Estado de uma troca, com os nomes da LCU.
// `Deserialize` à mão para aceitar valores novos como `UNKNOWN`, como em
// `ChampSelectPhase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SwapState {
    /// Dá para pedir a troca.
    Available,
    /// O aliado está em outra troca.
    Busy,
    /// Não dá para trocar agora (por exemplo, enquanto outra troca do
    /// jogador está aberta, ou o campeão de um dos dois não pode ir para o
    /// outro).
    Invalid,
    /// O jogador pediu e espera a resposta do aliado; dá para cancelar.
    Sent,
    /// O aliado pediu e espera a resposta do jogador: aceitar ou recusar.
    Received,
    /// A troca foi aceita (aparece por pouco tempo, antes da LCU refazer a
    /// lista).
    Accepted,
    /// A troca foi recusada.
    Declined,
    /// O pedido foi cancelado.
    Cancelled,
    /// Estado que o agente não conhece.
    Unknown,
}

impl SwapState {
    pub const ALL: [SwapState; 9] = [
        SwapState::Available,
        SwapState::Busy,
        SwapState::Invalid,
        SwapState::Sent,
        SwapState::Received,
        SwapState::Accepted,
        SwapState::Declined,
        SwapState::Cancelled,
        SwapState::Unknown,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SwapState::Available => "AVAILABLE",
            SwapState::Busy => "BUSY",
            SwapState::Invalid => "INVALID",
            SwapState::Sent => "SENT",
            SwapState::Received => "RECEIVED",
            SwapState::Accepted => "ACCEPTED",
            SwapState::Declined => "DECLINED",
            SwapState::Cancelled => "CANCELLED",
            SwapState::Unknown => "UNKNOWN",
        }
    }

    pub fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|s| s.as_str() == name)
            .unwrap_or(SwapState::Unknown)
    }
}

impl<'de> Deserialize<'de> for SwapState {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = std::borrow::Cow::<'de, str>::deserialize(deserializer)?;
        Ok(Self::from_name(&name))
    }
}

impl fmt::Display for SwapState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Tipo de troca.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum SwapKind {
    /// Troca de campeão (`trades` na sessão).
    Champion,
    /// Troca de vez no draft (`pickOrderSwaps`).
    PickOrder,
    /// Troca de cargo (`positionSwaps`).
    Position,
}

impl SwapKind {
    pub const ALL: [SwapKind; 3] = [SwapKind::Champion, SwapKind::PickOrder, SwapKind::Position];

    /// Trecho do caminho da LCU para este tipo.
    pub fn lcu_path(self) -> &'static str {
        match self {
            SwapKind::Champion => "champion-swaps",
            SwapKind::PickOrder => "pick-order-swaps",
            SwapKind::Position => "position-swaps",
        }
    }
}

/// O que o jogador quer fazer com uma troca.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapIntent {
    /// Pedir a troca (estado `AVAILABLE`).
    Request,
    /// Aceitar o pedido do aliado (estado `RECEIVED`).
    Accept,
    /// Recusar o pedido do aliado (estado `RECEIVED`).
    Decline,
    /// Cancelar o próprio pedido (estado `SENT`).
    Cancel,
}

impl SwapIntent {
    /// Último trecho do caminho da LCU.
    pub fn lcu_verb(self) -> &'static str {
        match self {
            SwapIntent::Request => "request",
            SwapIntent::Accept => "accept",
            SwapIntent::Decline => "decline",
            SwapIntent::Cancel => "cancel",
        }
    }
}

/// Por que um pedido de troca não vale. Nada foi mandado para a LCU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidSwap {
    /// Não há seleção de campeões aberta.
    NoChampSelect,
    /// Não há troca com esse `id` desse tipo na sessão.
    UnknownSwap,
    /// Pedir uma troca que não está `AVAILABLE`.
    NotAvailable,
    /// Aceitar ou recusar sem pedido do aliado (`RECEIVED`).
    NotReceived,
    /// Cancelar sem pedido do jogador (`SENT`).
    NotSent,
}

impl fmt::Display for InvalidSwap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            InvalidSwap::NoChampSelect => "não há seleção de campeões aberta",
            InvalidSwap::UnknownSwap => "essa troca não existe na seleção atual",
            InvalidSwap::NotAvailable => "essa troca não está disponível agora",
            InvalidSwap::NotReceived => "não há pedido dessa troca para responder",
            InvalidSwap::NotSent => "não há pedido dessa troca para cancelar",
        })
    }
}

impl std::error::Error for InvalidSwap {}

/// Confere um pedido de troca contra a sessão atual: a troca existe no tipo
/// pedido e está no estado certo (`AVAILABLE` para pedir, `RECEIVED` para
/// aceitar ou recusar, `SENT` para cancelar).
pub fn check_swap(
    champ_select: Option<&ChampSelect>,
    kind: SwapKind,
    swap_id: u64,
    intent: SwapIntent,
) -> Result<(), InvalidSwap> {
    let cs = champ_select.ok_or(InvalidSwap::NoChampSelect)?;
    let swap = cs
        .swaps(kind)
        .iter()
        .find(|s| s.id == swap_id)
        .ok_or(InvalidSwap::UnknownSwap)?;
    let (needed, error) = match intent {
        SwapIntent::Request => (SwapState::Available, InvalidSwap::NotAvailable),
        SwapIntent::Accept | SwapIntent::Decline => (SwapState::Received, InvalidSwap::NotReceived),
        SwapIntent::Cancel => (SwapState::Sent, InvalidSwap::NotSent),
    };
    if swap.state != needed {
        return Err(error);
    }
    Ok(())
}

/// Caminho da LCU para pedir, aceitar, recusar ou cancelar uma troca.
pub fn swap_path(kind: SwapKind, swap_id: u64, intent: SwapIntent) -> String {
    format!(
        "{CHAMP_SELECT_SESSION_PATH}/{}/{swap_id}/{}",
        kind.lcu_path(),
        intent.lcu_verb()
    )
}

/// Manda o pedido para a LCU. A resposta (o contrato da troca) é ignorada:
/// a sessão nova chega pelo WAMP (ou por
/// [`current_session`](crate::champ_select::current_session)).
pub async fn send(
    lcu: &LcuClient,
    kind: SwapKind,
    swap_id: u64,
    intent: SwapIntent,
) -> Result<(), LcuError> {
    lcu.post::<_, Option<Value>>(&swap_path(kind, swap_id, intent), &json!({}))
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swap(id: u64, cell_id: i32, state: SwapState) -> ChampSelectSwap {
        ChampSelectSwap { id, cell_id, state }
    }

    fn champ_select() -> ChampSelect {
        let mut cs = ChampSelect::from_json_at(
            &json!({"localPlayerCellId": 2, "timer": {"phase": "BAN_PICK"}, "myTeam": []}),
            0,
        )
        .unwrap();
        cs.trades = vec![
            swap(47, 4, SwapState::Available),
            swap(30, 0, SwapState::Received),
            swap(41, 1, SwapState::Invalid),
        ];
        cs.pick_order_swaps = vec![swap(28, 0, SwapState::Sent)];
        cs.position_swaps = vec![swap(11, 1, SwapState::Busy)];
        cs
    }

    #[test]
    fn confere_o_estado_da_troca() {
        use SwapIntent::{Accept, Cancel, Decline, Request};
        use SwapKind::{Champion, PickOrder, Position};
        let cs = champ_select();
        let check = |kind, id, intent| check_swap(Some(&cs), kind, id, intent);

        assert_eq!(check(Champion, 47, Request), Ok(()));
        assert_eq!(check(Champion, 30, Accept), Ok(()));
        assert_eq!(check(Champion, 30, Decline), Ok(()));
        assert_eq!(check(PickOrder, 28, Cancel), Ok(()));

        assert_eq!(
            check_swap(None, Champion, 47, Request),
            Err(InvalidSwap::NoChampSelect)
        );
        // O id existe, mas em outro tipo.
        assert_eq!(check(Position, 47, Request), Err(InvalidSwap::UnknownSwap));
        assert_eq!(check(Champion, 999, Request), Err(InvalidSwap::UnknownSwap));
        assert_eq!(check(Champion, 41, Request), Err(InvalidSwap::NotAvailable));
        assert_eq!(check(Position, 11, Request), Err(InvalidSwap::NotAvailable));
        assert_eq!(check(Champion, 30, Request), Err(InvalidSwap::NotAvailable));
        assert_eq!(check(Champion, 47, Accept), Err(InvalidSwap::NotReceived));
        assert_eq!(check(PickOrder, 28, Decline), Err(InvalidSwap::NotReceived));
        assert_eq!(check(Champion, 30, Cancel), Err(InvalidSwap::NotSent));
    }

    #[test]
    fn caminhos_das_trocas() {
        assert_eq!(
            swap_path(SwapKind::Champion, 47, SwapIntent::Request),
            "/lol-champ-select/v1/session/champion-swaps/47/request"
        );
        assert_eq!(
            swap_path(SwapKind::PickOrder, 28, SwapIntent::Cancel),
            "/lol-champ-select/v1/session/pick-order-swaps/28/cancel"
        );
        assert_eq!(
            swap_path(SwapKind::Position, 11, SwapIntent::Accept),
            "/lol-champ-select/v1/session/position-swaps/11/accept"
        );
        assert_eq!(
            swap_path(SwapKind::Position, 11, SwapIntent::Decline),
            "/lol-champ-select/v1/session/position-swaps/11/decline"
        );
    }

    #[test]
    fn formato_para_a_ui() {
        assert_eq!(
            serde_json::to_value(swap(47, 4, SwapState::Received)).unwrap(),
            json!({"id": 47, "cellId": 4, "state": "RECEIVED"})
        );
        for state in SwapState::ALL {
            assert_eq!(serde_json::to_value(state).unwrap(), json!(state.as_str()));
        }
        assert_eq!(
            serde_json::from_value::<SwapState>(json!("ESTADO_NOVO")).unwrap(),
            SwapState::Unknown
        );
        for (kind, name) in SwapKind::ALL
            .into_iter()
            .zip(["champion", "pickOrder", "position"])
        {
            assert_eq!(serde_json::to_value(kind).unwrap(), json!(name));
        }
    }
}
